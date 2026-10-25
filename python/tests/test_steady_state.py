"""ADR 0089: the calibrated data state is the steady state (`ceteris.reference.calibrate`), on the
fixture and, with `CETERIS_PACK_EXPORT` set (a `ceteris export --pack` directory of the pack in
`CETERIS_PACK`, default `data/packs/2000`), on the full pack."""

import json
import os
from pathlib import Path

import numpy as np
import pyarrow as pa
import pytest
from pyarrow import ipc

from ceteris.reference import calibrate, demand, io, model

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = Path(os.environ.get("CETERIS_FIXTURE_DIR", ROOT / "tests" / "golden" / "p2-fixture"))
PACK_EXPORT = os.environ.get("CETERIS_PACK_EXPORT")
PACK = Path(os.environ.get("CETERIS_PACK", ROOT / "data" / "packs" / "2000"))


def _world(name: str):
    if name == "fixture":
        inp = io.load(FIXTURE)
        return inp, inp.u_star
    if not PACK_EXPORT:
        pytest.skip("skipping: CETERIS_PACK_EXPORT unset (a full-pack export directory)")
    return io.load(PACK_EXPORT), calibrate.read_u_star(PACK)


@pytest.fixture(scope="module", params=["fixture", "pack"])
def world(request):
    inp, u_star = _world(request.param)
    return inp, calibrate.calibrate(inp, u_star)


def test_inversion_at_given_prices_reproduces_the_shares():
    """Stage 4: weights inverted at buyer prices q and spending m give the final demand shares
    there exactly, and at prices 1 the inversion is the one of ADR 0021."""
    inp = io.load(FIXTURE)
    rng = np.random.default_rng(4)
    price = np.exp(rng.normal(0.0, 0.3, (inp.n_countries, inp.n_goods)))
    m = inp.base_spend * np.exp(rng.normal(0.0, 0.2, inp.n_countries))
    w = demand.invert_at(inp.omega, price, m, inp.sigma_income, inp.category, inp.sigma_demand, inp.pigl)
    c = np.arange(inp.n_countries)
    share = demand.shares(w, c, price, m, inp.sigma_income, inp.category, inp.sigma_demand, inp.pigl)
    np.testing.assert_allclose(share, inp.omega / inp.omega.sum(axis=1, keepdims=True), rtol=0, atol=1e-12)
    ones = np.ones_like(price)
    at_one = demand.invert_at(inp.omega, ones, inp.base_spend, inp.sigma_income, inp.category, inp.sigma_demand, inp.pigl)
    base = inp.pigl_weights
    for got, want in ((at_one.scale, base.scale), (at_one.omega, base.omega), (at_one.within, base.within)):
        np.testing.assert_allclose(got, want, rtol=1e-13, atol=1e-15)


def test_sinkhorn_is_unique_up_to_scale(world):
    """Stage 2: from two starts the scaling reaches the same weights up to one scale per good."""
    inp, s = world
    v = np.zeros((inp.n_provinces, inp.n_goods))
    np.add.at(v, (inp.ent_province, inp.ent_good), np.where(inp.traded[inp.ent_good], s.value / s.r[inp.ent_good], 0.0))
    spend = s.spend_hh + s.spend_int
    origin = inp.active & inp.traded[None, :]
    rng = np.random.default_rng(9)
    one = calibrate.sinkhorn(inp, spend, v)
    other = calibrate.sinkhorn(inp, spend, v, start=np.where(origin, rng.normal(0.0, 3.0, origin.shape), 0.0))
    for g in np.flatnonzero(inp.traded):
        d = (one.log_weight - other.log_weight)[origin[:, g], g]
        assert np.abs(d - d.mean()).max() <= 1e-10, g
    np.testing.assert_allclose(one.r, other.r, rtol=1e-12)
    np.testing.assert_allclose(one.r[inp.traded], s.r[inp.traded], rtol=1e-12)


def test_closed_forms_zero_the_reference_residual(world):
    """Stage 3: at the calibrated prices and base wages, with A = 1 / p_VA, coef = a_kg p_o / q_ik
    and L0 = s_L VA0 / w0, the reference model's residual vanishes (every market clears, labour at
    (1 - u*) L^s, the world price index 1), and VA, employment and output are the data's."""
    inp, s = world
    ref = calibrate.reference_inputs(inp, s)
    x = calibrate.reference_point(inp, s)
    with np.errstate(all="ignore"):
        assert np.abs(model.residual(x, ref)).max() <= 1e-10
        e = model.evaluate(x, ref)
    np.testing.assert_allclose(e["value_added"], s.va0, rtol=1e-10)
    np.testing.assert_allclose(e["employment"], s.l0, rtol=1e-10)
    np.testing.assert_allclose(e["output"], s.output, rtol=1e-10)
    traded = inp.traded[inp.ent_good]
    np.testing.assert_allclose(s.va0[traded], s.r[inp.ent_good][traded] * inp.ent_va_target[traded], rtol=1e-13)
    assert np.all(np.isfinite(s.a) & (s.a > 0)) and np.all(s.a_va > 0)


