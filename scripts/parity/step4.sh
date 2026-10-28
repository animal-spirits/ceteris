#!/usr/bin/env bash
# Step 4 parity: the reference's inputs, PIGL demand, dense kernels, balanced Sinkhorn and A.3
# bound against the Python reference on the fixture export and on an export of the pack. The
# Python side dumps what it derives (step4_dump.py); the Rust `parity` test of ceteris-reference
# reads the dump, recomputes everything from the same export and prints each quantity's largest
# relative difference. The dense kernels (P x P per good) are compared on the fixture only.
# The export and the dumps stay in WORK_DIR.
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
pack="${CETERIS_PACK:-$root/data/packs/2000}"
work="${1:?usage: step4.sh WORK_DIR (created; must not exist)}"
if [ -e "$work" ]; then
    echo "step4.sh: $work exists" >&2
    exit 1
fi
mkdir -p "$work"
cargo build --release --bin ceteris
"$root/target/release/ceteris" export --pack "$pack" --out "$work/pack-export"

dump() {
    uv run --project "$root/python" --locked python "$root/scripts/parity/step4_dump.py" "$@"
}

dump "$root/tests/golden/p2-fixture" "$work/fixture.json" --dense
dump "$work/pack-export" "$work/pack.json"
for name in fixture pack; do
    echo "== $name"
    CETERIS_REFERENCE_PARITY="$work/$name.json" \
        cargo test --release --quiet -p ceteris-reference --test parity -- --nocapture
done
