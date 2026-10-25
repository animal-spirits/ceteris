"""Productivity calibration (`specs/econ/SPEC.md`, Calibration step 2): the GE reference
equilibrium (`model`) with productivity A as unknowns, solved jointly with ln p and ln w.

The joint system: unknowns the reference's x (ln p of every active cell, ln w per province), ln A
of every traded enterprise and ln A per (country, local good); equations the reference's residual
(every market clears, the world price index is 1), each traded enterprise's value added equal to
its target, and each (country, local good)'s national value added equal to the sum of its
targets (ICIO: the pack splits national VA over provinces). Value added is nominal, sales less
inputs at buyer prices (the pack's convention: output less intermediate inputs, in USD), at the
equilibrium's prices with the world price index at 1.

The solve eliminates the traded A in closed form. At given prices and wages an enterprise with
value-added price p_VA > 0 matches nominal VA T with one A: the first-order condition
p_VA dVA/dL = w and p_VA VA = T give L / L0 = x with s T = w L0 ((1 - s) x^(1 - rho) + s x)
(x = s T / (w L0) for Cobb-Douglas), whatever p_VA, and A = T / (p_VA VA0 g(x)), g the normalized
CES. The reduced system keeps x and the local ln A, and its root is a root of the joint system
(`joint_residual` checks it). Where no prices give an enterprise p_VA > 0 it has no positive A,
and the solve stops short of a root; value-added equations above the tolerance at a root are the
residuals.

Path (`productivity`): the equilibrium at A = 1 is unique (ADR 0022 bound), so it anchors the
solve; its value added V1 is matched exactly by A = 1. The targets then move from V1 to the data
along T(lambda) = V1^(1 - lambda) T^lambda, each step corrected by Newton-Krylov (GMRES on
finite-difference products) from a secant prediction, the step halved where the corrector fails.
The calibration has several roots in general (Leontief inputs make a target's output T / (p -
unit input cost) steep in prices); this path selects the one connected to the A = 1 equilibrium.
"""

import dataclasses
import json
import time
from dataclasses import dataclass, field

import numpy as np
import pyarrow as pa
from pyarrow import ipc
from scipy.optimize import KrylovJacobian, NoConvergence

from . import model, solve
from .io import Inputs

VA_FLOOR = 1e-12  # added to value added, relative to the target: the log stays finite at zero
PVA_FLOOR = 1e-9  # p_VA floor relative to the own price, while iterating: A stays finite
LABOUR_TOL = 1e-15
MAX_LABOUR_ITERATIONS = 100
TOL = 1e-10  # max-norm scaled residual of the joint system, as the reference
STEP_TOL = 1e-8  # corrector tolerance at intermediate lambda
FIRST_STEP = 0.25
MAX_STEP = 0.5
MIN_STEP = 1e-6
MAX_CORRECTOR = 15
GMRES_SIZE = 300  # Krylov vectors per Newton step (one GMRES cycle)
FORMAT = "1"


@dataclass(frozen=True)
class Layout:
    """Where the unknowns sit: x, then ln A per traded enterprise (joint system only), then ln A
    per pair (country, local good)."""

    n_x: int
    traded: np.ndarray  # (T,) traded enterprises
    local: np.ndarray  # (L,) local enterprises
    pair: np.ndarray  # (L,) pair of each local enterprise
    pair_key: np.ndarray  # (n_pairs,) country * G + good

    @property
    def n_pairs(self) -> int:
        return len(self.pair_key)


def layout(inp: Inputs) -> Layout:
    traded = inp.traded[inp.ent_good]
    local = np.flatnonzero(~traded)
    key, pair = np.unique(inp.ent_country[local] * inp.n_goods + inp.ent_good[local], return_inverse=True)
    return Layout(
        n_x=len(inp.active_cells) + inp.n_provinces, traded=np.flatnonzero(traded), local=local, pair=pair, pair_key=key
    )


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


