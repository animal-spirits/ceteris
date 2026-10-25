"""`python -m ceteris.reference solve --inputs DIR --out DIR`: solves the GE reference of a
`ceteris export` and writes it with the column layout of the run export (`prices`, `wages`,
`enterprises`, `flows`, `home`) plus `solution.json`.

`python -m ceteris.reference calibrate --pack DIR [--out OUT]`: exports the pack's world with
`cargo run --release --bin ceteris -- export --pack DIR` to a temporary directory, calibrates the
data state as the steady state (`calibrate.calibrate`, ADR 0089) at the pack's u* and writes the
calibration tables (`calibrate.write`) into OUT (default DIR), printing the summary. With
`--inputs EXPORT` instead of `--pack` it calibrates an export directory (a generated world: its
2000 unemployment rates serve as u*, its provinces as their own pack provinces)."""

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
import pyarrow as pa
from pyarrow import ipc
from scipy.optimize import NoConvergence

from . import calibrate, io, solve
from .model import Equilibrium

FORMAT = "1"
ROOT = Path(__file__).resolve().parents[3]


def _write(path: Path, columns: dict[str, pa.Array]) -> None:
    table = pa.table(columns).replace_schema_metadata({"reference_format": FORMAT})
    with pa.OSFile(str(path), "wb") as f, ipc.new_file(f, table.schema) as w:
        w.write_table(table)


def write(out: Path, inp: io.Inputs, eq: Equilibrium) -> None:
    out.mkdir(parents=True, exist_ok=True)
    n_p, n_g = inp.n_provinces, inp.n_goods
    cell_p, cell_g = np.divmod(np.arange(n_p * n_g), n_g)
    _write(
        out / "prices.arrow",
        {
            "province": pa.array(cell_p, pa.uint32()),
            "good": pa.array(cell_g, pa.uint8()),
            "price": pa.array(eq.price.ravel(), pa.float64()),
            "active": pa.array(inp.active.ravel(), pa.bool_()),
        },
    )
    _write(out / "wages.arrow", {"province": pa.array(np.arange(n_p), pa.uint32()), "wage": pa.array(eq.wage, pa.float64())})
    _write(
        out / "enterprises.arrow",
        {
            "enterprise": pa.array(np.arange(len(eq.output)), pa.uint32()),
            "province": pa.array(inp.ent_province, pa.uint32()),
            "good": pa.array(inp.ent_good, pa.uint8()),
            "employment": pa.array(eq.employment, pa.float64()),
            "output": pa.array(eq.output, pa.float64()),
        },
    )
    ex, im, g = np.nonzero(eq.flows_by_country_pair_good > 0)
    _write(
        out / "flows.arrow",
        {
            "exporter_country": pa.array(ex, pa.uint32()),
            "importer_country": pa.array(im, pa.uint32()),
            "good": pa.array(g, pa.uint8()),
            "value": pa.array(eq.flows_by_country_pair_good[ex, im, g], pa.float64()),
            "units": pa.array(eq.units_by_country_pair_good[ex, im, g], pa.float64()),
        },
    )
    traded = inp.traded[cell_g]
    _write(
        out / "home.arrow",
        {
            "province": pa.array(cell_p[traded], pa.uint32()),
            "good": pa.array(cell_g[traded], pa.uint8()),
            "value": pa.array(eq.home_value.ravel()[traded], pa.float64()),
            "units": pa.array(eq.home_units.ravel()[traded], pa.float64()),
        },
    )
    # no timing, so a re-solve writes the same files
    info = {k: v for k, v in eq.info.items() if k != "seconds"}
    summary = {"format": FORMAT, "static_digest": inp.static_digest, "price_index": eq.price_index, **info}
    (out / "solution.json").write_text(json.dumps(summary, indent=2) + "\n")


def run_calibration(inputs: Path, u_star, pack_province, out: Path) -> None:
    inp = io.load(inputs)
    if u_star is None:
        u_star = inp.unemployment
    if pack_province is None:
        pack_province = np.arange(inp.n_provinces)
    try:
        s = calibrate.calibrate(inp, u_star, progress=lambda line: print(line, flush=True))
    except ValueError as e:
        sys.exit(f"no calibration: {e}")
    calibrate.write(out, inp, s, pack_province)
    print(json.dumps(calibrate.summary(inp, s), indent=2))
    print(f"wrote {out}")


def calibrate_pack(pack: Path, out: Path) -> None:
    with tempfile.TemporaryDirectory() as tmp:
        cmd = ["cargo", "run", "--release", "--bin", "ceteris", "--", "export", "--pack", str(pack.resolve()), "--out", tmp]
        subprocess.run(cmd, check=True, cwd=ROOT)
        pack_province = io._table(Path(tmp) / "pack_provinces.arrow")["pack"].astype(np.int64)
        run_calibration(Path(tmp), calibrate.read_u_star(pack), pack_province, out)


def main() -> None:
    parser = argparse.ArgumentParser(prog="python -m ceteris.reference")
    sub = parser.add_subparsers(dest="command", required=True)
    s = sub.add_parser("solve", help="solve the GE reference of an export")
    s.add_argument("--inputs", type=Path, required=True)
    s.add_argument("--out", type=Path, required=True)
    c = sub.add_parser("calibrate", help="calibrate the 2000 data state as the steady state (ADR 0089)")
    source = c.add_mutually_exclusive_group(required=True)
    source.add_argument("--pack", type=Path)
    source.add_argument("--inputs", type=Path, help="an export directory of a generated world")
    c.add_argument("--out", type=Path, help="directory of the calibration tables (default: PACK)")
    args = parser.parse_args()
    if args.command == "calibrate":
        if args.pack:
            calibrate_pack(args.pack, args.out or args.pack)
        elif args.out is None:
            parser.error("--inputs needs --out")
        else:
            run_calibration(args.inputs, None, None, args.out)
        return
    inp = io.load(args.inputs)
    try:
        eq = solve.equilibrium(inp)
    except (NoConvergence, ValueError) as e:
        sys.exit(f"no equilibrium: {e}")
    write(args.out, inp, eq)
    print(json.dumps(eq.info, indent=2))


if __name__ == "__main__":
    main()
