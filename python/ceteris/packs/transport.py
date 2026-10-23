"""Multimodal transport graph: nodes and undirected edges with physical attributes (Section 5.3, ADR 0007).

Costs are not computed here: the Rust core prices edges from mode parameters (register P04), so
calibration never needs a pack rebuild.
"""

import csv
import tomllib
from dataclasses import dataclass, field
from enum import IntEnum
from pathlib import Path

import h3
import numpy as np
import pyogrio
import shapely
from shapely import STRtree, ops

from . import geo
from .provinces import Province

CHOKEPOINTS = tomllib.loads(Path(__file__).with_name("chokepoints.toml").read_text())["chokepoint"]

# Ports farther than this from the sea grid (inland lakes, rivers) are dropped until waterways exist.
MAX_PORT_SNAP_KM = 150.0
# Shared borders shorter than this are corner contacts, not adjacency.
MIN_BORDER_KM = 1.0
AIR_NEAREST = 12
AIR_PER_CONTINENT = 3
# Sea components smaller than this without a port are fjord and lagoon fragments.
MIN_SEA_COMPONENT = 20


class NodeKind(IntEnum):
    PROVINCE = 0
    RAIL = 1
    PORT = 2
    AIRPORT = 3
    SEA = 4


class Mode(IntEnum):
    ROAD = 0
    RAIL = 1
    SEA = 2
    AIR = 3
    RAIL_ACCESS = 4
    PORT_ACCESS = 5
    MINOR_PORT_ACCESS = 6
    AIR_ACCESS = 7
    CANAL = 8


@dataclass
class Graph:
    kind: list[int] = field(default_factory=list)
    province: list[int] = field(default_factory=list)
    lon: list[float] = field(default_factory=list)
    lat: list[float] = field(default_factory=list)
    label: list[str] = field(default_factory=list)
    zone: list[int] = field(default_factory=list)
    edges: list[tuple[int, int, int, float, float, int]] = field(default_factory=list)

    def node(self, kind: NodeKind, province: int, lon: float, lat: float, label: str, zone: int = -1) -> int:
        self.kind.append(kind)
        self.province.append(province)
        self.lon.append(lon)
        self.lat.append(lat)
        self.label.append(label)
        self.zone.append(zone)
        return len(self.kind) - 1

    def edge(self, a: int, b: int, mode: Mode, length_km: float, tri_m: float = 0.0, chokepoint: int = 0) -> None:
        a, b = min(a, b), max(a, b)
        self.edges.append((a, b, int(mode), float(length_km), float(tri_m), chokepoint))


def _read_geoms(path: Path, columns: list[str] | None = None):
    meta, t = pyogrio.read_arrow(path, columns=columns)
    geoms = shapely.from_wkb(t.column(meta["geometry_name"] or "wkb_geometry").to_numpy(zero_copy_only=False))
    return t, geoms


