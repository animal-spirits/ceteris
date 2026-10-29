"""Dumps PPML designs and the Python reference's fits as Arrow, for the Rust reference's
`ppml_parity` test (`scripts/parity/step4.sh` runs both).

OUT/designs/NAME.arrow is one design: `flow`, the fixed-effect codes `fe_0`, `fe_1`, ..., `cluster`
when the fit clusters, and the regressors as `x.NAME` in order; the schema metadata `kind` is `hdfe`
(`ppml.hdfe`) or `fit` (`ppml.fit_robust`, `fe_0` the exporter and `fe_1` the importer). OUT/python
holds the fits: NAME.arrow one row per regressor (`name`, `coef`, `se` and for hdfe the covariance row
`cov_0`, ...), NAME.keep.arrow the estimated rows of an hdfe fit, summary.arrow one row per design, and
seconds.json each fit's time.

With --synthetic, seeded designs covering every path; with --baci PACK, every `ppml.hdfe` call that
`gravity.estimate` makes on 2000 BACI and PACK; with --yotov ZIP, those of `gravity.yotov`.

    python step4_ppml_dump.py OUT (--synthetic | --baci PACK | --yotov ZIP)

The Python reference and calibration packages are deleted: run it from a checkout of the commit
before their deletion (`git log --diff-filter=D -- python/ceteris/reference`).
"""

import argparse
import inspect
import json
import time
from pathlib import Path

import numpy as np
import pyarrow as pa
from pyarrow import ipc

from ceteris.calibrate import gravity
from ceteris.reference import ppml

HDFE = ppml.hdfe


class Dump:
    def __init__(self, out: Path):
        self.out = out
        (out / "designs").mkdir(parents=True)
        (out / "python").mkdir()
        self.summary: list[dict] = []
        self.seconds: dict[str, float] = {}

    def write(self, path: Path, columns: dict | pa.Table, metadata: dict | None = None) -> None:
        table = columns if isinstance(columns, pa.Table) else pa.table(columns)
        if metadata:
            table = table.replace_schema_metadata(metadata)
        options = ipc.IpcWriteOptions(compression="zstd")
        with ipc.new_file(str(path), table.schema, options=options) as w:
            w.write_table(table)

    def design(self, name, kind, flows, fixed_effects, regressors, cluster=None) -> None:
        cols = {"flow": np.asarray(flows, dtype=np.float64)}
        for i, g in enumerate(fixed_effects):
            cols[f"fe_{i}"] = np.asarray(g).astype(np.int64)
        if cluster is not None:
            cols["cluster"] = np.asarray(cluster).astype(np.int64)
        for k, v in regressors.items():
            cols[f"x.{k}"] = np.asarray(v, dtype=np.float64)
        self.write(self.out / "designs" / f"{name}.arrow", cols, {"kind": kind})

    def hdfe(self, name, flows, fixed_effects, regressors, cluster=None) -> ppml.Fit:
        self.design(name, "hdfe", flows, fixed_effects, regressors, cluster)
        start = time.perf_counter()
        fit = HDFE(flows, fixed_effects, regressors, cluster)
        self.seconds[name] = time.perf_counter() - start
        k = len(fit.names)
        cols = {"name": list(fit.names), "coef": fit.coef, "se": np.sqrt(np.diag(fit.cov))}
        cols |= {f"cov_{j}": fit.cov[:, j] for j in range(k)}
        self.write(self.out / "python" / f"{name}.arrow", cols)
        self.write(self.out / "python" / f"{name}.keep.arrow", {"keep": fit.keep})
        self.summary.append(
            {"design": name, "n": fit.n, "n_separated": fit.n_separated, "n_separated_exporters": None,
             "n_separated_importers": None, "pseudo_r2": fit.pseudo_r2}
        )  # fmt: skip
        return fit

    def fit(self, name, flows, exporter, importer, regressors) -> None:
        self.design(name, "fit", flows, [exporter, importer], regressors)
        start = time.perf_counter()
        coef, se, sep, r2 = ppml._fit(flows, exporter, importer, regressors)
        self.seconds[name] = time.perf_counter() - start
        cols = {"name": list(coef), "coef": list(coef.values()), "se": list(se.values())}
        self.write(self.out / "python" / f"{name}.arrow", cols)
        self.summary.append(
            {"design": name, "n": len(flows) - sep.n_rows, "n_separated": sep.n_rows,
             "n_separated_exporters": sep.n_exporters, "n_separated_importers": sep.n_importers, "pseudo_r2": r2}
        )  # fmt: skip

    def finish(self) -> None:
        rows = sorted(self.summary, key=lambda r: r["design"])
        schema = pa.schema(
            [("design", pa.string()), ("n", pa.int64()), ("n_separated", pa.int64()),
             ("n_separated_exporters", pa.int64()), ("n_separated_importers", pa.int64()), ("pseudo_r2", pa.float64())]
        )  # fmt: skip
        self.write(self.out / "python" / "summary.arrow", pa.Table.from_pylist(rows, schema=schema))
        (self.out / "python" / "seconds.json").write_text(json.dumps(self.seconds, indent=1))


