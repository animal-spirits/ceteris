#!/usr/bin/env bash
# Step 4 parity: the reference's inputs, PIGL demand, dense kernels, balanced Sinkhorn, A.3 bound,
# model evaluation and equilibrium against the Python reference on the fixture export and on an
# export of the pack. The Python side dumps what it derives (step4_dump.py); the Rust `parity`
# test of ceteris-reference reads the dump, recomputes everything from the same export and prints
# each quantity's largest relative difference. The dense kernels (P x P per good) and the solve
# from the data state are compared on the fixture; on the pack the equilibrium is solved with the
# pack's calibrated inputs from a perturbed reference point. Both entry points then solve the
# fixture into WORK_DIR, compared table by table, and the routing test checks the pack export's
# kernel. The export, the dumps and the solutions stay in WORK_DIR.
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

fixture="$root/tests/golden/p2-fixture"
dump "$fixture" "$work/fixture.json" --dense --solve
dump "$work/pack-export" "$work/pack.json" --reference "$pack"
for name in fixture pack; do
    echo "== $name"
    CETERIS_REFERENCE_PARITY="$work/$name.json" \
        cargo test --release --quiet -p ceteris-reference --test parity -- --nocapture
done

echo "== reference solve of the fixture"
uv run --project "$root/python" --locked python -m ceteris.reference solve \
    --inputs "$fixture" --out "$work/fixture-python"
"$root/target/release/ceteris" reference solve --inputs "$fixture" --out "$work/fixture-rust"
"$root/target/release/ceteris" pack compare "$work/fixture-python" "$work/fixture-rust" --iterative

echo "== routing on the pack"
CETERIS_PACK="$pack" cargo test --release --quiet -p ceteris-cli --test reference_cli routing \
    -- --nocapture
