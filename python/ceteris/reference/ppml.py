"""PPML gravity estimation for the P2 gate's H2 tests (`specs/econ/SPEC.md`, P2 gate): Poisson
pseudo-maximum likelihood on country-pair trade with exporter and importer fixed effects, ln
population-weighted distance and contiguity as regressors, domestic pairs excluded; and per theta
class, ln(1 + tariff) and ln distance (ADR 0018). `hdfe` absorbs any number of fixed effects and clusters
standard errors, for direct estimation on BACI (ADR 0022, `ceteris.calibrate.gravity`). numpy only."""

from collections.abc import Mapping, Sequence
from dataclasses import dataclass

import numpy as np

TOL = 1e-10  # relative change of the deviance
MAX_ITER = 200


@dataclass(frozen=True)
class Elasticity:
    elasticity: float  # coefficient on ln distw
    contiguity: float
    se_elasticity: float  # Eicker-Huber-White
    se_contiguity: float
    n_pairs: int  # ordered non-domestic pairs actually estimated
    n_dropped: int  # pairs without a usable distance or contiguity (before estimation)
    n_separated_rows: int  # estimable pairs removed because their exporter or importer has no positive flow
    n_separated_exporters: int  # countries that lost all their exporter rows (zero exports)
    n_separated_importers: int  # countries that lost all their importer rows (zero imports)


def _deviance(y: np.ndarray, mu: np.ndarray) -> float:
    with np.errstate(divide="ignore", invalid="ignore"):
        ylogy = np.where(y > 0, y * np.log(y / mu), 0.0)
    return 2.0 * float(np.sum(ylogy - (y - mu)))


def _active(y: np.ndarray, exporter: np.ndarray, importer: np.ndarray) -> np.ndarray:
    """Observations kept: those whose exporter and importer have some positive flow. A country
    with none has a fixed effect at minus infinity (perfect prediction of zeros), so its rows carry
    no information; dropping one can empty another, hence the loop."""
    keep = np.ones(len(y), dtype=bool)
    while True:
        out = np.bincount(exporter[keep], y[keep], minlength=exporter.max() + 1)
        inn = np.bincount(importer[keep], y[keep], minlength=importer.max() + 1)
        now = keep & (out[exporter] > 0) & (inn[importer] > 0)
        if now.sum() == keep.sum():
            return keep
        keep = now


def _design(exporter: np.ndarray, importer: np.ndarray, regressors: np.ndarray) -> np.ndarray:
    """Regressors first, then exporter dummies, then importer dummies without the first (identification)."""
    _, e = np.unique(exporter, return_inverse=True)
    _, m = np.unique(importer, return_inverse=True)
    n = len(e)
    x = np.zeros((n, regressors.shape[1] + e.max() + 1 + m.max()))
    x[:, : regressors.shape[1]] = regressors
    x[np.arange(n), regressors.shape[1] + e] = 1.0
    sel = m > 0
    x[np.flatnonzero(sel), regressors.shape[1] + e.max() + m[sel]] = 1.0
    return x


