"""Steady state of the P2 tick pipeline with frictions off (`specs/econ/SPEC.md`, GE reference
model). Mirrors `ceteris-trade` `route` and `ceteris-econ` E1 to E4 at a fixed point: frictionless
wage (full employment), D = Q on every active market, input stocks at two months of use (never
binding, orders equal use), pops spending their whole income.

Unknowns x: ln p of every active (province, good) cell in row-major order, then ln w per province.
Equations: excess demand of every active market, ln(D / Q) (with a floor far below market size),
and of every labour market, (sum L* - L^s) / L^s. Market cash flow values supply Q at the posted
price and households are paid w L^s (full employment), so Walras' law reads
sum p (D - Q) + sum w (L* - L^s) = 0 identically; the labour equation of the province with the
largest base wage bill is replaced by ln(world price index) = 0.
"""

from dataclasses import dataclass, field

import numpy as np

from .io import Inputs

RHO_CD = 1e-9  # Cobb-Douglas threshold on |rho|, as `production.rs`
MARKET_FLOOR = 1e-12  # added to D and Q, relative to the cell's base demand: ln(D / Q) stays finite


@dataclass
class Equilibrium:
    x: np.ndarray
    price: np.ndarray  # (P, G): posted price of active cells, netback of inactive traded cells
    wage: np.ndarray  # (P,)
    employment: np.ndarray  # (E,)
    output: np.ndarray  # (E,)
    supply: np.ndarray  # (P, G) traded output routed from each cell
    home_value: np.ndarray  # (P, G) supply routed to stay home, at the origin price
    home_units: np.ndarray  # (P, G)
    flows_by_country_pair_good: np.ndarray  # (C, C, G) FOB value, exporter x importer
    units_by_country_pair_good: np.ndarray  # (C, C, G) origin units at dispatch
    income: np.ndarray  # (P,) household income
    price_index: float
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


def _routing(inp: Inputs, price: np.ndarray):
    """Route weights (p / p_max)^theta with the weight floor, access sums v, u, Pi and the
    per-unit netback of every cell, as `route.rs` computes them. Good-major (G, P) and (G * N)."""
    n_p, n_g, n_n = inp.n_provinces, inp.n_goods, inp.n_gateways
    dom, acc, cro = inp.domestic, inp.access, inp.crossing
    d_from, d_to, a_prov, a_gate, c_from, c_to = _legs(inp)
    p = price.T
    act = inp.active.T
    p_max = np.where(act, p, 0.0).max(axis=1, keepdims=True)
    with np.errstate(divide="ignore", invalid="ignore"):
        pt = np.where(act & (p_max > 0), (p / p_max) ** inp.theta, 0.0)
    pt[pt < inp.weight_floor] = 0.0
    pt[~inp.traded] = 0.0
    ptf, pf = pt.ravel(), p.ravel()

    v = _sum(a_gate, acc.k * ptf[a_prov], n_g * n_n)
    u = _sum(c_from, cro.k * v[c_to], n_g * n_n)
    pi = ptf + _sum(d_from, dom.k * ptf[d_to], n_g * n_p) + _sum(a_prov, acc.k * u[a_gate], n_g * n_p)

    # destination value per unit entering gateway b, then per unit leaving gateway a
    dest = _sum(a_gate, acc.k * ptf[a_prov] * pf[a_prov] / acc.tau, n_g * n_n)
    dest = np.divide(dest, v, out=np.zeros_like(v), where=v > 0)
    onward = _sum(c_from, cro.k * v[c_to] * dest[c_to] / (cro.tau * cro.border_tau), n_g * n_n)
    onward = np.divide(onward, u, out=np.zeros_like(u), where=u > 0)
    back = (
        ptf * pf
        + _sum(d_from, dom.k * ptf[d_to] * pf[d_to] / dom.tau, n_g * n_p)
        + _sum(a_prov, acc.k * u[a_gate] * onward[a_gate] / acc.tau, n_g * n_p)
    )
    netback = np.divide(back, pi, out=np.zeros_like(pi), where=pi > 0)
    return pt, v, u, pi, netback.reshape(n_g, n_p).T


