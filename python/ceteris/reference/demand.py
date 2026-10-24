"""PIGL final demand (ADR 0021), mirroring `ceteris-econ` demand.rs: the multi-sector PIGL of Alder,
Boppart and Mueller (2022) across three categories, a CES over goods within each category, and a
pop's lognormal income integrated in closed form through E[y^(1 - eps)]. Vectorized over a
leading axis of pops (or draws)."""

from dataclasses import dataclass

import numpy as np
from scipy.special import erfinv

N_CATEGORIES = 3


@dataclass(frozen=True)
class Pigl:
    """P46 sigma_b, phi, gamma, theta (A, M, S); P47 eps."""

    sigma_b: float
    phi: float
    gamma: float
    theta: np.ndarray
    eps: float


@dataclass(frozen=True)
class Weights:
    """Per country: scale lambda (C,), category weights omega (C, 3), within-category weights (C, G)."""

    scale: np.ndarray
    omega: np.ndarray
    within: np.ndarray


def lognormal_sigma(gini) -> np.ndarray:
    """sqrt(2) Phi^-1((G + 1) / 2) = 2 erfinv(G)."""
    return 2.0 * erfinv(np.asarray(gini, dtype=np.float64))


def invert(omega: np.ndarray, base_spend: np.ndarray, sigma: np.ndarray, category: np.ndarray, p: Pigl) -> Weights:
    """`invert_pigl` for every country: omega (C, G), base spending per person and sigma (C,). Raises
    where a weight would be negative."""
    om = omega / omega.sum(axis=1, keepdims=True)
    onehot = (category[:, None] == np.arange(N_CATEGORIES)[None, :]).astype(np.float64)
    cat = om @ onehot
    with np.errstate(divide="ignore", invalid="ignore"):
        within = np.where(om > 0, om / cat[:, category], 0.0)
    z = cat[:, 0] / p.theta[0]
    weight = (cat - z[:, None] * p.theta[None, :]) / (1.0 - z[:, None])
    weight[:, 0] = 0.0
    bad = (z >= 1.0) | (weight < 0).any(axis=1)
    if bad.any():
        raise ValueError(f"PIGL inversion gives a negative weight in countries {np.flatnonzero(bad).tolist()}")
    eps = p.eps
    scale = z * base_spend**eps * np.exp(eps * (1.0 - eps) * sigma**2 / 2.0)
    return Weights(scale=scale, omega=weight, within=within)


def _aggregate(weight: np.ndarray, index: np.ndarray, sigma: float) -> tuple[np.ndarray, np.ndarray]:
    """CES aggregator over categories with a weight and a finite index, weights renormalized over
    them: the aggregate (K,) and each category's share (K, 3)."""
    on = (weight > 0) & np.isfinite(index)
    term = np.where(on, weight * np.where(on, index, 1.0) ** (1.0 - sigma), 0.0)
    total = term.sum(axis=1)
    mass = np.where(on, weight, 0.0).sum(axis=1)
    with np.errstate(divide="ignore", invalid="ignore"):
        return (total / mass) ** (1.0 / (1.0 - sigma)), term / total[:, None]


def shares(
    w: Weights,
    country: np.ndarray,
    price: np.ndarray,
    income: np.ndarray,
    sigma: np.ndarray,
    category: np.ndarray,
    sigma_demand: float,
    p: Pigl,
) -> np.ndarray:
    """`pigl_shares`: budget shares (K, G) of pops of `country` (K,) at buyer prices (K, G), mean
    income per person and log income dispersion (K,). Negative category shares are clamped to 0
    and the rest rescaled; goods without a weight or at an infinite price get 0."""
    within_w = w.within[country]
    onehot = (category[:, None] == np.arange(N_CATEGORIES)[None, :]).astype(np.float64)
    buys = (within_w > 0) & np.isfinite(price)
    term = np.where(buys, within_w * np.where(buys, price, 1.0) ** (1.0 - sigma_demand), 0.0)
    total = term @ onehot
    avail = total > 0
    with np.errstate(divide="ignore", invalid="ignore"):
        index = np.where(avail, np.where(avail, total, 1.0) ** (1.0 / (1.0 - sigma_demand)), np.inf)
        within = np.where(term > 0, term / total[:, category], 0.0)

    theta = np.broadcast_to(p.theta, index.shape)
    dt, s_d = _aggregate(theta, index, p.phi)
    omega = w.omega[country]
    b, s_b = _aggregate(omega, index, p.sigma_b)
    eps = p.eps
    with np.errstate(divide="ignore", invalid="ignore"):
        k = w.scale[country] * (dt / b) ** p.gamma * (income / b) ** -eps * np.exp(-eps * (1.0 - eps) * sigma**2 / 2.0)
        raw = np.where(np.isfinite(index), s_b + k[:, None] * (s_d - s_b), 0.0)
    # no reachable category in B(P): the income-effect bundle is all there is
    no_b = ~((omega > 0) & np.isfinite(index)).any(axis=1)
    raw = np.where(no_b[:, None], s_d, raw)
    raw = np.maximum(raw, 0.0)
    raw = raw / raw.sum(axis=1, keepdims=True)
    return raw[:, category] * within


def quantities(share: np.ndarray, price: np.ndarray, money: np.ndarray) -> np.ndarray:
    """`pigl`: units bought from budget shares (K, G) and spending (K,); none without money."""
    with np.errstate(divide="ignore", invalid="ignore"):
        x = np.where(share > 0, share * money[:, None] / price, 0.0)
    return np.where(money[:, None] > 0, x, 0.0)
