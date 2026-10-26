"""Data-state steady-state calibration (ADR 0089; `specs/econ/SPEC.md`, Calibration): the 2000
values are held fixed and prices are solved first, so the data state is the steady state.

Stage 1, values. Output value V = VA / (1 - sum_k a_kg) per traded enterprise, VA the pack's.
Destinations spend on traded goods E_dg = households' PIGL spending at q_d of their income
(their province's wages s_L VA by effective labour share; their country's profits (1 - s_L) VA
and tariff rebate by population, national ownership, owner 2026-10-25)
plus sum a_kg V of the province's enterprises. One scalar r_g per traded good rescales its V (and
VA) so that world spending net of tariffs equals world output value. Rebates depend on sourcing,
local output on income and income on local output, so passes of stages 1, 2 and 4 repeat until
rebates, r, buyer prices and the PIGL weights settle (PASS_TOL).

Stage 2, origin prices. Per traded good, the balanced scaling
w_o sum_d K_od / (1 + t_od) E_d / Phi_d = r_g V_o, Phi_d = sum_o K_od w_o, on the two-level
kernel (`sinkhorn`), unique up to scale (Sinkhorn 1967). Then p_o = w_o^(-1/theta), one scale per
good, set so that the good's part of the world price index (base consumption weights at the
calibrated incomes) is 1; local goods take p = 1, so the index is 1. q_d, the arrival slots and
the lead L_d follow from sourcing (`model._sourcing`).

Stage 3, technology. coef_kg = a_kg p_o / q_ik (the 2000 cost shares at the solved prices),
a_VA = 1 - sum_k a_kg, so p_VA = p_o, A = 1 / p_VA and Y = V / p_o. Local output solves the 3 x 3
Leontief system of each province against its local demand (households at p = 1 and every
enterprise's a_kg V), and its VA, a_VA Y, is an output of calibration. w0 = sum_g s_L VA0 /
((1 - u*) L^s) and L0 = s_L VA0 / w0.

Stage 4, demand. Per country, PIGL weights inverted at its buyer prices (the unit value of its
households' purchases of each good) and its mean household income per person (`demand.invert_at`);
households then spend by PIGL at their own province's q and income, which is the E of stage 1.

Stage 5, the state at the start of a tick (Rust `systems`): input stocks (1 + COVER) months of
use; the pipeline holds, j ticks ahead, one month of shipments of every arrival slot j or later;
merchant stocks coverage times output (S* = coverage D at D = Q); pops hold one month of income
less the wages of the tick (dividends and rebates); pi^e = 0.
"""

import json
import time
from dataclasses import dataclass, field, replace
from pathlib import Path

import numpy as np
import pyarrow as pa
from pyarrow import ipc

from . import demand, model
from .io import Inputs

COVER = 1.0  # `systems::COVER`
SLOTS = 4  # `pipeline::SLOTS`: arrival slots 1 to 4 enter the pipeline, slot 0 is home
SINKHORN_TOL = 1e-13  # max |ln(received / target)| over origins
MAX_SWEEPS = 20_000
ANDERSON_MEMORY = 8
ANDERSON_STALL = 20  # sweeps without a new smallest residual before the mixing restarts
# Largest relative change of rebates, r, buyer prices and PIGL weights that ends the passes: the
# Sinkhorn tolerance leaves a floor near 1e-12 on the full pack.
PASS_TOL = 1e-11
MAX_PASSES = 100
LOCAL_TOL = 1e-14  # largest relative change of any local output that ends the income fixed point
MAX_LOCAL = 1_000
FORMAT = "3"


@dataclass
class Scaling:
    log_weight: np.ndarray  # (P, G) ln w of active traded origins, 0 elsewhere
    r: np.ndarray  # (G,) rescale of each traded good's output value, 1 for local goods
    sweeps: int
    residual: float  # max |ln(received / (r V))|


