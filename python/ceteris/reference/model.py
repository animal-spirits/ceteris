"""Steady state of the P2 tick pipeline with frictions off (`specs/econ/SPEC.md`, GE reference
model). Mirrors `ceteris-trade` `source` and `dispatch` (ADRs 0015 and 0016) and `ceteris-econ` E1
to E4 at a fixed point: frictionless wage (employment at the natural rate, (1 - u*) L^s, ADR
0089), every origin market selling its
output (orders = Q), every local market clearing, input stocks never binding (orders replace use),
pops spending their whole income. Expected inflation (ADR 0019) is zero at a steady state, since
the world price index is constant there, so the price and wage rules' pi^e term leaves this
solution unchanged.

Destination d spends E_d on traded good g, households (each pop by PIGL across categories and CES
within them, ADR 0021) at the delivered unit price q_d = 1 / alpha_d, alpha_d the units arriving
per unit of spending (owner decision, 2026-10-24), and enterprises a Y q_d, so arrivals replace
use; X_od = E_d K_od w_o / Phi_d with w_o = (p_o / p_min)^-theta_g (theta per good, ADR 0018), K
carrying the tariff on crossings. Origin o ships X_od / (p_o (1 +
t)) units and receives p_o per unit, the importer's rebate account t p_o. Weights are relative to
one p_min per good: Rust holds them per country in log scale (ADR 0093), which gives the same
shares wherever the global scale does not underflow, as on every pack and fixture price vector. A
destination is reachable where Phi_d > 0. Losses and duties multiply by the kernel's reciprocals.

Unknowns x: ln p of every active (province, good) cell in row-major order (origin markets of
traded goods, province markets of local goods), then ln w per province. Equations: excess demand
of every active market, ln(D / Q) (with a floor far below market size), D the orders placed at an
origin or the demand at a local market, and of every labour market, (sum L* - N) / N with N =
(1 - u*) L^s employed at the natural rate. Market cash flow is zero at a fixed point and left out,
and households are paid w N, so Walras' law reads sum p (D - Q) + sum w (L* - N) = 0 identically; the labour equation of the
province with the largest base wage bill is replaced by ln(world price index) = 0, the index
weighting consumer prices (q_d for traded goods, the local price for local goods) and leaving out
cells with no reachable origin (q_d infinite), as `EconWorld::world_price_index`.
"""

from dataclasses import dataclass, field

import numpy as np

from .demand import shares as pigl_shares
from .io import Inputs

RHO_CD = 1e-9  # Cobb-Douglas threshold on |rho|, as `production.rs`
REBATE_TOL = 1e-15  # relative change of every country's rebate that ends the income iteration
MAX_REBATE_ITERATIONS = 200
MARKET_FLOOR = 1e-12  # added to D and Q, relative to the cell's base size: ln(D / Q) stays finite


@dataclass
class Equilibrium:
    x: np.ndarray
    price: np.ndarray  # (P, G): posted price of active cells, 0 elsewhere
    wage: np.ndarray  # (P,)
    employment: np.ndarray  # (E,)
    output: np.ndarray  # (E,)
    supply: np.ndarray  # (P, G) traded output of each origin cell
    consumer_price: np.ndarray  # (P, G) q_d of traded goods, the local price of local goods
    expenditure: np.ndarray  # (P, G) destination spending E_d on traded goods, tariff included
    home_value: np.ndarray  # (P, G) home purchases at the province's price
    home_units: np.ndarray  # (P, G)
    flows_by_country_pair_good: np.ndarray  # (C, C, G) origin value of shipped units, exporter x importer
    units_by_country_pair_good: np.ndarray  # (C, C, G) origin units at dispatch
    income: np.ndarray  # (P,) household income
    price_index: float  # world price index
    info: dict = field(default_factory=dict)


def rho(inp: Inputs) -> float:
    return (inp.sigma_fl - 1.0) / inp.sigma_fl


def data_state(inp: Inputs) -> np.ndarray:
    """Prices 1 and base wages."""
    return np.concatenate([np.zeros(len(inp.active_cells)), np.log(inp.w0)])


