#!/usr/bin/env bash
# Step 2 raster parity: per-province totals of WorldPop, summed MapSPAM and Kummu GDP
# (`ceteris pack dump-reader population RASTER` against readers.py; the texts must be identical),
# then the pixel to cell mapping of the border windows: the fixture of
# `pixel_cells_match_python_on_border_windows` is rewritten from Python and must equal the
# committed one, and the Rust test runs against it.
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
"$root/scripts/parity/step2-readers.sh" population:worldpop population:mapspam population:kummu

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
fixture=crates/ceteris-pack/tests/data/pixel_cells.txt
(cd python && uv run python -I "$root/scripts/parity/pixel_cells.py" "$work/pixel_cells.txt")
if cmp -s "$work/pixel_cells.txt" "$fixture"; then
    echo "ok   pixel cells ($(grep -c '^window' "$fixture") windows, $(grep -vc '^window' "$fixture") pixels)"
else
    echo "FAIL pixel cells: Python's mapping differs from $fixture"
    exit 1
fi
cargo test --release -p ceteris-pack --test raster
