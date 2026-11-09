#!/usr/bin/env bash
# Fails unless every ban in clippy.toml (ADR 0097) still fires on the fixture crate.
set -uo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
expected=76 # 10 rayon + 10 algebraic + 50 float transcendental methods + 3 HashMap + 3 HashSet uses

# Clippy does not track CLIPPY_CONF_DIR in its fingerprint: force a re-check.
touch "$root/scripts/clippy-bans/src/main.rs"
# The fixture names unstable float_algebraic methods.
out="$(cd "$root/scripts/clippy-bans" \
  && RUSTC_BOOTSTRAP=1 CLIPPY_CONF_DIR="$root" CARGO_TARGET_DIR="$root/target/clippy-bans" \
     cargo clippy --color never -- -D warnings 2>&1)"
status=$?

found="$(grep -cE '^error: use of a disallowed (method|type)' <<<"$out")"
if [ "$status" -eq 0 ] || [ "$found" -ne "$expected" ]; then
  echo "$out" >&2
  echo "expected clippy to fail with $expected disallowed diagnostics, got $found (exit $status)" >&2
  exit 1
fi
echo "clippy bans: $found disallowed diagnostics, as expected"
