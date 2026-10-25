"""Productivity calibration with exact derivatives (`jax_model`; optional, the `jax` extra).

Same system and homotopy as `calibrate.productivity` (that module documents both), so the same
root, reached continuously from the data state. What changes is the linear algebra of each
Newton step, and the path tracking (`Path`: exact tangents, step control):

- Jacobian-vector products are exact (forward mode) instead of finite differences.
- GMRES is right-preconditioned by a two-level preconditioner built from exact derivatives,
  rebuilt when a solve takes more than REBUILD_ITERATIONS:
  - fine level: the exact Jacobian of the residual with the gateway sums held fixed
    (`jax_model`, `cut`), which couples only unknowns of one country, so it is block diagonal by
    country. Its columns are coloured by (rank of the province in its country, good or wage):
    one batched product per colour gives every country's block. The index row is dense across
    countries: its exact gradient (reverse mode) enters as a rank-one update (Sherman-Morrison).
    Blocks are LU-factored.
  - coarse level: the Galerkin operator P^T J P of the full Jacobian on one aggregate per
    (country, good) and per country's wages, P the indicator of each aggregate's unknowns,
    applied first (multiplicative): the price-level modes that trade between countries creates,
    which the country blocks miss.
"""

import time
from dataclasses import dataclass, field

import numpy as np
import scipy.linalg as sla
from scipy.optimize import NoConvergence
from scipy.sparse.linalg import LinearOperator, gmres

from . import calibrate, model, solve
from .io import Inputs
from .jax_model import System


