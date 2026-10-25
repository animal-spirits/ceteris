"""The calibration's reduced residual (`calibrate.residual`) in JAX, for exact derivatives
(optional: the `jax` extra, DESIGN 19.1). The same composition as numpy: `productivity_of`, then
`model.evaluate` at those A, then `model.market_residual`, rule for rule (`model` documents the
equations). Float64 throughout.

Derivatives are forward mode (`jax.jvp`, `jax.linearize`). Two loops are differentiated
implicitly or to convergence: the labour ratio's Newton iteration runs on held inputs and one
differentiable Newton step follows (the implicit derivative), and the rebate fixed point carries
its tangents to the same relative tolerance as its values.

`cut=True` holds the gateway sums (the international legs of the kernel) fixed in the
derivative: the residual's Jacobian then couples only unknowns of one country, the index row
aside (`precondition.py` uses it). The value is unchanged.
"""

from dataclasses import dataclass
from functools import partial

import jax
import jax.numpy as jnp
import numpy as np

from . import calibrate, model
from .demand import N_CATEGORIES
from .io import Inputs

jax.config.update("jax_enable_x64", True)

BATCH = 32  # tangents per batched Jacobian-vector product


@dataclass(frozen=True)
class Static:
    """Python scalars of the problem (static under jit)."""

    n_p: int
    n_g: int
    n_n: int
    n_c: int
    n_cells: int
    index_row: int
    rho: float
    weight_floor: float
    sigma_demand: float
    sigma_b: float
    phi: float
    gamma: float
    eps: float


def _arrays(inp: Inputs) -> dict:
    n_p, n_g, n_n = inp.n_provinces, inp.n_goods, inp.n_gateways
    dom, acc, cro = inp.domestic, inp.access, inp.crossing
    ep, eg = inp.ent_province, inp.ent_good
    kp = inp.pop_province
    kc = inp.province_country[kp]
    people = inp.pop_population
    country_people = np.bincount(kc, people, inp.n_countries)
    w = inp.pigl_weights
    cells = inp.active_cells
    loss = cro.tau * cro.border_tau * (1.0 + cro.tariff)
    a = {
        "cells": cells,
        "traded": inp.traded,
        "theta": inp.theta,
        "act_t": inp.active.T & inp.traded[:, None],
        # flat leg indices, provinces g * P + i, gateways g * N + a
        "d_from": dom.good * n_p + dom.row,
        "d_to": dom.good * n_p + dom.col,
        "a_prov": acc.good * n_p + acc.row,
        "a_gate": acc.good * n_n + acc.col,
        "c_from": cro.good * n_n + cro.row,
        "c_to": cro.good * n_n + cro.col,
        "dom_k": dom.k,
        "dom_kt": dom.k / dom.tau,
        "acc_k": acc.k,
        "acc_kt": acc.k / acc.tau,
        "cro_k": cro.k,
        "cro_kt": cro.k / loss,
        "cro_net": cro.k / (1.0 + cro.tariff),
        "cro_duty": cro.k * cro.tariff / (1.0 + cro.tariff),
        "ep": ep,
        "eg": eg,
        "ecell": ep * n_g + eg,
        "coef": inp.ent_inputs,
        "a_va": inp.ent_a_va,
        "va0": inp.ent_va0,
        "l0": inp.ent_l0,
        "s_l": inp.ent_s_l,
        "target": inp.ent_va_target,
        "pc": inp.province_country,
        "kp": kp,
        "kc": kc,
        "people": people,
        "people_share": people / country_people[kc],
        "eff_share": inp.pop_effective / inp.labour_supply[kp],
        "sigma_k": inp.sigma_income[kc],
        "within_k": w.within[kc],
        "omega_k": w.omega[kc],
        "scale_k": w.scale[kc],
        "pigl_theta": np.asarray(inp.pigl.theta, dtype=np.float64),
        "category": inp.category,
        "onehot": (inp.category[:, None] == np.arange(N_CATEGORIES)[None, :]).astype(np.float64),
        "labour_supply": inp.labour_supply,
        "index_weights": inp.index_weights,
        "eps_cells": model.MARKET_FLOOR * (inp.base_demand + inp.base_output).ravel()[cells],
    }
    return {k: jnp.asarray(v) for k, v in a.items()}


def _seg(data, ids, n):
    return jax.ops.segment_sum(data, ids, num_segments=n)


