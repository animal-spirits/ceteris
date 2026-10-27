import dataclasses
import json
import os
from pathlib import Path

import numpy as np
import pyarrow as pa
import pytest
from hypothesis import given, settings
from hypothesis import strategies as st
from pyarrow import ipc as pa_ipc

from ceteris.reference import demand, io, model, ppml, sinkhorn, solve, uniqueness

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = Path(os.environ.get("CETERIS_FIXTURE_DIR", ROOT / "tests" / "golden" / "p2-fixture"))


@pytest.fixture(scope="module")
def inputs():
    return io.load(FIXTURE)


@pytest.fixture(scope="module")
def fixture(inputs):
    return inputs, solve.equilibrium(inputs)


def test_residual_zero_at_solution(fixture):
    inp, eq = fixture
    r = model.residual(eq.x, inp)
    assert np.abs(r).max() <= 1e-10
    # Walras' law: the labour equation replaced by the price index holds too.
    full = model.excess(eq.x, inp)
    assert np.abs(full).max() <= 1e-9
    assert eq.price_index == pytest.approx(1.0, abs=1e-10)
    assert np.all(eq.price[inp.active] > 0) and np.all(eq.wage > 0)
    assert np.all(eq.income > 0)


UNREACHABLE, ISLAND_USER = 6, 4  # as `fixture::UNREACHABLE` and `fixture::ISLAND_USER`


def unreachable_island(inp: io.Inputs) -> io.Inputs:
    """The fixture as `fixture::unreachable_island` exports it: machinery without a route to or
    from the island port (kernel factor 0, tau infinite so nothing arrives), and of the island's
    sectors only light manufacturing using machinery."""
    island = inp.n_countries - 1
    port = np.flatnonzero(inp.gateway_country == island)
    cro = inp.crossing
    cut = (cro.good == UNREACHABLE) & (np.isin(cro.row, port) | np.isin(cro.col, port))
    crossing = dataclasses.replace(cro, k=np.where(cut, 0.0, cro.k), inv_loss=np.where(cut, 0.0, cro.inv_loss))
    a = inp.io.copy()
    a[island, UNREACHABLE, np.arange(inp.n_goods) != ISLAND_USER] = 0.0
    return dataclasses.replace(inp, io=a, crossing=crossing)


def test_unreachable_destination_solves(inputs):
    """Review Focus 1 at world level: with machinery unreachable at the island its delivered price
    there is infinite, the world price index leaves that cell out (as `world_price_index`), the
    island spends nothing on machinery, the island's light manufacturing (its one user) employs
    and produces nothing, and the reference solves with a finite residual."""
    inp = unreachable_island(inputs)
    island = inp.n_provinces - 1
    assert np.isinf(model.evaluate(model.data_state(inp), inp)["consumer_price"][island, UNREACHABLE])
    assert np.isfinite(model.residual(model.data_state(inp), inp)).all()
    eq = solve.equilibrium(inp)
    r = model.residual(eq.x, inp)
    assert np.isfinite(r).all() and np.abs(r).max() <= 1e-10
    assert eq.price_index == pytest.approx(1.0, abs=1e-10)
    assert eq.consumer_price[island, UNREACHABLE] == np.inf
    assert eq.expenditure[island, UNREACHABLE] == 0.0
    assert np.isfinite(np.delete(eq.consumer_price.ravel(), island * inp.n_goods + UNREACHABLE)).all()
    on_island = inp.ent_province == island
    user = on_island & (inp.ent_good == ISLAND_USER)
    assert eq.employment[user] == 0.0 and eq.output[user] == 0.0
    assert np.all(eq.output[on_island & ~user] > 0)
    for v in (eq.price, eq.wage, eq.income, eq.flows_by_country_pair_good, eq.units_by_country_pair_good):
        assert np.isfinite(v).all()
    assert np.all(eq.wage > 0) and np.all(eq.income > 0)


def test_scale_invariance(inputs):
    inp = inputs
    rng = np.random.default_rng(7)
    x = model.data_state(inp)
    x = x + rng.normal(0.0, 0.2, len(x))
    base = model.residual(x, inp)
    scaled = model.residual(x + np.log(3.7), inp)
    row = model.index_row(inp)
    homogeneous = np.arange(len(x)) != row
    np.testing.assert_allclose(scaled[homogeneous], base[homogeneous], rtol=1e-10, atol=1e-13)
    assert scaled[row] - base[row] == pytest.approx(np.log(3.7), abs=1e-12)


def _weights(inp: io.Inputs, g: int, p: np.ndarray, act: np.ndarray) -> np.ndarray:
    """Origin weights (p / p_min)^-theta_g of the active origins, one p_min per good."""
    with np.errstate(divide="ignore"):
        return np.where(act, (p / p[act].min()) ** -inp.theta[g], 0.0)


