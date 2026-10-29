"""Python side of the step 3 build parity: the P1 tables and attribute files of `build.build`.

    python scripts/parity/build_tables.py OUT

Runs `build.build(OUT)` with its map and econ steps replaced, so that it writes the eight P1
tables, `web/provinces.json` and `web/counties.json` as the pack build does, and nothing that
step 3 compares elsewhere: `simplify` returns empty polygons (the countries layer then unions
nothing), `write_tiles` writes no tiles, `fills.write` writes empty fill files, and `econ_build`
writes no tables (the econ tables are Rust on both sides). Prints the build stats without the
fills and econ entries, as `json.dumps(stats, indent=2)`.
"""

import json
import sys
import types
from pathlib import Path

import numpy as np
import shapely

from ceteris.packs import build


def main(out: Path) -> None:
    build.simplify = lambda geoms: np.array([shapely.Polygon()] * len(geoms), dtype=object)
    build.write_tiles = lambda path, layers: {}
    build.fills = types.SimpleNamespace(write=lambda path, geoms: path.write_bytes(b""))
    build.econ_build = lambda out: {"tables": {}, "stats": {}}
    stats = build.build(out)["stats"]
    del stats["fills"], stats["econ"]
    print(json.dumps(stats, indent=2))


if __name__ == "__main__":
    main(Path(sys.argv[1]))
