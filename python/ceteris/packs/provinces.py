"""Province set: Natural Earth admin-1 at 2000 borders, oversized regions split (Section 5.2, ADR 0006)."""

import math
import tomllib
from dataclasses import dataclass
from pathlib import Path

import h3
import numpy as np
import pyogrio
import shapely
from shapely.geometry.base import BaseGeometry

from . import geo

# A county split needs at least this many counties per part to balance areas.
MIN_COUNTIES_PER_PART = 2
# A county split whose largest part exceeds this multiple of the area limit falls back to Voronoi.
MAX_PART_FACTOR = 1.5

BORDERS = tomllib.loads(Path(__file__).with_name("borders_2000.toml").read_text())


@dataclass
class Province:
    id: str  # Natural Earth adm1_code, with ".k" for the k-th part of a split region
    name: str
    country: str
    sovereign: str
    country_now: str
    geom: BaseGeometry
    area_km2: float = 0.0
    lon: float = 0.0
    lat: float = 0.0
    split_rule: str = ""  # "counties" or "voronoi" for parts of a split region


def split_count(area_km2: float, max_area_km2: float) -> int:
    """Number of parts an oversized province is split into."""
    return max(1, math.ceil(area_km2 / max_area_km2))


def _sovereign_codes(admin0: Path) -> dict[str, str]:
    """Natural Earth sov_a3 (which uses codes like GB1, NL1) to the sovereign's adm0_a3."""
    _, t = pyogrio.read_arrow(admin0, columns=["SOV_A3", "SOVEREIGNT", "ADMIN", "ADM0_A3"], read_geometry=False)
    rows = t.to_pylist()
    by_name = {r["ADMIN"]: r["ADM0_A3"] for r in rows}
    return {r["SOV_A3"]: by_name.get(r["SOVEREIGNT"], r["SOV_A3"]) for r in rows}


def load(admin1: Path, admin0: Path) -> list[Province]:
    sov_codes = _sovereign_codes(admin0)
    meta, t = pyogrio.read_arrow(admin1, columns=["adm1_code", "name", "adm0_a3", "sov_a3"])
    geoms = shapely.from_wkb(t.column(meta["geometry_name"] or "wkb_geometry").to_numpy(zero_copy_only=False))
    out = []
    for row, g in zip(t.select(["adm1_code", "name", "adm0_a3", "sov_a3"]).to_pylist(), geoms):
        now = row["adm0_a3"]
        if now in BORDERS["exclude"]["countries"]:
            continue
        country = BORDERS["province"].get(row["adm1_code"], BORDERS["country"].get(now, now))
        # A remapped province is under its 2000 country's sovereignty.
        default = country if country != now else sov_codes.get(row["sov_a3"], row["sov_a3"])
        sovereign = BORDERS["sovereign"].get(country, default)
        g = shapely.make_valid(g) if not g.is_valid else g
        out.append(Province(row["adm1_code"], row["name"] or row["adm1_code"], country, sovereign, now, g))
    out.sort(key=lambda p: (p.country, p.id))
    return out


def _bisect(points: np.ndarray, order: np.ndarray, k: int, weights: np.ndarray | None = None) -> list[np.ndarray]:
    """Recursive coordinate bisection into k groups of near-equal size (or weight); deterministic."""
    if k == 1:
        return [order]
    pts = points[order]
    axis = int(np.ptp(pts[:, 1]) > np.ptp(pts[:, 0]))
    ranked = order[np.lexsort((order, pts[:, axis]))]
    k1 = k // 2
    if weights is None:
        cut = round(len(ranked) * k1 / k)
    else:
        cum = np.cumsum(weights[ranked])
        cut = int(np.searchsorted(cum, cum[-1] * k1 / k)) + 1
    cut = min(max(cut, k1), len(ranked) - (k - k1))
    return _bisect(points, ranked[:cut], k1, weights) + _bisect(points, ranked[cut:], k - k1, weights)


