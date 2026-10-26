"""Stability table of the stock-flow price rule (`specs/trade/SPEC.md`, Stability region): the Jury conditions
on the 3 x 3 transition matrix per traded good, over the demand elasticities E in [sigma, theta_g + 1] and the
supply elasticities eps of the fixture's enterprises of the good. Prints the rows of the table.

    uv run --project python python -m ceteris.calibrate.stability [FIXTURE_DIR]
"""

import json
import sys
from collections import defaultdict
from decimal import ROUND_HALF_DOWN, Decimal
from pathlib import Path

import numpy as np
from pyarrow import ipc

FIXTURE = Path(__file__).resolve().parents[3] / "tests" / "golden" / "p2-fixture"
NAMES = (
    "Agriculture",
    "Crude oil and gas",
    "Coal and other mining",
    "Food",
    "Light manufacturing",
    "Chemicals, refined fuels, metals",
    "Machinery, electronics, vehicles",
)
SIGMA_DEMAND = 0.6  # lower end of E: total household demand (P06)
ETAS = (0.3, 1.0)  # default mode, frictionless (P41)
STEP = 0.0005
TINY = 1e-9  # kappa1 limit "as kappa2 goes to 0"


def stable(e: float, eta: float, eps: float, cover: float, k1: float, k2: float) -> bool:
    a2 = e * (k1 + k2) + e * k2 / cover + eps * eta * (k1 + k2 / cover) + eta - 3
    a1 = (eta - 2) * e * (k1 + k2) + (eta - 1) * e * k2 / cover - eps * eta * k1 - 2 * eta + 3
    a0 = (1 - eta) * (e * (k1 + k2) - 1)
    p1 = 1 + a2 + a1 + a0  # zero on the kappa2 = 0 edge, which counts as inside
    return (p1 > 0 or abs(p1) < 1e-12) and -1 + a2 - a1 + a0 < 0 and abs(a0) < 1 and 1 - a0 * a0 > abs(a1 - a0 * a2)


def first_failing(theta: float, cover: float, eta: float, eps: tuple[float, float], k1: float | None, k2: float | None):
    """First value of the free coordinate (kappa1 if `k1` is None, else kappa2) on the STEP grid that leaves
    the region for some E and eps."""
    es = np.append(np.linspace(SIGMA_DEMAND, theta + 1, 80), [SIGMA_DEMAND, theta + 1])
    epss = np.linspace(*eps, 6)
    for i in range(1, 4000):
        v = i * STEP
        a, b = (v, k2) if k1 is None else (k1, v)
        if not all(stable(e, eta, s, cover, a, b) for e in es for s in epss):
            return v
    raise ValueError("no limit below 2")


def eps_by_good(fixture: Path) -> dict[int, tuple[float, float]]:
    """Supply elasticity of the labour target, s_L sigma_FL / ((1 - s_L) a_VA), range over the enterprises of a good."""
    rows = {
        n: ipc.open_file(str(fixture / f"{n}.arrow")).read_all().to_pylist()
        for n in ("enterprises", "provinces", "countries", "country_io")
    }
    sigma_fl = json.loads((fixture / "params.json").read_text())["econ"]["sigma_fl"]
    s_l = {c["country"]: c["s_l"] for c in rows["countries"]}
    country = {p["province"]: p["country"] for p in rows["provinces"]}
    inputs = defaultdict(float)
    for r in rows["country_io"]:
        inputs[(r["country"], r["sector"])] += r["a"]
    eps = defaultdict(list)
    for e in rows["enterprises"]:
        c = country[e["province"]]
        eps[e["good"]].append(s_l[c] * sigma_fl / ((1 - s_l[c]) * (1 - inputs[(c, e["good"])])))
    return {g: (min(v), max(v)) for g, v in eps.items()}


def fmt(x: float) -> str:
    return str(Decimal(str(round(x, 4))).quantize(Decimal("0.001"), rounding=ROUND_HALF_DOWN))


def main(fixture: Path = FIXTURE) -> None:
    trade = json.loads((fixture / "params.json").read_text())["trade"]
    eps = eps_by_good(fixture)
    for g, name in enumerate(NAMES):
        theta, cover = trade["theta"][g], trade["coverage"][g]
        cells = [
            [first_failing(theta, cover, eta, eps[g], *at) for eta in ETAS] for at in ((None, TINY), (0.05, None), (0.02, None))
        ]
        print(f"| {g} {name} | {theta:.2f} | {cover:g} | " + " | ".join(f"{fmt(a)} / {fmt(b)}" for a, b in cells) + " |")


if __name__ == "__main__":
    main(Path(sys.argv[1]) if len(sys.argv) > 1 else FIXTURE)
