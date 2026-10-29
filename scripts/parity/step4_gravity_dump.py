"""Dumps what the Python gravity estimation derives, for the `gravity_parity` test of
ceteris-calibrate (`scripts/parity/step4.sh` runs both).

OUT/gravity.txt is what `python -m ceteris.calibrate.gravity --pack PACK` prints (report, notes,
stop rules); OUT/gravity_2000.toml the estimates as `to_toml` writes them, whether or not a stop
rule fires; OUT/estimates.json every field of `gravity.estimate` at full precision;
OUT/country_costs.arrow `country_costs(PACK)` one row per (exporter, importer, good) with
`reach`; OUT/yotov.json the fits of `gravity.yotov` (names, coef, se, n, n_separated,
pseudo_r2); OUT/seconds.json the time of the estimate and of the Yotov fits.

gravity.baci_flows calls int() on every key of `ceteris pack dump-reader hs-goods`, which also
keys the EU aggregate lines ("12EU20"); they match no BACI line, so the wrapper leaves them out.

    python step4_gravity_dump.py OUT --pack PACK --yotov ZIP
"""

import argparse
import contextlib
import dataclasses
import io
import json
import time
from pathlib import Path

import numpy as np
import pyarrow as pa
from pyarrow import ipc

from ceteris.calibrate import gravity
from ceteris.packs import fetch


def plain(x):
    if dataclasses.is_dataclass(x):
        return {f.name: plain(getattr(x, f.name)) for f in dataclasses.fields(x)}
    if isinstance(x, dict):
        return {k: plain(v) for k, v in x.items()}
    if isinstance(x, (list, tuple, np.ndarray)):
        return [plain(v) for v in x]
    if isinstance(x, (np.integer, int)) and not isinstance(x, bool):
        return int(x)
    if isinstance(x, (np.bool_, bool)):
        return bool(x)
    return float(x)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("out", type=Path)
    parser.add_argument("--pack", type=Path, required=True)
    parser.add_argument("--yotov", type=Path, required=True)
    args = parser.parse_args()
    args.out.mkdir(parents=True)

    read = gravity.dump_reader

    def dump_reader(name):
        out = read(name)
        return {k: v for k, v in out.items() if k.isdigit()} if name == "hs-goods" else out

    gravity.dump_reader = dump_reader
    costs_of = gravity.country_costs
    recorded = {}

    def country_costs(pack, p=gravity.P04):
        recorded["costs"] = costs_of(pack, p)
        return recorded["costs"]

    gravity.country_costs = country_costs
    seconds = {}
    start = time.perf_counter()
    est = gravity.estimate(args.pack, fetch.CACHE)
    seconds["estimate"] = time.perf_counter() - start

    text = io.StringIO()
    with contextlib.redirect_stdout(text):
        print(gravity.report(est))
        print(json.dumps(est.notes, indent=2))
        for f in gravity.stop_rules(est):
            print(f"stop rule: {f}")
    (args.out / "gravity.txt").write_text(text.getvalue())
    (args.out / "gravity_2000.toml").write_text(gravity.to_toml(est))
    (args.out / "estimates.json").write_text(json.dumps(plain(est), indent=1))

    costs = recorded["costs"]
    nc = costs.reach.shape[0]
    o, d, g = (a.ravel() for a in np.meshgrid(np.arange(nc), np.arange(nc), np.arange(gravity.N_TRADED), indexing="ij"))
    cols = {"exporter": o.astype(np.int64), "importer": d.astype(np.int64), "good": g.astype(np.int64)}
    cols |= {f"q_{k}": costs.q[o, d, g, k] for k in range(gravity.N_Q)}
    cols["reach"] = costs.reach[o, d]
    table = pa.table(cols)
    with ipc.new_file(str(args.out / "country_costs.arrow"), table.schema) as w:
        w.write_table(table)

    start = time.perf_counter()
    fits = gravity.yotov(args.yotov)
    seconds["yotov"] = time.perf_counter() - start
    yotov = {
        name: {"names": list(f.names), "coef": plain(f.coef), "se": plain(np.sqrt(np.diag(f.cov))), "n": int(f.n),
               "n_separated": int(f.n_separated), "pseudo_r2": float(f.pseudo_r2)}
        for name, f in fits.items()
    }  # fmt: skip
    (args.out / "yotov.json").write_text(json.dumps(yotov, indent=1))
    (args.out / "seconds.json").write_text(json.dumps(seconds, indent=1))


if __name__ == "__main__":
    main()
