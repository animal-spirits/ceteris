#!/usr/bin/env bash
# Quiet benchmark: pauses the Mac CI runners (SIGSTOP), runs one criterion bench single-threaded
# under caffeinate, resumes the runners on exit, appends rows to tests/perf/history.csv.
# Usage: bench_quiet.sh [-f] CRATE BENCH [criterion filter/args...]   (-f: stay in the foreground)
# Detached by default; log in ~/ceteris-logs/. Never use caffeinate -d.
set -euo pipefail
shopt -s lastpipe 2>/dev/null || true
root=$(cd "$(dirname "$0")/.." && pwd)
hist=$root/tests/perf/history.csv

# Process tree of every runner listener, listeners first.
runner_pids() {
  local p
  for p in $(pgrep -f '(^|/)Runner\.Listener( |$)' || true); do
    echo "$p"
    descendants "$p"
  done
}
descendants() {
  local c
  for c in $(pgrep -P "$1" || true); do echo "$c"; descendants "$c"; done
}

if [[ ${1:-} == --inner ]]; then
  shift
else
  fg=0; [[ ${1:-} == -f ]] && { fg=1; shift; }
  [[ $# -ge 2 ]] || { sed -n '2,4p' "$0"; exit 2; }
  if [[ $fg == 0 ]]; then
    log=$HOME/ceteris-logs/bench-$(date -u +%Y%m%dT%H%M%SZ).log
    mkdir -p "$(dirname "$log")"
    if command -v caffeinate >/dev/null; then wrap=(caffeinate -imsu); else wrap=(); fi
    nohup "${wrap[@]}" "$0" --inner "$@" >"$log" 2>&1 &
    echo "detached pid $!; log $log"
    exit 0
  fi
fi

crate=$1 bench=$2; shift 2
paused=()
resume() {
  [[ ${#paused[@]} -gt 0 ]] && kill -CONT "${paused[@]}" 2>/dev/null || true
  [[ -n ${sampler:-} ]] && kill "$sampler" 2>/dev/null || true
  [[ ${#paused[@]} -gt 0 ]] && echo "runners resumed (${#paused[@]} processes)"
  return 0
}
trap resume EXIT
trap 'exit 130' INT TERM HUP

mapfile -t paused < <(runner_pids)
[[ ${#paused[@]} -gt 0 ]] && kill -STOP "${paused[@]}" && echo "runners paused (${#paused[@]} processes)"

loadlog=$(mktemp)
load() { uptime | sed 's/.*load averages*: //; s/,//g' | awk '{print $1}'; }
( while sleep 10; do echo "load $(load)" >>"$loadlog"; done ) &
sampler=$!

cd "$root"
cargo bench -p "$crate" --no-run --bench "$bench" 2>&1 | tail -n 3
echo "load start: $(load)"
l0=$(load)
out=$(mktemp)
t0=$(date +%s.%N)
# user+sys of the whole bench process tree; one rayon thread unless RAYON_NUM_THREADS is set, so CPU time tracks wall time
export RAYON_NUM_THREADS=${RAYON_NUM_THREADS:-1}
TIMEFORMAT='%U %S'
{ time cargo bench -p "$crate" --bench "$bench" -- --noplot "$@" 2>&1 | tee "$out"; } 2>"$out.time" || { echo "bench failed; no history row"; exit 1; }
t1=$(date +%s.%N)
l1=$(load)
cpu=$(tail -n 1 "$out.time" | awk '{printf "%.2f", $1 + $2}')
wall=$(echo "$t1 - $t0" | bc | awk '{printf "%.2f", $1}')
echo "load end: $l1; process CPU ${cpu}s (user+sys, cargo bench tree), wall ${wall}s"

[[ -f $hist ]] || echo "utc,commit,dirty,host,os,load_start,load_end,crate,bench,id,median_ms,total_cpu_s,total_wall_s,threads" >"$hist"
commit=$(git rev-parse --short HEAD)
dirty=$(git diff --quiet HEAD -- . ':!tests/perf/history.csv' && echo 0 || echo 1)
host=$(hostname -s)
now=$(date -u +%Y-%m-%dT%H:%M:%SZ)
# criterion: "id   time:   [lo mid hi]", id possibly on the preceding line
awk -v pre="$now,$commit,$dirty,$host,$(uname -s),$l0,$l1,$crate,$bench" -v tail="$cpu,$wall,$RAYON_NUM_THREADS" '
  function ms(v, u) { return v * (u == "ns" ? 1e-6 : u == "µs" || u == "us" ? 1e-3 : u == "s" ? 1e3 : 1) }
  /^[^ ].*[^ ]$/ && !/time:/ && !/^(Benchmarking|Found|Gnuplot|change|Performance|No change|Change)/ { prev = $0 }
  /time: *\[/ {
    id = $0; sub(/ *time:.*/, "", id); if (id == "") id = prev
    match($0, /\[.*\]/); n = split(substr($0, RSTART + 1, RLENGTH - 2), f, " ")
    printf "%s,\"%s\",%.6f,%s\n", pre, id, ms(f[3], f[4]), tail
  }' "$out" >>"$hist"
echo "load samples: $(tr "\n" " " <"$loadlog" | sed "s/load //g")"
rm -f "$out" "$out.time" "$loadlog"
echo "appended to $hist"
