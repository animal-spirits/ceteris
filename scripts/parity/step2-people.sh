#!/usr/bin/env bash
# Step 2 people parity: the people columns of econ_countries, econ_provinces and econ_pops.
# Builds the econ tables with Python and with `ceteris pack build --econ-only`, keeps of the
# Python tables only the columns the Rust build writes (in its order), and runs
# `ceteris pack compare` on the two directories, then the people tests on the real data
# (optimised: the WorldPop tests skip in debug builds).
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
cargo build --release --bin ceteris
bin="$root/target/release/ceteris"
pack="$root/data/packs/2000"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

uv run --project python --locked python -m ceteris.packs.econ.build --pack "$pack" --out "$work/python" >/dev/null
"$bin" pack build --econ-only --pack "$pack" --out "$work/rust" >/dev/null
mkdir "$work/subset"
uv run --project python --locked python -I - "$work/python" "$work/rust" "$work/subset" <<'PY'
import sys
from pathlib import Path

import pyarrow as pa
from pyarrow import ipc

python, rust, subset = map(Path, sys.argv[1:])
for path in sorted(rust.glob("*.arrow")):
    names = ipc.open_file(str(path)).schema.names
    table = ipc.open_file(str(python / path.name)).read_all().select(names)
    with pa.OSFile(str(subset / path.name), "wb") as f, ipc.new_file(f, table.schema) as w:
        w.write_table(table)
PY
"$bin" pack compare "$work/subset" "$work/rust"
cargo test --release -p ceteris-pack --test people
