#!/usr/bin/env bash
# Golden-hash gate for cross-platform runs (run on x86 and arm64; both must pass unchanged).
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
cargo test -p ceteris-core --release --test golden --test p0_acceptance
cargo test -p ceteris-econ --release --test golden --test p2_acceptance
