"""Balanced Sinkhorn on a dense kernel: the cross-check of the merchant logit on small worlds
(`specs/trade/SPEC.md`, Equilibrium nesting). With both marginals hard, flows have the form
a_o K_od b_d; the logit at equilibrium has that form, so the two coincide."""

import numpy as np

from .io import Inputs

MAX_ITER = 100_000
TOL = 1e-14


def dense_kernel(inp: Inputs, g: int) -> np.ndarray:
    """(P, P) K_od of traded good `g`: 1 at home, the direct leg within a country, and
    sum over gateway pairs of K_oa K_ab K_bd across borders."""
    n_p, n_n = inp.n_provinces, inp.n_gateways
    dom, acc, cro = inp.domestic, inp.access, inp.crossing
    k = np.eye(n_p)
    m = dom.good == g
    k[dom.row[m], dom.col[m]] = dom.k[m]
    access = np.zeros((n_p, n_n))
    m = acc.good == g
    access[acc.row[m], acc.col[m]] = acc.k[m]
    crossing = np.zeros((n_n, n_n))
    m = cro.good == g
    crossing[cro.row[m], cro.col[m]] = cro.k[m]
    return k + access @ crossing @ access.T


def balanced(k: np.ndarray, supply: np.ndarray, demand: np.ndarray) -> np.ndarray:
    """Flows a_o K_od b_d with rows summing to `supply` and columns to `demand` (equal totals);
    zero rows and columns stay zero. Iterates until both marginals hold to TOL relative."""
    a, b = np.ones(len(supply)), np.ones(len(demand))
    total = supply.sum()
    for _ in range(MAX_ITER):
        kb = k @ b
        a = np.divide(supply, kb, out=np.zeros_like(supply), where=kb > 0)
        ka = k.T @ a
        b = np.divide(demand, ka, out=np.zeros_like(demand), where=ka > 0)
        if np.abs(a * (k @ b) - supply).max() <= TOL * total:
            return a[:, None] * k * b[None, :]
    raise RuntimeError(f"Sinkhorn did not converge in {MAX_ITER} iterations")
