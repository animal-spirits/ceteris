"""Productivity calibration (`specs/econ/SPEC.md`, Calibration step 2): the GE reference
equilibrium (`model`) with productivity A as unknowns, solved jointly with ln p and ln w.

The joint system: unknowns the reference's x (ln p of every active cell, ln w per province) and ln
A of every enterprise (traded, and local: one A per local enterprise, owner decision 2026-10-25);
equations the reference's residual (every market clears, the world price index is 1) and each
enterprise's value added equal to its target (the pack's VA; for local goods the population split
of national VA, so national totals are ICIO). Value added is nominal, sales less inputs at buyer
prices (the pack's convention: output less intermediate inputs, in USD), at the equilibrium's
prices with the world price index at 1.

The solve eliminates every A in closed form. At given prices and wages an enterprise with
value-added price p_VA > 0 matches nominal VA T with one A: the first-order condition
p_VA dVA/dL = w and p_VA VA = T give L / L0 = x with s T = w L0 ((1 - s) x^(1 - rho) + s x)
(x = s T / (w L0) for Cobb-Douglas), whatever p_VA, and A = T / (p_VA VA0 g(x)), g the normalized
CES. The reduced system is the reference's residual in x alone at those A (`residual`, a pure
function of x and the inputs, composed of `productivity_of`, `model.evaluate` and
`model.market_residual`); its roots are the joint system's (`joint_residual` checks). Where no
prices give an enterprise p_VA > 0 it has no positive A, and the solve stops short of a root.

Path (`productivity`): the data state x0 (prices 1, base wages; with cost-share coefficients every
p_VA is 1 there, every A 1 and every L = L0) starts a Newton homotopy F(x) - (1 - lambda) F(x0),
followed by pseudo-arclength continuation through folds to lambda = 1 (`_continue`), Newton with
GMRES on finite-difference products correcting each arc. The calibration has several roots in
general; this keeps the one reached continuously from the data state (controller ruling,
2026-10-25).
"""

import json
import time
from dataclasses import dataclass, field

import numpy as np
import pyarrow as pa
from pyarrow import ipc
from scipy.optimize import NoConvergence
from scipy.sparse.linalg import LinearOperator, gmres

from . import model, solve
from .io import Inputs

VA_FLOOR = 1e-12  # added to value added, relative to the target: the log stays finite at zero
PVA_FLOOR = 1e-9  # p_VA floor relative to the own price, while iterating: A stays finite
LABOUR_TOL = 1e-15
MAX_LABOUR_ITERATIONS = 100
TOL = 1e-10  # max-norm scaled residual of the joint system, as the reference
STEP_TOL = 1e-8  # corrector tolerance along the path
ARC_FIRST = 0.02  # arclength steps, in the weighted norm of `_continue`
ARC_MAX = 2.0
MIN_STEP = 1e-6
MAX_CORRECTOR = 15
MAX_ARC_CORRECTOR = 6  # Newton steps per arc before the arc is halved
GMRES_SIZE = 300  # Krylov vectors per Newton step (one GMRES cycle)
FORMAT = "2"


def labour_ratio(w: np.ndarray, l0: np.ndarray, t: np.ndarray, s_l: np.ndarray, r: float) -> np.ndarray:
    """x = L / L0 of enterprises at their first-order condition with nominal value added t:
    s t = w L0 ((1 - s) x^(1 - r) + s x), increasing in x; Newton in ln x."""
    if abs(r) < model.RHO_CD:
        return s_l * t / (w * l0)
    b = w * l0 / (s_l * t)
    y = np.zeros_like(b)
    for _ in range(MAX_LABOUR_ITERATIONS):
        e1, e2 = (1.0 - s_l) * np.exp((1.0 - r) * y), s_l * np.exp(y)
        f = np.log(b * (e1 + e2))
        y = y - f * (e1 + e2) / ((1.0 - r) * e1 + e2)
        if np.abs(f).max(initial=0.0) <= LABOUR_TOL:
            break
    return np.exp(y)


def value_added_price(x: np.ndarray, inp: Inputs) -> np.ndarray:
    """p_VA of every enterprise at prices x, as `model.evaluate` computes it."""
    price, _ = model._split(x, inp)
    consumer = np.where(inp.traded[None, :], model._sourcing(inp, price)["index"], price)
    ep = inp.ent_province
    coef = inp.ent_inputs
    with np.errstate(invalid="ignore"):
        cost = np.where(coef > 0, coef * consumer[ep], 0.0).sum(axis=1)
    return (price[ep, inp.ent_good] - cost) / inp.ent_a_va


