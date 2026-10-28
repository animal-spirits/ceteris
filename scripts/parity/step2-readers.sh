#!/usr/bin/env bash
# Step 2 reader parity: `ceteris pack dump-reader NAME` against scripts/parity/readers.py for
# every econ source reader on the real cache. Outputs must be identical; a difference prints the
# first differing path and the largest relative float difference. Pass reader names to run a
# subset (`wdi:INDICATOR` and `xlsx:FILE:SHEET` for the ones with arguments).
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
cargo build --release --bin ceteris
bin="$root/target/release/ceteris"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

readers=(
    codes ilostat-participation ilostat-unemployment ilostat-natural wic barro-lee wdi-regions pwt observed
    adb-income maddison load-wdi icio-aggregate icio-exports
    hs-cpa baci-by-product baci-values theta theta-bands theta-fixture trade-by-country
    rates cepii dta
    dose province-names us-states gem-coal gem-oil-gas seds
)
for f in data/cache/wdi_*.zip; do
    f="${f#data/cache/wdi_}"
    readers+=("wdi:${f%.zip}")
done
readers+=(
    "xlsx:pwt110.xlsx:Data"
    "xlsx:mpd2023_web.xlsx:Full data"
    "xlsx:OECD-Bilateral-Trade-in-Goods-End-use-Conversion-Key.xlsx:BTiGE FromHS-ToCPA-ToENDUSE"
    "xlsx:DTA 1.0 - Horizontal Content (v2).xlsx:Agreements"
    "xlsx:DTA 1.0 - Horizontal Content (v2).xlsx:Bilateral Information"
    "xlsx:Global Coal Mine Tracker, August 2026.xlsx:Non-closed mines"
    "xlsx:Global Coal Mine Tracker, August 2026.xlsx:Closed mines"
    "xlsx:Global-Oil-and-Gas-Extraction-Tracker-March-2026.xlsx:Field-level production data"
    "xlsx:Global-Oil-and-Gas-Extraction-Tracker-March-2026.xlsx:Field-level main data"
)
[ "$#" -gt 0 ] && readers=("$@")

# Readers compared to a relative tolerance instead of exactly. theta-bands is a matrix product
# that numpy hands to BLAS, whose summation order is not reproducible; it adds at most 20 terms
# of one sign, so the measured gap is 1.8e-16 (about one ulp), far inside ADR 0113's 1e-12.
tolerance() {
    case "$1" in
    theta-bands) echo 1e-12 ;;
    *) echo 0 ;;
    esac
}

failed=0
for spec in "${readers[@]}"; do
    IFS=: read -r -a args <<<"$spec"
    (cd python && uv run python -I "$root/scripts/parity/readers.py" "${args[@]}") >"$work/py.json"
    "$bin" pack dump-reader "${args[@]}" >"$work/rs.json"
    tol="$(tolerance "${args[0]}")"
    if cmp -s "$work/py.json" "$work/rs.json"; then
        echo "ok   $spec ($(wc -c <"$work/py.json" | tr -d ' ') bytes)"
    elif python3 -I "$root/scripts/parity/floatdiff.py" "$work/py.json" "$work/rs.json" "$tol"; then
        echo "ok   $spec within $tol"
    else
        echo "FAIL $spec"
        failed=1
    fi
done
exit "$failed"