@dataclass
class Steady:
    u_star: np.ndarray  # (C,)
    r: np.ndarray  # (G,)
    price: np.ndarray  # (P, G) posted: origin markets of traded goods, 1 for local goods, 0 elsewhere
    consumer: np.ndarray  # (P, G) buyer price q of traded goods, the local price of local goods
    lead: np.ndarray  # (P, G) expected months in transit of traded goods
    arrive: np.ndarray  # (SLOTS + 1, P, G) units arriving per unit spent, by arrival slot
    weights: demand.Weights
    rebate: np.ndarray  # (C,) tariffs per month
    value: np.ndarray  # (E,) output value V
    output: np.ndarray  # (E,) Y
    coef: np.ndarray  # (E, G)
    a_va: np.ndarray  # (E,)
    a: np.ndarray  # (E,)
    va0: np.ndarray  # (E,)
    l0: np.ndarray  # (E,)
    w0: np.ndarray  # (P,)
    ent_spend: np.ndarray  # (E, G) traded input spending per month
    pop_income: np.ndarray  # (K,)
    pop_wage: np.ndarray  # (K,)
    pop_spend: np.ndarray  # (K, G)
    spend_hh: np.ndarray  # (P, G) household spending (local goods: units at p = 1)
    spend_int: np.ndarray  # (P, G) traded input spending
    supply: np.ndarray  # (P, G) Q = D = sales of every active cell
    orders: np.ndarray  # (2, P, G) traded orders at the origin in units: intermediate, household
    coverage: np.ndarray  # (G,) P02
    pop_pipeline: np.ndarray  # (K, G, SLOTS) goods in flight at the start of a tick, as `ent_pipeline`
    ent_pipeline: np.ndarray  # (E, G, SLOTS) index j due j + 1 ticks ahead: the shipments of every slot j + 1 or later
    info: dict = field(default_factory=dict)

    @property
    def stock(self) -> np.ndarray:
        """(E, G) input stocks at the start of a tick."""
        return (1.0 + COVER) * self.coef * self.output[:, None]

    @property
    def inventory(self) -> np.ndarray:
        """(P, G) merchant stocks: coverage times output at traded origin markets."""
        return self.coverage[None, :] * self.supply

    @property
    def pop_money(self) -> np.ndarray:
        """(K,) money at the start of a tick: dividends and rebates, wages come at E2."""
        return self.pop_income - self.pop_wage



def read_u_star(pack: Path | str) -> np.ndarray:
    """(C,) u* of the pack's `econ_countries.arrow`, in its row order (the export's country order)."""
    return ipc.open_file(str(Path(pack) / "econ_countries.arrow")).read_all()["u_star"].to_numpy()


def in_flight(arrive: np.ndarray, spend: np.ndarray, province: np.ndarray) -> np.ndarray:
    """(buyers, G, SLOTS) goods in flight at the start of a tick for buyers spending `spend` per
    month at `province`: j ticks ahead, one month of the shipments of every arrival slot j or later."""
    n = arrive[1:, province, :].transpose(1, 2, 0) * spend[:, :, None]
    return np.cumsum(n[..., ::-1], axis=2)[..., ::-1]


def _flat(a: np.ndarray) -> np.ndarray:
    return a.T.ravel()


def _cells(a: np.ndarray, inp: Inputs) -> np.ndarray:
    return a.reshape(inp.n_goods, inp.n_provinces).T


