#!/usr/bin/env bash
# Step 3 build parity: the P1 tables and map attribute files of build.build against `ceteris
# pack build`, both on GEOS 3.15 (see README.md for the shapely build). `build_tables.py` runs
# build.build with its map and econ steps left out; `ceteris pack build --out DIR` runs the whole
# Rust build. `pack compare` checks the eight P1 tables in file row order, never sorted;
# `json_diff.py` compares web/provinces.json and web/counties.json byte for byte; the build stats
# (without fills and econ) must be the same text.
#
# Two Rust builds:
# - with h3o's cell centres: split provinces' areas and those of the counties clipped to them are
#   held to SPLIT_TOL (`build_split_rows.py` separates those rows), the transport graph's sea
#   positions and the lengths that reach them to LISTED_TOL, and the attribute files' differing
#   rows are listed, not failed;
# - with h3's centres (`h3_centres.py` at the land and the sea resolutions): every column must
#   match and the attribute files must be identical.
# In both, H3 cell areas and the means they weight are held to RULING_TOL (as step3-geo.sh).
#
# GEOS_PYTHON names a Python whose shapely is built on GEOS 3.15 and has the packages of
# python/pyproject.toml. Outputs go to WORK (default: a new temporary directory), kept.
set -euo pipefail

RULING_TOL=1e-8
SPLIT_TOL=1e-7
LISTED_TOL=1e-8
ruling_columns="cells.area_km2 provinces.elev_mean_m provinces.elev_std_m provinces.tri_m
counties.elev_mean_m counties.elev_std_m counties.tri_m"
split_columns="provinces_area_split.area_km2 counties_area_split.area_km2"
listed_columns="nodes.lon nodes.lat edges.length_km"
tables="provinces countries cells nodes edges counties sea_zones chokepoints"
split_tables="provinces_area provinces_area_split counties_area counties_area_split"

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
: "${GEOS_PYTHON:?set GEOS_PYTHON to a Python with shapely on GEOS 3.15}"
"$GEOS_PYTHON" -c 'import shapely, sys; v = shapely.geos_version_string; print(f"shapely on GEOS {v}"); sys.exit(not v.startswith("3.15."))'
echo "Rust on GEOS $(geos-config --version)"
cargo build --release --bin ceteris --features ceteris-cli/geos
bin="$root/target/release/ceteris"
pack="${CETERIS_PACK:-$root/data/packs/2000}"
work="${WORK:-$(mktemp -d)}"
mkdir -p "$work"
mkdir -p "$work"
echo "outputs in $work"

# `pack compare` of each of TABLES into OUT; fails when a table prints nothing, or exits
# non-zero with every line ok.
compare() {
    local out="$1" a="$2" b="$3" code t
    shift 3
    : >"$out"
    for t in "$@"; do
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
within() {
    python3 -I -c 'import sys; sys.exit(float(sys.argv[1]) > float(sys.argv[2]))' "$1" "$2"
}
# The Rust stats without econ, as json.dumps(stats, indent=2) writes them.
p1_stats() {
    python3 -I -c 'import json, sys; s = json.load(open(sys.argv[1])); del s["econ"]; print(json.dumps(s, indent=2))' "$1"
}

(cd python && PYTHONPATH="$root/python" "$GEOS_PYTHON" -P -s "$root/scripts/parity/build_tables.py" "$work/py" >"$work/py.stats")
for t in $tables; do
    if "$bin" pack compare "$work/py" "$pack" --table "$t" >/dev/null; then
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
"$bin" pack build --out "$work/rs" >"$work/rs.stats"
"$bin" pack build --out "$work/rs-h3" --centres "$work/centres.bin" >"$work/rs-h3.stats"
status=0

for variant in rs rs-h3; do
    if diff <(p1_stats "$work/$variant.stats") "$work/py.stats" >/dev/null; then
        echo "ok   $variant build stats"
    else
        echo "FAIL $variant build stats"
        diff <(p1_stats "$work/$variant.stats") "$work/py.stats" || true
        status=1
    fi
done

echo "== Rust with h3o centres"
mkdir "$work/py-masked" "$work/rs-masked"
for t in $tables; do
    cp "$work/py/$t.arrow" "$work/py-masked/"
    cp "$work/rs/$t.arrow" "$work/rs-masked/"
done
"$GEOS_PYTHON" -I "$root/scripts/parity/build_split_rows.py" "$work/py-masked" "$work/rs-masked"
# shellcheck disable=SC2086
compare "$work/compare" "$work/py-masked" "$work/rs-masked" $tables $split_tables || status=1
cat "$work/compare"
while read -r line; do
    column="${line%%:*}"
    rel="${line##*max_rel=}"
    if [[ $line != *max_rel=* ]]; then
        echo "FAIL $line"
        status=1
    elif listed "$column" "$ruling_columns" && within "$rel" "$RULING_TOL"; then
        echo "pending ruling: $column max_rel=$rel within $RULING_TOL"
    elif listed "$column" "$split_columns" && within "$rel" "$SPLIT_TOL"; then
        echo "pending ruling: $column max_rel=$rel within $SPLIT_TOL"
    elif listed "$column" "$listed_columns" && within "$rel" "$LISTED_TOL"; then
        echo "h3o centres: $column max_rel=$rel within $LISTED_TOL"
    else
        echo "FAIL $line"
        status=1
    fi
done < <(grep -v ': ok ' "$work/compare")
for name in provinces counties; do
    "$GEOS_PYTHON" -I "$root/scripts/parity/json_diff.py" "$work/py/web/$name.json" "$work/rs/web/$name.json" |
        sed 's/^FAIL /h3o centres: /' || true
done

echo "== Rust with h3 centres"
# shellcheck disable=SC2086
compare "$work/compare-h3" "$work/py" "$work/rs-h3" $tables || status=1
cat "$work/compare-h3"
while read -r line; do
    column="${line%%:*}"
    rel="${line##*max_rel=}"
    if [[ $line == *max_rel=* ]] && listed "$column" "$ruling_columns" && within "$rel" "$RULING_TOL"; then
        echo "pending ruling: $column max_rel=$rel within $RULING_TOL"
    else
        echo "FAIL $line"
        status=1
    fi
done < <(grep -v ': ok ' "$work/compare-h3")
for name in provinces counties; do
    "$GEOS_PYTHON" -I "$root/scripts/parity/json_diff.py" "$work/py/web/$name.json" "$work/rs-h3/web/$name.json" || status=1
done
exit $status
