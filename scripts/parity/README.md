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

The per-module scripts of step 2 compared each module's columns, every source reader's output as
canonical JSON and the raster pixel to H3 cell mapping while the port was under way. They were
removed with the Python econ package; their result is recorded in ADR 0113's step 2 As built
entry. The fixture of `pixel_cells_match_python_on_border_windows` stays as recorded.

`step3-geo.sh` compares the ports of `geo.py`, `raster.py`, `resources.py` and `fills.py`.
`geo_inputs.py` builds the provinces and clipped counties in Python, writes their vertices and
what the Python modules compute from them (anchors, spherical areas, H3 cells, terrain, mineral
site counts, fills of the unsimplified geometry), and its cells table must equal the pack's.
`ceteris pack dump-geo DIR` computes the same from the same vertices into `DIR/rust`. Tables go
through `pack compare`; fills through `fills.py`, which compares bytes, then each feature's
triangle set and area.

The scripts are deleted with the Python they drive (ADR 0113, step 5).