def sinkhorn(inp: Inputs, spend: np.ndarray, supply: np.ndarray, start: np.ndarray | None = None) -> Scaling:
    """Stage 2 for every traded good at once: ln w of the active origins with
    w_o sum_d K_od / (1 + t_od) E_d / Phi_d = r_g V_o, from `spend` E (P, G) and `supply` V (P, G).
    Log domain: the weights of each good are shifted by their largest before each pass over the
    kernel. The sweep is the Sinkhorn map ln w <- ln(r V) - ln(sum_d K_od / (1 + t_od) E_d / Phi_d),
    Anderson-accelerated. `start` (P, G) is the initial ln w (0 by default)."""
    dom, acc, cro = inp.domestic, inp.access, inp.crossing
    n_g = inp.n_goods
    good = np.repeat(np.arange(n_g), inp.n_provinces)
    origin = _flat(inp.active & inp.traded[None, :])
    v = _flat(supply)
    e = _flat(np.where(inp.traded[None, :], spend, 0.0))
    net = cro.k / (1.0 + cro.tariff)
    total = np.bincount(good, np.where(origin, v, 0.0), n_g)
    lw = np.where(origin, 0.0 if start is None else _flat(start), 0.0)

    def step(lw):
        top = np.zeros(n_g)
        np.maximum.at(top, good[origin], lw[origin])
        w = np.where(origin, np.exp(lw - top[good]), 0.0)
        phi = model._from_origins(inp, w, dom.k, acc.k, cro.k)
        b = np.divide(e, phi, out=np.zeros_like(e), where=phi > 0)
        reach = model._to_destinations(inp, b, dom.k, acc.k, net)
        got = w * reach
        r = np.divide(np.bincount(good, got, n_g), total, out=np.ones(n_g), where=total > 0)
        with np.errstate(divide="ignore", invalid="ignore"):
            target = np.log(r[good] * v)
            new = np.where(origin, target - np.log(reach) + top[good], 0.0)
            err = np.abs(np.where(origin, np.log(got) - target, 0.0)).max()
        return new, r, err

    xs, gs = [], []
    best, since = np.inf, 0
    for sweep in range(1, MAX_SWEEPS + 1):
        fx, r, err = step(lw)
        if err <= SINKHORN_TOL:
            return Scaling(_cells(lw, inp), np.where(inp.traded, r, 1.0), sweep, float(err))
        if not np.isfinite(err):
            raise ValueError(f"Sinkhorn residual not finite after {sweep} sweeps")
        best, since = (err, 0) if err < best else (best, since + 1)
        if since > ANDERSON_STALL:
            # Anderson mixing has stalled: restart it from a plain sweep
            xs, gs, best, since = [], [], np.inf, 0
        g = fx - lw
        xs, gs = [*xs[-ANDERSON_MEMORY:], lw], [*gs[-ANDERSON_MEMORY:], g]
        if len(gs) > 1:
            dx, dg = np.diff(xs, axis=0).T, np.diff(gs, axis=0).T
            gamma = np.linalg.lstsq(dg, g, rcond=None)[0]
            lw = lw + g - (dx + dg) @ gamma
        else:
            lw = fx
    raise ValueError(f"Sinkhorn did not converge in {MAX_SWEEPS} sweeps, residual {err:.3e}")


def arrivals(inp: Inputs, price: np.ndarray, src: dict) -> np.ndarray:
    """(SLOTS + 1, P, G) units arriving at each destination per unit spent, by arrival slot: 0 home,
    1 a domestic leg, 1 + lag a crossing (as `world::delivered_prices` reads `Sourced::arrive`)."""
    dom, acc, cro = inp.domestic, inp.access, inp.crossing
    wf, phi = src["w"], src["phi"]
    per_price = np.divide(wf, _flat(price), out=np.zeros_like(wf), where=wf > 0)
    loss = cro.tau * cro.border_tau * (1.0 + cro.tariff)
    out = np.zeros((SLOTS + 1, len(wf)))
    out[0] = per_price
    out[1] = model._from_origins(inp, per_price, dom.k / dom.tau, 0.0 * acc.k, 0.0 * cro.k) - per_price
    for lag in range(SLOTS):
        k = np.where(cro.lag == lag, cro.k / loss, 0.0)
        out[1 + lag] += model._from_origins(inp, per_price, 0.0 * dom.k, acc.k / acc.tau, k) - per_price
    reach = phi >= inp.weight_floor
    out = np.divide(out, phi, out=np.zeros_like(out), where=reach[None, :])
    return out.reshape(SLOTS + 1, inp.n_goods, inp.n_provinces).transpose(0, 2, 1)


def local_output(inp: Inputs, a_e: np.ndarray, local_demand: np.ndarray) -> np.ndarray:
    """(E,) output of local enterprises at p = 1, 0 for traded ones: per province the Leontief
    system y_k = d_k + sum_j a_kj y_j over its local producers, d (P, local goods) the demand of
    households and traded enterprises."""
    local = np.flatnonzero(~inp.traded)
    n_p, n_l = inp.n_provinces, len(local)
    slot = np.full(inp.n_goods, -1)
    slot[local] = np.arange(n_l)
    is_local = ~inp.traded[inp.ent_good]
    owner = np.full((n_p, n_l), -1)
    owner[inp.ent_province[is_local], slot[inp.ent_good[is_local]]] = np.flatnonzero(is_local)
    has = owner >= 0
    if np.any(~has & (local_demand > 0)):
        raise ValueError("local demand in a province without its producer")
    m = np.zeros((n_p, n_l, n_l))
    for j in range(n_l):
        m[has[:, j], :, j] = a_e[owner[has[:, j], j]][:, local]
    y = np.linalg.solve(np.eye(n_l)[None] - m, local_demand[..., None])[..., 0]
    out = np.zeros(len(inp.ent_good))
    out[owner[has]] = y[has]
    return out


