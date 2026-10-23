"""Build the world pack: Arrow IPC tables for the core, PMTiles and attributes for the map, and a manifest
(Section 16.1). Needs tippecanoe on PATH for the map tiles (ADR 0009).

    uv run python -m ceteris.packs [--out data/packs/2000]

Same inputs give byte-identical outputs; `manifest.json` records every table's sha256.
"""

import argparse
import hashlib
import json
import shutil
import subprocess
import tempfile
import tomllib
from pathlib import Path

import h3
import numpy as np
import pyarrow as pa
import pyogrio
import shapely
from pyarrow import ipc
from shapely import STRtree

from . import counties, fetch, geo, provinces, raster, resources, transport

CONFIG = tomllib.loads(Path(__file__).with_name("pack.toml").read_text())
ROOT = Path(__file__).resolve().parents[3]
PACK_FORMAT = 2
PALETTE_SIZE = 8
# Map geometry simplification tolerance in degrees (about 1 km).
SIMPLIFY_DEG = 0.01
# Map tiles: zoom range, and the zoom from which counties are drawn.
MAX_ZOOM = 9
COUNTY_MINZOOM = 4


def terrain_class(elev_mean_m: np.ndarray, tri_m: np.ndarray) -> np.ndarray:
    """0 plains, 1 plateau, 2 hills, 3 mountains. TRI is at the 1.85 km ETOPO pixel scale."""
    return np.select([tri_m >= 150, tri_m >= 40, elev_mean_m >= 600], [3, 2, 1], 0).astype(np.uint8)


def colour_countries(codes: list[str], borders: set[tuple[str, str]]) -> dict[str, int]:
    """Welsh-Powell greedy colouring so neighbouring countries differ."""
    nbrs = {c: set() for c in codes}
    for a, b in borders:
        nbrs[a].add(b)
        nbrs[b].add(a)
    colour: dict[str, int] = {}
    for c in sorted(codes, key=lambda c: (-len(nbrs[c]), c)):
        taken = {colour[n] for n in nbrs[c] if n in colour}
        colour[c] = next(k for k in range(len(codes)) if k not in taken) % PALETTE_SIZE
    return colour


def write_table(path: Path, columns: dict[str, pa.Array]) -> str:
    table = pa.table(columns).replace_schema_metadata({"pack_format": str(PACK_FORMAT)})
    with pa.OSFile(str(path), "wb") as f, ipc.new_file(f, table.schema) as w:
        w.write_table(table)
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _round(geom_json: dict, digits: int = 4) -> dict:
    def r(c):
        return [round(c[0], digits), round(c[1], digits)] if isinstance(c[0], float) else [r(x) for x in c]

    return {"type": geom_json["type"], "coordinates": r(geom_json["coordinates"])}


def write_geojson(path: Path, geoms, minzoom: int = 0) -> None:
    """Features numbered from 1, the map's feature IDs (tippecanoe drops ID 0)."""
    extra = {"tippecanoe": {"minzoom": minzoom}} if minzoom else {}
    features = [
        {"type": "Feature", "id": i + 1, "properties": {}, **extra, "geometry": _round(shapely.geometry.mapping(g))}
        for i, g in enumerate(geoms)
    ]
    path.write_text(json.dumps({"type": "FeatureCollection", "features": features}, separators=(",", ":")))


def simplify(geoms) -> np.ndarray:
    geoms = np.array(geoms, dtype=object)
    try:
        return shapely.coverage_simplify(geoms, SIMPLIFY_DEG)
    except shapely.errors.GEOSException:
        # Clipped counties are not always a valid coverage; simplify them one by one.
        return shapely.simplify(geoms, SIMPLIFY_DEG, preserve_topology=True)