@dataclass(frozen=True)
class Layout:
    """Country, colour and coarse aggregate of every unknown (cells, then wages)."""

    country: np.ndarray
    colour: np.ndarray
    aggregate: np.ndarray
    n_colours: int
    n_aggregates: int

    @staticmethod
    def of(inp: Inputs) -> "Layout":
        cells = inp.active_cells
        n_g = inp.n_goods
        prov = np.concatenate([cells // n_g, np.arange(inp.n_provinces)])
        slot = np.concatenate([cells % n_g, np.full(inp.n_provinces, n_g)])
        pc = inp.province_country
        order = np.lexsort((np.arange(inp.n_provinces), pc))
        rank = np.empty(inp.n_provinces, dtype=np.int64)
        start = np.searchsorted(pc[order], pc[order])
        rank[order] = np.arange(inp.n_provinces) - start
        colour = rank[prov] * (n_g + 1) + slot
        country = pc[prov]
        key = country * (n_g + 1) + slot
        _, aggregate = np.unique(key, return_inverse=True)
        return Layout(country, colour, aggregate, int(colour.max()) + 1, int(aggregate.max()) + 1)


def country_blocks(system: System, layout: Layout, x: np.ndarray) -> list[tuple[np.ndarray, np.ndarray]]:
    """(unknowns, block) of every country: the exact Jacobian of the cut residual on the
    country's unknowns, the index row (in its province's country) the full residual's."""
    n = system.n
    seeds = np.zeros((layout.n_colours, n))
    seeds[layout.colour, np.arange(n)] = 1.0
    compressed = system.jvps(x, seeds, cut=True)  # (n, colours)
    row = system.index_row
    grad = system.index_gradient(x)
    out = []
    for c in np.unique(layout.country):
        idx = np.flatnonzero(layout.country == c)
        block = compressed[idx][:, layout.colour[idx]]
        if c == layout.country[row]:
            block[np.searchsorted(idx, row)] = grad[idx]
        out.append((idx, block))
    return out


class CountryBlocks:
    """The fine level: exact country blocks of the cut Jacobian, the exact index row."""

    def __init__(self, system: System, layout: Layout, x: np.ndarray):
        n = system.n
        self.countries = [(idx, sla.lu_factor(block, check_finite=False)) for idx, block in country_blocks(system, layout, x)]
        # rank-one update: the index row's entries outside its own country
        row = system.index_row
        self.u = system.index_gradient(x)
        self.u[layout.country == layout.country[row]] = 0.0
        e = np.zeros(n)
        e[row] = 1.0
        self.b_e = self._solve_blocks(e)
        self.denom = 1.0 + self.u @ self.b_e

    def _solve_blocks(self, v: np.ndarray) -> np.ndarray:
        out = np.empty_like(v)
        for idx, lu in self.countries:
            out[idx] = sla.lu_solve(lu, v[idx], check_finite=False)
        return out

    def solve(self, v: np.ndarray) -> np.ndarray:
        z = self._solve_blocks(v)
        return z - self.b_e * (self.u @ z) / self.denom


class Coarse:
    """The coarse level: Galerkin P^T J P on the layout's aggregates, and J P."""

    def __init__(self, system: System, layout: Layout, x: np.ndarray):
        n, m = system.n, layout.n_aggregates
        p = np.zeros((m, n))
        p[layout.aggregate, np.arange(n)] = 1.0
        self.jp = system.jvps(x, p)  # (n, m)
        self.aggregate = layout.aggregate
        self.m = m
        self.lu = sla.lu_factor(self.restrict(self.jp), check_finite=False)

    def restrict(self, v: np.ndarray) -> np.ndarray:
        """P^T v (rows of v by unknown)."""
        out = np.zeros((self.m,) + v.shape[1:])
        np.add.at(out, self.aggregate, v)
        return out

    def correction(self, r: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
        """(P y, J P y) with y = (P^T J P)^-1 P^T r."""
        y = sla.lu_solve(self.lu, self.restrict(r), check_finite=False)
        return y[self.aggregate], self.jp @ y


@dataclass
class Preconditioner:
    """M^-1 r: coarse correction, then the fine level on what it leaves (or the fine level alone)."""

    fine: CountryBlocks | None
    coarse: Coarse | None
    seconds: float = 0.0
    info: dict = field(default_factory=dict)

    @staticmethod
    def build(system: System, layout: Layout, x: np.ndarray, kind: str = "two-level") -> "Preconditioner":
        t0 = time.perf_counter()
        fine = CountryBlocks(system, layout, x) if kind in ("blocks", "two-level") else None
        coarse = Coarse(system, layout, x) if kind in ("coarse", "two-level") else None
        return Preconditioner(fine, coarse, time.perf_counter() - t0, {"kind": kind})

    def __call__(self, r: np.ndarray) -> np.ndarray:
        z = np.zeros_like(r)
        if self.coarse is not None:
            z, jz = self.coarse.correction(r)
            r = r - jz
        if self.fine is not None:
            z = z + self.fine.solve(r)
        return z


def gmres_solve(matvec, b: np.ndarray, rtol: float, size: int, precondition=None) -> tuple[np.ndarray, dict]:
    """One right-preconditioned GMRES cycle of `size` vectors on J d = b: (d, info) with the
    iterations and the true residual ratio |J d - b| / |b|."""
    n = len(b)
    pre = precondition or (lambda v: v)
    its = [0]
    op = LinearOperator((n, n), matvec=lambda v: matvec(pre(v)))
    y, code = gmres(
        op, b, rtol=rtol, atol=0.0, restart=size, maxiter=1,
        callback=lambda _: its.__setitem__(0, its[0] + 1), callback_type="pr_norm",
    )  # fmt: skip
    d = pre(y)
    ratio = float(np.linalg.norm(matvec(d) - b) / np.linalg.norm(b))
    return d, {"code": int(code), "iterations": its[0], "ratio": ratio}


PREDICTOR_CAP = 0.25  # largest change of any ln p or ln w in one arc's prediction
# Step control (Allgower and Georg 1990, 6.1): an arc is rejected, and halved, when the corrector's
# first step moves any ln p or ln w more than FIRST_STEP, when a corrector step is not CONTRACTION
# times smaller than the one before it, or when the path's tangent at the new point turns from the
# last one by an angle with cosine below MIN_COSINE: each keeps the corrector on the branch it
# starts on. Tangents are solved exactly ([J f0; (W t)^T 0] tau = [0; 1]), not secants: a secant
# lags the tangent by a fixed angle on a curved path, however short the arc.
FIRST_STEP = 0.1
CONTRACTION = 0.5
MIN_COSINE = 0.95
# Arclength weight of each ln p and ln w against lambda's 1. `calibrate._continue` weighs them by
# 1 / n (root mean square), so a path that moves a few unknowns fast is almost parallel to lambda
# = const in that norm and the bordered corrector inherits the Jacobian's near-singularity; unit
# weights (Euclidean, the textbook choice) keep the arclength constraint across that direction.
# The weights change the parameterization of the path, not the path. "tangent" weighs x by
# 1 / |dx/dlambda|^2 at the start, so lambda carries half the start tangent's length.
X_WEIGHT = 1.0
GMRES_SIZE = 300
GMRES_RTOL = 1e-2  # relative, or the residual's max norm where smaller (inexact Newton)
REBUILD_ITERATIONS = 100  # rebuild the preconditioner when a solve takes more
TANGENT_RTOL = 1e-6


class Path:
    """Pseudo-arclength continuation of H(x, lambda) = f(x) - (1 - lambda) f(x0) from (x0, 0) to
    lambda = 1, then Newton at lambda = 1: the homotopy, corrector tolerances and final Newton of
    `calibrate._continue`, with exact products, the two-level preconditioner (reused until a solve
    takes more than REBUILD_ITERATIONS), exact tangents as predictors, a predictor capped in the
    max norm, the step control above and Euclidean arclength weights."""

    def __init__(
        self, system: System, x0: np.ndarray, kind: str = "two-level", log=None, deadline=None, checkpoint=None,
        predictor_cap: float = PREDICTOR_CAP, first_step: float = FIRST_STEP, contraction: float = CONTRACTION,
        min_cosine: float = MIN_COSINE, x_weight: float | str | None = X_WEIGHT,
    ):  # fmt: skip
        self.system, self.kind, self.log, self.deadline, self.checkpoint = system, kind, log, deadline, checkpoint
        self.predictor_cap, self.first_step, self.contraction, self.min_cosine = (
            predictor_cap,
            first_step,
            contraction,
            min_cosine,
        )
        self.layout = Layout.of(system.inp)
        self.x0 = x0
        self.n = len(x0)
        self.x_weight = x_weight
        w = 1.0 / self.n if x_weight is None else 1.0 if x_weight == "tangent" else x_weight
        self.weight = np.concatenate([np.full(self.n, w), [1.0]])
        self.arc_max = calibrate.ARC_MAX if x_weight is None else np.inf
        self.f0 = system.residual(x0)
        self.pre = None
        self.stats = {"arcs": 0, "rejected_arcs": 0, "newton_steps": 0, "gmres_iterations": [], "builds": 0, "build_seconds": 0.0}

    def _say(self, line: str) -> None:
        if self.log:
            self.log(line)

    def _build(self, x: np.ndarray) -> None:
        self.pre = Preconditioner.build(self.system, self.layout, x, self.kind)
        self.pre_f0 = self.pre(self.f0)
        self.stats["builds"] += 1
        self.stats["build_seconds"] += self.pre.seconds
        self._say(f"  preconditioner built in {self.pre.seconds:.1f} s")

    def _solve(self, x: np.ndarray, b: np.ndarray, rtol: float, border=None) -> tuple[np.ndarray, dict]:
        """J d = b, or the bordered system [J f0; c^T 0] d = b with c = `border`; the
        preconditioner is rebuilt (once) when the solve takes more than REBUILD_ITERATIONS."""
        for attempt in range(2):
            if self.pre is None:
                self._build(x)
            d, info = self._gmres(x, b, rtol, border)
            if info["iterations"] <= REBUILD_ITERATIONS or attempt == 1:
                break
            self._say(f"  gmres {info['iterations']} iterations, ratio {info['ratio']:.2e}: rebuilding")
            self.pre = None
        self.stats["gmres_iterations"].append(info["iterations"])
        return d, info

    def _gmres(self, x, b, rtol, border):
        sy, f0, n = self.system, self.f0, self.n
        if border is None:
            return gmres_solve(lambda v: sy.jvp(x, v), b, rtol, GMRES_SIZE, self.pre)
        c = border
        cb = c[:n] @ self.pre_f0

        def pre(r):
            a = self.pre(r[:n])
            dl = (c[:n] @ a - r[n]) / cb
            return np.concatenate([a - self.pre_f0 * dl, [dl]])

        def mv(v):
            return np.concatenate([sy.jvp(x, v[:n]) + f0 * v[n], [c @ v]])

        return gmres_solve(mv, b, rtol, GMRES_SIZE, pre)

    def _norm_w(self, v):
        return np.sqrt(self.weight @ (v * v))

    def _tangent(self, z: np.ndarray, t: np.ndarray) -> np.ndarray:
        """The unit tangent of the path at z oriented along t: [J f0; (W t)^T 0] tau = [0; 1]."""
        n = self.n
        b = np.zeros(n + 1)
        b[n] = 1.0
        tau, _ = self._solve(z[:n], b, TANGENT_RTOL, border=self.weight * t)
        return tau / self._norm_w(tau)

    def run(self, tol: float) -> np.ndarray:
        n, f0, sy = self.n, self.f0, self.system
        dx, info = self._solve(self.x0, -f0, TANGENT_RTOL)
        self._say(
            f"tangent: gmres {info['iterations']} iterations, ratio {info['ratio']:.2e}, max |dx/dlambda| {np.abs(dx).max():.3e}"
        )
        if self.x_weight == "tangent":  # x and lambda carry equal weight along the start tangent
            self.weight[:n] = 1.0 / (dx @ dx)
        z, t = np.concatenate([self.x0, [0.0]]), np.concatenate([dx, [1.0]])
        t /= self._norm_w(t)
        ds = calibrate.ARC_FIRST if self.arc_max < np.inf else np.inf  # else the predictor cap sets it
        while z[n] < 1.0:
            solve._check(self.deadline, f"jax calibration continuation at lambda {z[n]:.6e}")
            ds = min(ds, self.predictor_cap / np.abs(t[:n]).max())
            target = z + ds * t
            w, ok, norm, moved, why = target.copy(), False, np.inf, np.inf, "corrector"
            for k in range(calibrate.MAX_ARC_CORRECTOR):
                hw = sy.residual(w[:n]) - (1.0 - w[n]) * f0
                norm = float(np.abs(hw).max())
                gap = self.weight @ (t * (w - target))
                if not np.isfinite(norm):
                    break
                if norm <= calibrate.STEP_TOL and abs(gap) <= 1e-12:
                    ok = True
                    break
                d, info = self._solve(w[:n], -np.concatenate([hw, [gap]]), min(GMRES_RTOL, norm), border=self.weight * t)
                step = d * calibrate._cap(d, n)
                w = w + step
                self.stats["newton_steps"] += 1
                self._say(f"  newton {k} residual {norm:.3e}, gmres {info['iterations']} iterations (ratio {info['ratio']:.1e})")
                m = float(np.abs(step[:n]).max())
                if (k == 0 and m > self.first_step) or m > self.contraction * moved:
                    why = f"corrector step {m:.2e}"
                    break
                moved = m
            if ok:
                tangent = self._tangent(w, t)
                cosine = float(self.weight @ (tangent * t))
                if cosine < self.min_cosine:
                    ok, why = False, f"turn, cosine {cosine:.3f}"
            self.stats["arcs"] += 1
            self._say(
                f"arc {self.stats['arcs']} length {ds:.3e} lambda {w[n]:.6e} {'ok' if ok else 'failed (' + why + ')'}, residual {norm:.3e}"
            )
            if not ok:
                self.stats["rejected_arcs"] += 1
                ds /= 2.0
                if ds < calibrate.MIN_STEP * np.sqrt(self.weight[0]):  # MIN_STEP in unit-weight terms
                    raise NoConvergence(f"continuation stalled at lambda {z[n]:.6e}, residual {norm:.3e}")
                continue
            z, t, ds = w, tangent, min(self.arc_max, 1.5 * ds)
            if self.checkpoint:
                self.checkpoint(z[:n], z[n])
        x = z[:n] - t[:n] * (z[n] - 1.0) / t[n]
        for k in range(calibrate.MAX_CORRECTOR):
            fx = sy.residual(x)
            norm = float(np.abs(fx).max())
            self._say(f"final newton {k} residual {norm:.3e}")
            if norm <= tol:
                self.stats["final_newton_steps"] = k
                return x
            solve._check(self.deadline, f"jax calibration final Newton at residual {norm:.3e}")
            d, info = self._solve(x, -fx, min(GMRES_RTOL, norm))
            x = x + d * calibrate._cap(d, n)
            self.stats["newton_steps"] += 1
        raise NoConvergence(f"final Newton at lambda 1 left residual {norm:.3e}")


def productivity(
    inp: Inputs,
    tol: float = calibrate.TOL,
    progress=None,
    budget: float | None = None,
    start: np.ndarray | None = None,
    checkpoint=None,
    kind: str = "two-level",
    **path_options,
) -> tuple[np.ndarray, calibrate.Residuals]:
    """`calibrate.productivity` on the JAX system (`Path`): same arguments, result and checks."""
    t0 = time.perf_counter()
    deadline = None if budget is None else t0 + budget
    system = System(inp)
    x0 = model.data_state(inp) if start is None else start
    path = Path(system, x0, kind, progress, deadline, checkpoint, **path_options)
    x = path.run(tol)
    with np.errstate(all="ignore"):
        a = calibrate.productivity_of(x, inp)
        joint = calibrate.joint_residual(x, a, inp)
    if not np.abs(joint).max() <= tol:
        raise NoConvergence(f"joint residual {np.abs(joint).max():.3e} at the reduced root")
    miss = np.expm1(joint[len(x) :])
    bad = np.abs(miss) > tol
    its = path.stats["gmres_iterations"]
    info = {
        "method": f"pseudo-arclength newton homotopy, newton-gmres (jax, {kind} preconditioner)",
        **{k: v for k, v in path.stats.items() if k != "gmres_iterations"},
        "gmres_mean": float(np.mean(its)) if its else 0.0,
        "gmres_max": int(max(its, default=0)),
        "residual": float(np.abs(joint).max()),
        "max_abs_ln_a": float(np.abs(np.log(a)).max()),
        "seconds": time.perf_counter() - t0,
    }
    return a, calibrate.Residuals(np.flatnonzero(bad), miss[bad], info)
