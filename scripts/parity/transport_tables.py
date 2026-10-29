"""Python side of the step 3 transport parity: the graph tables of `build.build`.

    python scripts/parity/transport_tables.py OUT

Builds the provinces and their mean terrain ruggedness as `build.build` does, runs
`transport.build` on them, and writes to OUT/python the tables `build.build` writes from the
graph (nodes, edges, sea_zones, chokepoints), the provinces' coastal flags (transport.arrow) and
the build stats (transport_stats.arrow, one row).
"""

import sys
from pathlib import Path

import numpy as np
import pyarrow as pa

from ceteris.packs import build, counties, fetch, provinces, raster, transport

CONFIG = build.CONFIG


def main(out: Path) -> None:
    cache = fetch.CACHE
    admin1, admin0 = cache / "ne_10m_admin_1_states_provinces.zip", cache / "ne_10m_admin_0_countries.zip"
    etopo = cache / "ETOPO_2022_v1_60s_N90W180_surface.tif"
    units = counties.load(cache)
    provs = provinces.build(admin1, admin0, CONFIG["split_area_km2"], CONFIG["land_res"], units)
    cells, owner = raster.assign_cells(provs, CONFIG["land_res"])
    terr = raster.terrain(cells, etopo)
    area = raster.cell_area_km2(cells)
    tri = raster.province_means(owner, area, terr["tri_m"], len(provs))
    graph, zones, extra = transport.build(provs, tri, cache, CONFIG["sea_res"], CONFIG["sea_zone_res"])

    py = out / "python"
    py.mkdir(parents=True, exist_ok=True)
    node_prov = np.array(graph.province)
    build.write_table(
        py / "nodes.arrow",
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
    build.write_table(
        py / "edges.arrow",
        {
            "a": pa.array(e[:, 0].astype(np.uint32)),
            "b": pa.array(e[:, 1].astype(np.uint32)),
            "mode": pa.array(e[:, 2].astype(np.uint8)),
            "length_km": pa.array(e[:, 3].astype(np.float64)),
            "tri_m": pa.array(e[:, 4].astype(np.float64)),
            "chokepoint": pa.array(e[:, 5].astype(np.uint8)),
        },
    )
    build.write_table(py / "sea_zones.arrow", {"h3": pa.array(zones)})
    build.write_table(
        py / "chokepoints.arrow",
        {
            "name": pa.array([c["name"] for c in transport.CHOKEPOINTS]),
            "canal": pa.array([c.get("canal", False) for c in transport.CHOKEPOINTS]),
        },
    )
    build.write_table(py / "transport.arrow", {"coastal": pa.array(extra["coastal"])})
    stats = extra["stats"]
    build.write_table(py / "transport_stats.arrow", {k: pa.array([int(v)], pa.int64()) for k, v in sorted(stats.items())})
    print(stats)


if __name__ == "__main__":
    main(Path(sys.argv[1]))