def _from_origins(s: Static, d: dict, w, dom_k, acc_k, cro_k, cut: bool):
    """`model._from_origins`; `cut` holds the gateway sums fixed in the derivative."""
    v = _seg(acc_k * w[d["a_prov"]], d["a_gate"], s.n_g * s.n_n)
    u = _seg(cro_k * v[d["c_from"]], d["c_to"], s.n_g * s.n_n)
    if cut:
        u = jax.lax.stop_gradient(u)
    dom = 0.0 if dom_k is None else _seg(dom_k * w[d["d_from"]], d["d_to"], s.n_g * s.n_p)
    return w + dom + _seg(acc_k * u[d["a_gate"]], d["a_prov"], s.n_g * s.n_p)


def _to_destinations(s: Static, d: dict, r, dom_k, acc_k, cro_k, cut: bool):
    """`model._to_destinations`."""
    y = _seg(acc_k * r[d["a_prov"]], d["a_gate"], s.n_g * s.n_n)
    z = _seg(cro_k * y[d["c_to"]], d["c_from"], s.n_g * s.n_n)
    if cut:
        z = jax.lax.stop_gradient(z)
    return r + _seg(dom_k * r[d["d_to"]], d["d_from"], s.n_g * s.n_p) + _seg(acc_k * z[d["a_gate"]], d["a_prov"], s.n_g * s.n_p)


def _split(s: Static, d: dict, x):
    p = jnp.zeros(s.n_p * s.n_g).at[d["cells"]].set(jnp.exp(x[: s.n_cells]))
    return p.reshape(s.n_p, s.n_g), jnp.exp(x[s.n_cells :])


def _sourcing(s: Static, d: dict, price, cut: bool) -> dict:
    """`model._sourcing`. p_min is held in the derivative: every quantity is homogeneous of
    degree 0 in the weights, so its derivative terms cancel."""
    p = price.T
    act = d["act_t"]
    p_min = jax.lax.stop_gradient(jnp.min(jnp.where(act, p, jnp.inf), axis=1, keepdims=True))
    ratio = jnp.where(act, p / jnp.where(jnp.isfinite(p_min), p_min, 1.0), 1.0)
    w = jnp.where(act, ratio ** -d["theta"][:, None], 0.0)
    w = jnp.where(w < s.weight_floor, 0.0, w)
    wf = w.ravel()
    pf = p.ravel()
    phi = _from_origins(s, d, wf, d["dom_k"], d["acc_k"], d["cro_k"], cut)
    per_price = jnp.where(wf > 0, wf / jnp.where(wf > 0, pf, 1.0), 0.0)
    arrive = _from_origins(s, d, per_price, d["dom_kt"], d["acc_kt"], d["cro_kt"], cut)
    reachable = phi >= s.weight_floor
    alpha = jnp.where(reachable, arrive / jnp.where(reachable, phi, 1.0), 0.0)
    index = jnp.where(alpha > 0, 1.0 / jnp.where(alpha > 0, alpha, 1.0), jnp.inf)
    return {"w": wf, "phi": phi, "per_price": per_price, "index": index.reshape(s.n_g, s.n_p).T}


def _labour_ratio(s: Static, w, l0, t, s_l):
    """`calibrate.labour_ratio`: the Newton iteration on held inputs, then one differentiable step."""
    r = s.rho
    if abs(r) < model.RHO_CD:
        return s_l * t / (w * l0)

    def f(y, b):
        e1, e2 = (1.0 - s_l) * jnp.exp((1.0 - r) * y), s_l * jnp.exp(y)
        return jnp.log(b * (e1 + e2)), (e1 + e2) / ((1.0 - r) * e1 + e2)

    b = w * l0 / (s_l * t)
    bh = jax.lax.stop_gradient(b)

    def body(c):
        y, _, k = c
        fy, inv = f(y, bh)
        return y - fy * inv, jnp.max(jnp.abs(fy), initial=0.0), k + 1

    def cond(c):
        _, err, k = c
        return (err > calibrate.LABOUR_TOL) & (k < calibrate.MAX_LABOUR_ITERATIONS)

    y, _, _ = jax.lax.while_loop(cond, body, (jnp.zeros_like(bh), jnp.inf, 0))
    fy, inv = f(y, b)
    return jnp.exp(y - fy * inv)


