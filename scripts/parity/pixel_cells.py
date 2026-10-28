"""Pixel to H3 cell to province mapping of fixed raster windows, as `people.province_population` computes it.

    python scripts/parity/pixel_cells.py OUT

Writes the fixture of the Rust test `pixel_cells_match_python_on_border_windows`. Each window is a
header line `window NAME BASE ROW0 COL0 ROWS COLS A C E F` (the transform terms as `repr`), then one
line `CELL OWNER LAT LNG` per pixel in row-major order: the pixel centre's cell at the base
resolution, the province `CellOwner.of_point` gives it (-1 for none) and the centre (`repr`).
"""

import sys
import zipfile
from pathlib import Path

import h3.api.basic_int as h3
import numpy as np
import rasterio
from pyarrow import ipc

from ceteris.packs import fetch
from ceteris.packs.econ import downscale, people

CACHE = fetch.CACHE
PACK = Path(__file__).resolve().parents[2] / "data" / "packs" / "2000"
WORLDPOP = CACHE / "ppp_2000_1km_Aggregated.tif"
KUMMU = CACHE / downscale.KUMMU
SPAM = CACHE / downscale.SPAM
SIDE = 32
# (name, row0, col0): both sides of the antimeridian through Fiji and Chukotka, the first and last
# rows (pole-ward), the 0.083333 MapSPAM grid's east edge, and split cells (Vatican, Malta).
WINDOWS = (
    ("worldpop", 12040, 0),
    ("worldpop", 12040, 43200 - SIDE),
    ("worldpop", 0, 17624),
    ("worldpop", 18720 - SIDE, 21584),
    ("worldpop", 5037, 23079),
    ("worldpop", 5758, 23323),
    ("mapspam", 1264, 0),
    ("mapspam", 1264, 4320 - SIDE),
    ("mapspam", 280, 4320 - SIDE),
    ("mapspam", 2160 - SIDE, 4320 - SIDE),
    ("kummu", 0, 4320 - SIDE),
    ("kummu", 1264, 0),
)


def transform(name):
    if name == "mapspam":
        member = max(n for n in zipfile.ZipFile(SPAM).namelist() if n.endswith("_A.tif"))
        path = f"zip://{SPAM}!{member}"
    else:
        path = WORLDPOP if name == "worldpop" else KUMMU
    with rasterio.open(path) as ds:
        return ds.transform[:6]


def main() -> None:
    owner = people.CellOwner(ipc.open_file(PACK / "cells.arrow").read_all())
    lines = []
    for name, row0, col0 in WINDOWS:
        a, _, x0, _, e, y0 = transform(name)
        lines.append(f"window {name} {owner.base} {row0} {col0} {SIDE} {SIDE} {a!r} {x0!r} {e!r} {y0!r}")
        rows, cols = np.divmod(np.arange(SIDE * SIDE), SIDE)
        # The expressions of `province_population`, block offset folded into the row.
        lat = (y0 + (rows + row0 + 0.5) * e).tolist()
        lng = (x0 + (cols + col0 + 0.5) * a).tolist()
        for la, lo in zip(lat, lng):
            lines.append(f"{h3.latlng_to_cell(la, lo, owner.base)} {owner.of_point(la, lo)} {la!r} {lo!r}")
    Path(sys.argv[1]).write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
