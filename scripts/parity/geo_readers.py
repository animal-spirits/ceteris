"""Digests of the geometries pyogrio reads from the admin-1 shapefile, the geoBoundaries files
and the Natural Earth land, railways and ports of the transport graph.

    PYTHONPATH=TREE/python python -P scripts/parity/geo_readers.py

where TREE holds the Python pack package from before its deletion (`packs_tree.sh`, README.md).

Each polygon is hashed as: a type byte (0 none, 3 Polygon, 6 MultiPolygon), then u32 counts of
parts, rings and points, then the coordinates as little-endian f64; a point as the byte 1 and
its coordinates; a line as the byte 2 (LineString) or 5 (MultiLineString), then u32 counts of
parts and points and the coordinates. A geoBoundaries feature or a port is preceded by its name
(`shapeName`, `name`) as UTF-8 and a NUL. `tests/geo_readers.rs` holds the digests.

Also prints the digest of the invalid admin-1 polygons as `shapely.make_valid` repairs them (with
their ids); run it on shapely built on GEOS 3.15 (README.md) for that line.
"""

import hashlib
import struct

import pyogrio
import shapely

from ceteris.packs import counties, fetch


def feed(h, g) -> None:
    if g is None:
        h.update(b"\0")
        return
    if g.geom_type == "Point":
        h.update(b"\1" + struct.pack("<dd", g.x, g.y))
        return
    if g.geom_type in ("LineString", "MultiLineString"):
        h.update(bytes([2 if g.geom_type == "LineString" else 5]))
        parts = shapely.get_parts(g)
        h.update(struct.pack("<I", len(parts)))
        for p in parts:
            c = shapely.get_coordinates(p)
            h.update(struct.pack("<I", len(c)))
            h.update(c.astype("<f8").tobytes())
        return
    h.update(bytes([3 if g.geom_type == "Polygon" else 6]))
    parts = shapely.get_parts(g)
    h.update(struct.pack("<I", len(parts)))
    for p in parts:
        rings = [p.exterior, *p.interiors]
        h.update(struct.pack("<I", len(rings)))
        for r in rings:
            c = shapely.get_coordinates(r)
            h.update(struct.pack("<I", len(c)))
            h.update(c.astype("<f8").tobytes())


def geoms(path, columns):
    meta, t = pyogrio.read_arrow(path, columns=columns)
    return t, shapely.from_wkb(t.column(meta["geometry_name"] or "wkb_geometry").to_numpy(zero_copy_only=False))


def main() -> None:
    h = hashlib.sha256()
    t, gs = geoms(fetch.CACHE / "ne_10m_admin_1_states_provinces.zip", ["adm1_code"])
    for g in gs:
        feed(h, g)
    print(f"admin1 {len(gs)} {h.hexdigest()}")
    h = hashlib.sha256()
    invalid = [(c, g) for c, g in zip(t.column("adm1_code").to_pylist(), gs) if not g.is_valid]
    for _, g in invalid:
        feed(h, shapely.make_valid(g))
    print(f"admin1 repaired {[c for c, _ in invalid]} on GEOS {shapely.geos_version_string} {h.hexdigest()}")
    h = hashlib.sha256()
    n = 0
    for e in counties.entries():
        t, gs = geoms(fetch.CACHE / e["name"], ["shapeName"])
        for name, g in zip(t.column("shapeName").to_pylist(), gs):
            h.update((name or "").encode() + b"\0")
            feed(h, g)
            n += 1
    print(f"geoboundaries {n} {h.hexdigest()}")
    for name in ("ne_10m_land.zip", "ne_10m_railroads.zip"):
        h = hashlib.sha256()
        _, gs = geoms(fetch.CACHE / name, [])
        for g in gs:
            feed(h, g)
        print(f"{name} {len(gs)} {h.hexdigest()}")
    h = hashlib.sha256()
    t, gs = geoms(fetch.CACHE / "ne_10m_ports.zip", ["name"])
    for name, g in zip(t.column("name").to_pylist(), gs):
        h.update((name or "").encode() + b"\0")
        feed(h, g)
    print(f"ne_10m_ports.zip {len(gs)} {h.hexdigest()}")


if __name__ == "__main__":
    main()
