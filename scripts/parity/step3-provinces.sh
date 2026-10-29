#!/usr/bin/env bash
# Step 3 province and county parity: provinces.py, counties.py and the county clipping of
# build.build against geo/{provinces,counties}.rs, both on GEOS 3.15 (see README.md for the
# shapely build). `geo_inputs.py` builds the provinces and clipped counties in Python and writes
# their vertices, attributes and tables; `ceteris pack dump-geo DIR --build` builds and writes the
# same in Rust; `pack compare` checks every table in file row order.
#
# Exact: every attribute (ids, names, countries, split rules, county homes), every H3 cell's
# province and county, the mineral counts, and the areas and vertices of every province that is
# not split and every county in one. Pending rulings:
# - H3 cell areas and the means they weight, held to RULING_TOL (as step3-geo.sh);
# - areas of split provinces and of the counties clipped to them, held to SPLIT_TOL, and their
#   vertices listed, not failed (`split_rows.py` separates those rows): a Voronoi split seeds its
#   cells at H3 cell centres, which h3o computes differently from h3 in the last bits;
# - fills: the Rust triangles must cover each feature's polygon within 1e-4 where they differ.
# A second Rust build takes h3's centres (`h3_centres.py`); it must match Python exactly but for
# the H3 cell areas.
#
# GEOS_PYTHON names a Python whose shapely is built on GEOS 3.15 and has the packages of
# python/pyproject.toml.
set -euo pipefail

RULING_TOL=1e-8
SPLIT_TOL=1e-7
ruling_columns="cells.area_km2 county_cells.area_km2 provinces.elev_mean_m provinces.elev_std_m
provinces.tri_m counties.elev_mean_m counties.elev_std_m counties.tri_m"
split_columns="provinces_area_split.area_km2 counties_area_split.area_km2 clip_stats.fits_inside"
listed_tables="provinces_geom_split counties_geom_split"

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
: "${GEOS_PYTHON:?set GEOS_PYTHON to a Python with shapely on GEOS 3.15}"
"$GEOS_PYTHON" -c 'import shapely, sys; v = shapely.geos_version_string; print(f"shapely on GEOS {v}"); sys.exit(not v.startswith("3.15."))'
echo "Rust on GEOS $(geos-config --version)"
cargo build --release --bin ceteris --features ceteris-cli/geos
bin="$root/target/release/ceteris"
pack="${CETERIS_PACK:-$root/data/packs/2000}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
source "$root/scripts/parity/packs_tree.sh"
packs_tree "$work/tree"

# `pack compare A B > OUT`; fails when it prints nothing, or exits non-zero with every line ok.
compare() {
    local out="$1" code=0
    shift
    "$bin" pack compare "$@" >"$out" || code=$?
    if [[ ! -s "$out" ]]; then
        echo "FAIL pack compare $* printed nothing (exit $code)"
        return 1
    fi
    if ((code != 0)) && ! grep -qv ': ok ' "$out"; then
        echo "FAIL pack compare $* exited $code with every line ok"
        return 1
    fi
}
within() {
    python3 -I -c 'import sys; sys.exit(float(sys.argv[1]) > float(sys.argv[2]))' "$1" "$2"
}
listed() {
    [[ " ${2//$'\n'/ } " == *" $1 "* ]]
}

PYTHONPATH="$work/tree/python" "$GEOS_PYTHON" -P -s "$root/scripts/parity/geo_inputs.py" "$work/py"
cp "$work"/py/*.arrow "$work/py/python/"
if "$bin" pack compare "$work/py/python" "$pack" --table cells >/dev/null; then
    echo "ok   the Python build reproduces the pack's cells.arrow"
else
    echo "note the Python build on GEOS 3.15 differs from the pack's cells.arrow"
fi
"$GEOS_PYTHON" -I "$root/scripts/parity/h3_centres.py" "$(sed -n 's/^land_res = //p' python/ceteris/packs/pack.toml)" "$work/centres.bin"
"$bin" pack dump-geo "$work/rs" --build
"$bin" pack dump-geo "$work/rs-h3" --build --centres "$work/centres.bin"
status=0

echo "== Rust with h3o centres"
cp -R "$work/py/python" "$work/py-masked"
cp -R "$work/rs/rust" "$work/rs-masked"
"$GEOS_PYTHON" -I "$root/scripts/parity/split_rows.py" "$work/py-masked" "$work/rs-masked"
compare "$work/compare" "$work/py-masked" "$work/rs-masked" || status=1
cat "$work/compare"
while read -r line; do
    column="${line%%:*}"
    rel="${line##*max_rel=}"
    if [[ $line == *"table missing"* ]]; then
        echo "FAIL $line"
        status=1
    elif listed "$column" "$ruling_columns" && within "$rel" "$RULING_TOL"; then
        echo "pending ruling: $column max_rel=$rel within $RULING_TOL"
    elif listed "$column" "$split_columns" && within "$rel" "$SPLIT_TOL"; then
        echo "pending ruling: $column max_rel=$rel within $SPLIT_TOL"
    elif listed "$column" "$listed_tables" || listed "${column%.*}" "$listed_tables"; then
        echo "pending ruling: $line"
    else
        echo "FAIL $line"
        status=1
    fi
done < <(grep -v ': ok ' "$work/compare")

echo "== Rust with h3 centres"
compare "$work/compare-h3" "$work/py/python" "$work/rs-h3/rust" || status=1
cat "$work/compare-h3"
while read -r line; do
    column="${line%%:*}"
    rel="${line##*max_rel=}"
    if listed "$column" "$ruling_columns" && within "$rel" "$RULING_TOL"; then
        echo "pending ruling: $column max_rel=$rel within $RULING_TOL"
    else
        echo "FAIL $line"
        status=1
    fi
done < <(grep -v ': ok ' "$work/compare-h3")

for name in provinces counties; do
    PYTHONPATH="$work/tree/python" uv run --project "$root/python" --locked python -P -s "$root/scripts/parity/fills.py" \
        "$work/py/python/$name.fill" "$work/rs/rust/$name.fill" "$work/rs/rust/$name.geom.arrow" || {
        echo "FAIL $name.fill"
        status=1
    }
done
exit $status