def value_added_price(x: np.ndarray, inp: Inputs, ent: np.ndarray) -> np.ndarray:
    """p_VA of enterprises `ent` at prices x, as `model.evaluate` computes it."""
    price, _ = model._split(x, inp)
    consumer = np.where(inp.traded[None, :], model._sourcing(inp, price)["index"], price)
    ep = inp.ent_province[ent]
    coef = inp.ent_inputs[ent]
    with np.errstate(invalid="ignore"):
        cost = np.where(coef > 0, coef * consumer[ep], 0.0).sum(axis=1)
    return (price[ep, inp.ent_good[ent]] - cost) / inp.ent_a_va[ent]


def traded_productivity(x: np.ndarray, inp: Inputs, lay: Layout) -> np.ndarray:
    """A of every traded enterprise matching its target at x (p_VA floored, see PVA_FLOOR)."""
    e = lay.traded
    price, wage = model._split(x, inp)
    p_va = np.maximum(value_added_price(x, inp, e), PVA_FLOOR * price[inp.ent_province[e], inp.ent_good[e]])
    t = inp.ent_va_target[e]
    r = model.rho(inp)
    ratio = labour_ratio(wage[inp.ent_province[e]], inp.ent_l0[e], t, inp.ent_s_l[e], r)
    return t / (p_va * inp.ent_va0[e] * model.value_added(1.0, 1.0, 1.0, inp.ent_s_l[e], r, ratio))


def productivity_of(u: np.ndarray, inp: Inputs, lay: Layout) -> np.ndarray:
    """A per enterprise from the reduced unknowns u = (x, ln A per pair)."""
    a = np.empty(len(inp.ent_good))
    a[lay.traded] = traded_productivity(u[: lay.n_x], inp, lay)
    a[lay.local] = np.exp(u[lay.n_x :])[lay.pair]
    return a


def _va_residual(va: np.ndarray, target: np.ndarray) -> np.ndarray:
    eps = VA_FLOOR * target
    return np.log((np.maximum(va, 0.0) + eps) / (target + eps))


def _pair_residual(s: dict, inp: Inputs, lay: Layout) -> np.ndarray:
    va = np.bincount(lay.pair, s["value_added"][lay.local], lay.n_pairs)
    return _va_residual(va, np.bincount(lay.pair, inp.ent_va_target[lay.local], lay.n_pairs))


def residual(u: np.ndarray, inp: Inputs, lay: Layout) -> np.ndarray:
    """The reduced system: the reference's residual and the national VA of every pair."""
    s = model.evaluate(u[: lay.n_x], inp, productivity_of(u, inp, lay))
    return np.concatenate([model.market_residual(s, inp), _pair_residual(s, inp, lay)])


def joint_residual(x: np.ndarray, a: np.ndarray, inp: Inputs, lay: Layout) -> np.ndarray:
    """The joint system at prices and wages x and productivity a: the reference's residual, the
    VA of every traded enterprise, the national VA of every pair."""
    s = model.evaluate(x, inp, a)
    traded = _va_residual(s["value_added"][lay.traded], inp.ent_va_target[lay.traded])
    return np.concatenate([model.market_residual(s, inp), traded, _pair_residual(s, inp, lay)])


@dataclass
class Residuals:
    """Joint-system equations of value added left above the tolerance at the solution: traded
    enterprises (index) and pairs (country * G + good), with their relative misses VA / T - 1."""

    enterprise: np.ndarray
    enterprise_miss: np.ndarray
    pair: np.ndarray
    pair_miss: np.ndarray
    info: dict = field(default_factory=dict)

    def __len__(self) -> int:
        return len(self.enterprise) + len(self.pair)

    @property
    def largest_miss(self) -> float:
        return float(np.abs(np.concatenate([self.enterprise_miss, self.pair_miss])).max(initial=0.0))