def dropped_province(inp: Inputs) -> int:
    return int(np.argmax(inp.w0 * inp.labour_supply))


def index_row(inp: Inputs) -> int:
    return len(inp.active_cells) + dropped_province(inp)


def _split(x: np.ndarray, inp: Inputs) -> tuple[np.ndarray, np.ndarray]:
    n = len(inp.active_cells)
    p = np.zeros(inp.n_provinces * inp.n_goods)
    p[inp.active_cells] = np.exp(x[:n])
    return p.reshape(inp.n_provinces, inp.n_goods), np.exp(x[n:])


def _sum(index: np.ndarray, weights: np.ndarray, n: int) -> np.ndarray:
    return np.bincount(index, weights, n) if len(index) else np.zeros(n)


def _legs(inp: Inputs):
    """Flat indices of every kernel entry: provinces as `g * P + i`, gateways as `g * N + a`."""
    n_p, n_n = inp.n_provinces, inp.n_gateways
    dom, acc, cro = inp.domestic, inp.access, inp.crossing
    return (
        dom.good * n_p + dom.row,
        dom.good * n_p + dom.col,
        acc.good * n_p + acc.row,
        acc.good * n_n + acc.col,
        cro.good * n_n + cro.row,
        cro.good * n_n + cro.col,
    )


def _from_origins(inp: Inputs, w: np.ndarray, dom_k, acc_k, cro_k) -> np.ndarray:
    """sum over routes o -> d of the route's leg factors times w_o, for every destination d, home
    route (factor 1) included: the origin-side access sums of `Kernel::origin_access_into` with
    the given per-leg factors. Flat good-major (G * P)."""
    n_p, n_g, n_n = inp.n_provinces, inp.n_goods, inp.n_gateways
    d_from, d_to, a_prov, a_gate, c_from, c_to = _legs(inp)
    v = _sum(a_gate, acc_k * w[a_prov], n_g * n_n)
    u = _sum(c_to, cro_k * v[c_from], n_g * n_n)
    return w + _sum(d_to, dom_k * w[d_from], n_g * n_p) + _sum(a_prov, acc_k * u[a_gate], n_g * n_p)


def _to_destinations(inp: Inputs, r: np.ndarray, dom_k, acc_k, cro_k) -> np.ndarray:
    """sum over routes o -> d of the route's leg factors times r_d, for every origin o, home route
    included: the forward pass of `dispatch`. Flat good-major (G * P)."""
    n_p, n_g, n_n = inp.n_provinces, inp.n_goods, inp.n_gateways
    d_from, d_to, a_prov, a_gate, c_from, c_to = _legs(inp)
    y = _sum(a_gate, acc_k * r[a_prov], n_g * n_n)
    z = _sum(c_from, cro_k * y[c_to], n_g * n_n)
    return r + _sum(d_from, dom_k * r[d_to], n_g * n_p) + _sum(a_prov, acc_k * z[a_gate], n_g * n_p)


def _sourcing(inp: Inputs, price: np.ndarray) -> dict:
    """Origin weights w = (p / p_min)^-theta_g of active traded origins, Phi, alpha (units
    arriving per unit of spending) and the delivered unit price q = 1 / alpha (infinite where no
    active origin is reachable), as `EconWorld::refresh_price_index`. Weights good-major flat
    (G * P); index and alpha (P, G)."""
    n_p, n_g = inp.n_provinces, inp.n_goods
    dom, acc, cro = inp.domestic, inp.access, inp.crossing
    p = price.T
    act = inp.active.T & inp.traded[:, None]
    p_min = np.where(act, p, np.inf).min(axis=1, keepdims=True)  # weights relative to it
    with np.errstate(divide="ignore", invalid="ignore", over="ignore"):
        w = np.where(act, (p / p_min) ** -inp.theta[:, None], 0.0)
    wf = w.ravel()
    phi = _from_origins(inp, wf, dom.k, acc.k, cro.k)
    per_price = np.divide(wf, p.ravel(), out=np.zeros_like(wf), where=wf > 0)
    arrive = _from_origins(inp, per_price, dom.k * dom.inv_tau, acc.k * acc.inv_tau, cro.k * cro.inv_duty * cro.inv_loss)
    reachable = phi > 0
    with np.errstate(divide="ignore", invalid="ignore"):
        alpha = np.where(reachable, arrive / phi, 0.0)
        index = np.where(alpha > 0, 1.0 / alpha, np.inf)
    return {
        "w": wf,
        "phi": phi,
        "index": index.reshape(n_g, n_p).T,
        "alpha": alpha.reshape(n_g, n_p).T,
    }