def _nearest(lon: np.ndarray, lat: np.ndarray, to_lon: np.ndarray, to_lat: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    """Index of and distance to the nearest target for each point, by brute-force haversine in chunks."""
    idx = np.empty(len(lon), dtype=np.int64)
    dist = np.empty(len(lon))
    for s in range(0, len(lon), 256):
        d = geo.haversine_km(lon[s : s + 256, None], lat[s : s + 256, None], to_lon[None, :], to_lat[None, :])
        idx[s : s + 256] = d.argmin(axis=1)
        dist[s : s + 256] = d[np.arange(len(d)), idx[s : s + 256]]
    return idx, dist


def _locate(points: np.ndarray, provinces: list[Province], tree: STRtree, max_deg: float = 0.25) -> np.ndarray:
    """Province index containing each point, else the nearest within max_deg, else -1."""
    found = np.full(len(points), -1, dtype=np.int64)
    pi, gi = tree.query(points, predicate="within")
    for p, g in sorted(zip(pi, gi)):
        if found[p] < 0:
            found[p] = g
    missing = np.flatnonzero(found < 0)
    if len(missing):
        pi, gi = tree.query_nearest(points[missing], max_distance=max_deg, all_matches=False)
        found[missing[pi]] = gi
    return found


def adjacency(provinces: list[Province], tree: STRtree, rail: STRtree) -> list[tuple[int, int, bool]]:
    """Pairs of provinces sharing a land border, and whether a railway crosses it."""
    geoms = [p.geom for p in provinces]
    a_idx, b_idx = tree.query(geoms, predicate="dwithin", distance=0.005)
    out = []
    for a, b in sorted({(int(a), int(b)) for a, b in zip(a_idx, b_idx) if a < b}):
        border = geoms[a].boundary.intersection(geoms[b].buffer(0.005))
        if border.is_empty:
            continue
        lat = (border.bounds[1] + border.bounds[3]) / 2
        km_per_deg = 111.2 * max(geo.cos_lat(lat), 0.5)
        # Enclaves smaller than the threshold (Vatican) still border their host.
        smaller = min(geoms[a].length, geoms[b].length) * km_per_deg
        if border.length * km_per_deg < min(MIN_BORDER_KM, smaller / 4):
            continue
        crossed = len(rail.query(border.buffer(0.01), predicate="intersects")) > 0
        out.append((a, b, crossed))
    return out


def sea_grid(land: shapely.Geometry, res: int) -> tuple[list[str], np.ndarray, np.ndarray]:
    cells = sorted(c for r0 in h3.get_res0_cells() for c in h3.cell_to_children(r0, res))
    ll = np.array([h3.cell_to_latlng(c) for c in cells])
    shapely.prepare(land)
    wet = ~shapely.contains_xy(land, ll[:, 1], ll[:, 0])
    return [c for c, w in zip(cells, wet) if w], ll[wet, 1], ll[wet, 0]


def _components(n: int, pairs: list[tuple[int, int]]) -> np.ndarray:
    parent = np.arange(n)

    def find(x: int) -> int:
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x

    for a, b in pairs:
        ra, rb = find(a), find(b)
        if ra != rb:
            parent[max(ra, rb)] = min(ra, rb)
    return np.array([find(i) for i in range(n)])


def build(
    provinces: list[Province], tri_m: np.ndarray, cache: Path, sea_res: int, zone_res: int
) -> tuple[Graph, list[str], dict]:
    g = Graph()
    stats: dict = {}
    tree = STRtree([p.geom for p in provinces])
    for i, p in enumerate(provinces):
        g.node(NodeKind.PROVINCE, i, p.lon, p.lat, p.id)

    # Land borders: road everywhere, rail where a line crosses the border.
    _, rail_geoms = _read_geoms(cache / "ne_10m_railroads.zip", ["category"])
    pairs = adjacency(provinces, tree, STRtree(rail_geoms))
    plon, plat = np.array([p.lon for p in provinces]), np.array([p.lat for p in provinces])
    rail_node: dict[int, int] = {}
    for a, b, crossed in pairs:
        length = float(geo.haversine_km(plon[a], plat[a], plon[b], plat[b]))
        g.edge(a, b, Mode.ROAD, length, (tri_m[a] + tri_m[b]) / 2)
        if crossed:
            for p in (a, b):
                if p not in rail_node:
                    rail_node[p] = -1
    for p in sorted(rail_node):
        rail_node[p] = g.node(NodeKind.RAIL, p, plon[p], plat[p], f"rail:{provinces[p].id}")
        g.edge(p, rail_node[p], Mode.RAIL_ACCESS, 0.0)
    for a, b, crossed in pairs:
        if crossed:
            length = float(geo.haversine_km(plon[a], plat[a], plon[b], plat[b]))
            g.edge(rail_node[a], rail_node[b], Mode.RAIL, length, (tri_m[a] + tri_m[b]) / 2)
    stats["land_borders"], stats["rail_borders"] = len(pairs), sum(c for *_, c in pairs)

    # Sea grid, chokepoint links, and components.
    _, land_geoms = _read_geoms(cache / "ne_10m_land.zip")
    land = shapely.union_all(land_geoms)
    sea_cells, slon, slat = sea_grid(land, sea_res)
    index = {c: i for i, c in enumerate(sea_cells)}
    sea_pairs = sorted(
        {(i, index[n]) for i, c in enumerate(sea_cells) for n in h3.grid_disk(c, 1) if n in index and index[n] > i}
    )
    tag = np.zeros(len(sea_cells), dtype=np.int64)
    links = []
    for k, cp in enumerate(CHOKEPOINTS, start=1):
        d = geo.haversine_km(slon, slat, cp["lon"], cp["lat"])
        tag[(d <= cp["radius_km"]) & (tag == 0)] = k
        if "link" in cp:
            (x1, y1), *_, (x2, y2) = cp["link"]
            ends, _ = _nearest(np.array([x1, x2]), np.array([y1, y2]), slon, slat)
            path = np.array(cp["link"])
            length = float(np.sum(geo.haversine_km(path[:-1, 0], path[:-1, 1], path[1:, 0], path[1:, 1])))
            links.append((int(ends[0]), int(ends[1]), k, length, cp.get("canal", False)))
    comp = _components(len(sea_cells), sea_pairs + [(a, b) for a, b, *_ in links])
    sizes = np.bincount(comp, minlength=len(sea_cells))

    # Ports: listed (Natural Earth) and minor ones for coastal provinces without a listed port.
    t, port_geoms = _read_geoms(cache / "ne_10m_ports.zip", ["name"])
    port_names = t.column("name").to_pylist()
    port_pts = np.array([(p.x, p.y) for p in port_geoms])
    order = np.lexsort((port_pts[:, 1], port_pts[:, 0]))
    port_pts, port_names = port_pts[order], [port_names[i] for i in order]
    owner = _locate(shapely.points(port_pts), provinces, tree)
    big = sizes[comp] >= MIN_SEA_COMPONENT
    snap, dist = _nearest(port_pts[:, 0], port_pts[:, 1], slon[big], slat[big])
    snap = np.flatnonzero(big)[snap]
    ports = [
        (int(owner[i]), port_pts[i, 0], port_pts[i, 1], port_names[i] or f"port {i}", int(snap[i]), True)
        for i in range(len(port_pts))
        if owner[i] >= 0 and dist[i] <= MAX_PORT_SNAP_KM
    ]
    stats["ports_dropped"] = len(port_pts) - len(ports)
    coast = STRtree(shapely.get_parts(shapely.boundary(land_geoms)))
    has_port = {p[0] for p in ports}
    coastal = np.zeros(len(provinces), dtype=bool)
    near = coast.query([p.geom for p in provinces], predicate="dwithin", distance=0.01)[0]
    coastal[near] = True
    for i in np.flatnonzero(coastal):
        if i in has_port:
            continue
        j, d = _nearest(plon[i : i + 1], plat[i : i + 1], slon[big], slat[big])
        target = shapely.Point(slon[big][j[0]], slat[big][j[0]])
        at = ops.nearest_points(provinces[i].geom, target)[0]
        j, d = _nearest(np.array([at.x]), np.array([at.y]), slon[big], slat[big])
        if d[0] <= MAX_PORT_SNAP_KM:
            ports.append((int(i), at.x, at.y, f"minor port of {provinces[i].name}", int(np.flatnonzero(big)[j[0]]), False))
    used = {comp[p[4]] for p in ports} | {comp[np.argmax(sizes)]}
    keep = np.array([comp[i] in used for i in range(len(sea_cells))])

    # Sea nodes, zones and edges.
    zones = sorted({h3.cell_to_parent(c, zone_res) for c, k in zip(sea_cells, keep) if k})
    zone_index = {z: i for i, z in enumerate(zones)}
    sea_node = np.full(len(sea_cells), -1, dtype=np.int64)
    for i in np.flatnonzero(keep):
        zone = zone_index[h3.cell_to_parent(sea_cells[i], zone_res)]
        sea_node[i] = g.node(NodeKind.SEA, -1, slon[i], slat[i], sea_cells[i], zone)
    for a, b in sea_pairs:
        if keep[a] and keep[b]:
            length = float(geo.haversine_km(slon[a], slat[a], slon[b], slat[b]))
            g.edge(sea_node[a], sea_node[b], Mode.SEA, length, chokepoint=int(tag[a]) if tag[a] == tag[b] else 0)
    for a, b, k, length, canal in links:
        if keep[a] and keep[b]:
            g.edge(sea_node[a], sea_node[b], Mode.CANAL if canal else Mode.SEA, length, chokepoint=k)

    for p, x, y, name, cell, listed in sorted(ports, key=lambda q: (q[0], not q[5], q[1], q[2])):
        n = g.node(NodeKind.PORT, p, x, y, name)
        access = Mode.PORT_ACCESS if listed else Mode.MINOR_PORT_ACCESS
        g.edge(p, n, access, float(geo.haversine_km(plon[p], plat[p], x, y)))
        g.edge(n, sea_node[cell], Mode.SEA, float(geo.haversine_km(x, y, slon[cell], slat[cell])))
    stats["ports"], stats["minor_ports"] = sum(p[5] for p in ports), sum(not p[5] for p in ports)

    # Airports: large airports with scheduled service, linked to near neighbours and across continents.
    with (cache / "airports.csv").open(encoding="utf-8") as f:
        rows = [r for r in csv.DictReader(f) if r["type"] == "large_airport" and r["scheduled_service"] == "yes"]
    rows.sort(key=lambda r: r["ident"])
    alon = np.array([float(r["longitude_deg"]) for r in rows])
    alat = np.array([float(r["latitude_deg"]) for r in rows])
    aowner = _locate(shapely.points(alon, alat), provinces, tree)
    air = [i for i in range(len(rows)) if aowner[i] >= 0]
    air_node = {}
    for i in air:
        p = int(aowner[i])
        air_node[i] = g.node(NodeKind.AIRPORT, p, alon[i], alat[i], rows[i]["ident"])
        g.edge(p, air_node[i], Mode.AIR_ACCESS, float(geo.haversine_km(plon[p], plat[p], alon[i], alat[i])))
    air = np.array(air)
    cont = np.array([rows[i]["continent"] for i in air])
    d = geo.haversine_km(alon[air, None], alat[air, None], alon[None, air], alat[None, air])
    np.fill_diagonal(d, np.inf)
    links_air = set()
    for a in range(len(air)):
        links_air.update((min(a, b), max(a, b)) for b in np.argsort(d[a], kind="stable")[:AIR_NEAREST])
        for c in sorted(set(cont) - {cont[a]}):
            other = np.flatnonzero(cont == c)
            links_air.update((min(a, b), max(a, b)) for b in other[np.argsort(d[a, other], kind="stable")[:AIR_PER_CONTINENT]])
    for a, b in sorted(links_air):
        g.edge(air_node[air[a]], air_node[air[b]], Mode.AIR, float(d[a, b]))
    stats["airports"], stats["air_links"] = len(air), len(links_air)

    g.edges.sort()
    stats["coastal"] = int(coastal.sum())
    return g, zones, {"stats": stats, "coastal": coastal}