def _irls(y: np.ndarray, x: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    """Poisson IRLS with step halving. Returns the coefficients and fitted means."""
    mu = (y + y.mean()) / 2.0
    eta = np.log(mu)
    beta = np.zeros(x.shape[1])
    dev = _deviance(y, mu)
    for _ in range(MAX_ITER):
        sw = np.sqrt(mu)
        step, *_ = np.linalg.lstsq(x * sw[:, None], sw * (eta + (y - mu) / mu), rcond=None)
        t = 1.0
        while True:
            cand = (1.0 - t) * beta + t * step
            eta_c = np.clip(x @ cand, -50.0, 50.0)
            mu_c = np.exp(eta_c)
            dev_c = _deviance(y, mu_c)
            if dev_c <= dev or t < 1e-8:
                break
            t /= 2.0
        converged = abs(dev - dev_c) <= TOL * (abs(dev_c) + 0.1)
        beta, eta, mu, dev = cand, eta_c, mu_c, dev_c
        if converged:
            return beta, mu
    raise RuntimeError(f"PPML did not converge in {MAX_ITER} iterations")


@dataclass(frozen=True)
class TradeElasticity:
    theta: float  # minus the coefficient on ln(1 + tariff)
    distance: float  # coefficient on ln distance
    se_theta: float  # Eicker-Huber-White
    se_distance: float
    n_pairs: int  # observations actually estimated
    n_separated_rows: int  # observations removed because their exporter or importer has no positive flow


@dataclass(frozen=True)
class Separation:
    n_rows: int
    n_exporters: int
    n_importers: int


def _fit(
    flows: np.ndarray, exporter: np.ndarray, importer: np.ndarray, regressors: Mapping[str, np.ndarray]
) -> tuple[dict[str, float], dict[str, float], Separation]:
    y = np.asarray(flows, dtype=np.float64)
    exporter, importer = np.asarray(exporter), np.asarray(importer)
    names = list(regressors)
    r = np.column_stack([np.asarray(regressors[k], dtype=np.float64) for k in names])
    if np.any(y < 0) or not np.all(np.isfinite(y)) or not np.all(np.isfinite(r)):
        raise ValueError("flows must be finite and nonnegative, regressors finite")
    if len(y) == 0:
        raise ValueError("no observations")
    keep = _active(y, exporter, importer)
    if not keep.any():
        raise ValueError("no observation survives: every exporter or importer has zero total flow")
    separation = Separation(
        int((~keep).sum()),
        len(np.unique(exporter)) - len(np.unique(exporter[keep])),
        len(np.unique(importer)) - len(np.unique(importer[keep])),
    )
    y, r, exporter, importer = y[keep], r[keep], exporter[keep], importer[keep]
    scale = y.mean()  # slopes are invariant to it; it only shifts the fixed effects
    y = y / scale
    x = _design(exporter, importer, r)
    beta, mu = _irls(y, x)
    bread = np.linalg.pinv(x.T @ (x * mu[:, None]))
    meat = x.T @ (x * ((y - mu) ** 2)[:, None])
    se = np.sqrt(np.diag(bread @ meat @ bread))
    k = len(names)
    return dict(zip(names, beta[:k].tolist())), dict(zip(names, se[:k].tolist())), separation


def fit(flows: np.ndarray, exporter: np.ndarray, importer: np.ndarray, regressors: Mapping[str, np.ndarray]) -> dict[str, float]:
    """PPML coefficients of `regressors` with exporter and importer fixed effects. `exporter` and
    `importer` are integer country indices per observation; zero flows are kept."""
    return _fit(flows, exporter, importer, regressors)[0]


def fit_robust(
    flows: np.ndarray, exporter: np.ndarray, importer: np.ndarray, regressors: Mapping[str, np.ndarray]
) -> tuple[dict[str, float], dict[str, float]]:
    """`fit` plus Eicker-Huber-White standard errors (HC0, no clustering)."""
    return _fit(flows, exporter, importer, regressors)[:2]


def _column(table, name: str) -> np.ndarray:
    col = table[name]
    return np.asarray(col.to_pylist() if hasattr(col, "to_pylist") else col)


def distance_elasticity(run_flows, gravity_table, iso3: Sequence[str]) -> Elasticity:
    """H2: PPML of the pair value (summed over goods) on ln `distw` and `contig`.

    `run_flows`: table with `exporter_country`, `importer_country`, `value` (the run export's
    `run_flows`, sparse: absent pairs are zero flows and kept). `gravity_table`: table with
    `iso3_o`, `iso3_d`, `distw`, `contig` (CEPII). `iso3[c]`: code of country index `c`. Domestic
    pairs are excluded; pairs missing from the gravity table or with a non-finite or nonpositive
    `distw` or a non-finite `contig` are dropped and counted."""
    n = len(iso3)
    eo = _column(run_flows, "exporter_country").astype(np.int64)
    ei = _column(run_flows, "importer_country").astype(np.int64)
    value = _column(run_flows, "value").astype(np.float64)
    foreign = eo != ei
    pair_value = np.bincount(eo[foreign] * n + ei[foreign], value[foreign], minlength=n * n)

    lookup = {
        (o, d): (float(dw), float(c))
        for o, d, dw, c in zip(
            _column(gravity_table, "iso3_o"),
            _column(gravity_table, "iso3_d"),
            _column(gravity_table, "distw").astype(np.float64),
            _column(gravity_table, "contig").astype(np.float64),
            strict=True,
        )
    }
    o_idx, d_idx = (a.ravel() for a in np.meshgrid(np.arange(n), np.arange(n), indexing="ij"))
    off = o_idx != d_idx
    o_idx, d_idx = o_idx[off], d_idx[off]
    dist = np.full(len(o_idx), np.nan)
    contig = np.full(len(o_idx), np.nan)
    for i, (o, d) in enumerate(zip(o_idx.tolist(), d_idx.tolist(), strict=True)):
        dist[i], contig[i] = lookup.get((iso3[o], iso3[d]), (np.nan, np.nan))
    ok = np.isfinite(dist) & (dist > 0) & np.isfinite(contig)

    coef, se, sep = _fit(
        pair_value[o_idx[ok] * n + d_idx[ok]],
        o_idx[ok],
        d_idx[ok],
        {"ln_distw": np.log(dist[ok]), "contig": contig[ok]},
    )
    return Elasticity(
        elasticity=coef["ln_distw"],
        contiguity=coef["contig"],
        se_elasticity=se["ln_distw"],
        se_contiguity=se["contig"],
        n_pairs=int(ok.sum()) - sep.n_rows,
        n_dropped=int((~ok).sum()),
        n_separated_rows=sep.n_rows,
        n_separated_exporters=sep.n_exporters,
        n_separated_importers=sep.n_importers,
    )


def trade_elasticity(
    flows: np.ndarray, exporter: np.ndarray, importer: np.ndarray, tariff: np.ndarray, distance: np.ndarray
) -> TradeElasticity:
    """H2 per theta class (ADR 0018): PPML of `flows` on ln(1 + `tariff`) and ln `distance` with
    exporter and importer fixed effects; the coefficient on ln(1 + t) is -theta. One observation per
    pair (zero flows kept), all arrays aligned; domestic pairs are the caller's to leave out."""
    coef, se, sep = _fit(
        flows,
        exporter,
        importer,
        {"ln_tariff": np.log1p(np.asarray(tariff, dtype=np.float64)), "ln_dist": np.log(np.asarray(distance, dtype=np.float64))},
    )
    return TradeElasticity(
        theta=-coef["ln_tariff"],
        distance=coef["ln_dist"],
        se_theta=se["ln_tariff"],
        se_distance=se["ln_dist"],
        n_pairs=len(np.asarray(flows)) - sep.n_rows,
        n_separated_rows=sep.n_rows,
    )


DEMEAN_TOL = 1e-13  # weighted norm of a sweep's change of a demeaned column, relative to the column's
DEMEAN_MAX_SWEEPS = 10_000


@dataclass(frozen=True)
class Fit:
    """A PPML fit with absorbed fixed effects (`hdfe`)."""

    names: tuple[str, ...]
    coef: np.ndarray
    cov: np.ndarray  # cluster-robust when clusters were given, else Eicker-Huber-White (HC0)
    n: int  # observations estimated
    n_separated: int  # observations removed because some fixed-effect group has no positive flow
    pseudo_r2: float  # squared correlation of flows and fitted means
    keep: np.ndarray  # mask of the estimated observations over the input

    def __getitem__(self, name: str) -> float:
        return float(self.coef[self.names.index(name)])

    def se(self, name: str) -> float:
        i = self.names.index(name)
        return float(np.sqrt(self.cov[i, i]))


def _separated(y: np.ndarray, groups: Sequence[np.ndarray]) -> np.ndarray:
    """Observations kept: those in no fixed-effect group whose flows are all zero (such a group's effect
    is minus infinity and its rows carry no information); dropping rows can empty another group."""
    keep = np.ones(len(y), dtype=bool)
    while True:
        now = keep.copy()
        for g in groups:
            total = np.bincount(g[now], y[now], minlength=g.max() + 1)
            now &= total[g] > 0
        if now.sum() == keep.sum():
            return keep
        keep = now


def _two_way(v: np.ndarray, w: np.ndarray, a: np.ndarray, b: np.ndarray, na: int, nb: int) -> np.ndarray:
    """Weighted residuals of `v` on two fixed effects solved directly: the first is eliminated from the
    normal equations, the second solves the dense Schur complement (singular along each connected
    component's constant, hence least squares)."""
    m = np.zeros((na, nb))
    np.add.at(m, (a, b), w)
    da, db = m.sum(axis=1), m.sum(axis=0)
    ra = np.stack([np.bincount(a, w * v[:, j], minlength=na) for j in range(v.shape[1])], axis=1)
    rb = np.stack([np.bincount(b, w * v[:, j], minlength=nb) for j in range(v.shape[1])], axis=1)
    scaled = m / da[:, None]
    schur = np.diag(db) - m.T @ scaled
    gamma = np.linalg.lstsq(schur, rb - scaled.T @ ra, rcond=None)[0]
    alpha = (ra - m @ gamma) / da[:, None]
    return v - alpha[a] - gamma[b]


def _demean(v: np.ndarray, w: np.ndarray, groups: Sequence[np.ndarray], sizes: Sequence[int]) -> np.ndarray:
    """Weighted residuals of the columns of `v` on the fixed effects `groups`, by alternating
    projections (the within transformation of each fixed effect in turn until nothing moves). Two
    small fixed effects start from their direct solution, which the projections then only polish."""
    if len(groups) == 2 and sizes[0] * sizes[1] <= 4_000_000:
        v = _two_way(v, w, groups[0], groups[1], sizes[0], sizes[1])
    else:
        v = v.copy()
    weight = [np.bincount(g, w, minlength=s) for g, s in zip(groups, sizes, strict=True)]
    for _ in range(DEMEAN_MAX_SWEEPS):
        before = v.copy()
        for g, s, wg in zip(groups, sizes, weight, strict=True):
            means = np.stack([np.bincount(g, w * v[:, j], minlength=s) for j in range(v.shape[1])], axis=1) / wg[:, None]
            v -= means[g]
        # rows of negligible weight converge slowly but do not move the weighted least squares step
        moved = np.sqrt(w @ (v - before) ** 2)
        if np.all(moved <= DEMEAN_TOL * np.maximum(np.sqrt(w @ v**2), 1e-300)):
            return v
    raise RuntimeError(f"fixed effects did not converge in {DEMEAN_MAX_SWEEPS} sweeps")


def hdfe(
    flows: np.ndarray,
    fixed_effects: Sequence[np.ndarray],
    regressors: Mapping[str, np.ndarray],
    cluster: np.ndarray | None = None,
) -> Fit:
    """PPML of `flows` on `regressors` with any number of absorbed fixed effects (integer group codes
    per observation), as in Correia, Guimaraes and Zylkin (2020): IRLS whose weighted least squares step
    runs on the regressors and working variable demeaned by the fixed effects. Standard errors are
    clustered on `cluster` (integer codes) with the G / (G - 1) correction, else HC0. Zero flows are kept;
    rows of fixed-effect groups without any positive flow are dropped first."""
    y = np.asarray(flows, dtype=np.float64)
    names = tuple(regressors)
    x = np.column_stack([np.asarray(regressors[k], dtype=np.float64) for k in names])
    if np.any(y < 0) or not np.all(np.isfinite(y)) or not np.all(np.isfinite(x)):
        raise ValueError("flows must be finite and nonnegative, regressors finite")
    codes = [np.unique(np.asarray(g), return_inverse=True)[1] for g in fixed_effects]
    keep = _separated(y, codes)
    if not keep.any():
        raise ValueError("no observation survives: every fixed-effect group has zero total flow")
    y, x = y[keep], x[keep]
    groups = [np.unique(g[keep], return_inverse=True)[1] for g in codes]
    sizes = [int(g.max()) + 1 for g in groups]
    y = y / y.mean()

    mu = (y + y.mean()) / 2.0
    eta = np.log(mu)
    dev = _deviance(y, mu)
    for _ in range(MAX_ITER):
        z = eta + (y - mu) / mu
        both = _demean(np.column_stack([x, z]), mu, groups, sizes)
        xt, zt = both[:, :-1], both[:, -1]
        sw = np.sqrt(mu)
        beta, *_ = np.linalg.lstsq(xt * sw[:, None], zt * sw, rcond=None)
        eta = np.clip(z - (zt - xt @ beta), -700.0, 700.0)
        mu = np.exp(eta)
        dev_new = _deviance(y, mu)
        converged = abs(dev - dev_new) <= TOL * (abs(dev_new) + 0.1)
        dev = dev_new
        if converged:
            break
    else:
        raise RuntimeError(f"PPML did not converge in {MAX_ITER} iterations")

    xt = _demean(x, mu, groups, sizes)
    bread = np.linalg.pinv(xt.T @ (xt * mu[:, None]))
    score = xt * (y - mu)[:, None]
    if cluster is None:
        meat = score.T @ score
    else:
        c = np.unique(np.asarray(cluster)[keep], return_inverse=True)[1]
        g = int(c.max()) + 1
        sums = np.stack([np.bincount(c, score[:, j], minlength=g) for j in range(score.shape[1])], axis=1)
        meat = sums.T @ sums * (g / (g - 1))
    r2 = float(np.corrcoef(y, mu)[0, 1] ** 2)
    return Fit(names, beta, bread @ meat @ bread, int(keep.sum()), int((~keep).sum()), r2, keep)
