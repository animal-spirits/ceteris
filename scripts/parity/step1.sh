#!/usr/bin/env bash
# Step 1 parity: `ceteris pack check-sources` against check_sources.py, same stdout, stderr and
# exit code, for both profiles on the real files and on mutated copies of the configuration.
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
cargo build --release --bin ceteris
bin="$root/target/release/ceteris"
packs=python/ceteris/packs
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# run_case DIR NAME PROFILE: DIR is a checkout-shaped tree holding $packs and check_sources.py.
run_case() {
    local dir=$1 name=$2 profile=$3 py rs
    py=$(cd "$dir" && python3 -I "$packs/check_sources.py" --profile "$profile" 2>"$work/py.err" && echo "exit 0" || echo "exit $?")
    rs=$(cd "$dir" && "$bin" pack check-sources --profile "$profile" 2>"$work/rs.err" && echo "exit 0" || echo "exit $?")
    if diff <(cat "$work/py.err"; echo "$py") <(cat "$work/rs.err"; echo "$rs") >/dev/null; then
        echo "ok   $name $profile ($(echo "$py" | tail -2 | head -1 | sed 's/.*sources, //'))"
    else
        echo "FAIL $name $profile"
        diff <(cat "$work/py.err"; echo "$py") <(cat "$work/rs.err"; echo "$rs") || true
        failed=1
    fi
}

failed=0
for profile in research release; do
    run_case "$root" real "$profile"
done

# Each mutation edits one thing in a copy and prints the case name.
python3 -I - "$root/$packs" "$work/cases" <<'PY'
import re, shutil, sys, tomllib
from pathlib import Path

src, out = Path(sys.argv[1]), Path(sys.argv[2])
manifest = (src / "sources.toml").read_text()
first = re.search(r"\[\[source\]\]\n(?:.*\n)+?\n", manifest[manifest.index("[[source]]"):]).group(0)
first_id = re.search(r'id = "(.*)"', first).group(1)
parsed = tomllib.loads(manifest)["source"]
noncommercial = next(s["id"] for s in parsed if not s["commercial_use"] and s["policy"] == "fetch")
excluded = next(s["id"] for s in parsed if s["policy"] == "exclude")
closed_only = next(s["id"] for s in parsed if s.get("closed_source_only") and s["policy"] == "fetch")


def edit_first(pattern, repl):
    return lambda m, f, v: (m.replace(first, re.sub(pattern, repl, first, count=1), 1), f, v)


def drop(field):
    return edit_first(rf"{field} = .*\n", "")


cases = {}
for field in ["id", "name", "provider", "version", "role", "licence", "licence_url", "attribution",
              "commercial_use", "share_alike", "use", "policy", "reason", "checked", "confidence"]:
    cases[f"missing-{field}"] = drop(field)
cases["wrong-type-bool"] = edit_first(r"commercial_use = .*", 'commercial_use = "yes"')
cases["wrong-type-date"] = edit_first(r"checked = .*", 'checked = "2026-10-23"')
cases["wrong-type-str"] = edit_first(r"version = .*", "version = 5")
cases["unknown-policy"] = edit_first(r"policy = .*", 'policy = "bogus"')
cases["unknown-use"] = edit_first(r"use = .*", 'use = "bogus"')
cases["rejected-not-excluded"] = edit_first(r"use = .*", 'use = "rejected"')
cases["unknown-confidence"] = edit_first(r"confidence = .*", 'confidence = "bogus"')
cases["licence-url-http"] = edit_first(r'licence_url = "https', 'licence_url = "http')
cases["shipped-noncommercial"] = edit_first(r"commercial_use = .*", "commercial_use = false")
cases["shipped-low-confidence"] = edit_first(r"confidence = .*", 'confidence = "low"')
cases["share-alike-unset"] = edit_first(r"share_alike = .*", "share_alike = true")
cases["closed-only-shipped"] = edit_first(r"checked = (.*)\n", r"checked = \1\nclosed_source_only = true\n")
cases["unknown-replaces"] = edit_first(r'replaces = .*', 'replaces = "nope"')
cases["unknown-replaced-by"] = edit_first(r"checked = (.*)\n", r'checked = \1\nreplaced_by = "nope"\n')
cases["duplicate-id"] = lambda m, f, v: (m + "\n" + first, f, v)
cases["bad-distribution"] = lambda m, f, v: (m.replace('distribution = "closed"', 'distribution = "bogus"'), f, v)
cases["no-distribution"] = lambda m, f, v: (m.replace('distribution = "closed"', ""), f, v)
cases["open-distribution"] = lambda m, f, v: (m.replace('distribution = "closed"', 'distribution = "open"'), f, v)
cases["validation-unknown"] = lambda m, f, v: (m, f, v + '\n[[file]]\nsource = "nowhere"\nname = "x"\nurl = "u"\n')
cases["validation-wrong-use"] = lambda m, f, v: (m, f, v + f'\n[[file]]\nsource = "{first_id}"\nname = "x"\nurl = "u"\n')
for label, sid in [("noncommercial", noncommercial), ("excluded", excluded), ("closed-only", closed_only), ("unknown", "nowhere")]:
    cases[f"pack-reads-{label}"] = lambda m, f, v, sid=sid: (m, f + f'\n[[file]]\nsource = "{sid}"\nname = "x"\nurl = "u"\n', v)
cases["no-county-lock"] = lambda m, f, v: (m, f, v)

for name, mutate in cases.items():
    case = out / name
    shutil.copytree(src, case / "python/ceteris/packs")
    (case / "python/ceteris/packs/__pycache__").exists() and shutil.rmtree(case / "python/ceteris/packs/__pycache__")
    m, f, v = mutate(manifest, (src / "fetch.toml").read_text(), (src / "validation.toml").read_text())
    (case / "python/ceteris/packs/sources.toml").write_text(m)
    (case / "python/ceteris/packs/fetch.toml").write_text(f)
    (case / "python/ceteris/packs/validation.toml").write_text(v)
    if name == "no-county-lock":
        (case / "python/ceteris/packs/counties.lock.toml").unlink()
PY

for case in "$work"/cases/*/; do
    for profile in research release; do
        run_case "${case%/}" "$(basename "$case")" "$profile"
    done
done
exit "$failed"