def _unit_value_prices(inp: Inputs, spend: np.ndarray, q: np.ndarray) -> np.ndarray:
    """(C, G) per country the unit value of its households' purchases of each good, 1 where it buys none."""
    c = inp.province_country
    value = np.zeros((inp.n_countries, inp.n_goods))
    units = np.zeros_like(value)
    np.add.at(value, c, spend)
    np.add.at(units, c, spend / q)
    return np.divide(value, units, out=np.ones_like(value), where=units > 0)


def _relative(new: np.ndarray, old: np.ndarray) -> float:
    return float(np.abs(new - old).max() / max(np.abs(new).max(), 1e-300))


def calibrate(inp: Inputs, u_star: np.ndarray, progress=None) -> Steady:
    """The steady state of ADR 0089 (module documentation) at natural rates `u_star` (C,).
    `progress(line)` gets one line per pass."""
    t0 = time.perf_counter()
    if not np.isfinite(inp.base_price).all():
        raise ValueError("a traded good is unreachable at some province")
    n_p, n_g, n_c = inp.n_provinces, inp.n_goods, inp.n_countries
    ep, eg = inp.ent_province, inp.ent_good
    traded = inp.traded
    traded_e = traded[eg]
    a_e = inp.io[inp.ent_country, :, eg]
    a_va = 1.0 - a_e.sum(axis=1)
    c = inp.province_country
    kp, kc = inp.pop_province, c[inp.pop_province]
    people = inp.pop_population
    country_people = np.bincount(kc, people, n_c)
    wage_share = inp.pop_effective / inp.labour_supply[kp]  # a pop's share of its province's wages
    people_share = np.divide(people, country_people[kc], out=np.zeros_like(people), where=country_people[kc] > 0)
    province_people = np.bincount(c, inp.population, n_c)
    province_share = np.divide(inp.population, province_people[c], out=np.zeros(n_p), where=province_people[c] > 0)
    s_l = inp.ent_s_l
    sigma_k = inp.sigma_income[kc]
    om = inp.omega / inp.omega.sum(axis=1, keepdims=True)
    v_data = np.where(traded_e, inp.ent_va_target / a_va, 0.0)
    supply_data = np.zeros((n_p, n_g))
    np.add.at(supply_data, (ep, eg), v_data)
    origin = inp.active & traded[None, :]
    theta = np.where(traded, inp.theta, 1.0)

    def households(v, rebate, q, weights):
        va = a_va * v
        bill = np.bincount(ep, s_l * va, n_p)
        pool = np.bincount(inp.ent_country, (1.0 - s_l) * va, n_c)
        income = wage_share * bill[kp] + people_share * (pool + rebate)[kc]
        per_person = np.divide(income, people, out=np.zeros_like(income), where=people > 0)
        with np.errstate(divide="ignore", invalid="ignore"):
            share = demand.shares(weights, kc, q[kp], per_person, sigma_k, inp.category, inp.sigma_demand, inp.pigl)
        pop_spend = np.where(income[:, None] > 0, share * income[:, None], 0.0)
        spend_hh = np.zeros((n_p, n_g))
        np.add.at(spend_hh, kp, pop_spend)
        # household income per province rebates aside, the weights of the world price index (`io.base_income`)
        base = bill + province_share * pool[c]
        return base, income, pop_spend, spend_hh

    def settle(r, rebate, q, weights, y_local):
        """Income and local output at given r, rebates, buyer prices and weights: output values V."""
        for _ in range(MAX_LOCAL):
            v = np.where(traded_e, r[eg] * v_data, y_local)
            _, _, _, spend_hh = households(v, rebate, q, weights)
            use = np.zeros((n_p, n_g))
            np.add.at(use, ep[traded_e], a_e[traded_e] * v[traded_e, None])
            y_new = local_output(inp, a_e, (spend_hh + use)[:, ~traded])
            settled = np.all(np.abs(y_new - y_local) <= LOCAL_TOL * np.abs(y_new))
            y_local = y_new
            if settled:
                return np.where(traded_e, r[eg] * v_data, y_local)
        raise ValueError(f"income and local output did not settle in {MAX_LOCAL} iterations")

    q = np.where(traded[None, :], inp.base_price, 1.0)
    r = np.ones(n_g)
    rebate = np.zeros(n_c)
    y_local = np.where(traded_e, 0.0, inp.ent_va_target / a_va)
    weights = inp.pigl_weights
    lw = None
    history = []
    n_q = n_p * int(traded.sum())

    def pack_state(q, rebate, r, w):
        return np.concatenate([np.log(q[:, traded]).ravel(), rebate, np.log(r[traded]), w.scale, w.omega.ravel(), w.within.ravel()])

    def unpack_state(z):
        q = np.ones((n_p, n_g))
        q[:, traded] = np.exp(z[:n_q]).reshape(n_p, -1)
        at = n_q + n_c
        rebate = z[n_q:at]
        r = np.ones(n_g)
        r[traded] = np.exp(z[at : at + int(traded.sum())])
        at += int(traded.sum())
        scale, omega, within = np.split(z[at:], [n_c, n_c + n_c * demand.N_CATEGORIES])
        return q, rebate, r, demand.Weights(scale=scale, omega=omega.reshape(n_c, -1), within=within.reshape(n_c, -1))

    zs, fs = [], []
    for passes in range(1, MAX_PASSES + 1):
        v = settle(r, rebate, q, weights, y_local)
        y_local = np.where(traded_e, 0.0, v)
        base, income, _, spend_hh = households(v, rebate, q, weights)
        spend_int = np.zeros((n_p, n_g))
        np.add.at(spend_int, ep, a_e * v[:, None])
        spend = np.where(traded[None, :], spend_hh + spend_int, 0.0)

        scaling = sinkhorn(inp, spend, supply_data, start=lw)
        lw = scaling.log_weight
        top = np.where(origin, lw, -np.inf).max(axis=0)
        raw = np.where(origin, np.exp(-(lw - np.where(traded, top, 0.0)) / theta), 0.0)
        raw[:, ~traded] = 1.0
        index_weights = om[c] * base[:, None]
        mass = index_weights.sum(axis=0)
        index_raw = np.where(traded[None, :], model._sourcing(inp, raw)["index"], 1.0)
        level = (index_weights * index_raw).sum(axis=0)
        # a good without final demand has no weight in the index: its price level stays as solved
        level = np.divide(level, mass, out=np.ones(n_g), where=mass > 0)
        price = raw / np.where(traded, level, 1.0)
        src = model._sourcing(inp, price)
        q_new = np.where(traded[None, :], src["index"], 1.0)
        duty = model._flows(inp, price, np.ones((n_p, n_g)), src)["tariff"]
        rebate_new = np.bincount(c, (duty * spend).sum(axis=1), n_c)
        m0 = np.divide(np.bincount(kc, income, n_c), country_people, out=np.zeros(n_c), where=country_people > 0)
        prices_c = _unit_value_prices(inp, spend_hh, q)
        weights_new = demand.invert_at(inp.omega, prices_c, m0, inp.sigma_income, inp.category, inp.sigma_demand, inp.pigl)

        change = max(
            _relative(rebate_new, rebate),
            _relative(scaling.r, r),
            _relative(q_new, q),
            _relative(weights_new.scale, weights.scale),
            _relative(weights_new.omega, weights.omega),
        )
        history.append(
            {
                "pass": passes,
                "change": change,
                "sweeps": scaling.sweeps,
                "sinkhorn_residual": scaling.residual,
            }
        )
        if progress:
            progress(f"pass {passes}: change {change:.3e}, sinkhorn {scaling.sweeps} sweeps, residual {scaling.residual:.3e}")
        if change <= PASS_TOL:
            rebate, r, q, weights = rebate_new, scaling.r, q_new, weights_new
            break
        # Anderson mixing of the passes' fixed point (prices, rebates, r and the PIGL weights)
        z, f = pack_state(q, rebate, r, weights), pack_state(q_new, rebate_new, scaling.r, weights_new)
        zs, fs = [*zs[-ANDERSON_MEMORY:], z], [*fs[-ANDERSON_MEMORY:], f - z]
        if len(fs) > 1:
            dz, df = np.diff(zs, axis=0).T, np.diff(fs, axis=0).T
            gamma = np.linalg.lstsq(df, f - z, rcond=None)[0]
            q, rebate, r, weights = unpack_state(f - (dz + df) @ gamma)
        else:
            rebate, r, q, weights = rebate_new, scaling.r, q_new, weights_new
    else:
        raise ValueError(f"passes did not settle in {MAX_PASSES}, last change {change:.3e}")

    v = settle(r, rebate, q, weights, y_local)
    va0 = a_va * v
    _, income, pop_spend, spend_hh = households(v, rebate, q, weights)
    spend_int = np.zeros((n_p, n_g))
    np.add.at(spend_int, ep, a_e * v[:, None])
    spend_int[:, ~traded] = 0.0

    own = np.where(traded_e, price[ep, eg], 1.0)
    output = v / own
    bill = np.bincount(ep, s_l * va0, n_p)
    w0 = bill / ((1.0 - u_star[c]) * inp.labour_supply)
    arrive = arrivals(inp, price, src) * traded[None, None, :]
    alpha = arrive.sum(axis=0)
    lead = np.divide((np.arange(SLOTS + 1)[:, None, None] * arrive).sum(axis=0), alpha, out=np.zeros_like(alpha), where=alpha > 0)
    supply = np.zeros((n_p, n_g))
    np.add.at(supply, (ep, eg), output)
    orders = np.stack(
        [model._flows(inp, price, spend_int, src)["orders"], model._flows(inp, price, spend_hh, src)["orders"]]
    ) * traded[None, None, :]
    ent_spend = np.where(traded[None, :], a_e * v[:, None], 0.0)
    log10_weight = np.where(origin, lw - top, np.inf).min(axis=0) / np.log(10.0)
    info = {
        "r": r.tolist(),
        "passes": passes,
        "pass_history": history,
        "sinkhorn_sweeps": [h["sweeps"] for h in history],
        "sinkhorn_residual": history[-1]["sinkhorn_residual"],
        "min_log10_relative_weight": [float(x) if traded[g] else None for g, x in enumerate(log10_weight)],
        "seconds": time.perf_counter() - t0,
    }
    return Steady(
        u_star=np.asarray(u_star, dtype=np.float64),
        r=r,
        price=np.where(origin | ~traded[None, :], price, 0.0),
        consumer=q,
        lead=lead,
        arrive=arrive,
        weights=weights,
        rebate=rebate,
        value=v,
        output=output,
        coef=a_e * own[:, None] / q[ep],
        a_va=a_va,
        a=1.0 / own,
        va0=va0,
        l0=s_l * va0 / w0[ep],
        w0=w0,
        ent_spend=ent_spend,
        pop_income=income,
        pop_wage=wage_share * bill[kp],
        pop_spend=pop_spend,
        spend_hh=spend_hh,
        spend_int=spend_int,
        supply=supply,
        orders=orders,
        coverage=np.where(traded, inp.coverage, 0.0),
        pop_pipeline=in_flight(arrive, pop_spend, kp),
        ent_pipeline=in_flight(arrive, ent_spend, ep),
        info=info,
    )


