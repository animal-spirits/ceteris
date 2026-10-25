"""Direct estimation of trade costs (ADR 0022, PLAN Task 18 Step 1): the PPML estimator reproduces
Yotov et al. (2016) on their sample, and each estimator recovers known values on synthetic gravity."""

from pathlib import Path

import numpy as np
import pytest

from ceteris.calibrate import gravity
from ceteris.packs import fetch
from ceteris.reference import ppml

YOTOV_ZIP = fetch.CACHE / gravity.YOTOV


@pytest.fixture(scope="module")
def yotov_fits():
    if not YOTOV_ZIP.exists():
        pytest.skip(f"skipping: no {gravity.YOTOV} in {fetch.CACHE}; fetch it with `uv run python -m ceteris.packs.fetch`")
    return gravity.yotov(YOTOV_ZIP)


def test_gravity_reproduces_yotov_2016(yotov_fits):
    """Every published PPML estimate and pair-clustered standard error of Tables 1 to 3, to the printed
    three decimals, and the observation counts."""
    for column, (terms, n, _r2) in gravity.YOTOV_PUBLISHED.items():
        fit = yotov_fits[column]
        assert fit.n == n, column
        for term, (estimate, se) in terms.items():
            assert round(fit[term], 3) == pytest.approx(estimate, abs=1e-9), (column, term, fit[term])
            assert round(fit.se(term), 3) == pytest.approx(se, abs=1e-9), (column, term, fit.se(term))


def test_read_dta_round_trips_numbers_strings_and_missing():
    names = ["exporter", "year", "trade", "flag"]
    types = [3, 252, 255, 251]
    header = bytes([113, 2, 1, 0]) + (len(names)).to_bytes(2, "little") + (3).to_bytes(4, "little")
    header += b"\0" * 81 + b"\0" * 18
    body = bytes(types)
    body += b"".join(n.encode().ljust(33, b"\0") for n in names)
    body += b"\0" * (2 * (len(names) + 1)) + b"\0" * (12 * len(names)) + b"\0" * (33 * len(names)) + b"\0" * (81 * len(names))
    body += b"\0" * 5  # end of expansion fields
    rows = [("USA", 1986, 1.5, 1), ("CA", 2006, -2.0, 0), ("DEU", 1990, 8.98846567431158e307, 101)]
    data = b"".join(
        e.encode().ljust(3, b"\0")
        + y.to_bytes(2, "little", signed=True)
        + np.float64(t).tobytes()
        + f.to_bytes(1, "little", signed=True)
        for e, y, t, f in rows
    )
    out = gravity.read_dta(header + body + data)
    assert out["exporter"].tolist() == ["USA", "CA", "DEU"]
    assert out["year"].tolist() == [1986.0, 2006.0, 1990.0]
    assert out["trade"][:2].tolist() == [1.5, -2.0] and np.isnan(out["trade"][2])
    assert out["flag"][:2].tolist() == [1.0, 0.0] and np.isnan(out["flag"][2])


def _two_way(n=25, seed=3):
    rng = np.random.default_rng(seed)
    o, d = np.nonzero(~np.eye(n, dtype=bool))
    x1, x2 = rng.normal(size=len(o)), rng.uniform(0, 1, len(o))
    a, b = rng.normal(0, 1, n), rng.normal(0, 1, n)
    y = rng.poisson(np.exp(2.0 + a[o] + b[d] - 0.8 * x1 + 0.5 * x2)).astype(float)
    return y, o, d, {"x1": x1, "x2": x2}


def test_hdfe_matches_the_dummy_variable_fit():
    y, o, d, reg = _two_way()
    coef, se = ppml.fit_robust(y, o, d, reg)
    fit = ppml.hdfe(y, [o, d], reg)
    for k in reg:
        assert fit[k] == pytest.approx(coef[k], abs=1e-8)
        assert fit.se(k) == pytest.approx(se[k], rel=1e-6)
    assert fit.n == len(y) and fit.n_separated == 0 and 0 < fit.pseudo_r2 < 1


