"""Dumps what the Python reference derives from an export directory as JSON, for the Rust
reference's `parity` test: the derived inputs, PIGL weights, `invert_at`, PIGL shares and
quantities on random draws, erfinv on a grid, the A.3 bound, the spectral radius of random matrices, every quantity of
`model.evaluate` with the residual at the data state and at a perturbed state and, with `--dense`, the dense
kernels and a balanced Sinkhorn of each traded good. With `--solve`, the equilibrium from the data
state. With `--reference PACK`, the calibrated inputs installed in PACK (of this export), the evaluation
at their reference point and the equilibrium from that point perturbed by REFERENCE_SPREAD in every
ln p and ln w. Non-finite floats are written as
strings.

    python step4_dump.py EXPORT_DIR OUT.json [--dense] [--solve] [--reference PACK]
"""

import argparse
import json
import time
from pathlib import Path

import numpy as np
from scipy.special import erfinv

from ceteris.reference import calibrate, demand, io, model, sinkhorn, solve, uniqueness


def enc(x):
    a = np.asarray(x)
    if a.dtype == bool:
        return a.tolist()
    if np.issubdtype(a.dtype, np.integer):
        return a.astype(np.int64).tolist()
    flat = [v if np.isfinite(v) else str(v) for v in a.astype(np.float64).ravel().tolist()]
    return {"shape": list(a.shape), "data": flat}


def weights(w: demand.Weights) -> dict:
    return {"scale": enc(w.scale), "omega": enc(w.omega), "within": enc(w.within)}


def evaluation(inp: io.Inputs, x: np.ndarray) -> dict:
    """Every quantity of `model.evaluate` at x, with the residual and the excess demand."""
    s = model.evaluate(x, inp)
    f = s["flows"]
    point = {"x": enc(x), "residual": enc(model.market_residual(s, inp)), "excess": enc(model._excess(s, inp))}
    for key in ("price", "wage", "employment", "output", "value_added", "supply", "consumer_price", "spend", "demand", "q"):
        point[key] = enc(s[key])
    for key in ("labour_demand", "income", "index"):
        point[key] = enc(s[key])
    for key in ("orders", "home", "arrive", "paid", "received", "tariff", "pair_value", "pair_units"):
        point[f"flows.{key}"] = enc(f[key])
    return point


REFERENCE_SPREAD = 0.05


def solution(inp: io.Inputs, x0: np.ndarray | None = None) -> dict:
    """The equilibrium from x0 (default the data state), its fields and the solve's summary with
    its time."""
    start = time.perf_counter()
    eq = solve.equilibrium(inp, x0)
    seconds = time.perf_counter() - start
    fields = (
        "x",
        "price",
        "wage",
        "employment",
        "output",
        "supply",
        "consumer_price",
        "expenditure",
        "home_value",
        "home_units",
        "flows_by_country_pair_good",
        "units_by_country_pair_good",
        "income",
        "price_index",
    )
    out = {k: enc(getattr(eq, k)) for k in fields}
    if x0 is not None:
        out["start"] = enc(x0)
    return {**out, "info": {**eq.info, "seconds": seconds}}


