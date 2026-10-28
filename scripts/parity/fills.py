"""Compares two fill files decoded: bytes, then per feature the triangle set and the area.

    python scripts/parity/fills.py A.fill B.fill [GEOM.arrow]

Triangles are compared as sets of their three float32 vertices, rotated to start at the
smallest; areas are the triangles' summed planar areas in float64. With the features' vertex
table (as `geo_inputs.py` writes it), each feature whose areas differ is listed with both areas
relative to its polygon's area in Mercator. Exits 1 when any feature's area differs by more than
1e-9 relative.
"""

import sys
from pathlib import Path

import numpy as np
import shapely
from pyarrow import ipc

from ceteris.packs import fills


def decoded(path: Path):
    xy, feature, index, n = fills.read(path)
    tris = index.reshape(-1, 3)
    corners = xy[tris].astype(np.float64)
    a, b, c = corners[:, 0], corners[:, 1], corners[:, 2]
    area = np.abs((b[:, 0] - a[:, 0]) * (c[:, 1] - a[:, 1]) - (c[:, 0] - a[:, 0]) * (b[:, 1] - a[:, 1])) / 2
    owner = feature[tris[:, 0]]
    keys = [tuple(map(tuple, t)) for t in xy[tris].tolist()]
    keys = [min(k[i:] + k[:i] for i in range(3)) for k in keys]
    return n, owner, keys, np.bincount(owner, area, n), len(xy)


def by_feature(owner, keys) -> dict[int, set]:
    out: dict[int, set] = {}
    for o, k in zip(owner.tolist(), keys):
        out.setdefault(o, set()).add(k)
    return out


def polygon_area(table, f: int) -> float:
    """Planar area in Mercator of feature f of a vertex table: exteriors less holes."""
    g, p, r = (table[c].to_numpy() for c in ("geom", "part", "ring"))
    xy = np.column_stack([table["x"].to_numpy(), table["y"].to_numpy()])
    total = 0.0
    m = g == f
    for part in np.unique(p[m]):
        rings = [shapely.Polygon(fills.mercator(xy[m & (p == part) & (r == k)])).area for k in np.unique(r[m & (p == part)])]
        total += rings[0] - sum(rings[1:])
    return total


def main(a: Path, b: Path, geom: Path | None) -> int:
    if a.read_bytes() == b.read_bytes():
        print(f"{a.name}: bytes identical")
        return 0
    na, oa, ka, area_a, va = decoded(a)
    nb, ob, kb, area_b, vb = decoded(b)
    if na != nb:
        print(f"{a.name}: features {na} vs {nb}")
        return 1
    sets_a, sets_b = by_feature(oa, ka), by_feature(ob, kb)
    same = sum(sets_a.get(f) == sets_b.get(f) for f in range(na))
    rel = np.abs(area_a - area_b) / np.maximum(np.abs(area_a), np.finfo(float).tiny)
    print(
        f"{a.name}: bytes differ; vertices {va} vs {vb}, triangles {len(ka)} vs {len(kb)}; "
        f"{same}/{na} features with equal triangle sets; area max_rel={rel.max():.3e}"
    )
    if geom is not None:
        table = ipc.open_file(geom).read_all()
        for f in np.flatnonzero(rel > 1e-9):
            true = polygon_area(table, int(f))
            print(f"  feature {f}: area/polygon - 1 = {area_a[f] / true - 1:.3e} vs {area_b[f] / true - 1:.3e}")
    return 0 if rel.max() <= 1e-9 else 1


if __name__ == "__main__":
    sys.exit(main(Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]) if len(sys.argv) > 3 else None))