def split_by_counties(p: Province, k: int, members: list[BaseGeometry], max_area_km2: float, res: int) -> list[Province] | None:
    """Split along county borders: bisect the counties into k groups of equal area, then give each
    gap between the sources' borders to the part it shares most border with. Counties too large to
    balance are first cut by Voronoi, inside themselves. None if counties cannot carry the split."""
    clipped = [x for x in (geo.polygonal(m.intersection(p.geom)) for m in members) if not x.is_empty]
    if len(clipped) < MIN_COUNTIES_PER_PART * k:
        return None
    pieces = []
    for x in clipped:
        n = split_count(geo.area_km2(x), max_area_km2 / 2)
        pieces.extend([q.geom for q in split(Province("", "", "", "", "", x), n, res)] if n > 1 else [x])
    ll = np.array([geo.anchor(x) for x in pieces])
    if geo.crosses_antimeridian(p.geom):
        ll[:, 0] = np.where(ll[:, 0] < 0, ll[:, 0] + 360, ll[:, 0])
    xy = ll * [geo.cos_lat(float(np.mean(ll[:, 1]))), 1.0]
    weight = np.array([geo.area_km2(x) for x in pieces])
    groups = _bisect(xy, np.arange(len(pieces)), k, weight)
    pieces_arr = np.array(pieces, dtype=object)
    parts = [[shapely.union_all(pieces_arr[g])] for g in groups]
    covered = shapely.union_all([q[0] for q in parts])
    for gap in shapely.get_parts(geo.polygonal(p.geom.difference(covered))):
        shared = [shapely.length(shapely.intersection(gap.boundary, q[0].boundary)) for q in parts]
        j = int(np.argmax(shared)) if max(shared) > 0 else int(np.argmin([gap.distance(q[0]) for q in parts]))
        parts[j].append(gap)
    out = [
        Province(f"{p.id}.{i}", f"{p.name} ({i})", p.country, p.sovereign, p.country_now, geo.polygonal(shapely.union_all(q)))
        for i, q in enumerate(parts, start=1)
    ]
    return out if max(geo.area_km2(q.geom) for q in out) <= MAX_PART_FACTOR * max_area_km2 else None


def split(p: Province, k: int, res: int) -> list[Province]:
    """Voronoi split for regions without county data (ADR 0006)."""
    wrapped = geo.crosses_antimeridian(p.geom)
    g = geo.unwrap(p.geom) if wrapped else p.geom
    cells = sorted(cells_of(p.geom, res))
    if len(cells) < k:
        return [p]
    ll = np.array([h3.cell_to_latlng(c)[::-1] for c in cells])
    if wrapped:
        ll[:, 0] = np.where(ll[:, 0] < 0, ll[:, 0] + 360, ll[:, 0])
    # Bisect and draw Voronoi boundaries in a locally equal-distance plane.
    scale = geo.cos_lat(float(np.mean(ll[:, 1])))
    stretch = np.array([scale, 1.0])
    xy = ll * stretch
    groups = _bisect(xy, np.arange(len(cells)), k)
    seeds = shapely.multipoints([xy[gr].mean(axis=0) for gr in groups])
    plane = shapely.transform(g, lambda c: c * stretch)
    regions = shapely.get_parts(shapely.voronoi_polygons(seeds, extend_to=plane.envelope.buffer(1.0), ordered=True))
    parts = []
    for i, region in enumerate(regions, start=1):
        piece = geo.polygonal(shapely.transform(plane.intersection(region), lambda c: c / stretch))
        piece = geo.wrap(piece) if wrapped else piece
        if piece.is_empty:
            continue
        parts.append(Province(f"{p.id}.{i}", f"{p.name} ({i})", p.country, p.sovereign, p.country_now, piece))
    return parts


def cells_of(geom: BaseGeometry, res: int) -> set[str]:
    """H3 cells whose centres fall inside the geometry, one polygon part at a time (parts never cross 180)."""
    cells: set[str] = set()
    for part in shapely.get_parts(geom):
        cells.update(h3.geo_to_cells(part, res))
    return cells


def build(admin1: Path, admin0: Path, max_area_km2: float, res: int, counties: list | None = None) -> list[Province]:
    """Provinces at 2000 borders; oversized ones split along county borders where `counties` cover them."""
    from .counties import assign

    base = load(admin1, admin0)
    for p in base:
        p.area_km2 = geo.area_km2(p.geom)
    owner = assign(counties, [p.geom for p in base], [{p.country, p.country_now} for p in base]) if counties else None
    out = []
    for i, p in enumerate(base):
        k = split_count(p.area_km2, max_area_km2)
        if k == 1:
            out.append(p)
            continue
        members = [c.geom for c, o in zip(counties, owner) if o == i] if counties else []
        parts = split_by_counties(p, k, members, max_area_km2, res)
        rule = "counties" if parts else "voronoi"
        for q in parts or split(p, k, res):
            q.split_rule = rule
            out.append(q)
    for p in out:
        p.area_km2 = geo.area_km2(p.geom)
        p.lon, p.lat = geo.anchor(p.geom)
    return out
