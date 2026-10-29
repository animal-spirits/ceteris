#!/usr/bin/env bash
# Step 3 transport parity: transport.py against geo/transport.rs, both on GEOS 3.15 (see
# README.md for the shapely build). `transport_tables.py` builds the provinces, their mean
# ruggedness and the graph in Python and writes the tables `build.build` writes from it;
# `ceteris pack dump-geo DIR --build --transport` does the same in Rust; `pack compare` checks
# each table in file row order, never sorted.
#
# Two Rust builds:
# - with h3o's cell centres: the sea grid's node positions and the lengths that reach them, and
#   the anchors of Voronoi-split provinces, differ from h3's in the last bits, so nodes.lon,
#   nodes.lat and edges.length_km are listed, not failed; every other column must match;
# - with h3's centres (`h3_centres.py` at the land and the sea resolutions): every column must
#   match. Pending a ruling, edges.length_km does not: province anchors (geo's centroid against
#   GEOS's) differ in the last bits, which short access and border edges amplify.
#
# GEOS_PYTHON names a Python whose shapely is built on GEOS 3.15 and has the packages of
# python/pyproject.toml. Outputs go to WORK (default: a new temporary directory), kept.
set -euo pipefail

listed_columns="nodes.lon nodes.lat edges.length_km"
tables="nodes edges sea_zones chokepoints transport transport_stats"

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
: "${GEOS_PYTHON:?set GEOS_PYTHON to a Python with shapely on GEOS 3.15}"
"$GEOS_PYTHON" -c 'import shapely, sys; v = shapely.geos_version_string; print(f"shapely on GEOS {v}"); sys.exit(not v.startswith("3.15."))'
echo "Rust on GEOS $(geos-config --version)"
cargo build --release --bin ceteris --features ceteris-cli/geos
bin="$root/target/release/ceteris"
pack="${CETERIS_PACK:-$root/data/packs/2000}"
work="${WORK:-$(mktemp -d)}"
echo "outputs in $work"

# `pack compare` of every table into OUT; fails when it prints nothing, or exits non-zero with
# every line ok.
compare() {
    local out="$1" a="$2" b="$3" code t
    : >"$out"
    for t in $tables; do
        code=0
        "$bin" pack compare "$a" "$b" --table "$t" >>"$out" || code=$?
        if ! grep -q "^$t[.:]" "$out"; then
            echo "FAIL pack compare --table $t printed nothing (exit $code)"
            return 1
        fi
        if ((code != 0)) && ! grep "^$t[.:]" "$out" | grep -qv ': ok '; then
            echo "FAIL pack compare --table $t exited $code with every line ok"
            return 1
        fi
    done
}
listed() {
    [[ " ${2//$'\n'/ } " == *" $1 "* ]]
}

(cd python && PYTHONPATH="$root/python" "$GEOS_PYTHON" -P -s "$root/scripts/parity/transport_tables.py" "$work/py")
for t in nodes edges sea_zones chokepoints; do
    if "$bin" pack compare "$work/py/python" "$pack" --table "$t" >/dev/null; then
        echo "ok   the Python build reproduces the pack's $t.arrow"
    else
        echo "note the Python build on GEOS 3.15 differs from the pack's $t.arrow"
    fi
done
land_res="$(sed -n 's/^land_res = //p' python/ceteris/packs/pack.toml)"
sea_res="$(sed -n 's/^sea_res = //p' python/ceteris/packs/pack.toml)"
"$GEOS_PYTHON" -I "$root/scripts/parity/h3_centres.py" "$land_res" "$work/land.bin"
"$GEOS_PYTHON" -I "$root/scripts/parity/h3_centres.py" "$sea_res" "$work/sea.bin"
cat "$work/land.bin" "$work/sea.bin" >"$work/centres.bin"
"$bin" pack dump-geo "$work/rs" --build --transport
"$bin" pack dump-geo "$work/rs-h3" --build --transport --centres "$work/centres.bin"
status=0

echo "== Rust with h3o centres"
compare "$work/compare" "$work/py/python" "$work/rs/rust" || status=1
cat "$work/compare"
while read -r line; do
    column="${line%%:*}"
    if listed "$column" "$listed_columns" && [[ $line != *"table missing"* ]]; then
        echo "listed (h3o centres): $line"
    else
        echo "FAIL $line"
        status=1
    fi
done < <(grep -v ': ok ' "$work/compare")

echo "== Rust with h3 centres"
compare "$work/compare-h3" "$work/py/python" "$work/rs-h3/rust" || status=1
cat "$work/compare-h3"
while read -r line; do
    echo "FAIL $line"
    status=1
done < <(grep -v ': ok ' "$work/compare-h3")
exit $status