def reference_inputs(inp: Inputs, s: Steady) -> Inputs:
    """The reference model's inputs at the calibrated point: A, VA0, L0, w0, the coefficients, a_VA,
    the PIGL weights and the natural rates of `s` (labour clears at (1 - u*) L^s)."""
    return replace(
        inp,
        ent_a=s.a,
        ent_va0=s.va0,
        ent_l0=s.l0,
        w0=s.w0,
        u_star=s.u_star,
        calibrated_coef=s.coef,
        calibrated_a_va=s.a_va,
        calibrated_weights=s.weights,
    )


def reference_point(inp: Inputs, s: Steady) -> np.ndarray:
    """x of the reference model: ln p of the active cells, ln w0."""
    return np.concatenate([np.log(s.price.ravel()[inp.active_cells]), np.log(s.w0)])


def read_reference(pack: Path | str, inp: Inputs, pack_province: np.ndarray) -> tuple[Inputs, np.ndarray]:
    """`reference_inputs` and `reference_point` from the calibration tables installed in `pack`, for the
    export `inp` of that pack's data state (`pack_province` its `pack_provinces` column), without calibrating
    again; raises ValueError for tables of other inputs."""
    pack = Path(pack)

    def read(name):
        t = ipc.open_file(str(pack / name)).read_all()
        if t.schema.metadata.get(b"calibration_format") != FORMAT.encode():
            raise ValueError(f"{name}: calibration format is not {FORMAT}")
        return t

    ent = read("econ_calib.arrow")
    meta = json.loads(ent.schema.metadata[b"calibration"])
    if meta["static_digest"] != inp.static_digest:
        raise ValueError("the calibration tables belong to other inputs")
    row = {k: r for r, k in enumerate(zip(ent["province"].to_pylist(), ent["good"].to_pylist()))}
    e = np.array([row[(int(pack_province[p]), int(g))] for p, g in zip(inp.ent_province, inp.ent_good)])
    col = {name: ent[name].to_numpy()[e] for name in ("a", "va0", "l0", "a_va")}
    coef = np.stack([ent[f"coef_{k}"].to_numpy()[e] for k in range(inp.n_goods)], axis=1)
    provinces = read("econ_calib_provinces.arrow")
    at = {p: r for r, p in enumerate(provinces["province"].to_pylist())}
    w0 = provinces["w0"].to_numpy()[[at[int(p)] for p in pack_province]]
    countries = read("econ_calib_countries.arrow")
    weights = demand.Weights(
        scale=countries["pigl_scale"].to_numpy(),
        omega=np.stack([countries[f"pigl_omega_{c}"].to_numpy() for c in range(demand.N_CATEGORIES)], axis=1),
        within=np.stack([countries[f"pigl_within_{g}"].to_numpy() for g in range(inp.n_goods)], axis=1),
    )
    cells = read("econ_calib_cells.arrow")
    at = {k: r for r, k in enumerate(zip(cells["province"].to_pylist(), cells["good"].to_pylist()))}
    n_p, n_g = inp.n_provinces, inp.n_goods
    price = cells["price"].to_numpy()[[at[(int(pack_province[p]), g)] for p in range(n_p) for g in range(n_g)]]
    ref = replace(
        inp,
        ent_a=col["a"],
        ent_va0=col["va0"],
        ent_l0=col["l0"],
        w0=w0,
        u_star=countries["u_star"].to_numpy(),
        calibrated_coef=coef,
        calibrated_a_va=col["a_va"],
        calibrated_weights=weights,
    )
    return ref, np.concatenate([np.log(price[inp.active_cells]), np.log(w0)])


