#!/usr/bin/env bash
# Step 1 parity: `ceteris pack check-sources` against check_sources.py and `ceteris pack fetch`
# against `python -m ceteris.packs.fetch`: same stdout, stderr and exit code on the real files
# and cache and on mutated copies of the configuration. check_sources.py is deleted from the
# tree, so it is restored from the commit before the port; the Python fetch module is deleted
# too, so the fetch cases run in a tree of the package before its deletion (`packs_tree.sh`).
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
cargo build --release --bin ceteris
bin="$root/target/release/ceteris"
packs=python/ceteris/packs
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
source "$root/scripts/parity/packs_tree.sh"
packs_tree "$root" "$work/tree"

git show 6ebe94a^:python/ceteris/packs/check_sources.py >"$work/check_sources.py"
mkdir -p "$work/real/python/ceteris"
cp -R "$packs" "$work/real/python/ceteris/packs"
rm -rf "$work/real/python/ceteris/packs/__pycache__"

# run_case DIR NAME PROFILE: DIR is a checkout-shaped tree holding $packs.
run_case() {
    local dir=$1 name=$2 profile=$3 py rs
    cp "$work/check_sources.py" "$dir/$packs/check_sources.py"
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
    run_case "$work/real" real "$profile"
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

# run_fetch DIR NAME [FLAGS]: DIR is a checkout-shaped tree with python/ceteris and a data link.
run_fetch() {
    local dir=$1 name=$2 py rs
    shift 2
    py=$(cd "$dir" && PYTHONPATH="$dir/python" uv run --project "$root/python" python -m ceteris.packs.fetch "$@" 2>"$work/py.err" && echo "exit 0" || echo "exit $?")
    rs=$(cd "$dir" && "$bin" pack fetch "$@" 2>"$work/rs.err" && echo "exit 0" || echo "exit $?")
    if diff <(cat "$work/py.err"; echo "$py") <(cat "$work/rs.err"; echo "$rs") >/dev/null; then
        echo "ok   fetch $name $* ($(echo "$py" | tail -1))"
    else
        echo "FAIL fetch $name $*"
        diff <(cat "$work/py.err"; echo "$py") <(cat "$work/rs.err"; echo "$rs") || true
        failed=1
    fi
}

run_fetch "$work/tree" real
run_fetch "$work/tree" real --record

# Each mutation edits fetch.toml in a copy of the tree and prints nothing.
python3 -I - "$work/fetch" "$root/data" "$work/tree/python/ceteris" <<'PY'
import re, shutil, sys
from pathlib import Path

out, data, src = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
toml = (src / "packs/fetch.toml").read_text()
first = re.search(r"\[\[file\]\]\n(?:.*\n)+?\n", toml[toml.index("[[file]]"):]).group(0)
manual = next(b for b in re.split(r"\n(?=\[\[file\]\])", toml) if "manual = true" in b and "optional" not in b and "sha256" in b)
cases = {
    "unpinned": toml.replace(first, re.sub(r'(sha256|bytes) = .*\n', "", first, count=2), 1),
    "wrong-sha": toml.replace(first, re.sub(r'sha256 = "..', 'sha256 = "00', first, count=1), 1),
    "wrong-bytes": toml.replace(first, re.sub(r"bytes = \d+", "bytes = 1", first, count=1), 1),
    "missing-manual": toml.replace(manual, re.sub(r'name = "(.*)"', r'name = "absent-\1"', manual, count=1), 1),
    "absent-optional": toml + '\n[[file]]\nsource = "' + re.search(r'source = "(.*)"', first).group(1) + '"\nname = "absent.bin"\noptional = true\nurl = "http://127.0.0.1:9/x"\n',
    "unknown-source": toml + '\n[[file]]\nsource = "nowhere"\nname = "x"\nurl = "u"\n',
}
for name, body in cases.items():
    case = out / name
    (case / "python").mkdir(parents=True)
    shutil.copytree(src, case / "python/ceteris", ignore=shutil.ignore_patterns("__pycache__"))
    (case / "data").symlink_to(data.resolve())
    (case / "python/ceteris/packs/fetch.toml").write_text(body)
PY

for case in "$work"/fetch/*/; do
    for flags in "" "--record"; do
        # shellcheck disable=SC2086
        run_fetch "${case%/}" "$(basename "$case")" $flags
    done
done
exit "$failed"
