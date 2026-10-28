"""Dumps what the Python reference derives from an export directory as JSON, for the Rust
reference's `parity` test: the derived inputs, PIGL weights, `invert_at`, PIGL shares and
quantities on random draws, erfinv on a grid, the A.3 bound, the spectral radius of random matrices and, with `--dense`, the dense
kernels and a balanced Sinkhorn of each traded good. Non-finite floats are written as strings.

    python step4_dump.py EXPORT_DIR OUT.json [--dense]
"""

import json
import sys
from pathlib import Path

import numpy as np
from scipy.special import erfinv

from ceteris.reference import demand, io, sinkhorn, uniqueness


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


def main(export: str, out: str, dense: bool) -> None:
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

    Path(out).write_text(json.dumps(d))


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], "--dense" in sys.argv[3:])