def write_tiles(path: Path, layers: dict[str, tuple[list, int]]) -> dict[str, str]:
    """One PMTiles archive with a vector layer per entry: name -> (geometries, minzoom).

    tippecanoe writes the same tiles in a varying order, so reproducibility is tracked by the
    sha256 of each layer's input, which this returns.
    """
    inputs = {}
    with tempfile.TemporaryDirectory() as tmp:
        args = ["tippecanoe", "-q", "-f", "-o", str(path), "-Z0", f"-z{MAX_ZOOM}"]
        args += ["--no-feature-limit", "--no-tile-size-limit", "--detect-shared-borders"]
        for name, (geoms, minzoom) in layers.items():
            src = Path(tmp) / f"{name}.geojson"
            write_geojson(src, geoms, minzoom)
            inputs[f"web/map.pmtiles:{name}"] = sha256(src)
            args += ["-L", f"{name}:{src}"]
        subprocess.run(args, check=True)
    return inputs


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def build(out: Path) -> dict:
    cache = fetch.CACHE
    entries = fetch.entries()
    for e in entries:
        fetch.fetch(e)
    admin1, admin0 = cache / "ne_10m_admin_1_states_provinces.zip", cache / "ne_10m_admin_0_countries.zip"
    etopo = cache / "ETOPO_2022_v1_60s_N90W180_surface.tif"

    units = counties.load(cache)
    provs = provinces.build(admin1, admin0, CONFIG["split_area_km2"], CONFIG["land_res"], units)
    n = len(provs)

    # Counties: each in the province it overlaps most, clipped to it (ADR 0009).
    home = counties.assign(units, [p.geom for p in provs], [{p.country, p.country_now} for p in provs])
    clipped = []
    # Misfit between the sources, over counties small enough to fit a province (larger ones must be cut).
    fits, fits_inside = 0.0, 0.0
    for c, h in zip(units, home):
        if h >= 0:
            piece = geo.polygonal(c.geom.intersection(provs[h].geom))
            whole = geo.area_km2(c.geom)
            if whole <= CONFIG["split_area_km2"] / 2:
                fits += whole
                fits_inside += geo.area_km2(piece)
            if not piece.is_empty:
                clipped.append((int(h), counties.County(c.iso, c.level, c.name, piece)))
    clipped.sort(key=lambda hc: hc[0])
    county_prov = np.array([h for h, _ in clipped], dtype=np.uint32)
    cty = [c for _, c in clipped]
    for c in cty:
        c.lon, c.lat = geo.anchor(c.geom)
    cty_area = np.array([geo.area_km2(c.geom) for c in cty])
    ccells, cowner = raster.assign_cells(cty, CONFIG["county_res"])
    cterr = raster.terrain(ccells, etopo, depth=0)
    carea = raster.cell_area_km2(ccells)
    with np.errstate(invalid="ignore"):
        cstat = {k: raster.province_means(cowner, carea, v, len(cty)) for k, v in cterr.items()}
    # Slivers whose only cell went to a neighbour are sampled at their anchor.
    bare = np.flatnonzero(np.isnan(cstat["elev_mean_m"]))
    if len(bare):
        point = raster.terrain([h3.latlng_to_cell(cty[i].lat, cty[i].lon, raster.FINEST) for i in bare], etopo, depth=0)
        for k, values in cstat.items():
            values[bare] = point[k]
    cells, owner = raster.assign_cells(provs, CONFIG["land_res"])
    terr = raster.terrain(cells, etopo)
    area = raster.cell_area_km2(cells)
    elev = raster.province_means(owner, area, terr["elev_mean_m"], n)
    elev_std = raster.province_means(owner, area, terr["elev_std_m"], n)
    tri = raster.province_means(owner, area, terr["tri_m"], n)
    tree = STRtree([p.geom for p in provs])
    res = resources.counts(cache / "mrds-csv.zip", tree, n)
    graph, zones, extra = transport.build(provs, tri, cache, CONFIG["sea_res"], CONFIG["sea_zone_res"])

    kinds = np.array(graph.kind)
    node_prov = np.array(graph.province)

    def per_province(kind):
        return np.bincount(node_prov[(kinds == kind) & (node_prov >= 0)], minlength=n).astype(np.uint32)

    codes = sorted({p.country for p in provs})
    by_index = {i: p.country for i, p in enumerate(provs)}
    borders = {
        tuple(sorted((by_index[a], by_index[b])))
        for a, b, mode, *_ in graph.edges
        if mode == transport.Mode.ROAD and by_index[a] != by_index[b]
    }
    colour = colour_countries(codes, borders)
    _, t0 = pyogrio.read_arrow(admin0, columns=["ADM0_A3", "ADMIN"], read_geometry=False)
    names = dict(zip(t0.column("ADM0_A3").to_pylist(), t0.column("ADMIN").to_pylist()))
    names.update(provinces.BORDERS["names"])
    sovereign = {p.country: p.sovereign for p in provs}
    slice_set = set(CONFIG["dev_slice"])

    out.mkdir(parents=True, exist_ok=True)
    web = out / "web"
    shutil.rmtree(web, ignore_errors=True)
    web.mkdir()
    hashes = {}
    hashes["provinces.arrow"] = write_table(
        out / "provinces.arrow",
        {
            "id": pa.array([p.id for p in provs]),
            "name": pa.array([p.name for p in provs]),
            "country": pa.array([p.country for p in provs]),
            "sovereign": pa.array([p.sovereign for p in provs]),
            "country_now": pa.array([p.country_now for p in provs]),
            "lon": pa.array([p.lon for p in provs], pa.float64()),
            "lat": pa.array([p.lat for p in provs], pa.float64()),
            "area_km2": pa.array([p.area_km2 for p in provs], pa.float64()),
            "elev_mean_m": pa.array(elev),
            "elev_std_m": pa.array(elev_std),
            "tri_m": pa.array(tri),
            "terrain": pa.array(terrain_class(elev, tri)),
            "coastal": pa.array(extra["coastal"]),
            "dev_slice": pa.array([p.country in slice_set for p in provs]),
            "cells": pa.array(np.bincount(owner, minlength=n).astype(np.uint32)),
            "ports": pa.array(per_province(transport.NodeKind.PORT)),
            "airports": pa.array(per_province(transport.NodeKind.AIRPORT)),
            "rail": pa.array(per_province(transport.NodeKind.RAIL) > 0),
            **{f"res_{g}": pa.array(v) for g, v in res.items()},
        },
    )
    hashes["countries.arrow"] = write_table(
        out / "countries.arrow",
        {
            "code": pa.array(codes),
            "name": pa.array([names.get(c, c) for c in codes]),
            "sovereign": pa.array([sovereign[c] for c in codes]),
            "sovereign_name": pa.array([names.get(sovereign[c], sovereign[c]) for c in codes]),
            "colour": pa.array([colour[c] for c in codes], pa.uint8()),
        },
    )
    hashes["cells.arrow"] = write_table(
        out / "cells.arrow",
        {
            "cell": pa.array([int(c, 16) for c in cells], pa.uint64()),
            "province": pa.array(owner),
            "area_km2": pa.array(area),
            **{k: pa.array(v) for k, v in terr.items()},
        },
    )
    hashes["nodes.arrow"] = write_table(
        out / "nodes.arrow",
        {
            "kind": pa.array(graph.kind, pa.uint8()),
            "province": pa.array(np.where(node_prov < 0, 0xFFFFFFFF, node_prov).astype(np.uint32)),
            "lon": pa.array(graph.lon, pa.float64()),
            "lat": pa.array(graph.lat, pa.float64()),
            "label": pa.array(graph.label),
            "sea_zone": pa.array(np.where(np.array(graph.zone) < 0, 0xFFFFFFFF, graph.zone).astype(np.uint32)),
        },
    )
    e = np.array(graph.edges, dtype=object)
    hashes["edges.arrow"] = write_table(
        out / "edges.arrow",
        {
            "a": pa.array(e[:, 0].astype(np.uint32)),
            "b": pa.array(e[:, 1].astype(np.uint32)),
            "mode": pa.array(e[:, 2].astype(np.uint8)),
            "length_km": pa.array(e[:, 3].astype(np.float64)),
            "tri_m": pa.array(e[:, 4].astype(np.float64)),
            "chokepoint": pa.array(e[:, 5].astype(np.uint8)),
        },
    )
    hashes["counties.arrow"] = write_table(
        out / "counties.arrow",
        {
            "name": pa.array([c.name for c in cty]),
            "country": pa.array([c.iso for c in cty]),
            "level": pa.array([c.level for c in cty]),
            "province": pa.array(county_prov),
            "area_km2": pa.array(cty_area),
            **{k: pa.array(v) for k, v in cstat.items()},
        },
    )
    hashes["sea_zones.arrow"] = write_table(out / "sea_zones.arrow", {"h3": pa.array(zones)})
    hashes["chokepoints.arrow"] = write_table(
        out / "chokepoints.arrow",
        {
            "name": pa.array([c["name"] for c in transport.CHOKEPOINTS]),
            "canal": pa.array([c.get("canal", False) for c in transport.CHOKEPOINTS]),
        },
    )

    simple = simplify([p.geom for p in provs])
    groups = {}
    for i, p in enumerate(provs):
        groups.setdefault(p.country, []).append(i)
    hashes |= write_tiles(
        web / "map.pmtiles",
        {
            "provinces": (simple, 0),
            "countries": ([shapely.union_all(simple[groups[c]]) for c in codes], 0),
            "counties": (simplify([c.geom for c in cty]), COUNTY_MINZOOM),
        },
    )
    attrs = {
        "id": [p.id for p in provs],
        "name": [p.name for p in provs],
        "country": [codes.index(p.country) for p in provs],
        "area_km2": [round(p.area_km2, 1) for p in provs],
        "elev_mean_m": [round(float(v)) for v in elev],
        "tri_m": [round(float(v), 1) for v in tri],
        "terrain": terrain_class(elev, tri).tolist(),
        "coastal": extra["coastal"].tolist(),
        "dev_slice": [p.country in slice_set for p in provs],
        "ports": per_province(transport.NodeKind.PORT).tolist(),
        "airports": per_province(transport.NodeKind.AIRPORT).tolist(),
        "rail": (per_province(transport.NodeKind.RAIL) > 0).tolist(),
        **{f"res_{g}": v.tolist() for g, v in res.items()},
        "countries": {
            "code": codes,
            "name": [names.get(c, c) for c in codes],
            "sovereign": [names.get(sovereign[c], sovereign[c]) for c in codes],
            "colour": [colour[c] for c in codes],
        },
    }
    (web / "provinces.json").write_text(json.dumps(attrs, separators=(",", ":"), ensure_ascii=False))
    hashes["web/provinces.json"] = sha256(web / "provinces.json")
    county_attrs = {
        "name": [c.name for c in cty],
        "level": [c.level for c in cty],
        "province": county_prov.tolist(),
        "area_km2": [round(float(a), 1) for a in cty_area],
        "elev_mean_m": [round(float(v)) for v in cstat["elev_mean_m"]],
        "tri_m": [round(float(v), 1) for v in cstat["tri_m"]],
    }
    (web / "counties.json").write_text(json.dumps(county_attrs, separators=(",", ":"), ensure_ascii=False))
    hashes["web/counties.json"] = sha256(web / "counties.json")

    stats = extra["stats"] | {
        "provinces": n,
        "countries": len(codes),
        "cells": len(cells),
        "nodes": len(graph.kind),
        "edges": len(graph.edges),
        "sea_zones": len(zones),
        "largest_province_km2": round(max(p.area_km2 for p in provs)),
        "split_by_counties": len({p.id.split(".")[0] for p in provs if p.split_rule == "counties"}),
        "split_by_voronoi": len({p.id.split(".")[0] for p in provs if p.split_rule == "voronoi"}),
        "counties": len(cty),
        "counties_unassigned": int((home < 0).sum()),
        "county_area_outside_province": round(1 - fits_inside / fits, 4),
    }
    manifest = {
        "pack_format": PACK_FORMAT,
        "year": CONFIG["year"],
        "config": CONFIG,
        "sources": [{k: e[k] for k in ("source", "name", "sha256")} for e in entries],
        "tables": dict(sorted(hashes.items())),
        "stats": stats,
    }
    (out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    return manifest


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, default=ROOT / "data" / "packs" / str(CONFIG["year"]))
    args = parser.parse_args()
    manifest = build(args.out)
    print(json.dumps(manifest["stats"], indent=2))
