import json
from pathlib import Path

import numpy as np
import pyarrow as pa
import pytest
from hypothesis import given, settings
from hypothesis import strategies as st
from pyarrow import ipc as pa_ipc

from ceteris.reference import io, model, ppml, sinkhorn, solve

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / "tests" / "golden" / "p2-fixture"


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


def test_sinkhorn_and_dense_kernel_reproduce_equilibrium_gravity(fixture):
    """Checks `sinkhorn.balanced` (any a K b matrix is recovered from its own marginals, so this
    does not test the model's routing) and the dense two-level kernel against the model's low-rank
    country-pair units at dispatch. Trade money is checked by the dense route oracle below."""
    inp, eq = fixture
    countries = inp.province_country
    for g in np.flatnonzero(inp.traded):
        k = sinkhorn.dense_kernel(inp, g)
        p = eq.price[:, g]
        act = inp.active[:, g]
        pt = np.where(act, (p / p[act].max()) ** inp.theta, 0.0)
        pt[pt < inp.weight_floor] = 0.0
        pull = k * pt[None, :]
        supply = eq.supply[:, g]
        logit = supply[:, None] * pull / pull.sum(axis=1, keepdims=True)
        flows = sinkhorn.balanced(k, supply, logit.sum(axis=0))
        big = logit > 1e-12 * logit.sum()
        np.testing.assert_allclose(flows[big], logit[big], rtol=1e-8)
        assert np.abs(flows[~big]).max(initial=0.0) <= 1e-10 * logit.sum()
        # The dense logit aggregates to the low-rank model's country-pair units.
        shipped = logit.copy()
        np.fill_diagonal(shipped, 0.0)
        pairs = np.zeros((inp.n_countries, inp.n_countries))
        np.add.at(pairs, (countries[:, None], countries[None, :]), shipped)
        np.testing.assert_allclose(eq.units_by_country_pair_good[:, :, g], pairs, rtol=1e-9, atol=1e-9 * pairs.sum())


def dense_route_oracle(inp: io.Inputs, price: np.ndarray, supply: np.ndarray) -> dict:
    """Every route enumerated from the exported legs: home, o -> d inside a country, and
    o -> a -> b -> d over every gateway pair, with its own K, iceberg and tariff. Destination pays
    p_d per arriving unit, t / (1 + t) of it is tariff, the rest goes to the origin."""
    n_p, n_g = price.shape
    out = {key: np.zeros((n_p, n_g)) for key in ("home", "arrive", "paid", "received", "tariff", "netback")}
    n_c, country = inp.n_countries, inp.province_country
    out["pair_value"], out["pair_units"] = np.zeros((n_c, n_c, n_g)), np.zeros((n_c, n_c, n_g))
    dom, acc, cro = inp.domestic, inp.access, inp.crossing
    for g in np.flatnonzero(inp.traded):
        p, act = price[:, g], inp.active[:, g]
        pt = np.where(act, (p / p[act].max()) ** inp.theta, 0.0)
        pt[pt < inp.weight_floor] = 0.0
        legs_out = [[] for _ in range(n_p)]  # o -> d: (d, k, tau, tariff)
        for j in np.flatnonzero(dom.good == g):
            legs_out[dom.row[j]].append((dom.col[j], dom.k[j], dom.tau[j], 0.0))
        gate_in = [[] for _ in range(inp.n_gateways)]  # b -> d: (d, k, tau)
        for j in np.flatnonzero(acc.good == g):
            gate_in[acc.col[j]].append((acc.row[j], acc.k[j], acc.tau[j]))
        for ja in np.flatnonzero(acc.good == g):
            o, a = acc.row[ja], acc.col[ja]
            for jc in np.flatnonzero((cro.good == g) & (cro.row == a)):
                b, t = cro.col[jc], cro.tariff[jc]
                for d, k_bd, tau_bd in gate_in[b]:
                    k = acc.k[ja] * cro.k[jc] * k_bd
                    tau = acc.tau[ja] * cro.tau[jc] * cro.border_tau[jc] * tau_bd
                    legs_out[o].append((d, k, tau, t))
        for o in range(n_p):
            routes = [(o, 1.0, 1.0, 0.0), *legs_out[o]]
            weights = np.array([k * pt[d] for d, k, _, _ in routes])
            share = weights / weights.sum()
            out["netback"][o, g] = sum(s * p[d] / (tau * (1 + t)) for s, (d, _, tau, t) in zip(share, routes))
            out["home"][o, g] = supply[o, g] * share[0]
            for s, (d, _, tau, t) in zip(share[1:], routes[1:]):
                got = supply[o, g] * s / tau
                delivered = p[d] * got
                out["arrive"][d, g] += got
                out["paid"][d, g] += delivered
                out["tariff"][d, g] += delivered * t / (1 + t)
                out["received"][o, g] += delivered / (1 + t)
                out["pair_value"][country[o], country[d], g] += delivered / (1 + t)
                out["pair_units"][country[o], country[d], g] += supply[o, g] * s
    return out


