from pathlib import Path

import numpy as np
import pytest
from hypothesis import given, settings
from hypothesis import strategies as st

from ceteris.reference import io, model, sinkhorn, solve

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / "tests" / "golden" / "p2-fixture"


@pytest.fixture(scope="module")
def inputs():
    return io.load(FIXTURE)


@pytest.fixture(scope="module")
def fixture(inputs):
    return inputs, solve.equilibrium(inputs)


@pytest.mark.xfail(strict=True, reason="FOB payment rule has no fixture equilibrium; delivered pricing pending")
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


@pytest.mark.xfail(strict=True, reason="FOB payment rule has no fixture equilibrium; delivered pricing pending")
def test_balanced_sinkhorn_matches_logit_at_equilibrium(fixture):
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
        # The dense logit aggregates to the low-rank model's country-pair flows.
        shipped = logit * p[:, None]
        np.fill_diagonal(shipped, 0.0)
        pairs = np.zeros((inp.n_countries, inp.n_countries))
        np.add.at(pairs, (countries[:, None], countries[None, :]), shipped)
        np.testing.assert_allclose(eq.flows_by_country_pair_good[:, :, g], pairs, rtol=1e-9, atol=1e-9 * pairs.sum())


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
    assert eq.flows_by_country_pair_good[0, 0, 0] == pytest.approx(2.0 * y * k / (1.0 + k), rel=1e-9)
