"""Compares two attribute files of the map (`web/provinces.json`, `web/counties.json`).

    python scripts/parity/json_diff.py A B

Exit 0 when the bytes are identical. Otherwise lists, per key (`countries.<key>` inside the
countries object), the rows whose values differ, with both values, and exits 2; any other
failure (a missing or unreadable file) exits 1.
"""

import json
import sys
from pathlib import Path


def columns(d: dict, prefix: str = "") -> dict:
    out = {}
    for k, v in d.items():
        if isinstance(v, dict):
            out |= columns(v, f"{prefix}{k}.")
        else:
            out[prefix + k] = v
    return out


def main(a: Path, b: Path) -> int:
    ta, tb = a.read_bytes(), b.read_bytes()
    if ta == tb:
        print(f"ok   {a.name} bytes identical")
        return 0
    ca, cb = columns(json.loads(ta)), columns(json.loads(tb))
    print(f"FAIL {a.name}: bytes differ")
    if list(ca) != list(cb):
        print(f"  keys: {list(ca)} vs {list(cb)}")
    for k in ca.keys() & cb.keys():
        va, vb = ca[k], cb[k]
        if len(va) != len(vb):
            print(f"  {k}: {len(va)} vs {len(vb)} rows")
            continue
        rows = [(i, x, y) for i, (x, y) in enumerate(zip(va, vb)) if x != y or type(x) is not type(y)]
        if rows:
            shown = ", ".join(f"{i}: {x!r} vs {y!r}" for i, x, y in rows[:10])
            print(f"  {k}: {len(rows)} rows differ ({shown}{', ...' if len(rows) > 10 else ''})")
    return 2


if __name__ == "__main__":
    sys.exit(main(Path(sys.argv[1]), Path(sys.argv[2])))