def _corrector(f, u: np.ndarray, tol: float) -> tuple[np.ndarray, bool, int, float]:
    """Newton-Krylov from u, steps capped as `solve.newton`; (u, converged, steps, residual)."""
    fu = f(u)
    jac = KrylovJacobian(method="gmres", inner_maxiter=GMRES_SIZE)
    jac.setup(u.copy(), fu, f)
    for k in range(MAX_CORRECTOR + 1):
        norm = float(np.abs(fu).max())
        if norm <= tol or not np.isfinite(norm) or k == MAX_CORRECTOR:
            return u, norm <= tol, k, norm
        dx = -jac.solve(fu, tol=min(0.1, norm))
        u = u + dx * min(1.0, solve.STEP_CAP / np.abs(dx).max())
        fu = f(u)
        jac.update(u.copy(), fu)
    raise AssertionError("unreachable")


def productivity(inp: Inputs, tol: float = TOL, progress=None) -> tuple[np.ndarray, Residuals]:
    """A per enterprise whose equilibrium matches `inp.ent_va_target` (the module documentation
    gives the system and the path), independent of `inp.ent_a`; raises NoConvergence with the
    path so far. `progress(line)` receives one line per continuation step."""
    start = time.perf_counter()
    lay = layout(inp)
    one = dataclasses.replace(inp, ent_a=np.ones(len(inp.ent_good)))
    with np.errstate(all="ignore"):
        anchor = solve.equilibrium(one, tol=tol)
        v1 = model.evaluate(anchor.x, one)["value_added"]
        if np.any(v1 <= 0):
            raise NoConvergence(f"{int((v1 <= 0).sum())} enterprises without value added at A = 1")
        target = inp.ent_va_target
        u = np.concatenate([anchor.x, np.zeros(lay.n_pairs)])
        lam, step, prev, path = 0.0, FIRST_STEP, None, []
        while lam < 1.0:
            nxt = min(1.0, lam + step)
            at = dataclasses.replace(inp, ent_va_target=v1 ** (1.0 - nxt) * target**nxt)
            guess = u if prev is None else u + (u - prev[0]) * (nxt - lam) / (lam - prev[1])
            w, ok, k, norm = _corrector(lambda v, at=at: residual(v, at, lay), guess, tol if nxt == 1.0 else STEP_TOL)
            path.append((nxt, ok, k, norm))
            if progress:
                progress(f"lambda {nxt:.6f} {'ok' if ok else 'failed'} after {k} Newton steps, residual {norm:.3e}, "
                         f"{time.perf_counter() - start:.0f} s")  # fmt: skip
            if ok:
                prev, u, lam = (u, lam), w, nxt
                step = min(MAX_STEP, 1.5 * step)
            else:
                step /= 2.0
                if step < MIN_STEP:
                    raise NoConvergence(f"continuation stalled at lambda {lam:.6f}, residual {norm:.3e} at {nxt:.6f}")
        a = productivity_of(u, inp, lay)
        joint = joint_residual(u[: lay.n_x], a, inp, lay)
    if not np.abs(joint).max() <= tol:
        raise NoConvergence(f"joint residual {np.abs(joint).max():.3e} at the reduced root")
    n_t = lay.n_x + len(lay.traded)
    miss_t, miss_p = np.expm1(joint[lay.n_x : n_t]), np.expm1(joint[n_t:])
    bad_t, bad_p = np.abs(miss_t) > tol, np.abs(miss_p) > tol
    info = {
        "method": "target continuation from the A = 1 equilibrium, newton_krylov",
        "anchor": anchor.info,
        "continuation_steps": len(path),
        "newton_steps": int(sum(p[2] for p in path)),
        "residual": float(np.abs(joint).max()),
        "seconds": time.perf_counter() - start,
    }
    residuals = Residuals(lay.traded[bad_t], miss_t[bad_t], lay.pair_key[bad_p], miss_p[bad_p], info)
    return a, residuals


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
