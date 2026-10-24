"""Uniqueness check before V1 (ADR 0022; `specs/econ/SPEC.md`, Equilibrium nesting): Theorem 1 of
Allen, Arkolakis and Li (2024, "On the Equilibrium Properties of Spatial Models", AER: Insights
6(4)). A system x_ih = sum_j f_ijh(x_j) whose elasticities are bounded by an H x H matrix A has a
unique solution if rho(A) < 1, at most one up to scale if rho(A) = 1 with constant elasticities,
and for rho(A) > 1 some kernel gives several.

P2 maps onto the paper's Online Appendix A.3 (Eaton-Kortum with intermediate goods and
quasi-symmetric trade costs), applied good by good; the SPEC states the special case and why."""

from typing import Literal

import numpy as np

from . import io


def spectral_radius(a: np.ndarray) -> float:
    """rho of the elasticity-bound matrix |a| (Theorem 1 bounds absolute elasticities)."""
    return float(np.abs(np.linalg.eigvals(np.abs(np.asarray(a, dtype=np.float64)))).max())


def change_of_variables(gamma: np.ndarray, b: np.ndarray, r: np.ndarray | None = None) -> np.ndarray:
    """Remark 5: the constant-elasticity system prod x_ih'^gamma_hh' = sum_j K_ijh prod x_ih'^r_hh'
    x_jh'^b_hh' (equation 8) has elasticity bounds |B (Gamma - R)^-1|."""
    g = np.asarray(gamma, dtype=np.float64)
    m = g if r is None else g - np.asarray(r, dtype=np.float64)
    return np.abs(np.linalg.solve(m.T, np.asarray(b, dtype=np.float64).T).T)


def trade_with_intermediates(elasticity: float, labour_share: float) -> np.ndarray:
    """Online Appendix A.3 (pp. 8-9): equations (29) and (30) in tradeables price index and wage,
    labour share beta of tradeables production, the rest the tradeables aggregate. The paper's
    theta is the inverse of the trade elasticity (Alvarez and Lucas 2007), so its 1/theta in
    Gamma and B is the trade elasticity."""
    t, beta = elasticity, labour_share
    gamma = np.array([[-t, 0.0], [(1.0 - beta) * t, 1.0 + beta * t]])
    b = np.array([[-(1.0 - beta) * t, -beta * t], [-(1.0 - beta) * t, -beta * t]])
    return change_of_variables(gamma, b)


def value_added_share(inp: io.Inputs) -> np.ndarray:
    """Per good, the smallest value-added share of gross output over countries, 1 - sum_k a_kg
    (the largest bound, since A.3's radius falls with the labour share)."""
    return (1.0 - inp.io.sum(axis=1)).min(axis=0)


def spectral_bound(
    theta: np.ndarray,
    agglomeration: float = 0.0,
    migration: float | None = None,
    *,
    value_added_share: np.ndarray | float = 1.0,
) -> float:
    """Largest A.3 radius over traded goods (theta > 0) at each good's trade elasticity and
    value-added share. Agglomeration and migration have no mapping yet: the paper treats them only
    without trade costs (A.6) or without intermediates and with spatial spillovers (A.2, where any
    spillover gives a radius above 1), so they are derived when A5 or migration lands."""
    if agglomeration != 0.0 or migration is not None:
        raise NotImplementedError("uniqueness bound with agglomeration or migration: no mapping onto Theorem 1 yet (ADR 0022)")
    theta = np.asarray(theta, dtype=np.float64)
    va = np.broadcast_to(np.asarray(value_added_share, dtype=np.float64), theta.shape)
    return max(spectral_radius(trade_with_intermediates(theta[g], va[g])) for g in np.flatnonzero(theta > 0))


def v1_mode(bound: float) -> Literal["tolerance", "report"]:
    """V1 compares with the 1% tolerances where the equilibrium is unique (bound at most 1, Theorem 1
    parts i and ii.b); otherwise it reports which equilibrium each model reaches."""
    return "tolerance" if bound <= 1.0 else "report"
