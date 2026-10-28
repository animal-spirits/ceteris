"""Selects one econ module's columns from the Python and Rust econ tables for `ceteris pack compare`.

    python -I econ_tables.py MODULE PYTHON_DIR RUST_DIR OUT_DIR

Every Rust table must have exactly the columns of `RUST` (the tables built in Rust so far), so a
column the port leaves out or adds fails here. The MODULE's columns of each table are written from
both builds into OUT_DIR/python and OUT_DIR/rust.
"""

import sys
from pathlib import Path

import pyarrow as pa
from pyarrow import ipc

GOODS = range(10)
PEOPLE = {
    "econ_countries.arrow": [
        "code",
        "participation",
        "unemployment",
        "u_star",
        "working_age_share",
        "labour_share",
        "gini",
        "edu_low",
        "edu_secondary",
        "edu_tertiary",
        "participation_imputed",
        "unemployment_imputed",
        "u_star_imputed",
        "working_age_imputed",
        "labour_share_imputed",
        "edu_imputed",
        "gini_imputed",
    ],
    "econ_provinces.arrow": ["population", "working_age", "working_age_imputed"],
    "econ_pops.arrow": ["province", "edu", "population", "working_age", "labour_force", "imputed"],
}
ICIO = {
    "econ_countries.arrow": [
        "code",
        "donor",
        "va_basis",
        "income_source",
        "region",
        "va_imputed",
        "gdp",
        "gdp_imputed",
        "coal_share",
        *(f"fd_{g}" for g in GOODS),
        *(f"va_{g}" for g in GOODS),
    ],
    "econ_io.arrow": ["country", "input", "good", "a"],
}
MODULES = {"people": PEOPLE, "icio": ICIO}
RUST = {
    "econ_countries.arrow": ICIO["econ_countries.arrow"] + PEOPLE["econ_countries.arrow"][1:],
    "econ_io.arrow": ICIO["econ_io.arrow"],
    "econ_provinces.arrow": PEOPLE["econ_provinces.arrow"],
    "econ_pops.arrow": PEOPLE["econ_pops.arrow"],
}


def main() -> None:
    module, python, rust, out = sys.argv[1], *map(Path, sys.argv[2:])
    built = sorted(p.name for p in rust.glob("*.arrow"))
    if built != sorted(RUST):
        sys.exit(f"Rust tables {built}, expected {sorted(RUST)}")
    for name, names in RUST.items():
        got = ipc.open_file(str(rust / name)).schema.names
        if got != names:
            sys.exit(f"{name}: Rust columns {got}, expected {names}")
    for side, source in (("python", python), ("rust", rust)):
        (out / side).mkdir(parents=True)
        for name, names in MODULES[module].items():
            table = ipc.open_file(str(source / name)).read_all().select(names)
            with pa.OSFile(str(out / side / name), "wb") as f, ipc.new_file(f, table.schema) as w:
                w.write_table(table)


if __name__ == "__main__":
    main()