def test_sinkhorn_and_dense_kernel_reproduce_equilibrium_gravity(fixture):
    """S02 (ADR 0016): balanced Sinkhorn on the dense kernel, with the equilibrium's origin supplies
    (value at origin prices) and destination expenditures (net of tariff) as marginals, reproduces
    the sourcing flows w_o K_od E_d / Phi_d; the dense flows aggregate to the model's low-rank
    country-pair value and units."""
    inp, eq = fixture
    countries = inp.province_country
    for g in np.flatnonzero(inp.traded):
        k, k_net = sinkhorn.dense_kernel(inp, g), sinkhorn.dense_kernel(inp, g, net_of_tariff=True)
        p = eq.price[:, g]
        w = _weights(inp, g, p, inp.active[:, g])
        phi = w @ k
        flows = w[:, None] * k_net * (eq.expenditure[:, g] / phi)[None, :]
        supply = p * eq.supply[:, g]
        # the origin markets clear: what destinations send them is their output at their price
        np.testing.assert_allclose(flows.sum(axis=1), supply, rtol=1e-8, atol=1e-12 * supply.sum())
        # balanced on the flows' own row sums: the solve's tolerance leaves supply inconsistent with
        # the column sums at about 1e-11, beyond what balancing to Sinkhorn's 1e-14 can absorb
        solved = sinkhorn.balanced(k_net, flows.sum(axis=1), flows.sum(axis=0))
        big = flows > 1e-12 * flows.sum()
        np.testing.assert_allclose(solved[big], flows[big], rtol=1e-8)
        assert np.abs(solved[~big]).max(initial=0.0) <= 1e-10 * flows.sum()
        shipped = flows.copy()
        np.fill_diagonal(shipped, 0.0)
        for got, per in (
            (eq.flows_by_country_pair_good, np.ones_like(p)),
            (eq.units_by_country_pair_good, np.where(p > 0, p, 1.0)),
        ):
            pairs = np.zeros((inp.n_countries, inp.n_countries))
            np.add.at(pairs, (countries[:, None], countries[None, :]), shipped / per[:, None])
            np.testing.assert_allclose(got[:, :, g], pairs, rtol=1e-9, atol=1e-9 * pairs.sum())


def dense_route_oracle(inp: io.Inputs, price: np.ndarray, spend: np.ndarray) -> dict:
    """Every route enumerated from the exported legs: home, o -> d inside a country, and
    o -> a -> b -> d over every gateway pair, with its own K, iceberg and tariff. Destination d
    spends E_d over routes by K w_o / Phi_d; the buyer pays p_o (1 + t) per shipped unit, t / (1 + t)
    of it is tariff, the origin receives p_o per unit; d receives the units over the route's tau.
    Routes carry 1 / tau and 1 / (1 + t), as the export does."""
    n_p, n_g = price.shape
    keys = ("orders", "home", "arrive", "paid", "received", "tariff", "index")
    out = {key: np.zeros((n_p, n_g)) for key in keys}
    n_c, country = inp.n_countries, inp.province_country
    out["pair_value"], out["pair_units"] = np.zeros((n_c, n_c, n_g)), np.zeros((n_c, n_c, n_g))
    dom, acc, cro = inp.domestic, inp.access, inp.crossing
    for g in np.flatnonzero(inp.traded):
        p = price[:, g]
        w = _weights(inp, g, p, inp.active[:, g])
        routes = [(o, o, 1.0, 1.0, 1.0) for o in range(n_p)]  # (o, d, k, 1 / tau, 1 / (1 + t))
        for j in np.flatnonzero(dom.good == g):
            routes.append((dom.row[j], dom.col[j], dom.k[j], dom.inv_tau[j], 1.0))
        gate_in = [[] for _ in range(inp.n_gateways)]  # b -> d: (d, k, 1 / tau)
        for j in np.flatnonzero(acc.good == g):
            gate_in[acc.col[j]].append((acc.row[j], acc.k[j], acc.inv_tau[j]))
        for ja in np.flatnonzero(acc.good == g):
            o, a = acc.row[ja], acc.col[ja]
            for jc in np.flatnonzero((cro.good == g) & (cro.row == a)):
                for d, k_bd, inv_bd in gate_in[cro.col[jc]]:
                    k = acc.k[ja] * cro.k[jc] * k_bd
                    arrives = acc.inv_tau[ja] * cro.inv_loss[jc] * inv_bd
                    routes.append((o, d, k, arrives, cro.inv_duty[jc]))
        phi = np.zeros(n_p)
        for o, d, k, _, _ in routes:
            phi[d] += k * w[o]
        alpha = np.zeros(n_p)
        for o, d, k, arrives, net in routes:
            if w[o] == 0.0:
                continue
            x = spend[d, g] * k * w[o] / phi[d]
            units = x * net / p[o]
            alpha[d] += k * w[o] / phi[d] * net / p[o] * arrives
            out["orders"][o, g] += units
            out["arrive"][d, g] += units * arrives
            out["paid"][d, g] += x
            out["tariff"][d, g] += x * (1.0 - net)
            out["received"][o, g] += x * net
            if o == d:
                out["home"][d, g] += units
            else:
                out["pair_value"][country[o], country[d], g] += x * net
                out["pair_units"][country[o], country[d], g] += units
        out["index"][:, g] = 1.0 / alpha
    return out