def main(export: str, out: str, dense: bool, solve_: bool, reference: str | None) -> None:
    inp = io.load(export)
    rng = np.random.default_rng(7)
    n_c, n_g = inp.n_countries, inp.n_goods
    d: dict = {"inputs": str(Path(export).resolve())}
    for name in (
        "labour_supply",
        "population",
        "pop_effective",
        "ent_inputs",
        "ent_a_va",
        "base_income",
        "base_spend",
        "sigma_income",
        "employed_supply",
        "active_cells",
        "index_weights",
        "base_output",
        "base_demand",
    ):
        d[name] = enc(getattr(inp, name))
    d["pigl_weights"] = weights(inp.pigl_weights)

    price = np.exp(rng.uniform(-0.5, 0.5, (n_c, n_g)))
    spend = inp.base_spend * np.exp(rng.uniform(-0.3, 0.3, n_c))
    d["invert_at"] = {
        "price": enc(price),
        "spend": enc(spend),
        "weights": weights(demand.invert_at(inp.omega, price, spend, inp.sigma_income, inp.category, inp.sigma_demand, inp.pigl)),
    }

    k = 500
    country = rng.integers(0, n_c, k)
    draw_price = np.exp(rng.uniform(-1.0, 1.0, (k, n_g)))
    draw_price[rng.uniform(size=(k, n_g)) < 0.1] = np.inf
    money = inp.base_spend[country] * np.exp(rng.uniform(-3.0, 3.0, k))
    money[:5] = 0.0
    sigma = rng.uniform(0.0, 1.5, k)
    # Draws without money divide 0 by 0, by design.
    with np.errstate(invalid="ignore"):
        share = demand.shares(inp.pigl_weights, country, draw_price, money, sigma, inp.category, inp.sigma_demand, inp.pigl)
    d["draws"] = {
        "country": enc(country),
        "price": enc(draw_price),
        "money": enc(money),
        "sigma": enc(sigma),
        "share": enc(share),
        "quantity": enc(demand.quantities(share, draw_price, money)),
    }

    x = np.concatenate(
        [
            np.linspace(-1.0, 1.0, 20001)[1:-1],
            1.0 - np.logspace(-16, -1, 200),
            -(1.0 - np.logspace(-16, -1, 200)),
            np.logspace(-300, -1, 200),
            [0.0, -0.0, 1.0, -1.0],
        ]
    )
    d["erfinv"] = {"x": enc(x), "y": enc(erfinv(x))}

    va = uniqueness.value_added_share(inp)
    traded = np.flatnonzero(inp.theta > 0)
    d["uniqueness"] = {
        "value_added_share": enc(va),
        "matrix": [enc(uniqueness.trade_with_intermediates(inp.theta[g], va[g])) for g in traded],
        "radius": enc([uniqueness.spectral_radius(uniqueness.trade_with_intermediates(inp.theta[g], va[g])) for g in traded]),
        "bound": enc(uniqueness.spectral_bound(inp.theta, value_added_share=va)),
        "bound_labour_only": enc(uniqueness.spectral_bound(inp.theta)),
    }
    # Signed matrices with structural zeros: reducible, periodic and defective cases among them.
    mats = []
    for _ in range(300):
        n = int(rng.integers(1, 7))
        m = rng.normal(size=(n, n)) * (rng.uniform(size=(n, n)) < 0.6)
        mats.append(m)
    # Small integers and permutation-like patterns: tied rows and ratios, cycles.
    for _ in range(300):
        n = int(rng.integers(1, 7))
        m = rng.integers(-2, 3, size=(n, n)).astype(np.float64) * (rng.uniform(size=(n, n)) < 0.4)
        mats.append(m)
    for n in range(2, 7):
        for c in (0.1, 0.25, 0.5, 0.9, 1.0, 2.0):
            m = np.roll(np.eye(n), 1, axis=1)
            m[-1, 0] = c
            mats.append(m)
    d["random_radius"] = {"matrix": [enc(m) for m in mats], "radius": enc([uniqueness.spectral_radius(m) for m in mats])}

    if dense:
        kernels = []
        for g in np.flatnonzero(inp.traded):
            kern = sinkhorn.dense_kernel(inp, g)
            net = sinkhorn.dense_kernel(inp, g, net_of_tariff=True)
            supply = rng.uniform(0.1, 2.0, inp.n_provinces)
            demand_ = rng.uniform(0.1, 2.0, inp.n_provinces)
            demand_ *= supply.sum() / demand_.sum()
            kernels.append(
                {
                    "good": int(g),
                    "kernel": enc(kern),
                    "net": enc(net),
                    "supply": enc(supply),
                    "demand": enc(demand_),
                    "balanced": enc(sinkhorn.balanced(net, supply, demand_)),
                }
            )
        d["kernels"] = kernels

    # A separate generator, so the draws above stay as they were.
    model_rng = np.random.default_rng(11)
    x0 = model.data_state(inp)
    points = [evaluation(inp, x) for x in (x0, x0 + model_rng.normal(0.0, 0.2, len(x0)))]
    d["model"] = points
    if solve_:
        d["solution"] = solution(inp)
    if reference:
        pack_province = io._table(Path(export) / "pack_provinces.arrow")["pack"].astype(np.int64)
        ref, x = calibrate.read_reference(reference, inp, pack_province)
        w = ref.calibrated_weights
        d["reference"] = {
            "ent_a": enc(ref.ent_a),
            "ent_va0": enc(ref.ent_va0),
            "ent_l0": enc(ref.ent_l0),
            "w0": enc(ref.w0),
            "u_star": enc(ref.u_star),
            "coef": enc(ref.calibrated_coef),
            "a_va": enc(ref.calibrated_a_va),
            "weights": weights(w),
            "point": evaluation(ref, x),
            "solution": solution(ref, x + np.random.default_rng(13).normal(0.0, REFERENCE_SPREAD, len(x))),
        }

    Path(out).write_text(json.dumps(d))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("export")
    parser.add_argument("out")
    parser.add_argument("--dense", action="store_true")
    parser.add_argument("--solve", action="store_true")
    parser.add_argument("--reference", help="a pack with calibration tables of this export")
    args = parser.parse_args()
    main(args.export, args.out, args.dense, args.solve, args.reference)
