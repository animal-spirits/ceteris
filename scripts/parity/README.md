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
(sorted keys, floats as Python's `repr`), and the two texts must be identical.

`step2-rasters.sh` compares the per-province raster totals the same way (`dump-reader population
worldpop|mapspam|kummu`), then rewrites the pixel to H3 cell fixture of the Rust test
`pixel_cells_match_python_on_border_windows` with `pixel_cells.py` and checks it equals the
committed one.

The scripts are deleted with the Python they drive (ADR 0113, step 5).