def _value_added(s: Static, a, va0, l0, s_l, labour):
    """`model.value_added`, with the limit at zero labour taken explicitly."""
    r = s.rho
    pos = labour > 0
    x = jnp.where(pos, labour / l0, 1.0)
    if abs(r) < model.RHO_CD:
        return jnp.where(pos, a * va0 * x**s_l, 0.0)
    zero = 0.0 if r < 0 else a * va0 * (1.0 - s_l) ** (1.0 / r)
    return jnp.where(pos, a * va0 * ((1.0 - s_l) + s_l * x**r) ** (1.0 / r), zero)


def _labour_target(s: Static, p_va, w, a, va0, l0, s_l):
    """`model.labour_target` without the L^s cap (as `model.evaluate`)."""
    r = s.rho
    ok = (p_va > 0) & (w > 0)
    q = jnp.where(ok, w * l0 / (jnp.where(ok, p_va, 1.0) * a * va0 * s_l), 1.0)
    if abs(r) < model.RHO_CD:
        target = l0 * q ** (1.0 / (s_l - 1.0))
    else:
        bracket = (q ** (r / (1.0 - r)) - s_l) / (1.0 - s_l)
        corner = 0.0 if r < 0 else jnp.inf
        pos = bracket > 0
        target = jnp.where(pos, l0 * jnp.abs(jnp.where(pos, bracket, 1.0)) ** (-1.0 / r), corner)
    target = jnp.where(w <= 0, jnp.inf, target)
    return jnp.where(p_va <= 0, 0.0, target)


def _aggregate(weight, index, sigma: float):
    on = (weight > 0) & jnp.isfinite(index)
    term = jnp.where(on, weight * jnp.where(on, index, 1.0) ** (1.0 - sigma), 0.0)
    total = term.sum(axis=1)
    mass = jnp.where(on, weight, 0.0).sum(axis=1)
    safe = jnp.where(total > 0, total, 1.0)
    agg = jnp.where(total > 0, (safe / jnp.where(mass > 0, mass, 1.0)) ** (1.0 / (1.0 - sigma)), jnp.nan)
    return agg, term / safe[:, None]


def _pigl_shares(s: Static, d: dict, price, income):
    """`demand.shares` for every pop."""
    within_w, onehot, category = d["within_k"], d["onehot"], d["category"]
    buys = (within_w > 0) & jnp.isfinite(price)
    term = jnp.where(buys, within_w * jnp.where(buys, price, 1.0) ** (1.0 - s.sigma_demand), 0.0)
    total = term @ onehot
    avail = total > 0
    tot1 = jnp.where(avail, total, 1.0)
    index = jnp.where(avail, tot1 ** (1.0 / (1.0 - s.sigma_demand)), jnp.inf)
    within = jnp.where(term > 0, term / tot1[:, category], 0.0)

    theta = jnp.broadcast_to(d["pigl_theta"], index.shape)
    dt, s_d = _aggregate(theta, index, s.phi)
    omega = d["omega_k"]
    b, s_b = _aggregate(omega, index, s.sigma_b)
    eps = s.eps
    no_b = ~((omega > 0) & jnp.isfinite(index)).any(axis=1)
    bs = jnp.where(no_b, 1.0, b)
    dts = jnp.where(no_b, 1.0, dt)
    inc = jnp.where(income > 0, income, 1.0)
    k = d["scale_k"] * (dts / bs) ** s.gamma * (inc / bs) ** -eps * jnp.exp(-eps * (1.0 - eps) * d["sigma_k"] ** 2 / 2.0)
    k = jnp.where(income > 0, k, jnp.inf)
    raw = jnp.where(jnp.isfinite(index), s_b + k[:, None] * (s_d - s_b), 0.0)
    raw = jnp.where(no_b[:, None], s_d, raw)
    raw = jnp.maximum(raw, 0.0)
    raw = raw / raw.sum(axis=1, keepdims=True)
    return raw[:, category] * within


def _value_added_price(s: Static, d: dict, price, consumer):
    coef = d["coef"]
    uses = coef > 0
    q = consumer[d["ep"]]
    cost = jnp.where(uses, coef * jnp.where(uses, q, 0.0), 0.0).sum(axis=1)
    return (price[d["ep"], d["eg"]] - cost) / d["a_va"]


def _consumer(s: Static, d: dict, price, cut: bool):
    src = _sourcing(s, d, price, cut)
    return src, jnp.where(d["traded"][None, :], src["index"], price)