def _flows(inp: Inputs, price: np.ndarray, spend: np.ndarray, src: dict) -> dict:
    """Sourcing of `spend` (P, G), spending by destination, fully filled (`source` and
    `dispatch`): orders (origin units), home purchases, arrivals (destination units after every
    leg's loss); what each destination pays (`paid`, its spending), the tariff t / (1 + t) of
    crossings (`tariff`), what each origin receives (`received`, p_o times its orders); the
    country-pair origin value and units as `flows.rs` records them."""
    n_p, n_g, n_c = inp.n_provinces, inp.n_goods, inp.n_countries
    dom, acc, cro = inp.domestic, inp.access, inp.crossing
    d_from, d_to, a_prov, a_gate, c_from, c_to = _legs(inp)
    pf, wf, phi = price.T.ravel(), src["w"], src["phi"]
    ef = np.where(inp.traded[:, None], spend.T, 0.0).ravel()
    reach = np.divide(ef, phi, out=np.zeros_like(ef), where=phi > 0)
    per_price = np.divide(wf, pf, out=np.zeros_like(wf), where=wf > 0)
    net = cro.k * cro.inv_duty

    orders = per_price * _to_destinations(inp, reach, dom.k, acc.k, net)
    home = per_price * reach
    arrive = reach * _from_origins(inp, per_price, dom.k * dom.inv_tau, acc.k * acc.inv_tau, net * cro.inv_loss)
    duty = reach * (_from_origins(inp, wf, 0.0 * dom.k, acc.k, cro.k - net) - wf)
    paid = np.where(phi > 0, ef, 0.0)

    # country pairs: domestic legs by province countries, crossings by gateway countries
    pc, gc = inp.province_country, inp.gateway_country
    n_pairs = n_c * n_c * n_g
    dom_value = wf[d_from] * dom.k * reach[d_to]
    y = _sum(a_gate, acc.k * reach[a_prov], n_g * inp.n_gateways)
    v = _sum(a_gate, acc.k * wf[a_prov], n_g * inp.n_gateways)
    v_units = _sum(a_gate, acc.k * per_price[a_prov], n_g * inp.n_gateways)
    pair_dom = (pc[dom.row] * n_c + pc[dom.col]) * n_g + dom.good
    pair_int = (gc[cro.row] * n_c + gc[cro.col]) * n_g + cro.good
    value = _sum(pair_dom, dom_value, n_pairs) + _sum(pair_int, v[c_from] * net * y[c_to], n_pairs)
    dom_units = per_price[d_from] * dom.k * reach[d_to]
    units = _sum(pair_dom, dom_units, n_pairs) + _sum(pair_int, v_units[c_from] * net * y[c_to], n_pairs)

    def cells(a):
        return a.reshape(n_g, n_p).T

    return {
        "orders": cells(orders),
        "home": cells(home),
        "arrive": cells(arrive),
        "paid": cells(paid),
        "received": cells(pf * orders),
        "tariff": cells(duty),
        "pair_value": value.reshape(n_c, n_c, n_g),
        "pair_units": units.reshape(n_c, n_c, n_g),
    }


def labour_target(p_va, w, a, va0, l0, s_l, r, l_supply):
    """`production::labour_target`, vectorized over enterprises."""
    with np.errstate(divide="ignore", invalid="ignore", over="ignore"):
        q = w * l0 / (p_va * a * va0 * s_l)
        if abs(r) < RHO_CD:
            target = l0 * q ** (1.0 / (s_l - 1.0))
        else:
            bracket = (q ** (r / (1.0 - r)) - s_l) / (1.0 - s_l)
            corner = 0.0 if r < 0 else l_supply
            target = np.where(bracket > 0, l0 * np.abs(bracket) ** (-1.0 / r), corner)
        target = np.where(w <= 0, l_supply, target)
        target = np.where(p_va <= 0, 0.0, target)
    return np.minimum(target, l_supply)


