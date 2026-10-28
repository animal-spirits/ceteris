#!/usr/bin/env bash
# Checks the exit status of floatdiff.py on hand-made JSON pairs.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
failed=0

# expect STATUS TOLERANCE PYTHON_JSON RUST_JSON NAME
expect() {
    printf '%s' "$3" >"$work/py.json"
    printf '%s' "$4" >"$work/rs.json"
    local got=0
    python3 -I "$here/floatdiff.py" "$work/py.json" "$work/rs.json" "$2" >/dev/null || got=$?
    if [ "$got" = "$1" ]; then
        echo "ok   $5"
    else
        echo "FAIL $5: exit $got, expected $1"
        failed=1
    fi
}

expect 0 0 '{"a": [1.5, 2]}' '{"a": [1.5, 2]}' "identical passes at tolerance 0"
expect 0 1e-12 '{"a": [1.5, 2]}' '{"a": [1.5, 2]}' "identical passes at 1e-12"
expect 1 0 '{"a": 1.0, "b": 2}' '{"b": 2, "a": 1.0}' "a byte-only difference fails at tolerance 0"
expect 1 0 '{"a": 1.0}' '{"a": 1.0000000000000002}' "any float difference fails at tolerance 0"
expect 0 1e-12 '{"a": 1.0}' '{"a": 1.0000000000000002}' "a 1e-16 relative difference passes at 1e-12"
expect 1 1e-12 '{"a": 1.0}' '{"a": 1.000000001}' "a 1e-9 relative difference fails at 1e-12"
expect 1 1e-12 '{"a": NaN}' '{"a": 1.0}' "NaN against a number fails at 1e-12"
expect 1 1e-12 '{"a": 1.0}' '{"a": NaN}' "a number against NaN fails at 1e-12"
expect 0 1e-12 '{"a": NaN, "b": 1.0}' '{"a": NaN, "b": 1.0000000000000002}' "NaN against NaN passes at 1e-12"
expect 1 1e-12 '{"a": 1.0}' '{"a": 1}' "a float against an integer fails"
expect 1 1e-12 '{"a": [1.0]}' '{"a": {"0": 1.0}}' "a list against an object fails"
expect 1 1e-12 '{"a": [1.0]}' '{"a": [1.0, 2.0]}' "a length difference fails"
expect 1 1e-12 '{"a": 1.0}' '{"b": 1.0}' "a key difference fails"
exit "$failed"