def productivity_of(s: Static, d: dict, x, cut: bool = False):
    """`calibrate.productivity_of`."""
    price, wage = _split(s, d, x)
    _, consumer = _consumer(s, d, price, cut)
    ep = d["ep"]
    p_va = jnp.maximum(_value_added_price(s, d, price, consumer), calibrate.PVA_FLOOR * price[ep, d["eg"]])
    t = d["target"]
    ratio = _labour_ratio(s, wage[ep], d["l0"], t, d["s_l"])
    return t / (p_va * d["va0"] * _value_added(s, 1.0, 1.0, 1.0, d["s_l"], ratio))


def evaluate(s: Static, d: dict, x, a, cut: bool = False) -> dict:
    """The quantities of `model.evaluate` that `model.market_residual` reads."""
    n_p, n_g = s.n_p, s.n_g
    traded = d["traded"][None, :]
    price, wage = _split(s, d, x)
    src, consumer = _consumer(s, d, price, cut)
    ep, eg = d["ep"], d["eg"]
    coef = d["coef"]
    uses = coef > 0
    sale = price[ep, eg]
    p_va = _value_added_price(s, d, price, consumer)
    l_star = _labour_target(s, p_va, wage[ep], a, d["va0"], d["l0"], d["s_l"])
    y = _value_added(s, a, d["va0"], d["l0"], d["s_l"], l_star) / d["a_va"]
    out = _seg(y, d["ecell"], n_p * n_g).reshape(n_p, n_g)

    use = coef * y[:, None]
    q = consumer[ep]
    buys = uses & traded & jnp.isfinite(q)
    input_spend = jnp.where(buys, use * jnp.where(buys, q, 0.0), 0.0)
    cost = input_spend.sum(axis=1) + jnp.where(traded, 0.0, price[ep] * use).sum(axis=1)
    profit = sale * y - wage[ep] * l_star - cost
    spend_int = _seg(input_spend, ep, n_p)
    local_use = _seg(jnp.where(traded, 0.0, use), ep, n_p)

    # tariff per unit spent: `model._flows(..., ones, src)["tariff"]`
    phi, wf = src["phi"], src["w"]
    reached = phi >= s.weight_floor
    unit = jnp.where(d["traded"][:, None], 1.0, 0.0) * jnp.ones((n_g, n_p))
    reach1 = jnp.where(reached, unit.ravel() / jnp.where(reached, phi, 1.0), 0.0)
    duty_share = (reach1 * (_from_origins(s, d, wf, None, d["acc_k"], d["cro_duty"], cut) - wf)).reshape(n_g, n_p).T

    pc, kp, kc = d["pc"], d["kp"], d["kc"]
    people = d["people"]
    earned = d["eff_share"] * (wage * d["labour_supply"] + _seg(profit, ep, n_p))[kp]
    from_inputs = _seg((duty_share * spend_int).sum(axis=1), pc, s.n_c)
    prices_k = consumer[kp]

    def spend_of(rebate):
        pop_income = earned + d["people_share"] * rebate[kc]
        per_person = jnp.where(people > 0, pop_income / jnp.where(people > 0, people, 1.0), 0.0)
        share = _pigl_shares(s, d, prices_k, per_person)
        pop_spend = jnp.where(pop_income[:, None] > 0, share * pop_income[:, None], 0.0)
        spend_hh = _seg(pop_spend, kp, n_p)
        return spend_hh, from_inputs + _seg((duty_share * spend_hh).sum(axis=1), pc, s.n_c)

    def body(c):
        rebate, _, _, k = c
        spend_hh, nxt = spend_of(rebate)
        done = jnp.all(jnp.abs(nxt - rebate) <= model.REBATE_TOL * jnp.maximum(jnp.abs(nxt), 1e-300))
        return nxt, spend_hh, done, k + 1

    def cond(c):
        return (~c[2]) & (c[3] < model.MAX_REBATE_ITERATIONS)

    zero = jnp.zeros(s.n_c)
    _, spend_hh, _, _ = jax.lax.while_loop(cond, body, (zero, jnp.zeros((n_p, n_g)), jnp.array(False), 0))

    spend = jnp.where(traded, spend_hh + spend_int, 0.0)
    ef = spend.T.ravel()
    reach = jnp.where(reached, ef / jnp.where(reached, phi, 1.0), 0.0)
    orders = src["per_price"] * _to_destinations(s, d, reach, d["dom_k"], d["acc_k"], d["cro_net"], cut)
    orders = orders.reshape(n_g, n_p).T
    pos = price > 0
    local = jnp.where(traded, 0.0, jnp.where(pos, spend_hh / jnp.where(pos, price, 1.0), 0.0) + local_use)
    demand = jnp.where(traded, orders, local)
    fin = jnp.isfinite(consumer)
    iw = jnp.where(fin, d["index_weights"], 0.0)
    index = (iw * jnp.where(fin, consumer, 0.0)).sum() / iw.sum()
    return {"demand": demand, "q": out, "labour_demand": _seg(l_star, ep, n_p), "index": index}


