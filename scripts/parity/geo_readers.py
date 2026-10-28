"""Digests of the polygons pyogrio reads from the admin-1 shapefile and the geoBoundaries files.

    python scripts/parity/geo_readers.py

Each geometry is hashed as: a type byte (0 none, 3 Polygon, 6 MultiPolygon), then u32 counts of
parts, rings and points, then the coordinates as little-endian f64; a geoBoundaries feature is
preceded by its `shapeName` as UTF-8 and a NUL. `tests/geo_readers.rs` holds the digests.
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
    _, gs = geoms(fetch.CACHE / "ne_10m_admin_1_states_provinces.zip", [])
    for g in gs:
        feed(h, g)
    print(f"admin1 {len(gs)} {h.hexdigest()}")
    h = hashlib.sha256()
    n = 0
    for e in counties.entries():
        t, gs = geoms(fetch.CACHE / e["name"], ["shapeName"])
        for name, g in zip(t.column("shapeName").to_pylist(), gs):
            h.update((name or "").encode() + b"\0")
            feed(h, g)
            n += 1
    print(f"geoboundaries {n} {h.hexdigest()}")


if __name__ == "__main__":
    main()