def _flows(inp: Inputs, price: np.ndarray, supply: np.ndarray, pt, v, u, pi) -> dict:
    """Shipments of `supply` (P, G) under the merchant logit: home flow, arrivals (destination
    units after every leg's loss), FOB received and paid, tariffs by destination, and country-pair
    value and origin units as `flows.rs` records them."""
    n_p, n_g, n_n, n_c = inp.n_provinces, inp.n_goods, inp.n_gateways, inp.n_countries
    dom, acc, cro = inp.domestic, inp.access, inp.crossing
    d_from, d_to, a_prov, a_gate, c_from, c_to = _legs(inp)
    pf, sf, ptf = price.T.ravel(), supply.T.ravel(), pt.ravel()

    # an origin with no route keeps its supply home
    scale = np.divide(sf, pi, out=np.zeros_like(sf), where=pi > 0)
    home = np.where(pi > 0, scale * ptf, sf)
    x_dom = scale[d_from] * dom.k * ptf[d_to]
    x_out = scale[a_prov] * acc.k * u[a_gate]

    gate_units = _sum(a_gate, x_out / acc.tau, n_g * n_n)
    gate_value = _sum(a_gate, pf[a_prov] * x_out, n_g * n_n)
    gate_origin = _sum(a_gate, x_out, n_g * n_n)
    share = np.divide(cro.k * v[c_to], u[c_from], out=np.zeros(len(c_from)), where=u[c_from] > 0)
    cross_value = gate_value[c_from] * share
    at_units = _sum(c_to, gate_units[c_from] * share / (cro.tau * cro.border_tau), n_g * n_n)
    at_value = _sum(c_to, cross_value, n_g * n_n)
    at_tariff = _sum(c_to, cro.tariff * cross_value, n_g * n_n)
    last = np.divide(acc.k * ptf[a_prov], v[a_gate], out=np.zeros(len(a_gate)), where=v[a_gate] > 0)

    arrive = _sum(d_to, x_dom / dom.tau, n_g * n_p) + _sum(a_prov, at_units[a_gate] * last / acc.tau, n_g * n_p)
    received = _sum(d_from, pf[d_from] * x_dom, n_g * n_p) + _sum(a_prov, pf[a_prov] * x_out, n_g * n_p)
    paid = _sum(d_to, pf[d_from] * x_dom, n_g * n_p) + _sum(a_prov, at_value[a_gate] * last, n_g * n_p)
    tariff = _sum(a_prov, at_tariff[a_gate] * last, n_g * n_p)

    pc, gc = inp.province_country, inp.gateway_country
    n_pairs = n_c * n_c * n_g
    pair_dom = (pc[dom.row] * n_c + pc[dom.col]) * n_g + dom.good
    pair_int = (gc[cro.row] * n_c + gc[cro.col]) * n_g + cro.good
    value = _sum(pair_dom, pf[d_from] * x_dom, n_pairs) + _sum(pair_int, cross_value, n_pairs)
    units = _sum(pair_dom, x_dom, n_pairs) + _sum(pair_int, gate_origin[c_from] * share, n_pairs)

    def cells(a):
        return a.reshape(n_g, n_p).T

    return {
        "home": cells(home),
        "arrive": cells(arrive),
        "received": cells(received),
        "paid": cells(paid),
        "tariff": cells(tariff),
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


def evaluate(x: np.ndarray, inp: Inputs) -> dict:
    """Every quantity of the steady state at prices and wages `x`."""
    n_p, n_g = inp.n_provinces, inp.n_goods
    price, wage = _split(x, inp)
    pt, v, u, pi, netback = _routing(inp, price)
    price = np.where(~inp.active & inp.traded[None, :], netback, price)

    # E1, E2: frictionless labour target and output; sale at the cell's price, inputs at local prices
    ep, eg = inp.ent_province, inp.ent_good
    cost = (inp.ent_inputs * price[ep]).sum(axis=1)
    sale = price[ep, eg]
    r = rho(inp)
    # Rust caps each target at L^s. At a clearing wage no target exceeds L^s unless one enterprise
    # alone employs the province (then Rust finds no clearing wage either), so the cap never binds
    # at a steady state; leaving it out keeps labour demand responsive to the wage everywhere.
    l_star = labour_target((sale - cost) / inp.ent_a_va, wage[ep], inp.ent_a, inp.ent_va0, inp.ent_l0, inp.ent_s_l, r, np.inf)
    y = value_added(inp.ent_a, inp.ent_va0, inp.ent_l0, inp.ent_s_l, r, l_star) / inp.ent_a_va
    out = np.zeros((n_p, n_g))
    np.add.at(out, (ep, eg), y)
    routed = np.where(inp.traded[None, :], out, 0.0)

    # T2: routing; T1: home flow plus arrivals make up Q of traded goods
    f = _flows(inp, price, routed, pt, v, u, pi)
    q = np.where(inp.traded[None, :], f["home"] + f["arrive"], out)
    inputs = np.zeros((n_p, n_g))
    np.add.at(inputs, ep, inp.ent_inputs * y[:, None])

    # E4: wages w L^s, profits, market cash flow (supply valued at the posted price), rebates
    profit = sale * y - wage[ep] * l_star - cost * y
    tariff = f["tariff"].sum(axis=1)
    market = (
        np.where(inp.active, price * q, 0.0).sum(axis=1)
        - _sum(ep, sale * y, n_p)
        + f["received"].sum(axis=1)
        - f["paid"].sum(axis=1)
        - tariff
    )
    c = inp.province_country
    country_tariff = np.bincount(c, tariff, inp.n_countries)
    country_pop = np.bincount(c, inp.population, inp.n_countries)
    rebate = country_tariff[c] * inp.population / country_pop[c]
    income = wage * inp.labour_supply + _sum(ep, profit, n_p) + market + rebate

    # E3: CES household demand at local prices; pops share prices, so the province total is exact
    om = inp.omega_p
    with np.errstate(divide="ignore"):
        weight = np.where(om > 0, om * price ** (-inp.sigma_demand), 0.0)
    households = weight * (income / (weight * price).sum(axis=1))[:, None]

    return {
        "price": price,
        "wage": wage,
        "employment": l_star,
        "output": y,
        "supply": routed,
        "flows": f,
        "demand": households + inputs,
        "q": q,
        "labour_demand": _sum(ep, l_star, n_p),
        "income": income,
        "index": (inp.index_weights * price).sum() / inp.index_weights.sum(),
    }


def _excess(s: dict, inp: Inputs) -> np.ndarray:
    cells = inp.active_cells
    eps = MARKET_FLOOR * inp.base_demand.ravel()[cells]
    goods = np.log((s["demand"].ravel()[cells] + eps) / (s["q"].ravel()[cells] + eps))
    labour = (s["labour_demand"] - inp.labour_supply) / inp.labour_supply
    return np.concatenate([goods, labour])


def excess(x: np.ndarray, inp: Inputs) -> np.ndarray:
    """Scaled excess demand of every market, all labour markets included."""
    return _excess(evaluate(x, inp), inp)


def residual(x: np.ndarray, inp: Inputs) -> np.ndarray:
    """`excess` with the dropped labour equation replaced by ln(world price index)."""
    s = evaluate(x, inp)
    r = _excess(s, inp)
    r[index_row(inp)] = np.log(s["index"])
    return r


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
        home_value=s["price"] * f["home"],
        home_units=f["home"],
        flows_by_country_pair_good=f["pair_value"],
        units_by_country_pair_good=f["pair_units"],
        income=s["income"],
        price_index=float(s["index"]),
        info=info or {},
    )
