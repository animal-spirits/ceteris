"""Mineral deposits per province from USGS MRDS (Section 5.2). Coverage is US-heavy (about 89% of sites)."""

import csv
import io
import zipfile
from pathlib import Path

import numpy as np
import shapely
from shapely import STRtree

# Primary commodity (MRDS commod1) to resource group; construction materials are left out.
GROUPS = {
    "precious": {"Gold", "Silver", "Platinum", "Palladium", "PGE", "Diamond", "Gemstone"},
    "base": {"Copper", "Lead", "Zinc", "Nickel", "Tin", "Aluminum", "Antimony", "Mercury", "Cobalt", "Molybdenum", "Tungsten"},
    "ferrous": {"Iron", "Manganese", "Chromium", "Vanadium", "Titanium"},
    "energy": {"Uranium", "Thorium", "Coal"},
    "industrial": {
        "Phosphorus-Phosphates",
        "Potassium",
        "Sulfur",
        "Salt",
        "Boron-Borates",
        "Fluorine-Fluorite",
        "Barium-Barite",
        "Graphite",
        "Lithium",
    },
}
PRODUCING = {"Producer", "Past Producer"}


def counts(mrds_zip: Path, tree: STRtree, n: int) -> dict[str, np.ndarray]:
    """Producing and past-producing sites per province, by group of their primary commodities."""
    with zipfile.ZipFile(mrds_zip) as z, z.open("mrds.csv") as f:
        rows = [
            r
            for r in csv.DictReader(io.TextIOWrapper(f, encoding="latin-1"))
            if r["dev_stat"] in PRODUCING and r["longitude"] and r["latitude"]
        ]
    rows.sort(key=lambda r: r["dep_id"])
    pts = shapely.points([float(r["longitude"]) for r in rows], [float(r["latitude"]) for r in rows])
    site, prov = tree.query(pts, predicate="within")
    first = {}
    for s, p in zip(site, prov):
        first.setdefault(int(s), int(p))
    out = {g: np.zeros(n, dtype=np.uint32) for g in GROUPS}
    for s, p in first.items():
        commodities = {c.strip() for c in rows[s]["commod1"].split(",")}
        for g, members in GROUPS.items():
            if commodities & members:
                out[g][p] += 1
    return out
