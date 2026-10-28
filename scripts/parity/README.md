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

`step4.sh` compares the reference model with the Python reference on the fixture export and on an
export of the pack. The Python side dumps what it derives from the export as JSON
(`step4_dump.py`): the derived inputs, PIGL weights at base and at given prices, PIGL shares and
quantities on random draws (some prices unreachable, some pops without money), erfinv on a grid,
the A.3 bound, the spectral radius of random matrices and, on the fixture, every traded good's
dense kernel and a balanced Sinkhorn on it. The `parity` test of `ceteris-reference` reads the
dump (`CETERIS_REFERENCE_PARITY`), recomputes each quantity and prints its largest relative
difference; it fails above 1e-12, or 1e-15 for erfinv.

The per-module scripts of step 2 compared each module's columns, every source reader's output as
canonical JSON and the raster pixel to H3 cell mapping while the port was under way. They were
removed with the Python econ package; their result is recorded in ADR 0113's step 2 As built
entry. The fixture of `pixel_cells_match_python_on_border_windows` stays as recorded.

The scripts are deleted with the Python they drive (ADR 0113, step 5).