def value_added(a, va0, l0, s_l, r, labour):
    """`production::value_added`, vectorized."""
    x = labour / l0
    with np.errstate(divide="ignore", over="ignore"):
        if abs(r) < RHO_CD:
            return a * va0 * x**s_l
        return a * va0 * ((1.0 - s_l) + s_l * x**r) ** (1.0 / r)


def evaluate(x: np.ndarray, inp: Inputs, a: np.ndarray | None = None) -> dict:
    """Every quantity of the steady state at prices and wages `x`, with productivity `a` per
    enterprise (default the inputs' A)."""
    n_p, n_g = inp.n_provinces, inp.n_goods
    a = inp.ent_a if a is None else a
    traded = inp.traded[None, :]
    price, wage = _split(x, inp)
    src = _sourcing(inp, price)
    consumer = np.where(traded, src["index"], price)

    # E1, E2: frictionless labour target at the price index of inputs, output at the own price
    ep, eg = inp.ent_province, inp.ent_good
    uses = inp.ent_inputs > 0
    with np.errstate(invalid="ignore"):
        input_index = np.where(uses, inp.ent_inputs * consumer[ep], 0.0).sum(axis=1)
    sale = price[ep, eg]
    r = rho(inp)
    # Rust caps each target at L^s. At a clearing wage no target exceeds L^s unless one enterprise
    # alone employs the province (then Rust finds no clearing wage either), so the cap never binds
    # at a steady state; leaving it out keeps labour demand responsive to the wage everywhere.
    l_star = labour_target((sale - input_index) / inp.ent_a_va, wage[ep], a, inp.ent_va0, inp.ent_l0, inp.ent_s_l, r, np.inf)
    y = value_added(a, inp.ent_va0, inp.ent_l0, inp.ent_s_l, r, l_star) / inp.ent_a_va
    out = np.zeros((n_p, n_g))
    np.add.at(out, (ep, eg), y)

    # E3, enterprises: traded inputs bought at q_d, so arrivals replace use, local inputs at the
    # province price
    use = inp.ent_inputs * y[:, None]
    q = consumer[ep]
    buys = uses & inp.traded[None, :] & np.isfinite(q)
    input_spend = np.where(buys, use * np.where(buys, q, 0.0), 0.0)
    cost = input_spend.sum(axis=1) + np.where(inp.traded[None, :], 0.0, price[ep] * use).sum(axis=1)
    profit = sale * y - wage[ep] * l_star - cost
    spend_int = np.zeros((n_p, n_g))
    np.add.at(spend_int, ep, input_spend)
    local_use = np.zeros((n_p, n_g))
    np.add.at(local_use, ep, np.where(inp.traded[None, :], 0.0, use))

    # E3, households: PIGL per pop (ADR 0021) at its province's consumer prices and its mean income
    # per person. E4: a pop earns its effective-labour share e_k LF_k / L^s of its province's wages
    # w (1 - u*) L^s, and its population share of its country's profits (national ownership, owner
    # 2026-10-25) and of its country's rebate, the tariff share of spending; rebates depend on
    # household spending, so income is a fixed point.
    duty_share = _flows(inp, price, np.ones((n_p, n_g)), src)["tariff"]  # tariff per unit spent
    c = inp.province_country
    kp = inp.pop_province
    kc = c[kp]
    people = inp.pop_population
    country_people = np.bincount(kc, people, inp.n_countries)
    country_profit = np.bincount(c[ep], profit, inp.n_countries)
    earned = inp.pop_effective / inp.labour_supply[kp] * (wage * inp.employed_supply)[kp] + people / country_people[kc] * country_profit[kc]
    from_inputs = np.bincount(c, (duty_share * spend_int).sum(axis=1), inp.n_countries)
    prices_k = consumer[kp]
    sigma_k = inp.sigma_income[kc]
    rebate = np.zeros(inp.n_countries)
    for _ in range(MAX_REBATE_ITERATIONS):
        pop_income = earned + people / country_people[kc] * rebate[kc]
        with np.errstate(divide="ignore", invalid="ignore"):
            per_person = np.where(people > 0, pop_income / people, 0.0)
        share = pigl_shares(inp.pigl_weights, kc, prices_k, per_person, sigma_k, inp.category, inp.sigma_demand, inp.pigl)
        pop_spend = np.where(pop_income[:, None] > 0, share * pop_income[:, None], 0.0)
        spend_hh = np.zeros((n_p, n_g))
        np.add.at(spend_hh, kp, pop_spend)
        nxt = from_inputs + np.bincount(c, (duty_share * spend_hh).sum(axis=1), inp.n_countries)
        done = np.all(np.abs(nxt - rebate) <= REBATE_TOL * np.maximum(np.abs(nxt), 1e-300))
        rebate = nxt
        if done:
            break
    pop_income = earned + people / country_people[kc] * rebate[kc]
    income = np.bincount(kp, pop_income, n_p)

    spend = np.where(traded, spend_hh + spend_int, 0.0)
    f = _flows(inp, price, spend, src)
    with np.errstate(divide="ignore", invalid="ignore"):
        local = np.where(traded, 0.0, np.divide(spend_hh, price, out=np.zeros_like(price), where=price > 0) + local_use)
    demand = np.where(traded, f["orders"], local)
    # as `EconWorld::world_price_index`: cells with no reachable origin are left out
    reached = np.isfinite(consumer)
    index_weights = np.where(reached, inp.index_weights, 0.0)

    return {
        "price": price,
        "wage": wage,
        "employment": l_star,
        "output": y,
        "value_added": sale * y - cost,  # nominal: sales less inputs at buyer prices
        "supply": np.where(traded, out, 0.0),
        "consumer_price": consumer,
        "price_index": consumer,  # as `EconWorld::price_index`
        "spend": spend,
        "flows": f,
        "demand": demand,
        "q": out,
        "labour_demand": _sum(ep, l_star, n_p),
        "income": income,
        "index": (index_weights * np.where(reached, consumer, 0.0)).sum() / index_weights.sum(),
    }


