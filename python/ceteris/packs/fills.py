"""GPU-ready fill buffers for the map (ADR 0010): triangulated polygons in Web Mercator.

The map uploads a file once and recolours features through a texture indexed by feature, so
an update never re-tessellates or rewrites vertex data. Layout, all little-endian:

    magic   b"CFIL"
    u32     format version (1)
    u32     vertices V
    u32     indices I (a multiple of 3)
    u32     features F
    f32     x, y for each vertex (Web Mercator, 0 to 1, y down), 2V values
    u32     feature index of each vertex, V values
    u32     triangle indices, I values
"""

import struct
from pathlib import Path

import mapbox_earcut as earcut
import numpy as np
import shapely

MAGIC = b"CFIL"
VERSION = 1
MAX_LAT = 85.0511287798066
# On the globe a triangle is drawn as a flat chord; edges longer than this (in Mercator units,
# about 1.4 degrees at the equator) are split so fills stay on the surface.
MAX_EDGE = 0.004


def mercator(lonlat: np.ndarray) -> np.ndarray:
    lon, lat = lonlat[:, 0], np.clip(lonlat[:, 1], -MAX_LAT, MAX_LAT)
    x = (lon + 180.0) / 360.0
    y = 0.5 - np.log(np.tan(np.pi / 4 + np.radians(lat) / 2)) / (2 * np.pi)
    return np.column_stack([x, y])


def triangulate(poly: shapely.Polygon) -> tuple[np.ndarray, np.ndarray]:
    """Vertices (projected) and triangle indices of one polygon with holes."""
    rings = [np.asarray(poly.exterior.coords)[:-1]] + [np.asarray(r.coords)[:-1] for r in poly.interiors]
    rings = [r for r in rings if len(r) >= 3]
    if not rings:
        return np.empty((0, 2)), np.empty(0, dtype=np.uint32)
    xy = mercator(np.vstack(rings))
    ends = np.cumsum([len(r) for r in rings]).astype(np.uint32)
    return xy, earcut.triangulate_float64(xy, ends).astype(np.uint32)


def subdivide(xy: np.ndarray, tri: np.ndarray, max_edge: float = MAX_EDGE) -> tuple[np.ndarray, np.ndarray]:
    """Split long edges at their midpoints until none exceeds max_edge.

    Whether an edge splits depends only on the edge, so the two triangles sharing it split it
    the same way and no cracks open between them.
    """
    corners = xy[tri.reshape(-1, 3)]
    if (np.linalg.norm(corners - np.roll(corners, 1, axis=1), axis=2) <= max_edge).all():
        return xy, tri
    pts = [tuple(p) for p in xy]
    mid: dict[tuple[int, int], int] = {}

    def midpoint(a: int, b: int) -> int:
        key = (min(a, b), max(a, b))
        if key not in mid:
            (ax, ay), (bx, by) = pts[a], pts[b]
            pts.append(((ax + bx) / 2, (ay + by) / 2))
            mid[key] = len(pts) - 1
        return mid[key]

    def long(a: int, b: int) -> bool:
        (ax, ay), (bx, by) = pts[a], pts[b]
        return (ax - bx) ** 2 + (ay - by) ** 2 > max_edge**2

    out = []
    stack = [tuple(t) for t in tri.reshape(-1, 3).tolist()]
    while stack:
        a, b, c = stack.pop()
        la, lb, lc = long(a, b), long(b, c), long(c, a)
        if not (la or lb or lc):
            out.append((a, b, c))
        elif la and lb and lc:
            ab, bc, ca = midpoint(a, b), midpoint(b, c), midpoint(c, a)
            stack += [(a, ab, ca), (ab, b, bc), (ca, bc, c), (ab, bc, ca)]
        else:
            # Rotate so (a, b) is long, then split it; the other edges are handled on recursion.
            while not long(a, b):
                a, b, c = b, c, a
            m = midpoint(a, b)
            stack += [(a, m, c), (m, b, c)]
    return np.array(pts), np.array(out, dtype=np.uint32).ravel()


def write(path: Path, geoms) -> dict:
    xy, feature, index = [], [], []
    base = 0
    for f, g in enumerate(geoms):
        for poly in shapely.get_parts(g):
            v, t = triangulate(poly)
            if len(t) == 0:
                continue
            v, t = subdivide(v, t)
            xy.append(v)
            feature.append(np.full(len(v), f, dtype=np.uint32))
            index.append(t + base)
            base += len(v)
    xy = np.vstack(xy).astype("<f4")
    feature = np.concatenate(feature).astype("<u4")
    index = np.concatenate(index).astype("<u4")
    header = MAGIC + struct.pack("<4I", VERSION, len(xy), len(index), len(geoms))
    path.write_bytes(header + xy.tobytes() + feature.tobytes() + index.tobytes())
    return {"vertices": len(xy), "triangles": len(index) // 3}


def read(path: Path) -> tuple[np.ndarray, np.ndarray, np.ndarray, int]:
    data = path.read_bytes()
    assert data[:4] == MAGIC
    version, v, i, f = struct.unpack_from("<4I", data, 4)
    assert version == VERSION
    off = 20
    xy = np.frombuffer(data, "<f4", 2 * v, off).reshape(v, 2)
    feature = np.frombuffer(data, "<u4", v, off + 8 * v)
    index = np.frombuffer(data, "<u4", i, off + 12 * v)
    return xy, feature, index, f
