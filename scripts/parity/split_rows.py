"""Separates the rows of split provinces, and of the counties clipped to them, in two compare
directories, so that only those rows take the split tolerance of `step3-provinces.sh`.

    python scripts/parity/split_rows.py A B

The masks come from A's `province_ids.arrow` (`split_rule` set) and `county_ids.arrow` (the
county's province is split). In each directory, `provinces.arrow` and `counties.arrow` lose their
`area_km2` column, which moves to `*_area.arrow` (other rows) and `*_area_split.arrow` (masked
rows); `provinces.geom.arrow` and `counties.geom.arrow` become `*_geom.arrow` and
`*_geom_split.arrow` the same way, by their `geom` column.
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
    geom = read(d / f"{name}.geom.arrow")
    in_split = split[geom["geom"].to_numpy()]
    write(d / f"{name}_geom.arrow", geom.filter(pa.array(~in_split)))
    write(d / f"{name}_geom_split.arrow", geom.filter(pa.array(in_split)))
    (d / f"{name}.geom.arrow").unlink()


def main(a: Path, b: Path) -> None:
    provinces = np.array([r != "" for r in read(a / "province_ids.arrow")["split_rule"].to_pylist()])
    counties = provinces[read(a / "county_ids.arrow")["province"].to_numpy()]
    print(f"split rows: {provinces.sum()} provinces, {counties.sum()} counties")
    for d in (a, b):
        separate(d, "provinces", provinces)
        separate(d, "counties", counties)


if __name__ == "__main__":
    main(Path(sys.argv[1]), Path(sys.argv[2]))
