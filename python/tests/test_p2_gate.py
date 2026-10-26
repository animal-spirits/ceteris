"""P2 gate on the full pack (`specs/econ/SPEC.md`, P2 gate): V1 after the uniqueness check, H2 (the
distance elasticity) and H2 per theta class (ADRs 0018 and 0023).

Needs `CETERIS_PACK_EXPORT`, a `ceteris export --pack` directory of the calibrated pack in `CETERIS_PACK`
(default `data/packs/2000`), and the pack's calibration tables; H2 also reads the CEPII gravity file of the
data cache. Without them every test skips. The runs go through `cargo run --release --bin ceteris`, about
a minute each.

The reference of V1 is the GE reference model at the calibrated point (ADR 0089): its inputs and root come
from the calibration tables (`calibrate.read_reference`), and the test checks that the point solves the
model. V1 runs from two starts, so that a run that never left the steady state cannot pass trivially:
(a) the steady state with every price, wage and stock moved by its own draw in [-10%, 10%]
(`EconWorld::perturb`, seed `PERTURB_SEED`) and (b) the 2000 data state of the calibrated inputs (prices
1, base wages, one month of base use in stock, nothing in flight), the start before ADR 0089."""

import json
import os
import subprocess
import zipfile
from pathlib import Path

import numpy as np
import pyarrow as pa
import pyarrow.compute as pc
import pyarrow.csv as pacsv
import pytest
from pyarrow import ipc

from ceteris.packs import fetch
from ceteris.reference import calibrate, io, model, ppml, uniqueness

ROOT = Path(__file__).resolve().parents[2]
PACK = Path(os.environ.get("CETERIS_PACK", ROOT / "data" / "packs" / "2000")).resolve()
PACK_EXPORT = os.environ.get("CETERIS_PACK_EXPORT")
TICKS = 1200
HISTORY_EVERY = 12
PERTURB_SEED = 20
V1_TOL = 0.01  # V1's 1% (Equilibrium nesting)
FLOW_FLOOR = 1e-6  # flows compared: above this share of world trade
REFERENCE_TOL = 1e-10  # the reference solver's tolerance on scaled residuals
H2_DISTANCE = (-1.1, -0.7)
# ADR 0023, Prior band: theta_c +/- 2 s.e. of the Caliendo-Parro class estimate, fixed before the gate ran.
THETA_BANDS = {0: (1.40, 3.84), 1: (3.36, 7.02), 2: (8.15, 12.99)}
GRAVITY = "Gravity_csv_V202211.zip"
YEAR = 2000
N_TRADED = 7


@pytest.fixture(scope="module")
def export():
    if not PACK_EXPORT:
        pytest.skip("skipping: CETERIS_PACK_EXPORT unset (a `ceteris export --pack` directory of the calibrated pack)")
    if not (PACK / "econ_calib.arrow").exists():
        pytest.skip(f"skipping: no calibration tables in {PACK}")
    inp = io.load(PACK_EXPORT)
    pack_province = ipc.open_file(str(Path(PACK_EXPORT) / "pack_provinces.arrow")).read_all()["pack"].to_numpy()
    return inp, pack_province


@pytest.fixture(scope="module")
def reference(export):
    """The reference inputs at the calibrated point, its root and the equilibrium there."""
    inp, pack_province = export
    ref, x = calibrate.read_reference(PACK, inp, pack_province)
    with np.errstate(all="ignore"):
        residual = np.abs(model.residual(x, ref)).max()
        eq = model.unpack(x, ref)
    assert residual <= REFERENCE_TOL, f"the calibrated point does not solve the reference: {residual:e}"
    print(f"reference: residual {residual:.3e} at the calibrated point, price index {eq.price_index}")
    return ref, eq


def run(out: Path, *flags: str) -> str:
    cmd = ["cargo", "run", "--release", "--quiet", "--bin", "ceteris", "--", "run", "--pack", str(PACK)]
    cmd += ["--ticks", str(TICKS), *flags, "--history", "--history-every", str(HISTORY_EVERY), "--out", str(out)]
    return subprocess.run(cmd, check=True, cwd=ROOT, capture_output=True, text=True).stdout


