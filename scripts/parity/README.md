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
`ceteris-calibrate` (`CETERIS_CALIBRATION_PARITY`) the schema metadata (format and static digest
equal, r within 1e-9) and the printed summary (within 1e-9; timings, pass counts, Sinkhorn sweeps
and the per-pass changes, which are iteration noise, printed but left out). The `steady_state` tests of
`ceteris-calibrate` then run on the pack export, including `EconInputs::from_pack` on a linked
copy of the pack holding the Rust tables.
For gravity, `step4_gravity_dump.py` runs `gravity.estimate` on 2000 BACI and the pack and
`gravity.yotov` on the Yotov et al. (2016) files, dumping the printed report and notes, the TOML,
every field of the estimates, the country costs of the least-cost trees and the Yotov fits, with
both timings; `ceteris gravity` prints its report beside them, and the `gravity_parity` test of
`ceteris-calibrate` (`CETERIS_GRAVITY_PARITY`) estimates again in process: the printed text must
be equal, the estimates, the TOML values and the Yotov fits within 1e-9 relative (counts equal)
and the country costs within 1e-12. Last, both entry points print the stability table of the
fixture, compared byte for byte.

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

The package versions are those of `python/uv.lock`; only shapely is built from source.

`split_rows.py` moves the areas and vertices of split provinces, and of the counties clipped to
them, into tables of their own, so that only those rows take the split tolerance. A second Rust
build seeds the Voronoi splits from h3's cell centres (`h3_centres.py`, `dump-geo --build
--centres FILE`) and must match Python exactly but for the H3 cell areas.

`step3-transport.sh` compares the port of `transport.py`. `transport_tables.py` builds the
provinces, their mean ruggedness and the graph in Python and writes the tables `build.build`
writes from it (nodes, edges, sea_zones, chokepoints) with the coastal flags and the build stats;
`ceteris pack dump-geo DIR --build --transport` writes the same from Rust. Every table is
compared in file row order. Built on h3o's cell centres, sea node positions and the lengths that
reach them differ in the last bits and are held to 1e-8; a second Rust build takes h3's centres at the
land and the sea resolutions and must match on every column. The script keeps its outputs in
`WORK` (default: a new temporary directory).

`step3-web.sh` compares the port of the map outputs of `build.build` (`simplify`, the countries
layer, `write_geojson`, `write_tiles`, the fills of the simplified geometry) with
`ceteris_pack::web`, on GEOS 3.15 as above (`GEOS_PYTHON`). `web_inputs.py` builds the provinces
and clipped counties and writes their vertices, countries and Polygon or MultiPolygon type;
`web_outputs.py` and `ceteris pack dump-web DIR` rebuild the geometries from those files and
write the outputs. Each layer's GeoJSON must match byte for byte, the simplified vertices
exactly, and the fills by `fills.py`'s rule; the PMTiles archives are compared for information.
`step3-web.sh WORK` keeps its outputs in WORK.

`step3-build.sh` compares the whole pack of `build.build` (`python -m ceteris.packs --out DIR`,
which runs the Rust econ build itself) with `ceteris pack build --out DIR`, on GEOS 3.15 as above
(`GEOS_PYTHON`, and tippecanoe on PATH). Every P1 and econ table is compared in file row order,
the two attribute files byte for byte (`json_diff.py` lists differing rows), the manifests by
`manifest_diff.py` (config, sources and stats equal but the fill stats; the tile layers' GeoJSON
and attribute files' digests equal; the Arrow tables' digests listed, as pyarrow and arrow-rs
write different bytes, and the Rust file must be `json.dumps(indent=2)` text), the stats each
build prints (but the fill stats), and the fills by `fills.py`'s rule against each Rust build's
own simplified polygons (`pack build --simple DIR`). The tiles archive is not compared.
Built on h3o's centres, split provinces' areas and their counties' (`build_split_rows.py`) take
the split tolerance, the graph's sea positions and lengths `LISTED_TOL`, and the attribute
files' differing rows and the layer digests are listed; a second Rust build takes h3's centres
(`--centres`) and must match on every column but the H3 cell areas and the means they weight,
with identical attribute files and layer digests. The script keeps its outputs in `WORK`
(default: a new temporary directory).

`py_round4.py` writes the fixture of `tests/py_round.rs`: CPython's `round(x, 4)` and `repr`,
with which the GeoJSON writes coordinates.

`geo_readers.py` prints digests of the geometries pyogrio reads from the admin-1 shapefile, the
geoBoundaries files and the Natural Earth land, railways and ports; `tests/geo_readers.rs`
checks the Rust readers against them.

The Python pack modules (`python/ceteris/packs/*.py` but `__init__.py`) and `test_packs.py`
are deleted after step 3's parity; the configuration files stay in `python/ceteris/packs/` until
step 5. Each step 3 script takes the package from the commit before that deletion: `packs_tree.sh`
extracts `python/ceteris` from it (`git archive`) into `tree/` in the script's work directory and
links every other top-level entry of the checkout beside it, because the package finds
`data/cache`, its configuration and the cargo workspace from its own path. The Python runs with
`PYTHONPATH=tree/python`; `build.build`'s econ step runs `cargo run` in that tree.
`geo_readers.py` needs the same `PYTHONPATH`.

The Python reference and calibration packages (`python/ceteris/reference`,
`python/ceteris/calibrate`) and their tests are deleted after step 4's parity; `step4.sh` and its
dump scripts stay and must be run from a checkout of the commit before that deletion.

The scripts are deleted with the Python they drive (ADR 0113, step 5).
