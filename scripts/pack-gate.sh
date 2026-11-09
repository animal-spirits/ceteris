#!/usr/bin/env bash
# Full-pack gate: export the pack, then every pack-gated test in release.
# CETERIS_PACK comes from the runner's environment (absolute path to data/packs/2000).
set -euo pipefail

logs=${1:?usage: scripts/pack-gate.sh LOG_DIR}
: "${CETERIS_PACK:?set CETERIS_PACK in the runner .env file}"
case $CETERIS_PACK in /*) ;; *) echo "CETERIS_PACK must be absolute" >&2; exit 2 ;; esac
[ -d "$CETERIS_PACK" ] || { echo "no pack at $CETERIS_PACK" >&2; exit 2; }

mkdir -p "$logs"
export CETERIS_PACK_EXPORT
CETERIS_PACK_EXPORT=$(mktemp -d)
trap 'rm -rf "$CETERIS_PACK_EXPORT"' EXIT

step() {
  local name=$1; shift
  local start=$SECONDS
  echo "== $name"
  "$@" 2>&1 | tee "$logs/$name.log"
  echo "== $name: $((SECONDS - start)) s" | tee -a "$logs/timings.log"
}

step export cargo run --release --bin ceteris -- export --pack "$CETERIS_PACK" --out "$CETERIS_PACK_EXPORT"
# One workspace run covers the econ golden/p2_acceptance, cli p2_gate and calibrate targets.
step workspace cargo test --workspace --release --no-fail-fast -- --nocapture
