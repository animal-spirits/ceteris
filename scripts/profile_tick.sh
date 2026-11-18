#!/usr/bin/env bash
# Profile or time the 45-goods tick (crates/ceteris-econ/benches/goods.rs).
# Usage: profile_tick.sh MODE [criterion filter, default tick/45]
#   hyperfine   5 runs of setup plus GOODS_SPANS=24 sim ticks per goods count (whole process,
#               criterion skipped); for before/after of one change, not for the ms per tick
#   samply      sampling profile of the criterion tick/45 loop, opens the profiler UI
#   flamegraph  flamegraph.svg in the repo root (cargo-flamegraph; may need sudo on Linux/macOS)
# Install: cargo install samply flamegraph; hyperfine from your package manager.
# Set CETERIS_PACK for the real pack, else the fixture runs. Quote numbers only from a quiet
# host (scripts/bench_quiet.sh); the per-tick ms is the bench's own `tick/45` and span output.
set -euo pipefail
cd "$(dirname "$0")/.."
mode=${1:-}; filter=${2:-tick/45}
[[ $mode =~ ^(hyperfine|samply|flamegraph)$ ]] || { sed -n 2,10p "$0"; exit 2; }
command -v "$mode" >/dev/null || { echo "$mode not installed" >&2; exit 1; }

# Line tables give the profilers symbols without changing the optimised code.
export CARGO_PROFILE_BENCH_DEBUG=line-tables-only
bin=$(cargo bench -p ceteris-econ --bench goods --no-run 2>&1 | sed -n 's/.*Executable .*(\(.*\)).*/\1/p' | tail -1)
[[ -x $bin ]] || { echo "bench binary not found" >&2; exit 1; }
loop=(--bench --profile-time 10 "$filter")   # criterion: run 10 s without analysis

case $mode in
  hyperfine) hyperfine --runs 5 --warmup 1 "env GOODS_SPANS=24 $bin --bench '^\$'" ;;
  samply) samply record "$bin" "${loop[@]}" ;;
  flamegraph) flamegraph -o flamegraph.svg -- "$bin" "${loop[@]}" ;;
esac
