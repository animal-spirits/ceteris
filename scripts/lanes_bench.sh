#!/usr/bin/env bash
# L4: lanes against lone runs at L = 1, 8, 16, 32, with peak memory. Usage: lanes_bench.sh [TICKS]
set -euo pipefail
ticks=${1:-12}
cargo build --release -p ceteris-econ --example lanes_bench
bin=target/release/examples/lanes_bench
if [[ $(uname) == Darwin ]]; then tm=(/usr/bin/time -l)
elif [[ -x /usr/bin/time ]]; then tm=(/usr/bin/time -v)
else tm=(); fi
echo "load: $(uptime)"
for n in 1 8 16 32; do
  for mode in scalar lanes; do
    [[ $mode == scalar && $n == 1 ]] && continue
    "${tm[@]}" "$bin" "$mode" "$n" "$ticks" 2>&1 | grep -E 'run_ticks_per_sec|maximum resident|Maximum resident' | tr '\n' ' '
    echo
  done
done