def test_local_output_is_nonnegative_and_clears_locally(world):
    """Local goods at p = 1: output solves each province's Leontief system against household and
    intermediate local demand."""
    inp, s = world
    local = ~inp.traded[inp.ent_good]
    assert np.all(s.output[local] >= 0)
    use = np.zeros((inp.n_provinces, inp.n_goods))
    np.add.at(use, inp.ent_province, s.coef * s.output[:, None])
    out = np.zeros_like(use)
    np.add.at(out, (inp.ent_province, inp.ent_good), s.output)
    lg = ~inp.traded
    np.testing.assert_allclose(out[:, lg], (s.spend_hh + use)[:, lg], rtol=1e-12, atol=0)


def test_every_province_produces_and_pays_a_wage(world):
    """With national profit ownership every province's households earn dividends, so every local
    enterprise produces and every province has employment and a positive base wage."""
    inp, s = world
    local = ~inp.traded[inp.ent_good]
    assert np.all(s.output[local] > 0)
    assert np.all(np.bincount(inp.ent_province, s.va0, inp.n_provinces) > 0)
    assert np.all(np.isfinite(s.w0) & (s.w0 > 0)) and np.all(np.isfinite(s.l0) & (s.l0 > 0))


def test_stage5_state_is_stationary_under_the_stock_rules(world):
    """Stage 5: at the start of a tick each input stock is (1 + COVER) months of use and the
    pipeline holds, j ticks ahead, one month of shipments of every arrival slot j or later, so
    the order-up-to rule of E3 orders exactly one month of use; merchant stocks are coverage times
    output; pops hold one month of income less this tick's wages."""
    inp, s = world
    use = s.coef * s.output[:, None]
    np.testing.assert_allclose(s.stock, (1.0 + calibrate.COVER) * use, rtol=1e-15)
    ahead = s.ent_pipeline  # (E, G, SLOTS): units due j + 1 ticks ahead
    traded = inp.traded
    slots = np.arange(calibrate.SLOTS + 1)
    arrive = s.arrive[:, inp.ent_province, :].transpose(1, 2, 0) * (s.ent_spend[:, :, None])  # (E, G, slot)
    home = arrive[..., 0]
    np.testing.assert_allclose(arrive.sum(axis=2)[:, traded], use[:, traded], rtol=1e-12)
    lead = s.lead[inp.ent_province]
    target = (1.0 + calibrate.COVER + lead) * use
    held = s.stock - home  # after T1 delivered slots 1 to 4 and E2 used one month
    on_order = ahead[..., 1:].sum(axis=2)  # after this tick's delivery
    order = target - held - on_order
    np.testing.assert_allclose(order[:, traded], use[:, traded], rtol=1e-10, atol=1e-12 * use.max())
    np.testing.assert_allclose(ahead[..., 0], arrive[..., 1:].sum(axis=2), rtol=1e-12, atol=0)
    assert np.all(ahead[:, ~traded] == 0) and slots.size == ahead.shape[2] + 1
    origin = inp.active & traded[None, :]
    np.testing.assert_allclose(s.inventory[origin], (s.coverage[None, :] * s.supply)[origin], rtol=1e-15)
    np.testing.assert_allclose(s.pop_money, s.pop_income - s.pop_wage, rtol=1e-15)
    np.testing.assert_allclose(s.pop_spend.sum(axis=1), s.pop_income, rtol=1e-12)


def test_calibration_tables_have_the_documented_layout(tmp_path):
    inp, u_star = _world("fixture")
    s = calibrate.calibrate(inp, u_star)
    pack = 2 * np.arange(inp.n_provinces) + 1
    calibrate.write(tmp_path, inp, s, pack)

    def read(name):
        return ipc.open_file(str(tmp_path / name)).read_all()

    ent = read("econ_calib.arrow")
    assert ent.column_names[:3] == ["province", "good", "a"] and ent.schema.field("province").type == pa.uint32()
    key = list(zip(ent["province"].to_pylist(), ent["good"].to_pylist()))
    assert key == sorted(key) and len(set(key)) == len(inp.ent_good)
    assert ent["province"].to_pylist() == pack[inp.ent_province].tolist()
    np.testing.assert_array_equal(ent["a"].to_numpy(), s.a)
    np.testing.assert_array_equal(np.stack([ent[f"coef_{k}"].to_numpy() for k in range(inp.n_goods)], axis=1), s.coef)
    meta = json.loads(ent.schema.metadata[b"calibration"])
    assert meta["format"] == calibrate.FORMAT and len(meta["r"]) == inp.n_goods
    cells = read("econ_calib_cells.arrow")
    assert cells.num_rows == inp.n_provinces * inp.n_goods
    np.testing.assert_array_equal(cells["consumer_price"].to_numpy().reshape(inp.n_provinces, -1), s.consumer)
    pops = read("econ_calib_pops.arrow")
    assert list(zip(pops["province"].to_pylist(), pops["edu"].to_pylist())) == sorted(
        zip(pack[inp.pop_province].tolist(), pops["edu"].to_pylist())
    )
    pipe = read("econ_calib_pipeline.arrow")
    assert set(pipe["ahead"].to_pylist()) <= {1, 2, 3, 4} and np.all(pipe["units"].to_numpy() > 0)
    total = s.pop_pipeline.sum() + s.ent_pipeline.sum()
    assert pipe["units"].to_numpy().sum() == pytest.approx(total, rel=1e-12)
    countries = read("econ_calib_countries.arrow")
    np.testing.assert_array_equal(countries["u_star"].to_numpy(), u_star)
    provinces = read("econ_calib_provinces.arrow")
    np.testing.assert_array_equal(provinces["w0"].to_numpy(), s.w0)