def test_hdfe_clusters_and_drops_separated_groups():
    y, o, d, reg = _two_way()
    y[o == 4] = 0.0  # exporter 4 never exports
    fit = ppml.hdfe(y, [o, d], reg)
    keep = o != 4
    want = ppml.hdfe(y[keep], [o[keep], d[keep]], {k: v[keep] for k, v in reg.items()})
    assert fit.n_separated == int((~keep).sum())
    assert fit["x1"] == pytest.approx(want["x1"], abs=1e-10)
    # one cluster per observation is HC0 times G / (G - 1)
    ones = ppml.hdfe(y, [o, d], reg, cluster=np.arange(len(y)))
    g = fit.n
    assert ones.se("x1") == pytest.approx(fit.se("x1") * np.sqrt(g / (g - 1)), rel=1e-9)


def _world(n=30, seed=11):
    """Synthetic countries on a plane with a mode mix per pair, so that S05-like path quantities exist."""
    rng = np.random.default_rng(seed)
    xy = rng.uniform(0, 1, (n, 2))
    km = 3000.0 * np.hypot(*(xy[:, None] - xy[None]).transpose(2, 0, 1)) + 50.0
    np.fill_diagonal(km, 200.0)
    share = rng.dirichlet([2.0, 1.0, 2.0, 0.3], (n, n))  # road, rail, sea, air km shares
    q = np.zeros((n, n, gravity.N_Q))
    q[..., :4] = km[..., None] * share
    q[..., gravity.Q_FEE] = rng.uniform(0, 20, (n, n))
    q[..., gravity.Q_HOURS] = km / 30.0
    agreement = rng.uniform(size=(n, n)) < 0.3
    agreement = agreement | agreement.T
    rate = np.where(agreement, 0.0, rng.uniform(0.02, 0.3, n)[:, None]).T  # [exporter, importer]: importer's rate
    np.fill_diagonal(rate, 0.0)
    return q, rate


def _synthetic_sample(theta, nu=0.0, cost=(0.10, 0.04, 0.008, 0.8), domestic=False, seed=5):
    """Flows exp(FE - theta ln(1 + t) - theta ln tau - theta ln(1 + nu) border) with tau the S05 form."""
    rng = np.random.default_rng(seed)
    q, rate = _world()
    n = len(rate)
    weight = np.array([0.0005, 0.0008, 0.002, 0.0016, 0.0003, 0.0008, 0.0001])  # light enough that every good trades
    time_cost = np.array(gravity.TIME_COST)
    rows = []
    for g in range(len(theta)):
        a, b = rng.normal(0, 1, n), rng.normal(0, 1, n)
        c = q[..., :4] @ np.array(cost) + q[..., gravity.Q_FEE]
        tau = 1.0 + weight[g] * c + time_cost[g] * q[..., gravity.Q_HOURS] / 24.0
        border = 1.0 - np.eye(n)
        ln_mu = a[:, None] + b[None] - theta[g] * (np.log1p(rate) + np.log(tau) + border * np.log1p(nu))
        mu = np.exp(ln_mu)
        mu *= 500.0 / mu.mean()
        flow = rng.poisson(mu).astype(float)
        o, d = np.nonzero(np.ones((n, n), dtype=bool) if domestic else ~np.eye(n, dtype=bool))
        rows.append((o, d, np.full(len(o), g), flow[o, d], rate[o, d], tau[o, d], q[o, d], np.full(len(o), weight[g])))
    cat = [np.concatenate(p) for p in zip(*rows)]
    return gravity.Sample(*cat[:7], weight=cat[7], time_cost=time_cost[cat[2]])


# Deliberately the first partition: synthetic data for the estimators, independent of the P01 defaults.
CLASSES = ((3,), (4, 6), (0, 1, 2, 5))
THETA = (12.0, 12.0, 12.0, 3.0, 6.0, 12.0, 6.0)


def test_theta_by_class_recovers_known_thetas():
    est = gravity.theta_by_class(_synthetic_sample(THETA), CLASSES)
    for c, e in enumerate(est):
        assert e.theta == pytest.approx(THETA[CLASSES[c][0]], rel=0.05), (c, e)
        assert 0 < e.se < 0.25 * e.theta and e.n > 0 and 0 < e.pseudo_r2 <= 1


