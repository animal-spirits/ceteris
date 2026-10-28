#!/usr/bin/env bash
# Step 3 web parity: `simplify`, the countries layer, `write_geojson`, `write_tiles` and the fills
# of build.build against `ceteris_pack::web`, both on GEOS 3.15 (see README.md for the shapely
# build). `web_inputs.py` builds the provinces and clipped counties in Python and writes their
# vertices; both sides rebuild the geometries from those files and write the web outputs:
# `web_outputs.py` in Python, `ceteris pack dump-web DIR` in Rust.
#
# Exact: each layer's GeoJSON byte for byte, the simplified vertices of every layer (`pack
# compare` in file row order), the digests the manifest records and the fill stats; the fill
# files byte for byte, or else by `fills.py`'s rule. The PMTiles archives are compared for
# information only: tippecanoe writes the same tiles in a varying order.
#
# GEOS_PYTHON names a Python whose shapely is built on GEOS 3.15 and has the packages of
# python/pyproject.toml. WORK, if given, keeps the outputs there (a new directory).
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
: "${GEOS_PYTHON:?set GEOS_PYTHON to a Python with shapely on GEOS 3.15}"
"$GEOS_PYTHON" -c 'import shapely, sys; v = shapely.geos_version_string; print(f"shapely on GEOS {v}"); sys.exit(not v.startswith("3.15."))'
echo "Rust on GEOS $(geos-config --version)"
cargo build --release --bin ceteris --features ceteris-cli/geos
bin="$root/target/release/ceteris"
if (($# > 0)); then
    work="$1"
    mkdir "$work"
else
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
fi
py() {
    PYTHONPATH="$root/python:$root/scripts/parity" "$GEOS_PYTHON" -P -s "$@"
}

py "$root/scripts/parity/web_inputs.py" "$work"
py "$root/scripts/parity/web_outputs.py" "$work"
"$bin" pack dump-web "$work"
status=0

for name in provinces countries counties; do
    if cmp -s "$work/python/$name.geojson" "$work/rust/$name.geojson"; then
        echo "ok   $name.geojson bytes identical"
    else
        echo "FAIL $name.geojson differs"
        status=1
    fi
done

code=0
"$bin" pack compare "$work/python" "$work/rust" >"$work/compare" || code=$?
cat "$work/compare"
if [[ ! -s "$work/compare" ]] || ((code != 0)); then
    echo "FAIL pack compare of the simplified vertices (exit $code)"
    status=1
fi

fills_equal=1
for name in provinces counties; do
    if cmp -s "$work/python/$name.fill" "$work/rust/$name.fill"; then
        echo "ok   $name.fill bytes identical"
    else
        fills_equal=0
        py "$root/scripts/parity/fills.py" "$work/python/$name.fill" "$work/rust/$name.fill" \
            "$work/rust/$name.simple.arrow" || {
            echo "FAIL $name.fill"
            status=1
        }
    fi
done

# web.json: the layer digests and the simplify paths must be equal; the fill digests and stats
# too when the fill bytes are.
"$GEOS_PYTHON" -I - "$work/python/web.json" "$work/rust/web.json" "$fills_equal" <<'PY' || status=1
import json, sys
a, b = (json.loads(open(p).read()) for p in sys.argv[1:3])
fills_equal = sys.argv[3] == "1"
print(f"simplify paths: python {a['simplify']}, rust {b['simplify']}")
bad = [k for k in sorted(a["hashes"].keys() | b["hashes"].keys())
       if a["hashes"].get(k) != b["hashes"].get(k) and (fills_equal or not k.endswith(".fill"))]
if a["simplify"] != b["simplify"]:
    bad.append("simplify")
if a["fills"] != b["fills"]:
    print(f"fill stats: python {a['fills']}, rust {b['fills']}")
    if fills_equal:
        bad.append("fills")
for k in bad:
    print(f"FAIL web.json {k}")
print("ok   web.json" if not bad else "FAIL web.json")
sys.exit(1 if bad else 0)
PY

if cmp -s "$work/python/map.pmtiles" "$work/rust/map.pmtiles"; then
    echo "note map.pmtiles bytes identical"
else
    echo "note map.pmtiles bytes differ (tippecanoe's order varies; not a parity criterion)"
fi
exit $status
