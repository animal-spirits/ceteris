"""The JAX port of the calibration residual (`ceteris.reference.jax_model`) and its solver
(`jax_calibrate`): skipped without the `jax` extra (`uv sync --project python --extra jax`)."""

import os
from pathlib import Path

import numpy as np
import pytest

from ceteris.reference import calibrate, io, model

pytest.importorskip("jax", reason="skipping: no jax (the `jax` extra)")
from ceteris.reference import jax_calibrate, jax_model

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = Path(os.environ.get("CETERIS_FIXTURE_DIR", ROOT / "tests" / "golden" / "p2-fixture"))
PACK_EXPORT = os.environ.get("CETERIS_PACK_EXPORT")  # a `ceteris export --pack data/packs/2000` directory


@pytest.fixture(scope="module")
def inputs():
    return io.load(FIXTURE)


@pytest.fixture(scope="module")
def system(inputs):
    return jax_model.System(inputs)


def _points(inp: io.Inputs, n: int = 3) -> list[np.ndarray]:
    """The data state and random points around it."""
    x0 = model.data_state(inp)
    rng = np.random.default_rng(7)
    return [x0] + [x0 + rng.normal(0.0, 0.1, len(x0)) for _ in range(n)]


def test_residual_matches_numpy(inputs, system):
    """The calibration's reduced residual, numpy and JAX, to 1e-12 at the data state and around it."""
    with np.errstate(all="ignore"):
        for x in _points(inputs):
            want = calibrate.residual(x, inputs)
            got = system.residual(x)
            np.testing.assert_allclose(got, want, rtol=0, atol=1e-12)


def test_productivity_matches_numpy(inputs, system):
    with np.errstate(all="ignore"):
        for x in _points(inputs):
            np.testing.assert_allclose(system.productivity(x), calibrate.productivity_of(x, inputs), rtol=1e-12, atol=0)


def test_jvp_matches_finite_differences(inputs, system):
    """The exact Jacobian-vector product agrees with central differences of the numpy residual."""
    x = _points(inputs)[1]
    rng = np.random.default_rng(3)
    v = rng.normal(size=len(x))
    h = 1e-6
    with np.errstate(all="ignore"):
        fd = (calibrate.residual(x + h * v, inputs) - calibrate.residual(x - h * v, inputs)) / (2 * h)
    np.testing.assert_allclose(system.jvp(x, v), fd, rtol=0, atol=1e-6 * np.abs(fd).max())


def test_country_blocks_are_the_cut_jacobian(inputs, system):
    """With the gateway sums held, the Jacobian couples unknowns of one country only (the index
    row aside), and the coloured products give each country's block exactly; the index row is
    the full Jacobian's."""
    x = _points(inputs)[1]
    eye = np.eye(system.n)
    cut = system.jvps(x, eye, cut=True)
    full = system.jvps(x, eye)
    row = system.index_row
    np.testing.assert_allclose(system.index_gradient(x), full[row], rtol=0, atol=1e-14)
    layout = jax_calibrate.Layout.of(inputs)
    blocks = jax_calibrate.country_blocks(system, layout, x)
    assert sorted(np.concatenate([idx for idx, _ in blocks])) == list(range(system.n))
    for idx, block in blocks:
        want = cut[np.ix_(idx, idx)]
        if row in idx:
            want[np.searchsorted(idx, row)] = full[row, idx]
        np.testing.assert_allclose(block, want, rtol=0, atol=1e-14)
        rows = idx[idx != row]
        assert np.abs(np.delete(cut[rows], idx, axis=1)).max() == 0.0
        assert np.abs(np.delete(full[rows], idx, axis=1)).max() > 0.1  # trade couples countries


@pytest.mark.parametrize("kind", ["blocks", "two-level"])
def test_preconditioned_gmres_solves_the_newton_step(inputs, system, kind):
    """Right-preconditioned GMRES reaches the exact Newton step in fewer iterations than without."""
    x = _points(inputs)[1]
    b = -system.residual(x)
    exact = np.linalg.solve(system.jvps(x, np.eye(system.n)), b)
    plain = jax_calibrate.gmres_solve(lambda v: system.jvp(x, v), b, 1e-8, 300)[1]
    pre = jax_calibrate.Preconditioner.build(system, jax_calibrate.Layout.of(inputs), x, kind)
    d, info = jax_calibrate.gmres_solve(lambda v: system.jvp(x, v), b, 1e-8, 300, pre)
    assert info["code"] == 0 and info["ratio"] <= 1e-8
    assert info["iterations"] < plain["iterations"] / 4
    np.testing.assert_allclose(d, exact, rtol=0, atol=1e-6 * np.abs(exact).max())


@pytest.mark.skipif(not PACK_EXPORT, reason="skipping: CETERIS_PACK_EXPORT unset (a full-pack export directory)")
def test_residual_matches_numpy_on_the_pack():
    inp = io.load(PACK_EXPORT)
    x = model.data_state(inp)
    with np.errstate(all="ignore"):
        want = calibrate.residual(x, inp)
    np.testing.assert_allclose(jax_model.System(inp).residual(x), want, rtol=0, atol=1e-12)
