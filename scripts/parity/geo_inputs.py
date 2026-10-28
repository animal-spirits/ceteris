"""Inputs and Python outputs of the step 3 geo parity (geo.py, raster.py, resources.py, fills.py).

    python scripts/parity/geo_inputs.py OUT

Builds the provinces and the clipped counties as `build.build` does, then writes:

- OUT/provinces.geom.arrow, OUT/counties.geom.arrow: their polygon coordinates, one row per vertex
  (geom, part, ring, x, y), rings closed, in shapely's order;
- OUT/python/*.arrow: what geo, raster and resources compute from them, as `build.build` does;
- OUT/python/{provinces,counties}.fill: `fills.write` of the same (unsimplified) geometry.

Prints how many MRDS sites lie in more than one province: the Python keeps the first in STRtree
order, the Rust the lowest index, which agree when that count is zero.
"""

import csv
import io
import sys
import zipfile
from pathlib import Path

import h3
import numpy as np
import pyarrow as pa
import shapely
from shapely import STRtree

from ceteris.packs import build, counties, fetch, fills, geo, provinces, raster, resources

CONFIG = build.CONFIG


def geom_table(path: Path, geoms) -> None:
    geom, part, ring, xy = [], [], [], []
    for i, g in enumerate(geoms):
        for j, p in enumerate(shapely.get_parts(g)):
            assert p.geom_type == "Polygon", p.geom_type
            for k, r in enumerate([p.exterior, *p.interiors]):
                c = shapely.get_coordinates(r)
                geom.append(np.full(len(c), i))
                part.append(np.full(len(c), j))
                ring.append(np.full(len(c), k))
                xy.append(c)
    xy = np.vstack(xy)
    build.write_table(
        path,
        {
            "geom": pa.array(np.concatenate(geom).astype(np.uint32)),
            "part": pa.array(np.concatenate(part).astype(np.uint32)),
            "ring": pa.array(np.concatenate(ring).astype(np.uint32)),
            "x": pa.array(xy[:, 0]),
            "y": pa.array(xy[:, 1]),
        },
    )


def multi_province_sites(cache: Path, tree: STRtree) -> int:
    with zipfile.ZipFile(cache / "mrds-csv.zip") as z, z.open("mrds.csv") as f:
        rows = [
            r
            for r in csv.DictReader(io.TextIOWrapper(f, encoding="latin-1"))
            if r["dev_stat"] in resources.PRODUCING and r["longitude"] and r["latitude"]
        ]
    pts = shapely.points([float(r["longitude"]) for r in rows], [float(r["latitude"]) for r in rows])
    site, _ = tree.query(pts, predicate="within")
    return int((np.bincount(site) > 1).sum())


def main(out: Path) -> None:
    cache = fetch.CACHE
    admin1, admin0 = cache / "ne_10m_admin_1_states_provinces.zip", cache / "ne_10m_admin_0_countries.zip"
    etopo = cache / "ETOPO_2022_v1_60s_N90W180_surface.tif"
    units = counties.load(cache)
    provs = provinces.build(admin1, admin0, CONFIG["split_area_km2"], CONFIG["land_res"], units)
    n = len(provs)
    home = counties.assign(units, [p.geom for p in provs], [{p.country, p.country_now} for p in provs])
    clipped = []
    for c, h in zip(units, home):
        if h >= 0:
            piece = geo.polygonal(c.geom.intersection(provs[h].geom))
            if not piece.is_empty:
                clipped.append((int(h), counties.County(c.iso, c.level, c.name, piece)))
    clipped.sort(key=lambda hc: hc[0])
    cty = [c for _, c in clipped]
    for c in cty:
        c.lon, c.lat = geo.anchor(c.geom)

    py = out / "python"
    py.mkdir(parents=True, exist_ok=True)
    geom_table(out / "provinces.geom.arrow", [p.geom for p in provs])
    geom_table(out / "counties.geom.arrow", [c.geom for c in cty])

    ccells, cowner = raster.assign_cells(cty, CONFIG["county_res"])
    cterr = raster.terrain(ccells, etopo, depth=0)
    carea = raster.cell_area_km2(ccells)
    with np.errstate(invalid="ignore"):
        cstat = {k: raster.province_means(cowner, carea, v, len(cty)) for k, v in cterr.items()}
    bare = np.flatnonzero(np.isnan(cstat["elev_mean_m"]))
    if len(bare):
        point = raster.terrain([h3.latlng_to_cell(cty[i].lat, cty[i].lon, raster.FINEST) for i in bare], etopo, depth=0)
        for k, values in cstat.items():
            values[bare] = point[k]
    build.write_table(
        py / "county_cells.arrow",
        {
            "cell": pa.array([int(c, 16) for c in ccells], pa.uint64()),
            "county": pa.array(cowner),
            "area_km2": pa.array(carea),
            **{k: pa.array(v) for k, v in cterr.items()},
        },
    )
    build.write_table(
        py / "counties.arrow",
        {
            "lon": pa.array([c.lon for c in cty], pa.float64()),
            "lat": pa.array([c.lat for c in cty], pa.float64()),
            "area_km2": pa.array([geo.area_km2(c.geom) for c in cty]),
            **{k: pa.array(v) for k, v in cstat.items()},
        },
    )

    cells, owner = raster.assign_cells(provs, CONFIG["land_res"])
    terr = raster.terrain(cells, etopo)
    area = raster.cell_area_km2(cells)
    tree = STRtree([p.geom for p in provs])
    res = resources.counts(cache / "mrds-csv.zip", tree, n)
    build.write_table(
        py / "cells.arrow",
        {
            "cell": pa.array([int(c, 16) for c in cells], pa.uint64()),
            "province": pa.array(owner),
            "area_km2": pa.array(area),
            **{k: pa.array(v) for k, v in terr.items()},
        },
    )
    build.write_table(
        py / "provinces.arrow",
        {
            "lon": pa.array([p.lon for p in provs], pa.float64()),
            "lat": pa.array([p.lat for p in provs], pa.float64()),
            "area_km2": pa.array([p.area_km2 for p in provs], pa.float64()),
            **{k: pa.array(raster.province_means(owner, area, v, n)) for k, v in terr.items()},
            "cells": pa.array(np.bincount(owner, minlength=n).astype(np.uint32)),
            **{f"res_{g}": pa.array(v) for g, v in res.items()},
        },
    )
    for name, geoms in (("provinces", [p.geom for p in provs]), ("counties", [c.geom for c in cty])):
        fills.write(py / f"{name}.fill", geoms)
    print(f"mrds sites within more than one province: {multi_province_sites(cache, tree)}")
    print(f"counties sampled at their anchor: {len(bare)}")


if __name__ == "__main__":
    main(Path(sys.argv[1]))
