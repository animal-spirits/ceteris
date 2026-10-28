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

`step2-readers.sh` compares the econ source readers instead of tables: `readers.py NAME` and
`ceteris pack dump-reader NAME` print one reader's output on the real cache as canonical JSON
(sorted keys, floats as Python's `repr`), and the two texts must be identical. A reader given a
tolerance there is checked by `floatdiff.py` instead, which prints the first structural difference
and the largest relative float difference (an infinity matches only the same infinity, NaN only
NaN); `scripts/parity/test_floatdiff.sh` checks its exit status on hand-made JSON pairs (run it
after changing `floatdiff.py`).

`step2-rasters.sh` compares the per-province raster totals the same way (`dump-reader population
worldpop|mapspam|kummu`), then rewrites the pixel to H3 cell fixture of the Rust test
`pixel_cells_match_python_on_border_windows` with `pixel_cells.py` and checks it equals the
committed one.

`step2-people.sh`, `step2-icio.sh`, `step2-goods.sh` and `step2-downscale.sh` run `step2-econ.sh
MODULE` (the tariffs and gateways run it directly: `step2-econ.sh tariffs`, `step2-econ.sh
gateways`): it builds the econ tables with Python and with `ceteris pack build --econ-only`, checks with `econ_tables.py`
that every Rust table has exactly the columns listed there, selects the module's columns from
both builds, compares them, and runs the module's tests in release mode (their WorldPop tests
skip in debug builds).

`step2-downscale.sh` also compares, as readers, the base H3 cell and owning province of every GEM
coal mine and oil and gas field (`point-owners coal|oil-gas`: the points `point_sums` places) and
every downscaling proxy with the DOSE match counts (`proxies`), so a point moved between provinces
of one country shows even where the province totals of the VA columns would not.

The theta functions write no table, so `step2-readers.sh theta theta-bands theta-fixture`
compares their output as readers: per-good theta and value, the value by sector, the classes and
class thetas (exact), and the class bands, which pass within 1e-12 because numpy's matrix product
runs in BLAS order (`step2-goods.sh` runs the three).

The scripts are deleted with the Python they drive (ADR 0113, step 5).