def test_inputs_carry_theta_per_good(inputs):
    """ADR 0018: theta is read per good from `goods.arrow`, and the sourcing weights use each good's."""
    inp = inputs
    goods = pa_ipc.open_file(str(FIXTURE / "goods.arrow")).read_all()
    np.testing.assert_array_equal(inp.theta, goods["theta"].to_numpy())
    traded = inp.theta[inp.traded]
    assert len(np.unique(traded)) > 1 and np.all(traded > 0) and np.all(inp.theta[~inp.traded] == 0)
    rng = np.random.default_rng(3)
    price = np.exp(rng.uniform(-0.5, 0.5, (inp.n_provinces, inp.n_goods)))
    w = model._sourcing(inp, price)["w"].reshape(inp.n_goods, inp.n_provinces)
    for g in np.flatnonzero(inp.traded):
        np.testing.assert_allclose(w[g], _weights(inp, g, price[:, g], inp.active[:, g]), rtol=1e-14)


def test_trade_money_matches_dense_route_oracle(inputs):
    inp = inputs
    assert np.any(inp.crossing.inv_duty < 1)
    rng = np.random.default_rng(11)
    price = np.exp(rng.uniform(-2.3, 2.3, (inp.n_provinces, inp.n_goods)))
    spend = np.where(inp.traded[None, :], rng.uniform(0.0, 1e6, price.shape), 0.0)
    want = dense_route_oracle(inp, price, spend)
    src = model._sourcing(inp, price)
    got = model._flows(inp, price, spend, src)
    got["index"] = src["index"]
    traded = inp.traded
    for key, w in want.items():
        np.testing.assert_allclose(got[key][..., traded], w[..., traded], rtol=1e-10, atol=1e-13 * np.abs(w).max(), err_msg=key)


def _assert_routing_matches_rust(inp: io.Inputs, export: Path) -> None:
    """The reference routes on the kernel Rust exports, and that kernel is the spec's: every crossing
    factor rebuilt from the legs and tariffs as K_ab = (tau (1 + nu) (1 + t))^-theta, with
    1 / (tau (1 + nu)) and 1 / (1 + t), to 1e-12; and the reference's delivered prices at prices 1
    are Rust's base prices to 1e-12."""

    def table(name):
        return pa_ipc.open_file(str(export / f"{name}.arrow")).read_all()

    legs, goods, tariffs = table("legs"), table("goods"), table("tariffs")
    nu = json.loads((export / "params.json").read_text())["trade"]["border_nu"]
    gc = inp.gateway_country
    rate = np.zeros((inp.n_countries, inp.n_countries, 2))
    rate[tariffs["importer"].to_numpy(), tariffs["exporter"].to_numpy(), tariffs["group"].to_numpy()] = tariffs["rate"].to_numpy()
    crossing = legs["kind"].to_numpy() == 2
    a, b = (legs[c].to_numpy()[crossing].astype(np.int64) for c in ("from", "to"))
    cost, hours = legs["cost_per_t"].to_numpy()[crossing], legs["hours"].to_numpy()[crossing]
    weight, time_cost, group = goods["weight"].to_numpy(), goods["time_cost"].to_numpy(), goods["tariff_group"].to_numpy()
    cro = inp.crossing
    for g in np.flatnonzero(inp.traded):
        tau = 1.0 + cost * weight[g] + time_cost[g] * hours / 24.0
        duty = 1.0 + rate[gc[b], gc[a], group[g]]
        want = {(x, y): i for i, (x, y) in enumerate(zip(a, b))}
        sel = np.flatnonzero(cro.good == g)
        at = np.array([want[(x, y)] for x, y in zip(cro.row[sel], cro.col[sel])])
        k = (tau * (1.0 + nu) * duty) ** -inp.theta[g]
        np.testing.assert_allclose(cro.k[sel], k[at], rtol=1e-12, atol=0)
        np.testing.assert_allclose(cro.inv_loss[sel], 1.0 / (tau * (1.0 + nu))[at], rtol=1e-12, atol=0)
        np.testing.assert_allclose(cro.inv_duty[sel], 1.0 / duty[at], rtol=1e-12, atol=0)
    q = model._sourcing(inp, np.ones((inp.n_provinces, inp.n_goods)))["index"]
    np.testing.assert_allclose(inp.base_price[:, inp.traded], q[:, inp.traded], rtol=1e-12)


def test_routing_matches_rust(inputs):
    _assert_routing_matches_rust(inputs, FIXTURE)


def test_routing_matches_rust_on_the_pack():
    """`_assert_routing_matches_rust` on `CETERIS_PACK_EXPORT`, a `ceteris export --pack` directory."""
    export = os.environ.get("CETERIS_PACK_EXPORT")
    if not export:
        pytest.skip("skipping: CETERIS_PACK_EXPORT unset (a `ceteris export --pack` directory)")
    _assert_routing_matches_rust(io.load(export), Path(export))