def summary(inp: Inputs, s: Steady) -> dict:
    """Full-pack report numbers: r, passes, Sinkhorn, local VA's national gap to ICIO, ln A."""
    local = ~inp.traded[inp.ent_good]
    c = inp.ent_country
    model_va = np.bincount(c[local], s.va0[local], inp.n_countries)
    data_va = np.bincount(c[local], inp.ent_va_target[local], inp.n_countries)
    has = data_va > 0
    gap = np.where(has, model_va / np.where(has, data_va, 1.0) - 1.0, np.nan)
    order = np.argsort(-np.abs(np.where(has, gap, 0.0)))
    q = [0, 1, 5, 25, 50, 75, 95, 99, 100]
    ln_a = np.log(s.a)

    def quantiles(x):
        return dict(zip(map(str, q), np.percentile(x, q).tolist()))

    return {
        **s.info,
        "local_va_gap_quantiles": quantiles(gap[has]),
        "local_va_gap_worst": [{"country": int(k), "gap": float(gap[k])} for k in order[:10]],
        "local_va_world_gap": float(model_va.sum() / data_va.sum() - 1.0),
        "ln_a_quantiles": quantiles(ln_a),
        "ln_a_quantiles_traded": quantiles(ln_a[~local]),
    }


def _write(path: Path, columns: dict, metadata: dict | None = None) -> None:
    table = pa.table(columns).replace_schema_metadata({"calibration_format": FORMAT, **(metadata or {})})
    with pa.OSFile(str(path), "wb") as f, ipc.new_file(f, table.schema) as w:
        w.write_table(table)


