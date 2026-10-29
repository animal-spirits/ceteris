#!/usr/bin/env bash
# Step 3 geo parity: geo.py, raster.py, resources.py and fills.py against geo/{sphere,
# raster_terrain,resources,fills}.rs. `geo_inputs.py` builds the provinces and clipped counties in
# Python and writes their vertices with what the Python modules compute from them; its cells table
# must equal the pack's. `ceteris pack dump-geo` computes the same from the same vertices, and
# `pack compare` checks every table in file row order.
#
# Pending a ruling: H3 cell areas differ from h3's in the last bits (h3 and h3o sum the same edge
# terms over boundary vertices that differ by ulps), so cell areas and the area-weighted terrain
# means are held to RULING_TOL, not 1e-12; and the fills compare by decoded triangles and area
# (mapbox_earcut 2.1.0 and the earcut crate triangulate some polygons with holes differently):
# where a feature's areas differ, the Rust triangles must cover its polygon within 1e-4.
set -euo pipefail

RULING_TOL=1e-8
ruling_columns="cells.area_km2 county_cells.area_km2 provinces.elev_mean_m provinces.elev_std_m
provinces.tri_m counties.elev_mean_m counties.elev_std_m counties.tri_m"

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
cargo build --release --bin ceteris
bin="$root/target/release/ceteris"
pack="${CETERIS_PACK:-$root/data/packs/2000}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
source "$root/scripts/parity/packs_tree.sh"
packs_tree "$work/tree"
py() {
    PYTHONPATH="$work/tree/python" uv run --project "$root/python" --locked python -P -s "$@"
}

py "$root/scripts/parity/geo_inputs.py" "$work"
"$bin" pack compare "$work/python" "$pack" --table cells >"$work/pack-cells" || {
    cat "$work/pack-cells"
    echo "FAIL the Python inputs do not reproduce the pack's cells.arrow"
    exit 1
}
echo "ok   the Python inputs reproduce the pack's cells.arrow"
"$bin" pack dump-geo "$work"
status=0
code=0
"$bin" pack compare "$work/python" "$work/rust" >"$work/compare" || code=$?
cat "$work/compare"
if [[ ! -s "$work/compare" ]]; then
    echo "FAIL pack compare printed nothing (exit $code)"
    status=1
elif ((code != 0)) && ! grep -qv ': ok ' "$work/compare"; then
    echo "FAIL pack compare exited $code with every line ok"
    status=1
fi
while read -r line; do
    column="${line%%:*}"
    rel="${line##*max_rel=}"
    if [[ " ${ruling_columns//$'\n'/ } " == *" $column "* ]] &&
        python3 -I -c 'import sys; sys.exit(float(sys.argv[1]) > float(sys.argv[2]))' "$rel" "$RULING_TOL"; then
        echo "pending ruling: $column max_rel=$rel within $RULING_TOL"
    else
        echo "FAIL $line"
        status=1
    fi
done < <(grep -v ': ok ' "$work/compare")

for name in provinces counties; do
    py "$root/scripts/parity/fills.py" \
        "$work/python/$name.fill" "$work/rust/$name.fill" "$work/$name.geom.arrow" || {
        echo "FAIL $name.fill"
        status=1
    }
done
exit $status
