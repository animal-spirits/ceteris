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
    hs-cpa baci-by-product baci-values trade-by-country
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

failed=0
for spec in "${readers[@]}"; do
    IFS=: read -r -a args <<<"$spec"
    (cd python && uv run python -I "$root/scripts/parity/readers.py" "${args[@]}") >"$work/py.json"
    "$bin" pack dump-reader "${args[@]}" >"$work/rs.json"
    if cmp -s "$work/py.json" "$work/rs.json"; then
        echo "ok   $spec ($(wc -c <"$work/py.json" | tr -d ' ') bytes)"
    else
        echo "FAIL $spec"
        python3 -I - "$work/py.json" "$work/rs.json" <<'PY' || true
import json, sys

py, rs = (json.load(open(p)) for p in sys.argv[1:])
worst = [0.0, None]


def walk(a, b, path):
    if isinstance(a, float) and isinstance(b, float):
        if a != b:
            rel = abs(a - b) / max(abs(a), abs(b))
            if rel > worst[0]:
                worst[:] = [rel, path]
        return None
    if type(a) is not type(b):
        return f"{path}: {a!r:.80} vs {b!r:.80}"
    if isinstance(a, dict):
        if a.keys() != b.keys():
            return f"{path}: keys only in python {sorted(a.keys() - b.keys())[:5]}, only in rust {sorted(b.keys() - a.keys())[:5]}"
        return next((d for k in a if (d := walk(a[k], b[k], f"{path}/{k}"))), None)
    if isinstance(a, list):
        if len(a) != len(b):
            return f"{path}: {len(a)} items in python, {len(b)} in rust"
        return next((d for i, (x, y) in enumerate(zip(a, b)) if (d := walk(x, y, f"{path}/{i}"))), None)
    return None if a == b else f"{path}: {a!r:.80} vs {b!r:.80}"


first = walk(py, rs, "")
print(f"     first structural difference: {first}" if first else "     same structure")
print(f"     largest relative float difference: {worst[0]:.3e} at {worst[1]}")
PY
        failed=1
    fi
done
exit "$failed"
