#!/usr/bin/env bash
# Stage the release app for the host platform into dist/<os>-<arch>/ with SHA256SUMS.
# Usage: scripts/package_app.sh [extra cargo args]
set -euo pipefail
cd "$(dirname "$0")/.."

os=$(uname -s | tr '[:upper:]' '[:lower:]')
arch=$(uname -m)
case "$arch" in arm64) arch=aarch64 ;; esac
case "$os-$arch" in
  linux-x86_64 | linux-aarch64 | darwin-aarch64) ;;
  *) echo "unsupported platform: $os-$arch" >&2; exit 1 ;;
esac

out="dist/$os-$arch"
rm -rf "$out"
mkdir -p "$out"

cargo build --release --locked --manifest-path app/Cargo.toml "$@"
cargo build --release --locked --bin ceteris
cp app/target/release/ceteris-app "$out/"
cp target/release/ceteris "$out/"
cp LICENSE-MIT LICENSE-APACHE "$out/"

(cd "$out" && if command -v sha256sum >/dev/null; then sha256sum *; else shasum -a 256 *; fi > SHA256SUMS)
echo "$out"
