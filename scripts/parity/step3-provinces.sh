#!/usr/bin/env bash
# Step 3 province and county parity: provinces.py, counties.py and the county clipping of
# build.build against geo/{provinces,counties}.rs, both on GEOS 3.15 (see README.md for the
# shapely build). `geo_inputs.py` builds the provinces and clipped counties in Python and writes
# their vertices, attributes and tables; `ceteris pack dump-geo DIR --build` builds and writes the
# same in Rust; `pack compare` checks every table in file row order.
#
# Exact: every attribute (ids, names, countries, split rules, county homes), every H3 cell's
# province and county, the mineral counts. Pending rulings:
# - H3 cell areas and the means they weight, held to RULING_TOL (as step3-geo.sh);
# - areas of split provinces and of the counties clipped to them, held to SPLIT_TOL: a Voronoi
#   split seeds its cells at H3 cell centres, which h3o computes differently from h3 in the last
#   bits, so those provinces' vertices differ by ulps (the vertex tables are listed, not failed);
# - fills: the Rust triangles must cover each feature's polygon within 1e-4 where they differ.
#
# GEOS_PYTHON names a Python whose shapely is built on GEOS 3.15 and has the packages of
# python/pyproject.toml.
set -euo pipefail

RULING_TOL=1e-8
SPLIT_TOL=1e-7
ruling_columns="cells.area_km2 county_cells.area_km2 provinces.elev_mean_m provinces.elev_std_m
provinces.tri_m counties.elev_mean_m counties.elev_std_m counties.tri_m"
split_columns="provinces.area_km2 counties.area_km2 clip_stats.fits clip_stats.fits_inside"
listed_tables="provinces.geom counties.geom"

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

(cd python && PYTHONPATH="$root/python" "$GEOS_PYTHON" -P -s "$root/scripts/parity/geo_inputs.py" "$work/py")
cp "$work"/py/*.arrow "$work/py/python/"
if "$bin" pack compare "$work/py/python" "$pack" --table cells >/dev/null; then
    echo "ok   the Python build reproduces the pack's cells.arrow"
else
    echo "note the Python build on GEOS 3.15 differs from the pack's cells.arrow"
fi
"$bin" pack dump-geo "$work/rs" --build
"$bin" pack compare "$work/py/python" "$work/rs/rust" >"$work/compare" || true
cat "$work/compare"

within() {
    python3 -I -c 'import sys; sys.exit(float(sys.argv[1]) > float(sys.argv[2]))' "$1" "$2"
}
listed() {
    [[ " ${2//$'\n'/ } " == *" $1 "* ]]
}
status=0
while read -r line; do
    column="${line%%:*}"
    rel="${line##*max_rel=}"
    if listed "$column" "$ruling_columns" && within "$rel" "$RULING_TOL"; then
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

for name in provinces counties; do
    (cd python && uv run --locked python -I "$root/scripts/parity/fills.py" \
        "$work/py/python/$name.fill" "$work/rs/rust/$name.fill" "$work/rs/rust/$name.geom.arrow") || {
        echo "FAIL $name.fill"
        status=1
    }
done
exit $status