def productivity_of(x: np.ndarray, inp: Inputs) -> np.ndarray:
    """A of every enterprise matching its target at x (p_VA floored, see PVA_FLOOR)."""
    price, wage = model._split(x, inp)
    p_va = np.maximum(value_added_price(x, inp), PVA_FLOOR * price[inp.ent_province, inp.ent_good])
    t = inp.ent_va_target
    r = model.rho(inp)
    ratio = labour_ratio(wage[inp.ent_province], inp.ent_l0, t, inp.ent_s_l, r)
    return t / (p_va * inp.ent_va0 * model.value_added(1.0, 1.0, 1.0, inp.ent_s_l, r, ratio))


def residual(x: np.ndarray, inp: Inputs) -> np.ndarray:
    """The reduced system: the reference's residual at x with every A from `productivity_of`."""
    return model.market_residual(model.evaluate(x, inp, productivity_of(x, inp)), inp)


def joint_residual(x: np.ndarray, a: np.ndarray, inp: Inputs) -> np.ndarray:
    """The joint system at prices and wages x and productivity a: the reference's residual, then
    ln(VA / target) of every enterprise (floored at VA_FLOOR)."""
    s = model.evaluate(x, inp, a)
    t = inp.ent_va_target
    eps = VA_FLOOR * t
    va = np.log((np.maximum(s["value_added"], 0.0) + eps) / (t + eps))
    return np.concatenate([model.market_residual(s, inp), va])


@dataclass
class Residuals:
    """Enterprises whose value-added equation is left above the tolerance at the solution, with
    their relative misses VA / T - 1."""

    enterprise: np.ndarray
    miss: np.ndarray
    info: dict = field(default_factory=dict)

    def __len__(self) -> int:
        return len(self.enterprise)

    @property
    def largest_miss(self) -> float:
        return float(np.abs(self.miss).max(initial=0.0))


def _gmres(op, b: np.ndarray, rtol: float) -> tuple[np.ndarray, int, int]:
    """One GMRES cycle of GMRES_SIZE vectors; (solution, info, iterations)."""
    its = [0]
    x, info = gmres(
        op, b, rtol=rtol, atol=0.0, restart=GMRES_SIZE, maxiter=1,
        callback=lambda _: its.__setitem__(0, its[0] + 1), callback_type="pr_norm",
    )  # fmt: skip
    return x, info, its[0]


def _jv(f, x: np.ndarray, fx: np.ndarray, v: np.ndarray) -> np.ndarray:
    """Forward-difference product of f's Jacobian at x with v."""
    nv = np.linalg.norm(v)
    if nv == 0:
        return np.zeros_like(fx)
    h = np.sqrt(np.finfo(np.float64).eps) * max(1.0, np.linalg.norm(x)) / nv
    return (f(x + h * v) - fx) / h


def _cap(d: np.ndarray, n: int) -> float:
    """Step factor keeping every ln p and ln w change within `solve.STEP_CAP`."""
    return min(1.0, solve.STEP_CAP / np.abs(d[:n]).max())


