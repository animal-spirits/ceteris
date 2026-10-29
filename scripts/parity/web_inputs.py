"""Inputs of the step 3 web parity: the provinces and clipped counties of `build.build`.

    python scripts/parity/web_inputs.py OUT

Builds them as `geo_inputs.py` does and writes their vertices (OUT/provinces.geom.arrow,
OUT/counties.geom.arrow, as `geo_inputs.geom_table`) and what the vertices leave out:
OUT/provinces.web.arrow (country, and whether the geometry is a MultiPolygon) and
OUT/counties.web.arrow (the same flag). Fails unless the geometries rebuilt from these files
(`web_outputs.geometries`) equal the built ones exactly.
"""

import sys
from pathlib import Path

import numpy as np
import pyarrow as pa
from geo_inputs import CONFIG, geom_table
from web_outputs import geometries, vertices

from ceteris.packs import build, counties, fetch, geo, provinces


def main(out: Path) -> None:
    cache = fetch.CACHE
    admin1, admin0 = cache / "ne_10m_admin_1_states_provinces.zip", cache / "ne_10m_admin_0_countries.zip"
    units = counties.load(cache)
    provs = provinces.build(admin1, admin0, CONFIG["split_area_km2"], CONFIG["land_res"], units)
    home = counties.assign(units, [p.geom for p in provs], [{p.country, p.country_now} for p in provs])
    clipped = []
    for c, h in zip(units, home):
        if h >= 0:
            piece = geo.polygonal(c.geom.intersection(provs[h].geom))
            if not piece.is_empty:
                clipped.append((int(h), piece))
    clipped.sort(key=lambda hc: hc[0])
    cty = [g for _, g in clipped]

    out.mkdir(parents=True, exist_ok=True)
    for name, geoms in (("provinces", [p.geom for p in provs]), ("counties", cty)):
        geom_table(out / f"{name}.geom.arrow", geoms)
        columns = {"multi": pa.array([g.geom_type == "MultiPolygon" for g in geoms])}
        if name == "provinces":
            columns = {"country": pa.array([p.country for p in provs], pa.string()), **columns}
        build.write_table(out / f"{name}.web.arrow", columns)
        back = geometries(out, name)
        assert len(back) == len(geoms), name
        for a, b in zip(geoms, back):
            assert a.geom_type == b.geom_type, name
            assert np.array_equal(vertices(a), vertices(b)), name
    print(f"provinces {len(provs)}, counties {len(cty)}")


if __name__ == "__main__":
    main(Path(sys.argv[1]))