def test_trade_money_matches_dense_route_oracle(inputs):
    inp = inputs
    assert np.any(inp.crossing.tariff > 0)
    rng = np.random.default_rng(11)
    price = np.exp(rng.uniform(-2.3, 2.3, (inp.n_provinces, inp.n_goods)))
    supply = np.where(inp.traded[None, :], rng.uniform(0.0, 1e6, price.shape), 0.0)
    want = dense_route_oracle(inp, price, supply)
    pt, v, u, pi, dest, onward, netback = model._routing(inp, price)
    got = model._flows(inp, price, supply, pt, v, u, pi, dest, onward)
    got["netback"] = netback
    traded = inp.traded
    for key, w in want.items():
        np.testing.assert_allclose(got[key][..., traded], w[..., traded], rtol=1e-10, atol=1e-13 * np.abs(w).max(), err_msg=key)


def two_provinces(theta, tau, sigma_demand, sigma_fl, s_l, a, labour, u) -> io.Inputs:
    """One traded good, one country, two identical provinces joined by a domestic leg, with the
    Rust base point: L0 = (1 - u) L^s, w0 = s_L VA0 / L0. Each province has two half-size
    enterprises: with a single one, the target's cap at L^s leaves labour demand at or below
    supply for every low wage, so no wage clears uniquely."""
    k = np.float32(tau**-theta).astype(np.float64)
    one = np.ones(2)
    l0 = (1.0 - u) * labour / 2.0
    empty = io.Entries(*(np.zeros(0, dtype=np.int64),) * 3, np.zeros(0), np.zeros(0))
    return io.Inputs(
        theta=theta,
        sigma_demand=sigma_demand,
        sigma_fl=sigma_fl,
        weight_floor=1e-100,
        traded=np.array([True]),
        province_country=np.zeros(2, dtype=np.int64),
        gateway_country=np.zeros(0, dtype=np.int64),
        labour_supply=labour * one,
        population=1e6 * one,
        w0=s_l * 50.0 / l0 * one,
        s_l=np.array([s_l]),
        unemployment=np.array([u]),
        omega=np.array([[1.0]]),
        io=np.zeros((1, 1, 1)),
        active=np.ones((2, 1), dtype=bool),
        ent_province=np.array([0, 0, 1, 1]),
        ent_good=np.zeros(4, dtype=np.int64),
        ent_a=np.full(4, a),
        ent_va0=np.full(4, 50.0),
        ent_l0=np.full(4, l0),
        domestic=io.Entries(np.zeros(2, dtype=np.int64), np.array([0, 1]), np.array([1, 0]), k * one, tau * one),
        access=empty,
        crossing=io.Entries(*(np.zeros(0, dtype=np.int64),) * 3, *(np.zeros(0),) * 4),
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
    np.testing.assert_allclose(eq.price[:, 0], [1.0, 1.0], rtol=1e-9)
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
    np.testing.assert_allclose(eq.wage, [w, w], rtol=1e-9)
    # Logit split between home and the other province.
    k = float(np.float32(tau**-theta))
    np.testing.assert_allclose(eq.home_units[:, 0], y / (1.0 + k), rtol=1e-9)
    assert eq.units_by_country_pair_good[0, 0, 0] == pytest.approx(2.0 * y * k / (1.0 + k), rel=1e-9)
    # delivered pricing: the destination pays p = 1 per arriving unit
    assert eq.flows_by_country_pair_good[0, 0, 0] == pytest.approx(2.0 * y * k / (1.0 + k) / tau, rel=1e-9)


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
