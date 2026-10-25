"""The JAX port of the reduced residual (`ceteris.reference.jax_model`) against its numpy twin
(`reduced`): skipped without the `jax` extra (`uv sync --project python --extra jax`)."""

import os
from pathlib import Path

import numpy as np
import pytest

from ceteris.reference import io, model, reduced

pytest.importorskip("jax", reason="skipping: no jax (the `jax` extra)")
from ceteris.reference import jax_model

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
    """The reduced residual, numpy and JAX, to 1e-12 at the data state and around it."""
    with np.errstate(all="ignore"):
        for x in _points(inputs):
            want = reduced.residual(x, inputs)
            got = system.residual(x)
            np.testing.assert_allclose(got, want, rtol=0, atol=1e-12)


def test_productivity_matches_numpy(inputs, system):
    with np.errstate(all="ignore"):
        for x in _points(inputs):
            np.testing.assert_allclose(system.productivity(x), reduced.productivity_of(x, inputs), rtol=1e-12, atol=0)


def test_jvp_matches_finite_differences(inputs, system):
    """The exact Jacobian-vector product agrees with central differences of the numpy residual."""
    x = _points(inputs)[1]
    rng = np.random.default_rng(3)
    v = rng.normal(size=len(x))
    h = 1e-6
    with np.errstate(all="ignore"):
        fd = (reduced.residual(x + h * v, inputs) - reduced.residual(x - h * v, inputs)) / (2 * h)
    np.testing.assert_allclose(system.jvp(x, v), fd, rtol=0, atol=1e-6 * np.abs(fd).max())


@pytest.mark.skipif(not PACK_EXPORT, reason="skipping: CETERIS_PACK_EXPORT unset (a full-pack export directory)")
def test_residual_matches_numpy_on_the_pack():
    inp = io.load(PACK_EXPORT)
    x = model.data_state(inp)
    with np.errstate(all="ignore"):
        want = reduced.residual(x, inp)
    np.testing.assert_allclose(jax_model.System(inp).residual(x), want, rtol=0, atol=1e-12)
