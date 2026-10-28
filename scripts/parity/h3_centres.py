"""Centres of every H3 cell at a resolution as h3 computes them, for `dump-geo --build --centres`.

    python scripts/parity/h3_centres.py RES OUT

One 24-byte row per cell: the index as u64, then latitude and longitude as f64, little-endian.
"""

import struct
import sys

import h3


def main(res: int, out: str) -> None:
    with open(out, "wb") as f:
        for base in h3.get_res0_cells():
            for c in h3.cell_to_children(base, res):
                lat, lng = h3.cell_to_latlng(c)
                f.write(struct.pack("<Qdd", int(c, 16), lat, lng))


if __name__ == "__main__":
    main(int(sys.argv[1]), sys.argv[2])
