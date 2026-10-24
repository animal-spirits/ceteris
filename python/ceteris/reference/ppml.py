"""PPML gravity estimation for the P2 gate's H2 test (`specs/econ/SPEC.md`, P2 gate): Poisson
pseudo-maximum likelihood on country-pair trade with exporter and importer fixed effects, ln
population-weighted distance and contiguity as regressors, domestic pairs excluded. numpy only."""

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
