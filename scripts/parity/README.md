# Parity scripts

One script per migration step, `scripts/parity/<step>.sh`, runs the Python implementation and the
Rust port into two directories and compares them:

```sh
cargo run --release --bin ceteris -- pack compare PYTHON_DIR RUST_DIR [--iterative] [--table NAME]
```

The comparison reads every `*.arrow` file in both directories. Integer, string, boolean and
categorical columns must be equal; float columns must agree to a relative difference of 1e-12 per
element, or 1e-9 with `--iterative` where an iterative solve sets the value. Each column prints
one line with its largest relative difference, and the exit status is 1 when anything differs.

Rows are compared in file order, so a port must reproduce the row order of the Python table.

`step2.sh` compares the seven econ tables of `python -m ceteris.packs.econ.build` and `ceteris
pack build --econ-only`, built from the same pack and cache: the same files, schemas (columns,
types, nullability, metadata), summaries and stats, and `pack compare` on every table in file row
order. It then does the same with each optional source the build reads (the two GEM trackers, the
Kummu GeoTIFF) left out of the cache; the Rust side reads that cache through a checkout-shaped
tree of links. The Python econ package is deleted, so the script takes the Python package from
the commit before the deletion (`git archive`).

`step4.sh WORK_DIR` compares the reference model with the Python reference on the fixture export and on an
export of the pack. The Python side dumps what it derives from the export as JSON
(`step4_dump.py`): the derived inputs, PIGL weights at base and at given prices, PIGL shares and
quantities on random draws (some prices unreachable, some pops without money), erfinv on a grid,
the A.3 bound, the spectral radius of random matrices, every quantity of `model.evaluate` with the
residual at the data state and at a perturbed state and, on the fixture, every traded good's dense
kernel and a balanced Sinkhorn on it and the equilibrium from the data state. On the pack it adds
the calibrated inputs installed in the pack, the evaluation at their reference point and the
equilibrium from that point perturbed by 0.05 in every ln p and ln w. The `parity` test of
`ceteris-reference` reads the dump (`CETERIS_REFERENCE_PARITY`), recomputes each quantity, solves
from the same start and prints its largest relative difference; it fails above 1e-12, 1e-9 for
the equilibria and 1e-15 for erfinv. Both `reference solve` entry points then solve the fixture
into WORK_DIR, compared with `pack compare --iterative`, and the `routing` tests of
`ceteris-cli` check the kernel of the fixture and of a pack export against its legs and tariffs.
For PPML, `step4_ppml_dump.py` dumps designs and the Python fits as Arrow: seeded synthetic
designs covering every path of `fit_robust` and `hdfe` (one to three fixed effects, clusters,
zero flows, separated rows, a repeated and an absorbed regressor, two fixed effects too large for
the direct solve), every `ppml.hdfe` call of `gravity.estimate` on 2000 BACI and the pack, and
those of `gravity.yotov`. The `ppml_parity` test (`CETERIS_PPML_PARITY`) fits each design, writes
the Rust fits beside Python's and fails above 1e-9 relative, in the mixed form
|a - b| <= 1e-9 max(1, |a|, |b|) for the absorbed design and the Gauss-Newton step coefficients
of `mode_costs`, which are rounding noise around 0; `pack compare --iterative` compares the Yotov
fits. For the calibration, `python -m ceteris.reference calibrate` and `ceteris calibrate`
calibrate the fixture (`--inputs`) and the pack (`--pack`), each timed, into
`WORK_DIR/calibration-<name>/{python,rust}` with their printed output beside them;
`pack compare --iterative` compares the six tables in file row order, and the `parity` test of
`ceteris-calibrate` (`CETERIS_CALIBRATION_PARITY`) the schema metadata (format, static digest and
passes equal, r within 1e-9) and the printed summary (within 1e-9; timings, Sinkhorn sweeps and
the per-pass changes, which are iteration noise, left out). The `steady_state` tests of
`ceteris-calibrate` then run on the pack export, including `EconInputs::from_pack` on a linked
copy of the pack holding the Rust tables.

The per-module scripts of step 2 compared each module's columns, every source reader's output as
canonical JSON and the raster pixel to H3 cell mapping while the port was under way. They were
removed with the Python econ package; their result is recorded in ADR 0113's step 2 As built
entry. The fixture of `pixel_cells_match_python_on_border_windows` stays as recorded.

The scripts are deleted with the Python they drive (ADR 0113, step 5).