def test_pigl_matches_rust(inputs):
    """The reference's PIGL (its own inversion of the country weights, its shares) reproduces
    `demand::pigl` on the export's `pigl_check` draws: prices (some unreachable), money per person
    far from base on both sides (clamped shares), income dispersion."""
    t = pa_ipc.open_file(str(FIXTURE / "pigl_check.arrow")).read_all()
    n_g = inputs.n_goods
    country = t["country"].to_numpy().astype(np.int64)
    money, sigma = t["money"].to_numpy(), t["sigma"].to_numpy()
    price = np.stack([t[f"price_{g}"].to_numpy() for g in range(n_g)], axis=1)
    want = np.stack([t[f"x_{g}"].to_numpy() for g in range(n_g)], axis=1)
    assert len(money) == 100 and np.isinf(price).any()
    share = demand.shares(inputs.pigl_weights, country, price, money, sigma, inputs.category, inputs.sigma_demand, inputs.pigl)
    got = demand.quantities(share, price, money)
    np.testing.assert_array_equal(got == 0, want == 0)
    np.testing.assert_allclose(got, want, rtol=1e-12, atol=0)
    # shares within [0, 1] adding up, also where the raw PIGL shares left the interval
    np.testing.assert_allclose(share.sum(axis=1), 1.0, rtol=1e-14)
    assert np.all((share >= 0) & (share <= 1))


def test_pigl_inversion_reproduces_base_shares(inputs):
    inp = inputs
    w = inp.pigl_weights
    c = np.arange(inp.n_countries)
    share = demand.shares(
        w, c, np.ones((inp.n_countries, inp.n_goods)), inp.base_spend, inp.sigma_income, inp.category, inp.sigma_demand, inp.pigl
    )
    np.testing.assert_allclose(share, inp.omega / inp.omega.sum(axis=1, keepdims=True), rtol=0, atol=1e-12)
    assert np.all(w.omega[:, 0] == 0) and np.all(w.scale > 0)


def two_provinces(theta, tau, sigma_demand, sigma_fl, s_l, a, labour, u) -> io.Inputs:
    """One traded good, one country, two identical provinces joined by a domestic leg, with the
    Rust base point: L0 = (1 - u) L^s, w0 = s_L VA0 / L0. Each province has two half-size
    enterprises: with a single one, the target's cap at L^s leaves labour demand at or below
    supply for every low wage, so no wage clears uniquely. The good is the only one of PIGL
    category S, so the inversion gives z = 0 (no category A demand) and demand is homothetic: the
    province spends its whole income on it, whatever the pops' incomes."""
    k = tau**-theta
    one = np.ones(2)
    l0 = (1.0 - u) * labour / 2.0
    empty = io.Entries(*(np.zeros(0, dtype=np.int64),) * 3, np.zeros(0), np.zeros(0))
    return io.Inputs(
        theta=np.array([theta]),
        sigma_demand=sigma_demand,
        pigl=demand.Pigl(sigma_b=0.26, phi=0.0, gamma=0.71, theta=np.array([0.634, 0.082, 0.284]), eps=0.71),
        category=np.array([2]),
        sigma_fl=sigma_fl,
        traded=np.array([True]),
        province_country=np.zeros(2, dtype=np.int64),
        gateway_country=np.zeros(0, dtype=np.int64),
        labour_supply=labour * one,
        population=1e6 * one,
        w0=s_l * 50.0 / l0 * one,
        base_price=np.ones((2, 1)),
        s_l=np.array([s_l]),
        unemployment=np.array([u]),
        u_star=np.zeros(1),
        gini=np.array([0.35]),
        omega=np.array([[1.0]]),
        io=np.zeros((1, 1, 1)),
        active=np.ones((2, 1), dtype=bool),
        ent_province=np.array([0, 0, 1, 1]),
        ent_good=np.zeros(4, dtype=np.int64),
        ent_a=np.full(4, a),
        ent_va0=np.full(4, 50.0),
        ent_l0=np.full(4, l0),
        pop_province=np.array([0, 0, 1, 1]),
        pop_effective=np.array([0.3, 0.7, 0.3, 0.7]) * labour,
        pop_population=np.array([4e5, 6e5, 4e5, 6e5]),
        domestic=io.Entries(np.zeros(2, dtype=np.int64), np.array([0, 1]), np.array([1, 0]), k * one, one / tau),
        access=empty,
        crossing=io.Entries(*(np.zeros(0, dtype=np.int64),) * 3, *(np.zeros(0),) * 4, lag=np.zeros(0, dtype=np.int64)),
    )


