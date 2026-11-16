#!/usr/bin/env bash
# Small-pack CI job (RFC 0353 D2, ADRs 0371, 0372): calibrate tests/fixtures/pack-ci with the
# fit cache, then run the benchmark on it. Report-only: the caller decides whether to fail.
set -euo pipefail

logs=${1:?usage: scripts/pack-ci.sh LOG_DIR [FIT_CACHE_DIR]}
cache=${2:-}
root=$(cd "$(dirname "$0")/.." && pwd)
mkdir -p "$logs"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

cp -R "$root/tests/fixtures/pack-ci" "$work/pack"
calib_args=(--pack "$work/pack" --out "$work/calib")
[ -n "$cache" ] && calib_args+=(--fit-cache "$cache")

step() {
  local name=$1; shift
  local start=$SECONDS
  echo "== $name"
  "$@" 2>&1 | tee "$logs/$name.log"
  echo "== $name: $((SECONDS - start)) s" | tee -a "$logs/timings.log"
}

step calibrate cargo run --release --bin ceteris -- calibrate "${calib_args[@]}"
cp "$work"/calib/* "$work/pack/"
export CETERIS_PACK="$work/pack"
step benchmark cargo test --release -p ceteris-calibrate --test benchmark -- --include-ignored --nocapture