def two_way(rng, n=25):
    o, d = np.nonzero(~np.eye(n, dtype=bool))
    x1, x2 = rng.normal(size=len(o)), rng.uniform(0, 1, len(o))
    a, b = rng.normal(0, 1, n), rng.normal(0, 1, n)
    y = rng.poisson(np.exp(2.0 + a[o] + b[d] - 0.8 * x1 + 0.5 * x2)).astype(float)
    return y, o, d, {"x1": x1, "x2": x2}


def synthetic(dump: Dump) -> None:
    rng = np.random.default_rng(2026)
    y, o, d, reg = two_way(rng)
    dump.fit("fit_two_way", y, o, d, reg)
    dump.hdfe("hdfe_two_way", y, [o, d], reg)
    dump.hdfe("hdfe_one_way", y, [o], reg)
    pair = np.minimum(o, d) * 25 + np.maximum(o, d)
    dump.hdfe("hdfe_cluster", y, [o, d], reg, pair)
    dump.hdfe("hdfe_singleton_clusters", y, [o, d], reg, np.arange(len(y)))

    y, o, d, reg = two_way(rng, 30)
    y[o == 4] = 0.0  # exporter 4 never exports
    y[(d == 7) & (o != 2)] = 0.0  # importer 7 buys only from 2
    dump.fit("fit_separated", y, o, d, reg)
    dump.hdfe("hdfe_separated", y, [o, d], reg, np.minimum(o, d) * 30 + np.maximum(o, d))

    y, o, d, reg = two_way(rng, 40)
    y = rng.poisson(np.exp(-2.0 + 0.5 * reg["x2"]), len(y)).astype(float)
    assert (y == 0).mean() > 0.5
    dump.fit("fit_zeros", y, o, d, reg)
    dump.hdfe("hdfe_zeros", y, [o, d], reg)

    # Three fixed effects: exporter-year, importer-year and pair, clustered by pair.
    n, years = 20, 4
    o, d, t = (a.ravel() for a in np.meshgrid(np.arange(n), np.arange(n), np.arange(years), indexing="ij"))
    off = o != d
    o, d, t = o[off], d[off], t[off]
    pair = np.minimum(o, d) * n + np.maximum(o, d)
    a, b, p = rng.normal(0, 1, (n, years)), rng.normal(0, 1, (n, years)), rng.normal(0, 0.5, n * n)
    rta = (rng.uniform(size=n * n) < 0.3)[pair] & (t >= 2)
    x = rng.normal(size=len(o))
    y = rng.poisson(np.exp(1.0 + a[o, t] + b[d, t] + p[pair] + 0.4 * rta - 0.3 * x)).astype(float)
    reg = {"rta": rta.astype(float), "x": x}
    dump.hdfe("hdfe_three_way", y, [o * years + t, d * years + t, pair], reg, pair)
    # A regressor twice: numpy's minimum-norm step splits its coefficient evenly.
    dump.hdfe("hdfe_collinear", y, [o * years + t, d * years + t], reg | {"x_again": x}, pair)
    # A regressor constant within each exporter-year: the fixed effect absorbs it.
    dump.hdfe("hdfe_absorbed", y, [o * years + t, d * years + t], reg | {"exporter_year": (o * 0.37 + t * 1.3)}, pair)

    # Two fixed effects too large for the direct solve: alternating projections only.
    na, nb, m = 2100, 2000, 30000
    ga, gb = rng.integers(0, na, m), rng.integers(0, nb, m)
    x1 = rng.normal(size=m)
    y = rng.poisson(np.exp(1.0 + rng.normal(0, 0.5, na)[ga] + rng.normal(0, 0.5, nb)[gb] + 0.6 * x1)).astype(float)
    dump.hdfe("hdfe_large_groups", y, [ga, gb], {"x1": x1})


def recorder(dump: Dump, names: list[str] | None = None):
    calls = []

    def hdfe(flows, fixed_effects, regressors, cluster=None):
        caller = inspect.stack()[1].function
        name = names[len(calls)] if names else f"{len(calls):02d}_{caller}"
        calls.append(name)
        return dump.hdfe(name, flows, fixed_effects, regressors, cluster)

    return hdfe


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("out", type=Path)
    which = parser.add_mutually_exclusive_group(required=True)
    which.add_argument("--synthetic", action="store_true")
    which.add_argument("--baci", type=Path, help="the pack whose countries and graph the samples use")
    which.add_argument("--yotov", type=Path, help=f"the {gravity.YOTOV} of the fetch cache")
    args = parser.parse_args()
    dump = Dump(args.out)
    if args.synthetic:
        synthetic(dump)
    elif args.baci:
        from ceteris.packs import fetch

        read = gravity.dump_reader

        def dump_reader(name):
            # The reader also keys EU aggregate lines ("12EU20"), which no BACI code matches and
            # on which gravity.baci_flows' int() fails.
            out = read(name)
            return {k: v for k, v in out.items() if k.isdigit()} if name == "hs-goods" else out

        gravity.dump_reader = dump_reader
        ppml.hdfe = recorder(dump)
        gravity.estimate(args.baci, fetch.CACHE)
    else:
        ppml.hdfe = recorder(dump, list(gravity.YOTOV_PUBLISHED))
        gravity.yotov(args.yotov)
    dump.finish()


if __name__ == "__main__":
    main()
