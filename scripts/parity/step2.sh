#!/usr/bin/env bash
# Step 2 parity: the seven econ tables built by `python -m ceteris.packs.econ.build` and by
# `ceteris pack build --econ-only` from the same pack and cache. Checks that both write the same
# files with the same schemas (columns, types, nullability, metadata), print the same summaries
# and return the same stats, and runs `ceteris pack compare` on every table in file row order.
# Then the same with each optional source the build reads left out of the cache (GEM coal, GEM
# oil and gas, gridded GDP).
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
cargo build --release --bin ceteris
bin="$root/target/release/ceteris"
pack="${CETERIS_PACK:-$root/data/packs/2000}"
cache="$root/data/cache"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
python_src="$root/python"

python_build() {
    PYTHONPATH="$python_src" uv run --project "$root/python" --locked python -c '
import json, sys
from pathlib import Path
from ceteris.packs.econ import build
stats = build.build(Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]))["stats"]
print(json.dumps(stats, sort_keys=True, separators=(",", ":")))
' "$@"
}

# run_case NAME ROOT: ROOT is a checkout-shaped tree whose data/cache the Rust build reads.
run_case() {
    local name=$1 tree=$2 dir="$work/$1"
    mkdir -p "$dir"
    python_build "$pack" "$dir/python" "$tree/data/cache" >"$dir/python.out"
    (cd "$tree" && "$bin" pack build --econ-only --pack "$pack" --out "$dir/rust") \
        >"$dir/rust.out" 2>"$dir/rust.err"
    sed '$d' "$dir/python.out" >"$dir/python.summary"
    tail -1 "$dir/python.out" >"$dir/python.stats"
    python3 -I -c 'import json, sys; print(json.dumps(json.load(sys.stdin)["stats"], sort_keys=True, separators=(",", ":")))' \
        <"$dir/rust.out" >"$dir/rust.stats"
    diff "$dir/python.summary" "$dir/rust.err"
    diff "$dir/python.stats" "$dir/rust.stats"
    uv run --project "$root/python" --locked python -I -c '
import sys
from pathlib import Path
from pyarrow import ipc
python, rust = Path(sys.argv[1]), Path(sys.argv[2])
names = sorted(p.name for p in python.glob("*.arrow"))
if names != sorted(p.name for p in rust.glob("*.arrow")) or len(names) != 7:
    sys.exit(f"tables differ: {names}")
for n in names:
    a, b = ipc.open_file(str(python / n)).schema, ipc.open_file(str(rust / n)).schema
    if not a.equals(b, check_metadata=True):
        sys.exit(f"{n}: schema\n{a}\nvs\n{b}")
' "$dir/python" "$dir/rust"
    echo "== $name: schemas, summaries and stats equal; $(cat "$dir/rust.stats")"
    "$bin" pack compare "$dir/python" "$dir/rust" >"$dir/compare" || {
        cat "$dir/compare"
        exit 1
    }
}

run_case full "$root"
cat "$work/full/compare"

n=0
for missing in "Global Coal Mine Tracker, August 2026.xlsx" \
    "Global-Oil-and-Gas-Extraction-Tracker-March-2026.xlsx" \
    "kummu_gdp_ppp_2000_5arcmin.tif"; do
    n=$((n + 1))
    tree="$work/tree-$n"
    mkdir -p "$tree/data/cache" "$tree/python/ceteris/packs"
    ln -s "$root/python/ceteris/packs/sources.toml" "$tree/python/ceteris/packs/sources.toml"
    for f in "$cache"/*; do
        [ "$(basename "$f")" = "$missing" ] || ln -s "$f" "$tree/data/cache/"
    done
    run_case "without $missing" "$tree"
    test ! -e "$tree/data/cache/$missing"
    echo "$(grep -c ': ok max_rel=0e0$' "$work/without $missing/compare") columns ok max_rel=0e0;" \
        "$(grep -vc ': ok max_rel=0e0$' "$work/without $missing/compare" || true) other"
done