def _table(path: Path) -> dict[str, np.ndarray]:
    t = ipc.open_file(str(path)).read_all()
    return {c: t[c].to_numpy() for c in t.column_names}


def _flows(out: Path, n_c: int) -> tuple[np.ndarray, np.ndarray]:
    """Value and units of `run_flows` (mean of the last 12 ticks) as (exporter, importer, good) arrays."""
    f = _table(out / "run_flows.arrow")
    value, units = np.zeros((n_c, n_c, 10)), np.zeros((n_c, n_c, 10))
    value[f["exporter_country"], f["importer_country"], f["good"]] = f["value"]
    units[f["exporter_country"], f["importer_country"], f["good"]] = f["units"]
    return value, units


def _worst(dev: np.ndarray, labels) -> tuple[float, str]:
    i = int(np.argmax(dev))
    return float(dev[i]), labels(i)


def _gaps(out: Path, ref: io.Inputs, eq: model.Equilibrium) -> dict:
    """V1's largest gaps at the final tick, prices and wages over the world price index, flows of the last 12
    ticks with flow values over the index; and the path of the largest price gap per history row."""
    world = _table(out / "run_history_world.arrow")
    index = float(world["world_price_index"][-1])
    n_p, n_g = ref.n_provinces, ref.n_goods
    price = _table(out / "run_prices.arrow")["price"].reshape(n_p, n_g)
    cells = np.flatnonzero(ref.active.ravel())
    dev = np.abs(price.ravel()[cells] / index / eq.price.ravel()[cells] - 1.0)
    out_price = _worst(dev, lambda i: f"province {cells[i] // n_g} good {cells[i] % n_g}")
    wage = _table(out / "run_wages.arrow")["wage"]
    out_wage = _worst(np.abs(wage / index / eq.wage - 1.0), lambda i: f"province {i}")
    value, units = _flows(out, ref.n_countries)
    want_v, want_u = eq.flows_by_country_pair_good, eq.units_by_country_pair_good
    big = np.argwhere(want_v > FLOW_FLOOR * want_v.sum())
    key = tuple(big.T)

    def label(i):
        return "exporter {} importer {} good {}".format(*big[i])

    out_value = _worst(np.abs(value[key] / index / want_v[key] - 1.0), label)
    out_units = _worst(np.abs(units[key] / want_u[key] - 1.0), label)
    markets = _table(out / "run_history_markets.arrow")
    path = []
    for t, level in zip(world["tick"].tolist(), world["world_price_index"].tolist(), strict=True):
        p = markets["price"][markets["tick"] == t]
        path.append((t, float(np.abs(p[cells] / level / eq.price.ravel()[cells] - 1.0).max())))
    return {
        "index": index,
        "price": out_price,
        "wage": out_wage,
        "flow value": out_value,
        "flow units": out_units,
        "flows compared": len(big),
        "flows missing": int((value[key] == 0).sum()),
        "path": path,
        "negative money pops": int(world["pops_negative_money"].max()),
    }


DATA_STATE_OPEN = pytest.mark.xfail(
    strict=True,
    reason="V1 from the shocked data state is a stress test, not the gate's V1 (owner, 2026-10-26): after the "
    "coal-led shock output stays at full employment while demand collapses, merchant losses become pop debts "
    "(Mali, Albania) and the price level returns slowly; see the Task 20c debug report, ADRs 0090 and 0112, "
    "and the econ SPEC open questions",
)


