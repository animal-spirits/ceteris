"""Python outputs of the step 3 web parity: `build.build`'s map layers, tiles and fills.

    python scripts/parity/web_outputs.py DIR

Rebuilds the provinces and counties from DIR (as `web_inputs.py` writes them), then, as
`build.build` does: simplifies both, unions the simplified provinces per country (codes sorted)
and writes the three layers with `build.write_tiles` and the fills with `fills.write`. Writes to
DIR/python each layer's GeoJSON (`build.write_geojson`, NAME.geojson) and simplified vertices
(NAME.simple.arrow, as `geo_inputs.geom_table`), the fills (NAME.fill), map.pmtiles, and
web.json: the layer and fill digests `build.build` records, the fill stats, and which simplify
path each layer took.
"""

import hashlib
import json
import sys
from pathlib import Path

import numpy as np
import shapely
from geo_inputs import geom_table
from pyarrow import ipc

from ceteris.packs import build, fills


def vertices(g) -> np.ndarray:
    """(part, ring, x, y) per vertex, in shapely's order."""
    rows = []
    for j, p in enumerate(shapely.get_parts(g)):
        for k, r in enumerate([p.exterior, *p.interiors]):
            c = shapely.get_coordinates(r)
            rows.append(np.column_stack([np.full(len(c), j), np.full(len(c), k), c]))
    return np.vstack(rows) if rows else np.empty((0, 4))


def geometries(dir: Path, name: str) -> list:
    t = ipc.open_file(dir / f"{name}.geom.arrow").read_all()
    multi = ipc.open_file(dir / f"{name}.web.arrow").read_all()["multi"].to_pylist()
    g, p, r = (t[c].to_numpy() for c in ("geom", "part", "ring"))
    xy = np.column_stack([t["x"].to_numpy(), t["y"].to_numpy()])
    # Rows come geometry by geometry, part by part, ring by ring.
    key = np.column_stack([g, p, r])
    cut = np.flatnonzero((key[1:] != key[:-1]).any(axis=1)) + 1
    parts: list[list[list]] = [[] for _ in multi]
    for s, e in zip([0, *cut], [*cut, len(g)]):
        rings = parts[g[s]]
        if r[s] == 0:
            assert p[s] == len(rings), (name, g[s])
            rings.append([xy[s:e]])
        else:
            assert p[s] == len(rings) - 1 and r[s] == len(rings[-1]), (name, g[s])
            rings[-1].append(xy[s:e])
    out = []
    for m, rings in zip(multi, parts):
        polys = [shapely.Polygon(ring[0], ring[1:]) for ring in rings]
        assert m or len(polys) == 1, name
        out.append(shapely.MultiPolygon(polys) if m else polys[0])
    return out


def path_taken(geoms) -> str:
    try:
        shapely.coverage_simplify(np.array(geoms, dtype=object), build.SIMPLIFY_DEG)
        return "coverage"
    except shapely.errors.GEOSException:
        return "per geometry"


def main(dir: Path) -> None:
    provs, cty = geometries(dir, "provinces"), geometries(dir, "counties")
    country = ipc.open_file(dir / "provinces.web.arrow").read_all()["country"].to_pylist()
    py = dir / "python"
    py.mkdir(parents=True, exist_ok=True)

    simple = build.simplify(provs)
    groups = {}
    for i, c in enumerate(country):
        groups.setdefault(c, []).append(i)
    codes = sorted(set(country))
    county_simple = build.simplify(cty)
    layers = {
        "provinces": (simple, 0),
        "countries": ([shapely.union_all(simple[groups[c]]) for c in codes], 0),
        "counties": (county_simple, build.COUNTY_MINZOOM),
    }
    hashes = build.write_tiles(py / "map.pmtiles", layers)
    for name, (geoms, minzoom) in layers.items():
        build.write_geojson(py / f"{name}.geojson", geoms, minzoom)
        geom_table(py / f"{name}.simple.arrow", geoms)
    fill_stats = {}
    for name, geoms in (("provinces", simple), ("counties", county_simple)):
        fill_stats[name] = fills.write(py / f"{name}.fill", geoms)
        hashes[f"web/{name}.fill"] = hashlib.sha256((py / f"{name}.fill").read_bytes()).hexdigest()
    report = {
        "hashes": hashes,
        "fills": fill_stats,
        "simplify": {"provinces": path_taken(provs), "counties": path_taken(cty)},
    }
    (py / "web.json").write_text(json.dumps(report, sort_keys=True, separators=(",", ":")))


if __name__ == "__main__":
    main(Path(sys.argv[1]))
