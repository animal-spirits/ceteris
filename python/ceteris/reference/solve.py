"""Equilibrium of the reference model: Newton-Krylov from the data state, Anderson-accelerated
tatonnement as the fallback."""

import time

import numpy as np
from scipy.optimize import KrylovJacobian, NoConvergence

from . import model
from .io import Inputs
from .model import Equilibrium

MAX_NEWTON = 200
# Largest change of any ln p or ln w in one Newton step. Full steps from the data state overshoot
# into regions where whole sectors shut down and the Jacobian loses rank.
STEP_CAP = 0.3
MAX_TATONNEMENT = 5000
TATONNEMENT_STEP = 0.2
TATONNEMENT_CLIP = 0.1  # as the price rule's c_max
TATONNEMENT_TOL = 1e-6
ANDERSON_MEMORY = 5


def newton(inp: Inputs, x0: np.ndarray, tol: float) -> tuple[np.ndarray, int]:
    """Newton-Krylov (scipy's KrylovJacobian, LGMRES on finite-difference products) with steps
    capped at STEP_CAP; raises NoConvergence."""

    def f(x):
        return model.residual(x, inp)

    x = x0.copy()
    fx = f(x)
    jac = KrylovJacobian(method="lgmres", inner_maxiter=100, outer_k=10)
    jac.setup(x.copy(), fx, f)
    for steps in range(MAX_NEWTON):
        norm = np.abs(fx).max()
        if not np.isfinite(norm):
            raise NoConvergence(f"residual not finite after {steps} Newton steps")
        if norm <= tol:
            return x, steps
        dx = -jac.solve(fx, tol=min(0.1, norm))
        dx *= min(1.0, STEP_CAP / np.abs(dx).max())
        x = x + dx
        fx = f(x)
        jac.update(x.copy(), fx)
    raise NoConvergence(f"residual {np.abs(fx).max()} after {MAX_NEWTON} Newton steps")


def tatonnement(inp: Inputs, x0: np.ndarray, tol: float = TATONNEMENT_TOL) -> np.ndarray:
    """ln p and ln w move with their market's scaled excess demand (step clipped like the price
    rule), shifted together after each step so the world price index is 1. The fixed point of that
    map is accelerated by Anderson mixing, restarted from the best iterate whenever the step
    grows tenfold; raises NoConvergence."""

    def step(x):
        y = x + np.clip(TATONNEMENT_STEP * model.excess(x, inp), -TATONNEMENT_CLIP, TATONNEMENT_CLIP)
        return y - np.log(model.evaluate(y, inp)["index"])

    xs, rs = [], []
    x, best, best_x = x0.copy(), np.inf, x0.copy()
    for _ in range(MAX_TATONNEMENT):
        r = step(x) - x
        norm = np.abs(r).max()
        if norm <= tol:
            return x
        if not norm < 10.0 * best:
            xs, rs, x = [], [], best_x
            x = x + step(x) - x
            continue
        if norm < best:
            best, best_x = norm, x
        xs, rs = [*xs[-ANDERSON_MEMORY:], x], [*rs[-ANDERSON_MEMORY:], r]
        if len(rs) > 1:
            dx, dr = np.diff(xs, axis=0).T, np.diff(rs, axis=0).T
            gamma = np.linalg.lstsq(dr, r, rcond=None)[0]
            x = x + r - (dx + dr) @ gamma
        else:
            x = x + r
    raise NoConvergence(f"tatonnement step {best} after {MAX_TATONNEMENT} iterations")


def equilibrium(inp: Inputs, x0: np.ndarray | None = None, tol: float = 1e-10) -> Equilibrium:
    """Solves `model.residual(x) = 0` to `tol` in the max norm, from the data state unless `x0`
    is given; raises NoConvergence, or ValueError for a root that is not a Rust steady state."""
    start = time.perf_counter()
    x0 = model.data_state(inp) if x0 is None else x0
    with np.errstate(all="ignore"):
        try:
            x, steps = newton(inp, x0, tol)
            method = "newton_krylov"
        except NoConvergence:
            x, steps = newton(inp, tatonnement(inp, x0), tol)
            method = "tatonnement, newton_krylov"
    info = {
        "method": method,
        "iterations": steps,
        "residual": float(np.abs(model.residual(x, inp)).max()),
        "dropped_province": model.dropped_province(inp),
        "dropped_residual": float(model.excess(x, inp)[model.index_row(inp)]),
        "seconds": time.perf_counter() - start,
    }
    eq = model.unpack(x, inp, info)
    if np.any(eq.income <= 0):
        # Rust pops with no money buy nothing, and with negative income their balance never recovers
        raise ValueError(f"household income not positive in provinces {np.flatnonzero(eq.income <= 0).tolist()}")
    return eq
