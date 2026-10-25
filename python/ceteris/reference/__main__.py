"""`python -m ceteris.reference solve --inputs DIR --out DIR`: solves the GE reference of a
`ceteris export` and writes it with the column layout of the run export (`prices`, `wages`,
`enterprises`, `flows`, `home`) plus `solution.json`.

`python -m ceteris.reference calibrate --pack DIR [--out FILE] [--budget SECONDS]`: exports the pack's world with
`cargo run --release --bin ceteris -- export --pack DIR` to a temporary directory, calibrates
productivity (`calibrate.productivity`) and writes `DIR/econ_calib.arrow` (or FILE), printing the
residual summary; progress lines follow the solve, which gives up after the budget (2 hours)."""

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


def run_calibration(pack: Path, out: Path, budget: float) -> None:
    with tempfile.TemporaryDirectory() as tmp:
        cmd = ["cargo", "run", "--release", "--bin", "ceteris", "--", "export", "--pack", str(pack.resolve()), "--out", tmp]
        subprocess.run(cmd, check=True, cwd=ROOT)
        inp = io.load(tmp)
        pack_province = io._table(Path(tmp) / "pack_provinces.arrow")["pack"].astype(np.int64)
    try:
        a, residuals = calibrate.productivity(inp, progress=lambda line: print(line, flush=True), budget=budget)
    except (NoConvergence, ValueError) as e:
        sys.exit(f"no calibration: {e}")
    calibrate.write(out, inp, pack_province, a, residuals)
    ln_a = np.log(a)
    summary = {
        "residual_count": len(residuals),
        "largest_relative_miss": residuals.largest_miss,
        "ln_a_quantiles": dict(
            zip(
                ["0", "1", "5", "25", "50", "75", "95", "99", "100"],
                np.percentile(ln_a, [0, 1, 5, 25, 50, 75, 95, 99, 100]).tolist(),
            )
        ),
        **residuals.info,
    }
    print(json.dumps(summary, indent=2))
    print(f"wrote {out}")


def main() -> None:
    parser = argparse.ArgumentParser(prog="python -m ceteris.reference")
    sub = parser.add_subparsers(dest="command", required=True)
    s = sub.add_parser("solve", help="solve the GE reference of an export")
    s.add_argument("--inputs", type=Path, required=True)
    s.add_argument("--out", type=Path, required=True)
    c = sub.add_parser("calibrate", help="calibrate productivity against the pack's 2000 value added")
    c.add_argument("--pack", type=Path, required=True)
    c.add_argument("--out", type=Path, help="default: PACK/econ_calib.arrow")
    c.add_argument("--budget", type=float, default=7200.0, help="wall-clock seconds before giving up (default 2 hours)")
    args = parser.parse_args()
    if args.command == "calibrate":
        run_calibration(args.pack, args.out or args.pack / "econ_calib.arrow", args.budget)
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