@settings(max_examples=25, deadline=None)
@given(
    theta=st.floats(4.0, 8.0),
    tau=st.floats(1.01, 2.0),
    sigma_demand=st.floats(0.3, 0.9),
    sigma_fl=st.floats(0.5, 1.2),
    s_l=st.floats(0.4, 0.7),
    a=st.floats(0.5, 2.0),
    labour=st.floats(10.0, 500.0),
    u=st.floats(0.0, 0.15),
)
def test_two_province_closed_form(theta, tau, sigma_demand, sigma_fl, s_l, a, labour, u):
    inp = two_provinces(theta, tau, sigma_demand, sigma_fl, s_l, a, labour, u)
    eq = solve.equilibrium(inp)
    # Buyers pay the delivered unit price q = 1 / alpha: a unit of spending buys home at p and the
    # other province's good at p tau, with sourcing shares 1 / (1 + k) and k / (1 + k); the world
    # index is q, normalized to 1.
    k = tau**-theta
    p = (1.0 + k / tau) / (1.0 + k)
    np.testing.assert_allclose(eq.price[:, 0], [p, p], rtol=1e-9)
    np.testing.assert_allclose(eq.consumer_price[:, 0], [1.0, 1.0], rtol=1e-9)
    # Full employment at the marginal product of labour; per province VA0 = 100, L0 = (1 - u) L^s.
    rho = (sigma_fl - 1.0) / sigma_fl
    l0 = (1.0 - u) * labour
    x = labour / l0
    if abs(rho) < 1e-9:
        y = a * 100.0 * x**s_l
        w = a * 100.0 * s_l * x ** (s_l - 1.0) / l0
    else:
        bracket = (1.0 - s_l) + s_l * x**rho
        y = a * 100.0 * bracket ** (1.0 / rho)
        w = a * 100.0 * bracket ** (1.0 / rho - 1.0) * s_l * x ** (rho - 1.0) / l0
    np.testing.assert_allclose(eq.output, np.full(4, y / 2), rtol=1e-9)
    np.testing.assert_allclose(eq.wage, [p * w, p * w], rtol=1e-9)
    # Each province spends its income p y over the two origins by the sourcing shares.
    np.testing.assert_allclose(eq.home_units[:, 0], y / (1.0 + k), rtol=1e-9)
    assert eq.units_by_country_pair_good[0, 0, 0] == pytest.approx(2.0 * y * k / (1.0 + k), rel=1e-9)
    # value is the origin value of the shipped units
    assert eq.flows_by_country_pair_good[0, 0, 0] == pytest.approx(2.0 * p * y * k / (1.0 + k), rel=1e-9)


def test_fixture_reference_is_current(fixture):
    """`tests/golden/p2-fixture/reference` (read by Rust: runs started from the equilibrium and
    the V1 check) is the current solve of the fixture. Regenerate with
    `python -m ceteris.reference solve --inputs tests/golden/p2-fixture --out tests/golden/p2-fixture/reference`."""
    inp, eq = fixture
    ref = FIXTURE / "reference"
    tables = {
        name: pa_ipc.open_file(ref / f"{name}.arrow").read_all() for name in ("prices", "wages", "enterprises", "flows", "home")
    }
    np.testing.assert_allclose(tables["prices"]["price"].to_numpy(), eq.price.ravel(), rtol=1e-8)
    np.testing.assert_allclose(tables["wages"]["wage"].to_numpy(), eq.wage, rtol=1e-8)
    np.testing.assert_allclose(tables["enterprises"]["employment"].to_numpy(), eq.employment, rtol=1e-8)
    np.testing.assert_allclose(tables["enterprises"]["output"].to_numpy(), eq.output, rtol=1e-8)
    flows = tables["flows"]
    ex, im, g = (flows[c].to_numpy() for c in ("exporter_country", "importer_country", "good"))
    for got, want in zip((ex, im, g), np.nonzero(eq.flows_by_country_pair_good > 0)):
        np.testing.assert_array_equal(got, want)
    scale = eq.flows_by_country_pair_good.max()
    np.testing.assert_allclose(flows["value"].to_numpy(), eq.flows_by_country_pair_good[ex, im, g], rtol=1e-8, atol=1e-8 * scale)
    np.testing.assert_allclose(flows["units"].to_numpy(), eq.units_by_country_pair_good[ex, im, g], rtol=1e-8, atol=1e-8 * scale)
    traded = inp.traded[np.tile(np.arange(inp.n_goods), inp.n_provinces)]
    np.testing.assert_allclose(tables["home"]["value"].to_numpy(), eq.home_value.ravel()[traded], rtol=1e-8, atol=1e-8 * scale)
    np.testing.assert_allclose(tables["home"]["units"].to_numpy(), eq.home_units.ravel()[traded], rtol=1e-8, atol=1e-8 * scale)
    meta = json.loads((ref / "solution.json").read_text())
    assert meta["static_digest"] == inp.static_digest
    assert meta["price_index"] == pytest.approx(eq.price_index, abs=1e-8)


def _gravity_world(n=40, beta=-0.9, contig=0.5, scale=60.0, seed=11):
    """Synthetic Poisson gravity: pair flows exp(a_i + b_j + beta ln d + contig c) with unit-mean noise."""
    rng = np.random.default_rng(seed)
    xy = rng.uniform(0.0, 10.0, (n, 2))
    dist = 500.0 * (1.0 + np.hypot(*(xy[:, None] - xy[None]).transpose(2, 0, 1)))
    near = dist < np.quantile(dist[~np.eye(n, dtype=bool)], 0.1)
    c = (near & ~np.eye(n, dtype=bool)).astype(float)
    a, b = rng.normal(0, 0.7, n), rng.normal(0, 0.7, n)
    mu = np.exp(a[:, None] + b[None] + beta * np.log(dist) + contig * c)
    mu *= scale / mu[~np.eye(n, dtype=bool)].mean()
    return rng.poisson(mu).astype(float), dist, c


