"""The reduced residual of the joint productivity inversion that ADR 0089 replaced: the reference
model's residual in ln p and ln w with every A eliminated in closed form so that each enterprise
matches its nominal value added. Kept as the numpy twin of `jax_model`, the exact residual for
ADR 0096; no calibration uses it.

At given prices and wages an enterprise with value-added price p_VA > 0 matches nominal VA T with
one A: the first-order condition p_VA dVA/dL = w and p_VA VA = T give L / L0 = x with
s T = w L0 ((1 - s) x^(1 - rho) + s x) (x = s T / (w L0) for Cobb-Douglas), whatever p_VA, and
A = T / (p_VA VA0 g(x)), g the normalized CES.
"""

import numpy as np

from . import model
from .io import Inputs

PVA_FLOOR = 1e-9  # p_VA floor relative to the own price: A stays finite
LABOUR_TOL = 1e-15
MAX_LABOUR_ITERATIONS = 100


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
    """A of every enterprise matching its target `inp.ent_va_target` at x (p_VA floored, see PVA_FLOOR)."""
    price, wage = model._split(x, inp)
    p_va = np.maximum(value_added_price(x, inp), PVA_FLOOR * price[inp.ent_province, inp.ent_good])
    t = inp.ent_va_target
    r = model.rho(inp)
    ratio = labour_ratio(wage[inp.ent_province], inp.ent_l0, t, inp.ent_s_l, r)
    return t / (p_va * inp.ent_va0 * model.value_added(1.0, 1.0, 1.0, inp.ent_s_l, r, ratio))


def residual(x: np.ndarray, inp: Inputs) -> np.ndarray:
    """The reference's residual at x with every A from `productivity_of`."""
    return model.market_residual(model.evaluate(x, inp, productivity_of(x, inp)), inp)
