"""Compares two pack manifests (`manifest.json`) for the step 3 build parity.

    python scripts/parity/manifest_diff.py A B [--listed]

`pack_format`, `year`, `config` and `sources` must be equal, keys in order. `tables` must name
the same files, with equal digests for the tile layers' GeoJSON (`web/map.pmtiles:<layer>`) and
the attribute files (`web/*.json`); the Arrow tables' digests are listed (pyarrow and arrow-rs
write different bytes), the fills' too (`fills.py` compares them decoded). `stats` must be equal,
keys in order, but for `fills`, which is listed (earcut's triangle order). With --listed, the
layer and attribute digests that differ are listed, not failed.

Exit 0 when everything required is equal, 2 when something required differs, 1 on any other
failure (a missing or unreadable file).
"""

import json
import sys
from pathlib import Path


def text(x) -> str:
    return json.dumps(x, indent=2)


def main(a: Path, b: Path, listed: bool) -> int:
    ma, mb = json.loads(a.read_text()), json.loads(b.read_text())
    failed = False

    def report(ok: bool, what: str, required: bool = True) -> None:
        nonlocal failed
        if ok:
            print(f"ok   manifest {what}")
        elif required:
            print(f"FAIL manifest {what}")
            failed = True
        else:
            print(f"note manifest {what}")

    if list(ma) != list(mb):
        report(False, f"keys: {list(ma)} vs {list(mb)}")
    for key in ("pack_format", "year", "config", "sources"):
        report(text(ma.get(key)) == text(mb.get(key)), key)

    ta, tb = ma["tables"], mb["tables"]
    report(list(ta) == list(tb), f"tables names ({len(ta)} vs {len(tb)})")
    for name in sorted(ta.keys() & tb.keys()):
        same = ta[name] == tb[name]
        if name.startswith("web/map.pmtiles:") or name.endswith(".json"):
            report(same, f"tables {name}", required=not listed)
        elif not same:
            print(f"note manifest tables {name}: digests differ ({ta[name][:12]} vs {tb[name][:12]})")
        else:
            print(f"ok   manifest tables {name}")

    sa, sb = dict(ma["stats"]), dict(mb["stats"])
    fa, fb = sa.pop("fills", None), sb.pop("fills", None)
    report(text(sa) == text(sb), "stats but fills")
    if text(sa) != text(sb):
        for k in sa.keys() | sb.keys():
            if text(sa.get(k)) != text(sb.get(k)):
                print(f"  {k}: {sa.get(k)!r} vs {sb.get(k)!r}")
    report(fa == fb, f"stats fills: {fa} vs {fb}", required=False)
    return 2 if failed else 0


if __name__ == "__main__":
    args = sys.argv[1:]
    listed = "--listed" in args
    paths = [Path(x) for x in args if x != "--listed"]
    sys.exit(main(paths[0], paths[1], listed))
