#!/usr/bin/env bash
# Detached, verified, cached calibration run (docs/cloud-runner.md).
#   scripts/cloud/fit.sh LABEL PACK_DIR [ceteris calibrate args...]
# Example: scripts/cloud/fit.sh eps-fit data/packs/2000 --fit-epsilon --out ~/runs/eps
#   scripts/cloud/fit.sh status LABEL
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"
logs=$HOME/ceteris-logs

if [ "${1:-}" = status ]; then
  label=${2:?usage: fit.sh status LABEL}
  latest=$(ls -t "$logs/$label"-*.log 2>/dev/null | head -1 || true)
  [ -n "$latest" ] || { echo "no run named $label"; exit 2; }
  echo "$latest"; cat "${latest%.log}.exit" 2>/dev/null || echo "running or killed"
  tail -n 5 "$latest"
  exit 0
fi

label=${1:?usage: fit.sh LABEL PACK_DIR [calibrate args...]}
pack=${2:?usage: fit.sh LABEL PACK_DIR [calibrate args...]}
shift 2
# shellcheck disable=SC1091
. "$HOME/.cargo/env"

[ -z "$(git status --porcelain --untracked-files=no)" ] || { echo "dirty tree: commit first" >&2; exit 2; }
[ -d "$pack" ] || { echo "no pack at $pack" >&2; exit 2; }
[ -z "$(find data/cache -type l)" ] || { echo "symlinks in data/cache" >&2; exit 2; }

cargo build --release --bin ceteris
bin=target/release/ceteris
"$bin" pack fetch   # exits 1 unless every pinned input matches its sha256 and size

# The fit cache is keyed by data inside the binary; the code revision is the directory.
rev=$(git rev-parse 'HEAD^{tree}')
cache=$HOME/ceteris-fit-cache/$rev
stamp=$(date -u +%Y%m%dT%H%M%SZ)
log=$logs/$label-$stamp.log
mkdir -p "$logs" "$cache"
{
  echo "commit $(git rev-parse HEAD) tree $rev"
  echo "pack $pack: $(find "$pack" -type f -print0 | sort -z | xargs -0 sha256sum | sha256sum | cut -d' ' -f1)"
  echo "args calibrate --pack $pack --fit-cache $cache $*"
} > "$log"

setsid nohup bash -c '
  "$@"; echo $? > "$0"
' "${log%.log}.exit" "$bin" calibrate --pack "$pack" --fit-cache "$cache" "$@" >> "$log" 2>&1 < /dev/null &
echo "started: $log (status: scripts/cloud/fit.sh status $label)"
