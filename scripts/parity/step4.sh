#!/usr/bin/env bash
# Step 4 parity: the reference's inputs, PIGL demand, dense kernels, balanced Sinkhorn, A.3 bound,
# model evaluation and equilibrium against the Python reference on the fixture export and on an
# export of the pack. The Python side dumps what it derives (step4_dump.py); the Rust `parity`
# test of ceteris-reference reads the dump, recomputes everything from the same export and prints
# each quantity's largest relative difference. The dense kernels (P x P per good) and the solve
# from the data state are compared on the fixture; on the pack the equilibrium is solved with the
# pack's calibrated inputs from a perturbed reference point. Both entry points then solve the
# fixture into WORK_DIR, compared table by table, and the routing test checks the pack export's
# kernel. PPML: step4_ppml_dump.py dumps seeded synthetic designs, every hdfe design
# gravity.estimate builds on 2000 BACI and the pack, and those of gravity.yotov, with the Python
# fits; the `ppml_parity` test fits each in Rust, writes the fits next to Python's and compares
# them (1e-9 relative; the mixed form where a value is rounding noise around 0, see the test).
# `pack compare --iterative` then compares the Yotov fits, which have no such value. Calibration:
# both `calibrate` entry points calibrate the fixture (--inputs) and the pack (--pack, timed),
# their six tables compared with `pack compare --iterative` and the metadata and printed summary
# by the `parity` test of ceteris-calibrate; the `steady_state` tests then run on the pack
# export, loading the Rust tables into `EconInputs::from_pack` from a linked copy of the pack.
# The export, the dumps, the solutions, the fits and the calibrations stay in WORK_DIR.
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

ppml_dump() {
    uv run --project "$root/python" --locked python "$root/scripts/parity/step4_ppml_dump.py" "$@"
}

ppml_dump "$work/ppml-synthetic" --synthetic
ppml_dump "$work/ppml-baci" --baci "$pack"
ppml_dump "$work/ppml-yotov" --yotov "$root/data/cache/AdvancedGuideFiles.zip"
for name in synthetic baci yotov; do
    echo "== ppml $name"
    CETERIS_PPML_PARITY="$work/ppml-$name" \
        cargo test --release --quiet -p ceteris-reference --test ppml_parity -- --nocapture
done
echo "== ppml yotov: pack compare"
"$root/target/release/ceteris" pack compare "$work/ppml-yotov/python" "$work/ppml-yotov/rust" \
    --iterative

calibrate_py() {
    uv run --project "$root/python" --locked python -m ceteris.reference calibrate "$@"
}

for name in fixture pack; do
    dir="$work/calibration-$name"
    mkdir "$dir"
    if [ "$name" = fixture ]; then
        source=(--inputs "$fixture")
    else
        source=(--pack "$pack")
    fi
    echo "== calibration of the $name"
    start=$SECONDS
    calibrate_py "${source[@]}" --out "$dir/python" >"$dir/python.txt"
    echo "python: $((SECONDS - start)) s"
    start=$SECONDS
    "$root/target/release/ceteris" calibrate "${source[@]}" --out "$dir/rust" >"$dir/rust.txt"
    echo "rust: $((SECONDS - start)) s"
    "$root/target/release/ceteris" pack compare "$dir/python" "$dir/rust" --iterative
    CETERIS_CALIBRATION_PARITY="$dir" \
        cargo test --release --quiet -p ceteris-calibrate --test parity -- --nocapture
done

echo "== steady state on the pack, the Rust tables loaded by EconInputs::from_pack"
CETERIS_PACK="$pack" CETERIS_PACK_EXPORT="$work/pack-export" \
    cargo test --release --quiet -p ceteris-calibrate --test steady_state -- --nocapture