def test_ppml_recovers_known_elasticity():
    flows, dist, c = _gravity_world()
    n = len(flows)
    o, d = np.nonzero(~np.eye(n, dtype=bool))
    coef = ppml.fit(flows[o, d], o, d, {"ln_dist": np.log(dist[o, d]), "contig": c[o, d]})
    assert coef["ln_dist"] == pytest.approx(-0.9, abs=0.02)
    assert coef["contig"] == pytest.approx(0.5, abs=0.1)


def test_ppml_scale_invariant_and_keeps_zeros():
    flows, dist, _c = _gravity_world(n=25, scale=0.8)  # many zero flows
    n = len(flows)
    o, d = np.nonzero(~np.eye(n, dtype=bool))
    assert (flows[o, d] == 0).mean() > 0.3
    reg = {"ln_dist": np.log(dist[o, d])}
    base = ppml.fit(flows[o, d], o, d, reg)
    scaled = ppml.fit(1e6 * flows[o, d], o, d, reg)
    assert scaled["ln_dist"] == pytest.approx(base["ln_dist"], abs=1e-8)
    coef, se = ppml.fit_robust(flows[o, d], o, d, reg)
    assert coef["ln_dist"] == pytest.approx(base["ln_dist"], abs=1e-12)
    assert 0.0 < se["ln_dist"] < 0.2


def test_ppml_drops_countries_without_any_flow():
    n = 20
    flows, dist, c = _gravity_world(n=n)
    flows[3, :] = 0.0  # country 3 never exports: its exporter effect diverges unless dropped
    o, d = np.nonzero(~np.eye(n, dtype=bool))
    reg = {"ln_dist": np.log(dist[o, d]), "contig": c[o, d]}
    got = ppml.fit(flows[o, d], o, d, reg)
    # Exactly the fit on the explicitly filtered rows: country 3 stays as an importer.
    keep = o != 3
    assert (d[keep] == 3).sum() == n - 1
    want = ppml.fit(flows[o, d][keep], o[keep], d[keep], {k: v[keep] for k, v in reg.items()})
    for k in reg:
        assert got[k] == pytest.approx(want[k], abs=1e-10)
    # Dropping the country altogether is a different fit, so its importer rows are used.
    both = (o != 3) & (d != 3)
    other = ppml.fit(flows[o, d][both], o[both], d[both], {k: v[both] for k, v in reg.items()})
    assert abs(other["ln_dist"] - want["ln_dist"]) > 1e-6


def test_ppml_reports_separated_rows_and_rejects_empty():
    n = 20
    flows, dist, c = _gravity_world(n=n)
    flows[3, :] = 0.0
    run, gravity, codes = _tables(flows, dist, c)
    res = ppml.distance_elasticity(run, gravity, codes)
    assert res.n_separated_rows == n - 1
    assert (res.n_separated_exporters, res.n_separated_importers) == (1, 0)
    assert res.n_pairs == n * (n - 1) - (n - 1) and res.n_dropped == 0
    o, d = np.nonzero(~np.eye(n, dtype=bool))
    with pytest.raises(ValueError, match="survives"):
        ppml.fit(np.zeros(len(o)), o, d, {"ln_dist": np.log(dist[o, d])})


def test_ppml_recovers_trade_elasticity():
    """ADR 0018, H2 per class: PPML on gravity with tariffs recovers theta from the coefficient on ln(1 + t)."""
    rng = np.random.default_rng(5)
    n, theta, beta = 40, 6.0, -1.0
    xy = rng.uniform(0.0, 10.0, (n, 2))
    dist = 500.0 * (1.0 + np.hypot(*(xy[:, None] - xy[None]).transpose(2, 0, 1)))
    tariff = rng.uniform(0.0, 0.3, (n, n))
    a, b = rng.normal(0, 0.7, n), rng.normal(0, 0.7, n)
    mu = np.exp(a[:, None] + b[None] - theta * np.log1p(tariff) + beta * np.log(dist))
    mu *= 200.0 / mu[~np.eye(n, dtype=bool)].mean()
    flows = rng.poisson(mu).astype(float)
    o, d = np.nonzero(~np.eye(n, dtype=bool))
    res = ppml.trade_elasticity(flows[o, d], o, d, tariff[o, d], dist[o, d])
    assert res.theta == pytest.approx(theta, abs=0.05)
    assert res.distance == pytest.approx(beta, abs=0.02)
    assert 0.0 < res.se_theta < 0.5 and res.n_pairs == n * (n - 1)


