"""Separates the areas of split provinces, and of the counties clipped to them, in the build
tables of two compare directories, so that only those rows take the split tolerance of
`step3-build.sh`.

    python scripts/parity/build_split_rows.py A B

A split province's id carries a `.k` suffix (A's `provinces.arrow`); a county is masked when its
province is. In each directory, `provinces.arrow` and `counties.arrow` lose their `area_km2`
column, which moves to `*_area.arrow` (other rows) and `*_area_split.arrow` (masked rows).
"""

import sys
from pathlib import Path

import numpy as np
import pyarrow as pa
from pyarrow import ipc


def read(path: Path) -> pa.Table:
    return ipc.open_file(path).read_all()


def write(path: Path, table: pa.Table) -> None:
    with pa.OSFile(str(path), "wb") as f, ipc.new_file(f, table.schema) as w:
        w.write_table(table)


def separate(d: Path, name: str, split: np.ndarray) -> None:
    rows = read(d / f"{name}.arrow")
    area = rows.select(["area_km2"])
    write(d / f"{name}.arrow", rows.drop_columns(["area_km2"]))
    write(d / f"{name}_area.arrow", area.filter(pa.array(~split)))
    write(d / f"{name}_area_split.arrow", area.filter(pa.array(split)))


def main(a: Path, b: Path) -> None:
    provinces = np.array(["." in i for i in read(a / "provinces.arrow")["id"].to_pylist()])
    counties = provinces[read(a / "counties.arrow")["province"].to_numpy()]
    print(f"split rows: {provinces.sum()} provinces, {counties.sum()} counties")
    for d in (a, b):
        separate(d, "provinces", provinces)
        separate(d, "counties", counties)


if __name__ == "__main__":
    main(Path(sys.argv[1]), Path(sys.argv[2]))
