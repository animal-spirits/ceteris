#!/usr/bin/env bash
# Step 3 build parity: the whole pack of build.build (`python -m ceteris.packs --out DIR`, which
# runs the Rust econ build itself) against `ceteris pack build --out DIR`, both on GEOS 3.15 (see
# README.md for the shapely build).
#
# - `pack compare` checks the eight P1 tables and the seven econ tables in file row order, never
#   sorted;
# - `json_diff.py` compares web/provinces.json and web/counties.json byte for byte;
# - `manifest_diff.py` compares the manifests: config, sources and stats (but the fill stats)
#   equal, the tile layers' and attribute files' digests equal, the Arrow tables' digests listed
#   (pyarrow and arrow-rs write different bytes), and the Rust file in json.dumps(indent=2) text;
# - the stats each build prints must be the same text, but the fill stats;
# - `fills.py` compares the fill files decoded: a Rust feature fails when it is off its own
#   simplified polygon (`pack build --simple DIR`) by more than 1e-4 and off Python's area by
#   more than that too.
# map.pmtiles is not compared: tippecanoe writes its tiles in a varying order.
#
# Two Rust builds:
# - with h3o's cell centres: split provinces' areas and those of the counties clipped to them are
#   held to SPLIT_TOL (`build_split_rows.py` separates those rows), the transport graph's sea
#   positions and the lengths that reach them to LISTED_TOL, and the attribute files' differing
#   rows and the layer digests are listed, not failed;
# - with h3's centres (`h3_centres.py` at the land and the sea resolutions): every column must
#   match and the attribute files and layer digests must be identical.
# In both, H3 cell areas and the means they weight are held to RULING_TOL (as step3-geo.sh).
#
# GEOS_PYTHON names a Python whose shapely is built on GEOS 3.15 and has the packages of
# python/pyproject.toml, and tippecanoe must be on PATH. Outputs go to WORK (default: a new
# temporary directory), kept.
set -euo pipefail

RULING_TOL=1e-8
SPLIT_TOL=1e-7
LISTED_TOL=1e-8
ruling_columns="cells.area_km2 provinces.elev_mean_m provinces.elev_std_m provinces.tri_m
counties.elev_mean_m counties.elev_std_m counties.tri_m"
split_columns="provinces_area_split.area_km2 counties_area_split.area_km2"
listed_columns="nodes.lon nodes.lat edges.length_km"
tables="provinces countries cells nodes edges counties sea_zones chokepoints
econ_goods econ_countries econ_io econ_provinces econ_pops econ_tariffs gateways"
split_tables="provinces_area provinces_area_split counties_area counties_area_split"

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
: "${GEOS_PYTHON:?set GEOS_PYTHON to a Python with shapely on GEOS 3.15}"
"$GEOS_PYTHON" -c 'import shapely, sys; v = shapely.geos_version_string; print(f"shapely on GEOS {v}"); sys.exit(not v.startswith("3.15."))'
echo "Rust on GEOS $(geos-config --version)"
pack="${CETERIS_PACK:-$root/data/packs/2000}"
work="${WORK:-$(mktemp -d)}"
mkdir -p "$work"
echo "outputs in $work"
source "$root/scripts/parity/packs_tree.sh"
packs_tree "$work/tree"
py() {
    (cd "$work/tree" && PYTHONPATH="$work/tree/python:$root/scripts/parity" "$GEOS_PYTHON" -P -s "$@")
}

# The Python build first: its econ step runs `cargo run --release` without the geos feature,
# which rebuilds target/release/ceteris; the geos binary is built after it and copied.
py -m ceteris.packs --out "$work/py" >"$work/py.stats"
cargo build --release --bin ceteris --features ceteris-cli/geos
mkdir -p "$work/bin"
cp "$root/target/release/ceteris" "$work/bin/ceteris"
bin="$work/bin/ceteris"

# `pack compare` of each of TABLES into OUT; fails when a table prints nothing, or exits
# non-zero with every line ok.
compare() {
    local out="$1" a="$2" b="$3" code t
    shift 3
    : >"$out"
    for t in "$@"; do
        code=0
        "$bin" pack compare "$a" "$b" --table "$t" >>"$out" || code=$?
        if ! grep -q "^${t}[.:]" "$out"; then
            echo "FAIL pack compare --table $t printed nothing (exit $code)"
            return 1
        fi
        if ((code != 0)) && ! grep "^${t}[.:]" "$out" | grep -qv ': ok '; then
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
"$bin" pack build --out "$work/rs" --simple "$work/rs-simple" >"$work/rs.stats"
"$bin" pack build --out "$work/rs-h3" --centres "$work/centres.bin" --simple "$work/rs-h3-simple" \
    >"$work/rs-h3.stats"
status=0

# The stats a build prints, without the fill stats; fails unless the printed text is
# json.dumps(indent=2) of them.
stats() {
    python3 -I - "$1" <<'PY'
import json, sys
t = open(sys.argv[1]).read()
s = json.loads(t)
if t != json.dumps(s, indent=2) + "\n":
    sys.exit(f"{sys.argv[1]}: not json.dumps(indent=2) text")
del s["fills"]
print(json.dumps(s, indent=2))
PY
}
for variant in rs rs-h3; do
    if stats "$work/py.stats" >"$work/py.p1-stats" && stats "$work/$variant.stats" >"$work/$variant.p1-stats" &&
        diff "$work/$variant.p1-stats" "$work/py.p1-stats"; then
        echo "ok   $variant printed stats but fills"
    else
        echo "FAIL $variant printed stats"
        status=1
    fi
done
# The fill files: bytes, or else each Rust feature's triangles against its own polygon.
fills() {
    local variant="$1" name code
    for name in provinces counties; do
        code=0
        py "$root/scripts/parity/fills.py" "$work/py/web/$name.fill" "$work/$variant/web/$name.fill" \
            "$work/$variant-simple/$name.simple.arrow" || code=$?
        if ((code == 2)); then
            echo "FAIL $variant $name.fill"
            status=1
        elif ((code != 0)); then
            echo "FAIL fills.py $variant $name.fill exited $code"
            status=1
        fi
    done
}
# The manifests; with --listed the layer and attribute digests are listed, not failed.
manifests() {
    local variant="$1" code=0
    shift
    "$GEOS_PYTHON" -I "$root/scripts/parity/manifest_diff.py" "$work/py/manifest.json" \
        "$work/$variant/manifest.json" "$@" || code=$?
    if ((code != 0)); then
        echo "FAIL $variant manifest (manifest_diff.py exit $code)"
        status=1
    fi
}

echo "== Rust with h3o centres"
mkdir -p "$work/py-masked" "$work/rs-masked"
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
    code=0
    "$GEOS_PYTHON" -I "$root/scripts/parity/json_diff.py" "$work/py/web/$name.json" "$work/rs/web/$name.json" \
        >"$work/json-$name" || code=$?
    sed 's/^FAIL /h3o centres: /' "$work/json-$name"
    if ((code != 0 && code != 2)); then
        echo "FAIL json_diff.py $name.json exited $code"
        status=1
    fi
done
manifests rs --listed
fills rs

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
manifests rs-h3
fills rs-h3
exit $status