def _tables(flows, dist, c, split=2):
    n = len(flows)
    codes = [f"C{i:02d}" for i in range(n)]
    oo, dd = np.meshgrid(np.arange(n), np.arange(n), indexing="ij")
    oo, dd = oo.ravel(), dd.ravel()
    gravity = pa.table(
        {
            "iso3_o": [codes[i] for i in oo],
            "iso3_d": [codes[j] for j in dd],
            "distw": dist.ravel(),
            "contig": c.ravel().astype(np.int8),
        }
    )
    keep = flows[oo, dd] > 0  # sparse like run_flows: absent = 0
    eo, ed, v = oo[keep], dd[keep], flows[oo, dd][keep]
    parts = [  # the value is split over goods; domestic rows are present and must be ignored
        (eo, ed, g, v * w) for g, w in enumerate(np.linspace(1.0, 2.0, split) / np.linspace(1.0, 2.0, split).sum())
    ]
    run = pa.table(
        {
            "exporter_country": pa.array(np.concatenate([p[0] for p in parts]), pa.uint32()),
            "importer_country": pa.array(np.concatenate([p[1] for p in parts]), pa.uint32()),
            "good": pa.array(np.concatenate([np.full(len(p[0]), p[2]) for p in parts]), pa.uint8()),
            "value": pa.array(np.concatenate([p[3] for p in parts]), pa.float64()),
            "units": pa.array(np.concatenate([p[3] for p in parts]), pa.float64()),
        }
    )
    return run, gravity, codes


def test_distance_elasticity_from_run_flows():
    flows, dist, c = _gravity_world()
    run, gravity, codes = _tables(flows, dist, c)
    res = ppml.distance_elasticity(run, gravity, codes)
    assert res.elasticity == pytest.approx(-0.9, abs=0.02)
    assert res.contiguity == pytest.approx(0.5, abs=0.1)
    assert res.n_dropped == 0 and res.n_pairs == 40 * 39
    # Same estimate when the table carries only some of the pairs: missing and nonpositive distance are dropped.
    keep = np.ones(gravity.num_rows, dtype=bool)
    keep[5] = False
    distw = gravity["distw"].to_numpy().copy()
    distw[100] = np.nan
    distw[200] = 0.0
    thin = gravity.filter(pa.array(keep)).set_column(2, "distw", pa.array(distw[keep]))
    res2 = ppml.distance_elasticity(run, thin, codes)
    assert res2.n_dropped == 3 and res2.n_pairs == 40 * 39 - 3
    assert res2.elasticity == pytest.approx(res.elasticity, abs=0.02)


def _urban(theta, alpha, beta):
    """Section 3 urban model, one type, constant elasticity spillovers (system 13) in the form of
    equation (8): types (workers, residents), own-location exponents theta alpha and theta beta."""
    return uniqueness.change_of_variables(
        gamma=np.eye(2),
        r=np.diag([theta * alpha, theta * beta]),
        b=np.array([[0.0, theta * beta], [theta * alpha, 0.0]]),
    )


def test_bound_reproduces_paper_examples():
    """Allen, Arkolakis and Li (2024), January 2024 version: worked examples of Theorem 1."""
    # Footnote 3 (p. 4): the stacked contraction bound is 2, the spectral radius sqrt(2/3).
    a = np.array([[0.0, 2.0], [1.0 / 3.0, 0.0]])
    assert uniqueness.spectral_radius(a) == pytest.approx(np.sqrt(2.0 / 3.0), rel=1e-14)
    assert np.abs(a).sum(axis=1).max() == 2.0
    # Footnote 6 (p. 11): x_i = sum_j K_ij x_i^beta x_j^alpha, alpha 1, beta -1/2: the change of
    # variables (Remark 5) gives |alpha / (1 - beta)| = 2/3 where the direct bound is 1.5.
    fn6 = uniqueness.change_of_variables(gamma=np.eye(1), r=np.array([[-0.5]]), b=np.array([[1.0]]))
    assert uniqueness.spectral_radius(fn6) == pytest.approx(2.0 / 3.0, rel=1e-14)
    # Section 3 (pp. 14-15, Figure 1 p. 21): theta 1, alpha = beta. The off-diagonal blocks are
    # theta alpha / (1 - theta alpha); at 0.5 the radius is 1 (unique up to scale, part ii.b: one
    # equilibrium in panels a, d, g); at 0.55 and 0.6 it exceeds 1 and panels f, h, i show three.
    for alpha, radius, mode in ((0.5, 1.0, "tolerance"), (0.55, 0.55 / 0.45, "report"), (0.6, 1.5, "report")):
        a = _urban(1.0, alpha, alpha)
        np.testing.assert_allclose(a, [[0.0, alpha / (1 - alpha)], [alpha / (1 - alpha), 0.0]], rtol=1e-14)
        assert uniqueness.spectral_radius(a) == pytest.approx(radius, rel=1e-12)
        assert uniqueness.v1_mode(uniqueness.spectral_radius(a)) == mode
    # Online Appendix A.3 (pp. 8-9): Eaton-Kortum with intermediates and quasi-symmetric costs,
    # rho(B Gamma^-1) = (beta + theta - beta theta) / (beta + theta) < 1, theta = 1 / trade elasticity.
    for elasticity in (0.5, 2.62, 5.88, 12.55, 100.0):
        for beta in (0.05, 0.46, 0.73, 1.0):
            t = 1.0 / elasticity
            a = uniqueness.trade_with_intermediates(elasticity, beta)
            assert uniqueness.spectral_radius(a) == pytest.approx((beta + t - beta * t) / (beta + t), rel=1e-12)
            assert uniqueness.spectral_radius(a) < 1.0


