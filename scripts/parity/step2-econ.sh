#!/usr/bin/env bash
# Step 2 econ table parity for one module (people, icio, goods, tariffs): builds the econ tables with Python and
# with `ceteris pack build --econ-only`, checks the Rust schemas and selects the module's columns
# from both (econ_tables.py), runs `ceteris pack compare` on them, then the module's tests on the
# real data (optimised: the WorldPop tests skip in debug builds).
set -euo pipefail

module="$1"
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
cargo build --release --bin ceteris
bin="$root/target/release/ceteris"
pack="$root/data/packs/2000"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

uv run --project python --locked python -m ceteris.packs.econ.build --pack "$pack" --out "$work/python" >/dev/null
"$bin" pack build --econ-only --pack "$pack" --out "$work/rust" >/dev/null
uv run --project python --locked python -I "$root/scripts/parity/econ_tables.py" "$module" "$work/python" "$work/rust" "$work/$module"
"$bin" pack compare "$work/$module/python" "$work/$module/rust"
cargo test --release -p ceteris-pack --test "$module"