def _continue(f, x0: np.ndarray, tol: float, deadline: float | None, log, checkpoint=None) -> tuple[np.ndarray, dict]:
    """Pseudo-arclength continuation of H(x, lambda) = f(x) - (1 - lambda) f(x0) from (x0, 0),
    through any fold, to lambda = 1, then Newton at lambda = 1; raises NoConvergence. Arclength
    weighs x by its root mean square against lambda; each step is a secant prediction and Newton
    on H with the arclength constraint (GMRES on the bordered system)."""
    n = len(x0)
    weight = np.concatenate([np.full(n, 1.0 / n), [1.0]])
    f0 = f(x0)

    def norm_w(v):
        return np.sqrt(weight @ (v * v))

    def bordered(z):
        return f(z[:n]) - (1.0 - z[n]) * f0

    op = LinearOperator((n, n), matvec=lambda v: _jv(f, x0, f0, v))
    dx, _, _ = _gmres(op, -f0, 1e-3)
    z, t = np.concatenate([x0, [0.0]]), np.concatenate([dx, [1.0]])
    t /= norm_w(t)
    ds, arcs, newton_steps = ARC_FIRST, 0, 0
    while z[n] < 1.0:
        solve._check(deadline, f"calibration continuation at lambda {z[n]:.6f}")
        target = z + ds * t
        w, ok = target.copy(), False
        for k in range(MAX_ARC_CORRECTOR):
            hw = bordered(w)
            norm = float(np.abs(hw).max())
            gap = weight @ (t * (w - target))
            if not np.isfinite(norm):
                break
            if norm <= STEP_TOL and abs(gap) <= 1e-12:
                ok = True
                break
            fw = hw + (1.0 - w[n]) * f0

            def mv(v, w=w, fw=fw, t=t):
                return np.concatenate([_jv(f, w[:n], fw, v[:n]) + f0 * v[n], [weight @ (t * v)]])

            d, info, its = _gmres(LinearOperator((n + 1, n + 1), matvec=mv), -np.concatenate([hw, [gap]]), min(0.1, norm))
            w = w + d * _cap(d, n)
            newton_steps += 1
            if log:
                log(f"  newton {k} residual {norm:.3e}, gmres {its} iterations (info {info})")
        arcs += 1
        if log:
            log(f"arc {arcs} length {ds:.3e} lambda {w[n]:.6f} {'ok' if ok else 'failed'}, residual {norm:.3e}")
        if not ok:
            ds /= 2.0
            if ds < MIN_STEP:
                raise NoConvergence(f"continuation stalled at lambda {z[n]:.6f}, residual {norm:.3e}")
            continue
        step = (w - z) / norm_w(w - z)
        z, t, ds = w, step if weight @ (step * t) >= 0 else -step, min(ARC_MAX, 1.5 * ds)
        if checkpoint:
            checkpoint(z[:n], z[n])
    x = z[:n] - t[:n] * (z[n] - 1.0) / t[n]
    for k in range(MAX_CORRECTOR):
        fx = f(x)
        norm = float(np.abs(fx).max())
        if log:
            log(f"final newton {k} residual {norm:.3e}")
        if norm <= tol:
            return x, {"arcs": arcs, "newton_steps": newton_steps + k}
        solve._check(deadline, f"calibration final Newton at residual {norm:.3e}")
        d, info, its = _gmres(LinearOperator((n, n), matvec=lambda v, x=x, fx=fx: _jv(f, x, fx, v)), -fx, min(0.1, norm))
        x = x + d * _cap(d, n)
    raise NoConvergence(f"final Newton at lambda 1 left residual {norm:.3e}")


def productivity(
    inp: Inputs, tol: float = TOL, progress=None, budget: float | None = None, start: np.ndarray | None = None, checkpoint=None
) -> tuple[np.ndarray, Residuals]:
    """A per enterprise whose equilibrium matches `inp.ent_va_target` (the module documentation
    gives the system and the path), independent of `inp.ent_a`; the path starts at the data
    state unless `start` (an x) is given. Raises NoConvergence, also once `budget` seconds of wall
    clock are spent. `progress(line)` receives one line per Newton step and per arc, `checkpoint(x,
    lambda)` every accepted point of the path."""
    t0 = time.perf_counter()
    deadline = None if budget is None else t0 + budget
    with np.errstate(all="ignore"):
        x0 = model.data_state(inp) if start is None else start
        x, info = _continue(lambda v: residual(v, inp), x0, tol, deadline, progress, checkpoint)
        a = productivity_of(x, inp)
        joint = joint_residual(x, a, inp)
    if not np.abs(joint).max() <= tol:
        raise NoConvergence(f"joint residual {np.abs(joint).max():.3e} at the reduced root")
    miss = np.expm1(joint[len(x) :])
    bad = np.abs(miss) > tol
    info = {
        "method": "pseudo-arclength newton homotopy, newton-gmres",
        **info,
        "residual": float(np.abs(joint).max()),
        "max_abs_ln_a": float(np.abs(np.log(a)).max()),
        "seconds": time.perf_counter() - t0,
    }
    return a, Residuals(np.flatnonzero(bad), miss[bad], info)


def write(path, inp: Inputs, pack_province: np.ndarray, a: np.ndarray, residuals: Residuals) -> None:
    """`econ_calib.arrow` as `EconInputs::from_pack` reads it: one row per enterprise, keyed by
    (pack province, good), sorted, with the residual summary in the schema metadata."""
    summary = {"residual_count": len(residuals), "largest_relative_miss": residuals.largest_miss}
    table = pa.table(
        {
            "province": pa.array(pack_province[inp.ent_province], pa.uint32()),
            "good": pa.array(inp.ent_good, pa.uint8()),
            "a": pa.array(a, pa.float64()),
        }
    ).replace_schema_metadata({"calibration_format": FORMAT, "residuals": json.dumps(summary)})
    with pa.OSFile(str(path), "wb") as f, ipc.new_file(f, table.schema) as w:
        w.write_table(table)
