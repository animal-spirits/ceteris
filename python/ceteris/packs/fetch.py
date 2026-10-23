"""Download pack sources into the cache and verify their checksums (Section 16.1).

uv run python -m ceteris.packs.fetch [--record]
"""

import argparse
import hashlib
import sys
import tomllib
import urllib.request
from pathlib import Path

FETCH = Path(__file__).with_name("fetch.toml")
SOURCES = Path(__file__).with_name("sources.toml")
CACHE = Path(__file__).resolve().parents[3] / "data" / "cache"


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        while chunk := f.read(1 << 20):
            h.update(chunk)
    return h.hexdigest()


def entries() -> list[dict]:
    files = tomllib.loads(FETCH.read_text())["file"]
    known = {s["id"] for s in tomllib.loads(SOURCES.read_text())["source"]}
    unknown = [f["name"] for f in files if f["source"] not in known]
    if unknown:
        raise SystemExit(f"fetch.toml names sources missing from sources.toml: {unknown}")
    return files


def path_of(name: str) -> Path:
    return CACHE / name


def fetch(entry: dict) -> Path:
    path = path_of(entry["name"])
    if not path.exists():
        CACHE.mkdir(parents=True, exist_ok=True)
        part = path.with_suffix(path.suffix + ".part")
        print(f"fetching {entry['url']}", file=sys.stderr)
        request = urllib.request.Request(entry["url"], headers={"User-Agent": "ceteris-packs"})
        with urllib.request.urlopen(request) as r, part.open("wb") as f:
            while chunk := r.read(1 << 20):
                f.write(chunk)
        part.rename(path)
    return path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--record", action="store_true", help="print checksums for unpinned files")
    args = parser.parse_args()
    bad = 0
    for entry in entries():
        path = fetch(entry)
        digest, size = sha256(path), path.stat().st_size
        if "sha256" not in entry:
            if args.record:
                print(f'{entry["name"]}\nsha256 = "{digest}"\nbytes = {size}\n')
            else:
                print(f"{entry['name']}: not pinned (run with --record)", file=sys.stderr)
                bad += 1
        elif (digest, size) != (entry["sha256"], entry["bytes"]):
            print(f"{entry['name']}: checksum mismatch, got {digest} ({size} bytes)", file=sys.stderr)
            bad += 1
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
