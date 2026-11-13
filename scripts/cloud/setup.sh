#!/usr/bin/env bash
# One-time setup of a Debian/Ubuntu box for unattended calibration runs (docs/cloud-runner.md).
# Run from the repository root as the user that will run the fits. Idempotent.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

if command -v apt-get >/dev/null; then
  sudo apt-get update -qq
  sudo apt-get install -y -qq build-essential pkg-config cmake curl git rsync unzip xz-utils tmux
fi

# rust-toolchain.toml pins the channel; rustup installs it on first cargo call.
command -v rustup >/dev/null || curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal
# shellcheck disable=SC1091
. "$HOME/.cargo/env"

cargo build --release --bin ceteris
mkdir -p "$HOME/ceteris-logs" "$HOME/ceteris-fit-cache" data/cache

# Cached inputs must be real files inside this checkout, never links into another one.
bad=$(find data/cache -type l 2>/dev/null || true)
if [ -n "$bad" ]; then
  echo "symlinks in data/cache (copy the files instead, then rerun):" >&2
  echo "$bad" >&2
  exit 1
fi

# Downloads what is missing and checks every file against the sha256 and size in fetch.toml.
# Manual entries (OECD ICIO) must already be in data/cache; copy them from the Mac with
#   rsync -a mac:~/path/to/ceteris/data/cache/ data/cache/
if ! target/release/ceteris pack fetch; then
  echo "pinned inputs missing or wrong: see docs/cloud-runner.md, 'Inputs'" >&2
  exit 1
fi
echo "setup ok: toolchain, binary, pinned inputs verified"