def write(out: Path, inp: Inputs, s: Steady, pack_province: np.ndarray) -> None:
    """The calibration tables (`specs/econ/SPEC.md`, Calibration table), keyed by pack province."""
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    n_p, n_g = inp.n_provinces, inp.n_goods
    goods = range(n_g)
    f64 = pa.float64()
    pack = pack_province.astype(np.uint32)
    stock = s.stock
    meta = {
        "format": FORMAT,
        "r": s.r.tolist(),
        "passes": s.info["passes"],
        "sinkhorn_residual": s.info["sinkhorn_residual"],
        "static_digest": inp.static_digest,
    }
    _write(
        out / "econ_calib.arrow",
        {
            "province": pa.array(pack[inp.ent_province], pa.uint32()),
            "good": pa.array(inp.ent_good, pa.uint8()),
            "a": pa.array(s.a, f64),
            "va0": pa.array(s.va0, f64),
            "l0": pa.array(s.l0, f64),
            "a_va": pa.array(s.a_va, f64),
            "price": pa.array(np.where(inp.traded[inp.ent_good], s.price[inp.ent_province, inp.ent_good], 1.0), f64),
            "output": pa.array(s.output, f64),
            "sales": pa.array(s.value, f64),
            "profit": pa.array((1.0 - inp.ent_s_l) * s.va0, f64),
            **{f"coef_{k}": pa.array(s.coef[:, k], f64) for k in goods},
            **{f"stock_{k}": pa.array(stock[:, k], f64) for k in goods},
        },
        {"calibration": json.dumps(meta)},
    )
    cell_p, cell_g = np.divmod(np.arange(n_p * n_g), n_g)
    _write(
        out / "econ_calib_cells.arrow",
        {
            "province": pa.array(pack[cell_p], pa.uint32()),
            "good": pa.array(cell_g, pa.uint8()),
            "active": pa.array(inp.active.ravel(), pa.bool_()),
            "price": pa.array(s.price.ravel(), f64),
            "consumer_price": pa.array(s.consumer.ravel(), f64),
            "lead": pa.array(s.lead.ravel(), f64),
            "supply": pa.array(s.supply.ravel(), f64),
            "demand": pa.array(s.supply.ravel(), f64),
            "demand_int": pa.array(s.orders[0].ravel(), f64),
            "demand_hh": pa.array(s.orders[1].ravel(), f64),
            "sales": pa.array(s.supply.ravel(), f64),
            "inventory": pa.array(s.inventory.ravel(), f64),
        },
    )
    _write(
        out / "econ_calib_provinces.arrow",
        {
            "province": pa.array(pack, pa.uint32()),
            "w0": pa.array(s.w0, f64),
            "employment": pa.array(np.bincount(inp.ent_province, s.l0, n_p), f64),
            "income": pa.array(np.bincount(inp.pop_province, s.pop_income, n_p), f64),
        },
    )
    edu = inp.pop_edu.astype(np.uint8)
    _write(
        out / "econ_calib_pops.arrow",
        {
            "province": pa.array(pack[inp.pop_province], pa.uint32()),
            "edu": pa.array(edu, pa.uint8()),
            "money": pa.array(s.pop_money, f64),
            "income": pa.array(s.pop_income, f64),
            "wage": pa.array(s.pop_wage, f64),
        },
    )
    rows = []
    for kind, (ahead, province, key) in enumerate(
        ((s.pop_pipeline, inp.pop_province, edu), (s.ent_pipeline, inp.ent_province, inp.ent_good.astype(np.uint8)))
    ):
        b, g, j = np.nonzero(ahead > 0)
        rows.append((np.full(len(b), kind, np.uint8), pack[province[b]], key[b], g.astype(np.uint8), (j + 1).astype(np.uint8), ahead[b, g, j]))
    kind, province, key, g, j, units = (np.concatenate(x) for x in zip(*rows))
    _write(
        out / "econ_calib_pipeline.arrow",
        {
            "buyer": pa.array(kind, pa.uint8()),
            "province": pa.array(province, pa.uint32()),
            "key": pa.array(key, pa.uint8()),
            "good": pa.array(g, pa.uint8()),
            "ahead": pa.array(j, pa.uint8()),
            "units": pa.array(units, f64),
        },
    )
    w = s.weights
    _write(
        out / "econ_calib_countries.arrow",
        {
            "country": pa.array(np.arange(inp.n_countries), pa.uint32()),
            "u_star": pa.array(s.u_star, f64),
            "rebate": pa.array(s.rebate, f64),
            "pigl_scale": pa.array(w.scale, f64),
            **{f"pigl_omega_{c}": pa.array(w.omega[:, c], f64) for c in range(demand.N_CATEGORIES)},
            **{f"pigl_within_{g}": pa.array(w.within[:, g], f64) for g in goods},
        },
    )