def market_residual(s: Static, d: dict, e: dict):
    """`model.market_residual`."""
    cells = d["cells"]
    eps = d["eps_cells"]
    goods = jnp.log((e["demand"].ravel()[cells] + eps) / (e["q"].ravel()[cells] + eps))
    labour = (e["labour_demand"] - d["labour_supply"]) / d["labour_supply"]
    return jnp.concatenate([goods, labour]).at[s.index_row].set(jnp.log(e["index"]))


def log_index(s: Static, d: dict, x):
    """ln(world price index), the index row of the residual."""
    price, _ = _split(s, d, x)
    _, consumer = _consumer(s, d, price, False)
    fin = jnp.isfinite(consumer)
    iw = jnp.where(fin, d["index_weights"], 0.0)
    return jnp.log((iw * jnp.where(fin, consumer, 0.0)).sum() / iw.sum())


def residual(s: Static, d: dict, x, cut: bool = False):
    """`calibrate.residual`."""
    return market_residual(s, d, evaluate(s, d, x, productivity_of(s, d, x, cut), cut))


class System:
    """The calibration residual of one set of inputs, jitted, with exact derivatives."""

    def __init__(self, inp: Inputs):
        self.inp = inp
        self.static = Static(
            n_p=inp.n_provinces,
            n_g=inp.n_goods,
            n_n=inp.n_gateways,
            n_c=inp.n_countries,
            n_cells=len(inp.active_cells),
            index_row=model.index_row(inp),
            rho=model.rho(inp),
            weight_floor=inp.weight_floor,
            sigma_demand=inp.sigma_demand,
            sigma_b=inp.pigl.sigma_b,
            phi=inp.pigl.phi,
            gamma=inp.pigl.gamma,
            eps=inp.pigl.eps,
        )
        self.data = _arrays(inp)
        self.n = self.static.n_cells + inp.n_provinces
        s = self.static
        self._residual = jax.jit(partial(residual, s), static_argnames="cut")
        self._productivity = jax.jit(partial(productivity_of, s))

        def jvp(d, x, v, cut):
            return jax.jvp(lambda z: residual(s, d, z, cut), (x,), (v,))[1]

        self._jvp = jax.jit(jvp, static_argnames="cut")
        self._jvps = jax.jit(jax.vmap(jvp, in_axes=(None, None, 0, None)), static_argnames="cut")
        self._index_grad = jax.jit(jax.grad(partial(log_index, s), argnums=1))

    @property
    def index_row(self) -> int:
        return self.static.index_row

    def residual(self, x: np.ndarray, cut: bool = False) -> np.ndarray:
        return np.array(self._residual(self.data, jnp.asarray(x), cut=cut))

    def productivity(self, x: np.ndarray) -> np.ndarray:
        return np.array(self._productivity(self.data, jnp.asarray(x)))

    def jvp(self, x: np.ndarray, v: np.ndarray, cut: bool = False) -> np.ndarray:
        """J(x) v, exact."""
        return np.array(self._jvp(self.data, jnp.asarray(x), jnp.asarray(v), cut=cut))

    def jvps(self, x: np.ndarray, v: np.ndarray, cut: bool = False, batch: int = BATCH) -> np.ndarray:
        """J(x) V for the rows of v (k, n), batched: (n, k)."""
        out = np.empty((self.n, len(v)))
        xj = jnp.asarray(x)
        for i in range(0, len(v), batch):
            chunk = v[i : i + batch]
            pad = np.zeros((batch, self.n))  # one shape, one compilation
            pad[: len(chunk)] = chunk
            out[:, i : i + len(chunk)] = np.asarray(self._jvps(self.data, xj, jnp.asarray(pad), cut))[: len(chunk)].T
        return out

    def index_gradient(self, x: np.ndarray) -> np.ndarray:
        """The index row of the Jacobian (reverse mode)."""
        return np.array(self._index_grad(self.data, jnp.asarray(x)))
