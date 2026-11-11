#!/usr/bin/env bash
# Measure each pack as it would ship in a Steam depot: files, uncompressed bytes, compressed bytes.
# Usage: scripts/pack_size.sh [PACK_DIR...]   (default: every directory under data/packs/)
# Compression is a stand-in for Steam's chunk compression: COMPRESS="xz -6 -T0" by default.
# Output is a markdown table; paste it into docs/pack-distribution.md.
set -euo pipefail
cd "$(dirname "$0")/.."

compress=${COMPRESS:-xz -6 -T0}
packs=("$@")
[ ${#packs[@]} -gt 0 ] || packs=(data/packs/*/)
[ -d "${packs[0]}" ] || { echo "no packs found; pass PACK_DIR or build data/packs" >&2; exit 2; }

mib() { awk -v b="$1" 'BEGIN { printf "%.1f", b / 1048576 }'; }

echo "| pack | files | uncompressed MiB | compressed MiB | ratio |"
echo "|---|---:|---:|---:|---:|"
for p in "${packs[@]}"; do
  p=${p%/}
  [ -d "$p" ] || { echo "not a directory: $p" >&2; exit 2; }
  files=$(find "$p" -type f | wc -l | tr -d ' ')
  raw=$(find "$p" -type f -exec wc -c {} + | awk '$2 != "total" { s += $1 } END { print s + 0 }')
  # tar in sorted order so the figure is reproducible
  comp=$(tar -C "$p" -cf - --sort=name . 2>/dev/null | $compress | wc -c | tr -d ' ')
  echo "| $(basename "$p") | $files | $(mib "$raw") | $(mib "$comp") | $(awk -v c="$comp" -v r="$raw" 'BEGIN { printf "%.2f", r ? c / r : 0 }') |"
done