def _excess(s: dict, inp: Inputs) -> np.ndarray:
    cells = inp.active_cells
    eps = MARKET_FLOOR * (inp.base_demand + inp.base_output).ravel()[cells]
    goods = np.log((s["demand"].ravel()[cells] + eps) / (s["q"].ravel()[cells] + eps))
    labour = (s["labour_demand"] - inp.employed_supply) / inp.employed_supply
    return np.concatenate([goods, labour])


def excess(x: np.ndarray, inp: Inputs, a: np.ndarray | None = None) -> np.ndarray:
    """Scaled excess demand of every market, all labour markets included."""
    return _excess(evaluate(x, inp, a), inp)


def market_residual(s: dict, inp: Inputs) -> np.ndarray:
    """`_excess` of an evaluation with the dropped labour equation replaced by ln(world price index)."""
    r = _excess(s, inp)
    r[index_row(inp)] = np.log(s["index"])
    return r


def residual(x: np.ndarray, inp: Inputs, a: np.ndarray | None = None) -> np.ndarray:
    """`excess` with the dropped labour equation replaced by ln(world price index)."""
    return market_residual(evaluate(x, inp, a), inp)


def unpack(x: np.ndarray, inp: Inputs, info: dict | None = None) -> Equilibrium:
    s = evaluate(x, inp)
    f = s["flows"]
    return Equilibrium(
        x=x,
        price=s["price"],
        wage=s["wage"],
        employment=s["employment"],
        output=s["output"],
        supply=s["supply"],
        consumer_price=s["consumer_price"],
        expenditure=s["spend"],
        home_value=s["price"] * f["home"],
        home_units=f["home"],
        flows_by_country_pair_good=f["pair_value"],
        units_by_country_pair_good=f["pair_units"],
        income=s["income"],
        price_index=float(s["index"]),
        info=info or {},
    )
