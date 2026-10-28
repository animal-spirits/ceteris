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

`step3-provinces.sh` compares the ports of `provinces.py`, `counties.py` and the county
clipping of `build.build`. Both sides build the provinces and counties from the cache:
`geo_inputs.py` in Python, `ceteris pack dump-geo DIR --build` in Rust, which needs the
owner-side `geos` feature. The Python must run shapely on the GEOS that the Rust links, 3.15,
or overlay results differ for reasons that are not the port's:

```sh
brew install geos                       # 3.15.0; the geos crate finds it through geos-config
uv venv --python 3.14.2 "$VENV"         # outside the project, so python/uv.lock is untouched
GEOS_CONFIG=/opt/homebrew/bin/geos-config uv pip install --python "$VENV/bin/python" \
    h3==4.5.0 mapbox-earcut==2.1.0 numpy==2.5.4 pyarrow==26.0.0 pyogrio==0.13.0 \
    rasterio==1.5.2 scipy==1.18.1
GEOS_CONFIG=/opt/homebrew/bin/geos-config uv pip install --python "$VENV/bin/python" \
    --no-binary shapely --no-cache shapely==2.2.0
"$VENV/bin/python" -c 'import shapely; print(shapely.geos_version_string)'   # 3.15.0
GEOS_PYTHON="$VENV/bin/python" scripts/parity/step3-provinces.sh
```

The package versions are those of `python/uv.lock`; only shapely is built from source. The
script imports `ceteris` from `python/` through `PYTHONPATH`.

`split_rows.py` moves the areas and vertices of split provinces, and of the counties clipped to
them, into tables of their own, so that only those rows take the split tolerance. A second Rust
build seeds the Voronoi splits from h3's cell centres (`h3_centres.py`, `dump-geo --build
--centres FILE`) and must match Python exactly but for the H3 cell areas.

`step3-web.sh` compares the port of the map outputs of `build.build` (`simplify`, the countries
layer, `write_geojson`, `write_tiles`, the fills of the simplified geometry) with
`ceteris_pack::web`, on GEOS 3.15 as above (`GEOS_PYTHON`). `web_inputs.py` builds the provinces
and clipped counties and writes their vertices, countries and Polygon or MultiPolygon type;
`web_outputs.py` and `ceteris pack dump-web DIR` rebuild the geometries from those files and
write the outputs. Each layer's GeoJSON must match byte for byte, the simplified vertices
exactly, and the fills by `fills.py`'s rule; the PMTiles archives are compared for information.
`step3-web.sh WORK` keeps its outputs in WORK.

`py_round4.py` writes the fixture of `tests/py_round.rs`: CPython's `round(x, 4)` and `repr`,
with which the GeoJSON writes coordinates.

`geo_readers.py` prints digests of the polygons pyogrio reads from the admin-1 shapefile and the
geoBoundaries files; `tests/geo_readers.rs` checks the Rust readers against them.

The scripts are deleted with the Python they drive (ADR 0113, step 5).