def test_trade_only_is_unique(inputs):
    """P2 defaults (theta per good of the fixture export, no agglomeration, no migration): the
    bound is below 1, so V1 runs in tolerance mode."""
    va = uniqueness.value_added_share(inputs)
    assert va.shape == (inputs.n_goods,) and np.all((va > 0) & (va <= 1))
    bound = uniqueness.spectral_bound(inputs.theta, value_added_share=va)
    traded = inputs.traded
    want = max(
        uniqueness.spectral_radius(uniqueness.trade_with_intermediates(inputs.theta[g], va[g])) for g in np.flatnonzero(traded)
    )
    assert bound == pytest.approx(want, rel=1e-14)
    assert bound < 1.0 and uniqueness.v1_mode(bound) == "tolerance"
    # Labour the only input (value-added share 1) gives the smaller bound theta / (1 + theta).
    labour_only = uniqueness.spectral_bound(inputs.theta)
    theta_max = inputs.theta[traded].max()
    assert labour_only == pytest.approx(theta_max / (1.0 + theta_max), rel=1e-12)
    assert labour_only <= bound
    with pytest.raises(NotImplementedError):
        uniqueness.spectral_bound(inputs.theta, agglomeration=0.05)
    with pytest.raises(NotImplementedError):
        uniqueness.spectral_bound(inputs.theta, migration=2.0)


def test_v1_mode():
    assert uniqueness.v1_mode(0.0) == "tolerance"
    assert uniqueness.v1_mode(0.93) == "tolerance"
    assert uniqueness.v1_mode(1.0) == "tolerance"
    assert uniqueness.v1_mode(np.nextafter(1.0, 2.0)) == "report"
    assert uniqueness.v1_mode(1.5) == "report"
    assert uniqueness.v1_mode(float("inf")) == "report"


def test_country_without_people_spends_nothing(inputs):
    """A country with no economy province (the pack's uninhabited territories keep a gateway) has
    base spending 0, not 0 / 0, and the PIGL inversion stays finite."""
    inp = inputs
    one = lambda v: np.concatenate([v, v[-1:]])
    empty = dataclasses.replace(
        inp, s_l=one(inp.s_l), unemployment=one(inp.unemployment), u_star=one(inp.u_star), gini=one(inp.gini), omega=one(inp.omega), io=one(inp.io)
    )
    assert empty.base_spend[-1] == 0.0
    np.testing.assert_array_equal(empty.base_spend[:-1], inp.base_spend)
    assert np.isfinite(empty.pigl_weights.scale).all()


def test_newton_steps_use_the_set_krylov_size():
    """scipy ignores inner_maxiter for LGMRES (one cycle of inner_m vectors): the reference sets
    inner_m, and one Newton step on a linear system that needs more than LGMRES's default 30
    vectors uses them."""
    jac = solve.krylov_jacobian()
    assert jac.method_kw["inner_m"] == solve.INNER_M > 30
    rng = np.random.default_rng(5)
    n = 80
    a = np.diag(np.logspace(0, 4, n)) + 0.01 * rng.normal(size=(n, n))
    b = rng.normal(size=n)
    calls = [0]

    def f(x):
        calls[0] += 1
        return a @ x - b

    x0 = np.zeros(n)
    jac.setup(x0, f(x0), f)
    calls[0] = 0
    dx = -jac.solve(f(x0), tol=1e-10)
    assert 30 < calls[0] <= solve.INNER_M + solve.OUTER_K + 2
    assert np.abs(a @ dx - b).max() <= 1e-6 * np.abs(b).max()


def test_base_prices_are_the_reference_sourcing_at_prices_one(inputs):
    """Owner decision 2026-10-25: coefficients are ICIO cost shares at base delivered prices. The
    exported base prices are the reference's own delivered unit prices at prices 1, so at the
    data state every enterprise's input cost share is its sector's a_kg."""
    inp = inputs
    q = model._sourcing(inp, np.ones((inp.n_provinces, inp.n_goods)))["index"]
    np.testing.assert_allclose(inp.base_price[:, inp.traded], q[:, inp.traded], rtol=1e-13)
    assert np.all(inp.base_price[:, ~inp.traded] == 1.0) and np.any(inp.base_price > 1.0)
    a = inp.io[inp.ent_country, :, inp.ent_good]
    np.testing.assert_allclose(inp.ent_inputs * inp.base_price[inp.ent_province], a, rtol=1e-15)
    np.testing.assert_allclose(inp.ent_a_va, 1.0 - a.sum(axis=1), rtol=1e-14)