def test_border_friction_recovers_known_nu():
    est = gravity.border_friction(_synthetic_sample(THETA, nu=0.4, domestic=True), CLASSES, shares=(0.1, 0.4, 0.5))
    assert est.nu == pytest.approx(0.4, rel=0.05)
    assert 0 < est.se < 0.1
    for c, nu in enumerate(est.nu_by_class):
        assert nu == pytest.approx(0.4, rel=0.1), c


def test_mode_costs_recover_known_ratios():
    """Road is the numeraire at its P04a default (0.10); rail, sea and air are estimated relative to it."""
    cost = (0.10, 0.03, 0.01, 0.5)
    est = gravity.mode_costs(_synthetic_sample(THETA, cost=cost), CLASSES)
    want = np.array(cost[1:]) / cost[0]
    assert est.ratio == pytest.approx(want, rel=0.1)
    assert np.all(est.se > 0) and np.all(est.se < 0.2 * want)


def test_edge_prices_follow_the_geo_spec():
    p = gravity.Transport()
    kinds = np.array([gravity.ROAD, gravity.RAIL, gravity.SEA, gravity.AIR, 5, 8])
    length = np.array([100.0, 100.0, 100.0, 100.0, 10.0, 7.0])
    tri = np.array([200.0, 1000.0, 500.0, 500.0, 300.0, 0.0])
    q = gravity.edge_quantities(kinds, length, tri, p)
    cost = gravity.edge_cost(q, p)
    # road: 0.10 * 130 km * min(1 + 0.5 * 2, 3); rail: 0.04 * 130 * 3 (capped); sea 0.008 * 100; air 0.8 * 105
    assert cost[:4] == pytest.approx([26.0, 15.6, 0.8, 84.0])
    assert cost[4] == pytest.approx(0.10 * 13.0 + 8.0)  # port access: road line, no terrain, plus the port fee
    assert cost[5] == pytest.approx(0.008 * 7.0 + 5.0)  # canal
    assert q[:, gravity.Q_HOURS] == pytest.approx([130 / 40, 130 / 25, 100 / 25, 105 / 500, 13 / 40 + 48, 7 / 25])


ROOT = Path(__file__).resolve().parents[2]
PACK = ROOT / "data" / "packs" / "2000"
CETERIS = ROOT / "target" / "release" / "ceteris"


def test_s05_costs_match_the_rust_crossing_legs(tmp_path):
    """The Python mirror of the edge prices and least-cost trees gives the kernel's crossing legs (cost and
    hours between every pair of gateways of different countries) of `ceteris export`."""
    import subprocess

    from pyarrow import ipc

    if not (PACK / "gateways.arrow").exists():
        pytest.skip(f"skipping: no pack at {PACK}; build it with `uv run python -m ceteris.packs`")
    if not CETERIS.exists():
        pytest.skip(f"skipping: no {CETERIS}; build it with `cargo build --release --bin ceteris`")
    subprocess.run([CETERIS, "export", "--pack", PACK, "--out", tmp_path], check=True, capture_output=True)

    def read(path):
        return ipc.open_file(str(path)).read_all()

    legs, nodes, edges = read(tmp_path / "legs.arrow"), read(PACK / "nodes.arrow"), read(PACK / "edges.arrow")
    gnode = read(PACK / "gateways.arrow")["node"].to_numpy().astype(np.int64)
    q = gravity.edge_quantities(edges["mode"].to_numpy(), edges["length_km"].to_numpy(), edges["tri_m"].to_numpy())
    graph = gravity.Graph(nodes.num_rows, edges["a"].to_numpy(), edges["b"].to_numpy(), q)
    dist, acc = graph.trees(gnode)
    x = legs["kind"].to_numpy() == 2
    a, b = legs["from"].to_numpy()[x], legs["to"].to_numpy()[x]
    assert x.sum() > 300_000
    assert dist[a, gnode[b]] == pytest.approx(legs["cost_per_t"].to_numpy()[x], rel=1e-12)
    assert gravity.edge_cost(acc[a, gnode[b]]) == pytest.approx(legs["cost_per_t"].to_numpy()[x], rel=1e-12)
    assert acc[a, gnode[b], gravity.Q_HOURS] == pytest.approx(legs["hours"].to_numpy()[x], rel=1e-12)