@pytest.mark.parametrize("start", ["perturbed", pytest.param("data_state", marks=DATA_STATE_OPEN)])
def test_v1_full_pack(reference, tmp_path, start):
    """V1 (SPEC, P2 gate): the uniqueness bound at the pack's thetas and value-added shares picks the mode;
    the frictionless run of 1,200 ticks from `start` is then compared with the reference: in tolerance mode
    every price and wage (index-normalized) and every flow above 1e-6 of world trade within 1%, in report mode
    the gaps are printed instead."""
    ref, eq = reference
    bound = uniqueness.spectral_bound(ref.theta, value_added_share=uniqueness.value_added_share(ref))
    mode = uniqueness.v1_mode(bound)
    flags = ["--frictionless", *(["--perturb", str(PERTURB_SEED)] if start == "perturbed" else ["--data-state"])]
    stdout = run(tmp_path, *flags)
    g = _gaps(tmp_path, ref, eq)
    path = " ".join(f"{t}:{d:.2e}" for t, d in g["path"][:: 120 // HISTORY_EVERY])
    summary = (
        f"V1 from {start} ({stdout.strip() or 'no perturbation'}): bound {bound:.4f}, {mode} mode; at tick {TICKS} "
        f"price {g['price'][0]:.3e} ({g['price'][1]}), wage {g['wage'][0]:.3e} ({g['wage'][1]}), flow units "
        f"{g['flow units'][0]:.3e} ({g['flow units'][1]}), flow value {g['flow value'][0]:.3e} ({g['flow value'][1]}); "
        f"{g['flows compared']} flows compared, {g['flows missing']} missing; world price index {g['index']}; "
        f"most pops with negative money {g['negative money pops']}; largest price gap every 120 ticks: {path}"
    )
    print(summary)
    if mode == "report":
        return
    assert g["flows missing"] == 0, summary
    for what in ("price", "wage", "flow units", "flow value"):
        assert g[what][0] < V1_TOL, f"{what}: {summary}"


@pytest.fixture(scope="module")
def default_run(export, tmp_path_factory):
    """The default run of 1,200 ticks from the calibrated steady state."""
    out = tmp_path_factory.mktemp("default_run")
    run(out)
    return out


@pytest.fixture(scope="module")
def gravity(export):
    """CEPII gravity of 2000 (`iso3_o`, `iso3_d`, `distw` the population-weighted harmonic distance
    `distw_harmonic`, `contig`) and the CEPII code of each pack country (its ISO code; pairs of countries
    CEPII does not list are dropped and counted)."""
    path = fetch.path_of(GRAVITY)
    if not path.exists():
        pytest.skip(f"skipping: no {GRAVITY} in {path.parent}; fetch it with `uv run python -m ceteris.packs.fetch`")
    from ceteris.packs.econ import downscale, people

    columns = {
        "year": pa.int32(),
        "iso3_o": pa.string(),
        "iso3_d": pa.string(),
        "distw_harmonic": pa.float64(),
        "contig": pa.float64(),
    }
    batches = []
    with zipfile.ZipFile(path) as z, z.open(next(n for n in z.namelist() if n.startswith("Gravity_V"))) as f:
        convert = pacsv.ConvertOptions(include_columns=list(columns), column_types=columns)
        for batch in pacsv.open_csv(f, convert_options=convert):
            batches.append(batch.filter(pc.fill_null(pc.equal(batch["year"], YEAR), False)))
    table = pa.Table.from_batches(batches).rename_columns(["year", "iso3_o", "iso3_d", "distw", "contig"])
    codes = ipc.open_file(str(PACK / "countries.arrow")).read_all()["code"].to_pylist()
    iso, _ = people.read_codes(fetch.path_of(downscale.ADMIN0))
    return table, [iso.get(c, c) for c in codes]


def test_h2_distance_elasticity(default_run, gravity, export):
    """H2: PPML of simulated country-pair trade (traded goods, FOB value, the last 12 ticks of the default
    run) on ln distance and contiguity with exporter and importer effects, domestic pairs excluded: the
    distance coefficient lies in -1.1 to -0.7 (owner decision; a miss stops work for the owner, ADR 0013)."""
    table, iso3 = gravity
    f = ipc.open_file(str(default_run / "run_flows.arrow")).read_all()
    traded = f.filter(pc.less(f["good"], N_TRADED))
    e = ppml.distance_elasticity(traded, table, iso3)
    lo, hi = H2_DISTANCE
    print(f"H2 distance: {json.dumps(e.__dict__)}")
    assert lo <= e.elasticity <= hi, f"distance elasticity {e.elasticity:.4f} (s.e. {e.se_elasticity:.4f}) outside {H2_DISTANCE}"


def test_h2_theta_by_class(default_run, gravity, export):
    """H2 per theta class (ADR 0018): per class, PPML of the simulated country-pair flows of its goods on
    ln(1 + tariff) and ln distance (CEPII population-weighted) with exporter and importer effects per good,
    domestic pairs excluded, zero flows kept; theta is minus the tariff coefficient and must lie in the class's
    prior band (ADR 0023). The flow is what the importer pays, FOB value times (1 + t): the kernel carries
    (1 + t)^-theta on spending (trade SPEC, Sourcing), so that is the flow whose tariff elasticity is theta
    (the FOB value's is theta + 1). The class with one good is also fitted by `ppml.trade_elasticity`."""
    inp, _ = export
    table, iso3 = gravity
    n_c = inp.n_countries
    value, _ = _flows(default_run, n_c)
    rate = np.zeros((n_c, n_c, 2))
    t = ipc.open_file(str(PACK / "econ_tariffs.arrow")).read_all()
    rate[t["importer"].to_numpy(), t["exporter"].to_numpy(), t["group"].to_numpy()] = t["rate"].to_numpy()
    params = json.loads((Path(PACK_EXPORT) / "params.json").read_text())["trade"]
    of = np.asarray(params["theta_class"][:N_TRADED])
    group = np.asarray([0, 0, 0, 0, 1, 1, 1])  # tariff groups: primary goods 0 to 3, manufactures 4 to 6
    index = {c: i for i, c in enumerate(iso3)}
    dist = np.full((n_c, n_c), np.nan)
    o = [index.get(c, -1) for c in table["iso3_o"].to_pylist()]
    d = [index.get(c, -1) for c in table["iso3_d"].to_pylist()]
    w = table["distw"].to_numpy(zero_copy_only=False)
    ok = (np.asarray(o) >= 0) & (np.asarray(d) >= 0)
    dist[np.asarray(o)[ok], np.asarray(d)[ok]] = w[ok]
    ex, im = np.nonzero(np.isfinite(dist) & (dist > 0) & ~np.eye(n_c, dtype=bool))
    lines, misses = [], []
    for c in range(int(of.max()) + 1):
        goods = np.flatnonzero(of == c)
        e, m, g = np.repeat(ex, len(goods)), np.repeat(im, len(goods)), np.tile(goods, len(ex))
        tariff = rate[m, e, group[g]]
        flow = value[e, m, g] * (1.0 + tariff)
        regressors = {"ln_tariff": np.log1p(tariff), "ln_dist": np.log(dist[e, m])}
        fit = ppml.hdfe(flow, [e * N_TRADED + g, m * N_TRADED + g], regressors)
        pairs = np.minimum(e, m) * n_c + np.maximum(e, m)
        clustered = ppml.hdfe(flow, [e * N_TRADED + g, m * N_TRADED + g], regressors, pairs)
        fob = ppml.hdfe(value[e, m, g], [e * N_TRADED + g, m * N_TRADED + g], regressors)
        theta, lo, hi = -fit["ln_tariff"], *THETA_BANDS[c]
        line = (
            f"class {c} goods {goods.tolist()}: theta {theta:.4f} (s.e. {fit.se('ln_tariff'):.4f}, pair-clustered "
            f"{clustered.se('ln_tariff'):.4f}), ln distance {fit['ln_dist']:.4f} (s.e. {fit.se('ln_dist'):.4f}), "
            f"pseudo-R2 {fit.pseudo_r2:.4f}, n {fit.n} ({fit.n_separated} separated); band {lo} to {hi}; FOB value: "
            f"tariff coefficient {fob['ln_tariff']:.4f} (s.e. {fob.se('ln_tariff'):.4f})"
        )
        if len(goods) == 1:
            dense = ppml.trade_elasticity(flow, e, m, tariff, dist[e, m])
            np.testing.assert_allclose(dense.theta, theta, rtol=1e-6)
            line += f"; trade_elasticity {dense.theta:.4f} (s.e. {dense.se_theta:.4f})"
        print(f"H2 per class: {line}")
        lines.append(line)
        if not lo <= theta <= hi:
            misses.append(c)
    assert not misses, f"classes {misses} outside their prior band: " + "; ".join(lines)
