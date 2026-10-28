"""Compares two canonical JSON outputs of one reader (Python, Rust) to a relative float tolerance.

    python3 -I floatdiff.py PYTHON_JSON RUST_JSON TOLERANCE

Prints the first structural difference and the largest relative float difference. Exits 0 when the
files are byte-identical, or when TOLERANCE is above 0, the structures match and every float pair
lies within it (NaN matches only NaN, an infinity only the same infinity); otherwise 1. At
tolerance 0 only identical bytes pass.
"""

import json
import math
import sys
from pathlib import Path


def main() -> None:
    py_path, rs_path, tol = Path(sys.argv[1]), Path(sys.argv[2]), float(sys.argv[3])
    if py_path.read_bytes() == rs_path.read_bytes():
        print("     identical")
        sys.exit(0)
    py, rs = json.loads(py_path.read_text()), json.loads(rs_path.read_text())
    worst = [0.0, None]

    def walk(a, b, path):
        if isinstance(a, float) and isinstance(b, float):
            if a != b and not (math.isnan(a) and math.isnan(b)):
                finite = math.isfinite(a) and math.isfinite(b)
                rel = abs(a - b) / max(abs(a), abs(b)) if finite else math.inf
                if rel > worst[0]:
                    worst[:] = [rel, path]
            return None
        if type(a) is not type(b):
            return f"{path}: {a!r:.80} vs {b!r:.80}"
        if isinstance(a, dict):
            if a.keys() != b.keys():
                only_py, only_rs = sorted(a.keys() - b.keys())[:5], sorted(b.keys() - a.keys())[:5]
                return f"{path}: keys only in python {only_py}, only in rust {only_rs}"
            return next((d for k in a if (d := walk(a[k], b[k], f"{path}/{k}"))), None)
        if isinstance(a, list):
            if len(a) != len(b):
                return f"{path}: {len(a)} items in python, {len(b)} in rust"
            return next((d for i, (x, y) in enumerate(zip(a, b)) if (d := walk(x, y, f"{path}/{i}"))), None)
        return None if a == b else f"{path}: {a!r:.80} vs {b!r:.80}"

    first = walk(py, rs, "")
    print(f"     first structural difference: {first}" if first else "     same structure")
    print(f"     largest relative float difference: {worst[0]:.3e} at {worst[1]}")
    if tol == 0:
        print("     bytes differ at tolerance 0")
    sys.exit(0 if tol > 0 and not first and worst[0] <= tol else 1)


if __name__ == "__main__":
    main()
