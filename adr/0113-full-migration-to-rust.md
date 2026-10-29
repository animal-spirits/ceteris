# 0113. Full migration of the Python code to Rust

Status: accepted (owner, 2026-10-27)
Date: 2026-10-27
Design sections: 14.8 (scripting API), 15.2, 16, 16.1, 19.1 (stack), 19.3 (repository layout), 21.1, 22.1 (core language); amends ADRs 0003, 0050, 0096

## Context

Python today is the pack pipeline (`python/ceteris/packs`, 4,020 lines), the reference model
(`reference`, 2,655 lines, including the JAX model) and calibration (`calibrate`, 984 lines),
with 4,089 lines of pytest and hypothesis tests. The Rust workspace already owns the core, the
app (ADR 0088) and the pack's Arrow reader.

Three things push toward one language:

- Licences. Several sources allow use but not redistribution (ADR 0005), and EPR 2021 is CC
  BY-NC-SA 4.0. A release that downloads those sources and builds their part of the pack on the
  user's machine needs the pipeline inside the app. Shipping a Python runtime with GDAL, numpy and
  scipy adds a second runtime of several hundred megabytes and a second set of platform builds.
- One maintainer. Two toolchains, two test suites and two dependency sets cost more than the
  ecosystem they buy, now that the work is mostly in the core.
- Calibration is already moving. ADR 0096 replaces the JAX relaxations with forward-mode dual
  numbers in the Rust core, so `jax_model.py` loses its role.

What Python still does better: geometry (GEOS through shapely), rasters (GDAL through rasterio and
pyogrio), a Newton-Krylov solver (scipy's `KrylovJacobian`), and exploratory analysis.

## Options

1. Keep Python, and ship an embedded Python runtime for builds on users' machines.
2. Port only what must run on users' machines: fetching, verification and the tabular builders
   for sources that cannot be redistributed. Everything else stays in Python and ships prebuilt.
3. Full migration. Pipeline, reference model and calibration move to Rust, and Python leaves the
   build, release and test path. Analysis from Python is decided separately (scripting interface below).
4. Option 3, and also drop any Python interface for analysts.

## Decision

Option 3, with no Python bindings for now (scripting interface below). This leaves the door open
to a later client, so it is not option 4.

Crates:

| Crate | Replaces | Main dependencies |
|---|---|---|
| `ceteris-pack` | `packs/` (fetch, manifest check, readers, build, econ pack) | `reqwest`, `sha2`, `zip`, `csv`, `calamine`, `toml`, `arrow`, `h3o`, `geo`, `earcutr`, `tiff`, `shapefile` |
| `ceteris-reference` | `reference/` without the JAX model | `faer` (dense and sparse linear algebra), `statrs` |
| `ceteris-calibrate` | `calibrate/` and `reference/calibrate.py` | `ceteris-reference`, `ceteris-econ` |

- The `ceteris` binary gains `pack fetch`, `pack build`, `pack check-sources --profile
  research|release`, `reference solve`, `calibrate` and `gravity`. Their arguments and outputs
  match today's Python entry points.
- `sources.toml`, `fetch.toml`, `validation.toml`, `pack.toml`, `borders_2000.toml`,
  `chokepoints.toml` and `counties.lock.toml` keep their schemas. Only their reader changes.
- Geometry uses `geo` (boolean operations through `i_overlay`). The `geos` crate is a fallback,
  allowed only on the owner-side build and only for an operation whose parity test fails. The
  user-side build stays pure Rust with no C toolchain.
- Rasters use `tiff`, plus our own reading of the GeoTIFF georeferencing tags (ModelPixelScale,
  ModelTiepoint, GeoKeyDirectory) for the four rasters the pack reads: ETOPO 2022, WorldPop 2000,
  and MapSPAM 2000 read from inside its zip. DMSP is planned. Nothing else is supported.
- The xlsx reader is `calamine`, which replaces `people._xlsx_sheet`.
- The reference solver is Newton-Krylov with restarted GMRES on finite-difference Jacobian
  products, with step caps and the Anderson-accelerated tatonnement fallback of `solve.py`. No
  crate matches scipy's `KrylovJacobian`, so it is written here (about 300 lines). PPML with high-
  dimensional fixed effects (`ppml.hdfe`) is ported as is, and the Yotov et al. (2016) reproduction
  test keeps its tolerances.
- Independence of the reference model. `ceteris-reference` must not depend on `ceteris-econ`,
  `ceteris-trade` or `ceteris-core`. It is written from `specs/econ/SPEC.md` and the Python code,
  never from the core's kernels. A workspace test reads `cargo metadata` and fails on any such
  edge. Only `ceteris-calibrate` sees both.
- `jax_model.py` is deleted, not ported (ADR 0096).
- Map tiles: `build.py` calls tippecanoe for the PMTiles archive. That stays an owner-side tool,
  because the tiles come from redistributable sources and ship prebuilt. A Rust tiler is out of
  scope.
- Licences of new dependencies: `cargo-deny` gets a licence allowlist (MIT, Apache-2.0, BSD,
  ISC, Zlib, Unicode, MPL-2.0). GPL and AGPL crates are refused because they would bind a closed or
  paid build. Slint keeps its royalty-free licence (ADR 0088).

Order. Each step deletes its Python only after its parity test passes:

1. Manifest check and fetch (`check_sources.py`, `fetch.py`). This step alone makes downloading
   on users' machines possible.
2. Tabular econ pack: `icio`, `goods`, `theta`, `tariffs`, `gateways`, `people`, `econ/build`.
3. Geography: `provinces`, `counties`, `raster`, `transport`, `fills`, `geo`, `resources`,
   `downscale`, `build`.
4. Reference model, PPML and calibration.
5. Remove `python/`, `uv.lock`, the CI `python` job and `check_sources.py` from CI. Update
   DESIGN 16.1, 19.1, 19.3 and 22.1 (Migration below).

Parity. During steps 2 to 4, a harness builds the same tables with both implementations and
compares them column by column:

- integer, string, boolean and categorical columns: equal;
- float columns: relative difference at most 1e-12 per element, or 1e-9 where an iterative solve
  sets the value (the reference equilibrium, PPML, the calibration tables);
- geometry: the same province and county for every H3 cell, and area per province within 1e-9
  relative.

After step 4, the pack and the calibration tables are regenerated from Rust, and the core's golden
hashes (`golden`, `p0_acceptance`, `p2_acceptance`) are re-pinned once in one commit that records
the old and new digests.

Pack-build determinism follows the core's rules (ADR 0097). Float reductions run in a fixed order
with fixed chunking, there are no parallel float reductions with dynamic splits, and two builds on
different machines from the same pinned inputs give identical bytes. A CI test builds the
fixture pack twice and compares digests.

Scripting interface (DESIGN 14.8, the P3 experiment API in 21.1), decided by the owner on
2026-10-27: no Python bindings for now. P3's experiment API is the `ceteris` CLI driven by a
scenario file (baseline, branch tick, edits, ensemble size and seeds) that writes run outputs as
Arrow IPC and Parquet, which Polars, DuckDB or any other language reads without glue code. A thin
PyO3 client (`ceteris-py`, DESIGN 19.1 and 19.3) that only drives runs and reads Arrow is added
later only if notebook-driven runs prove necessary; it would get its own ADR.

## Migration

- DESIGN 22.1: "Python only for pipeline, reference model, calibration" becomes Rust. DESIGN
  19.1: the Python tooling and the PyO3 + maturin bindings rows leave the stack table; 19.3 drops
  `ceteris-py/` and `python/ceteris/`. DESIGN 14.8: scripting goes through the CLI and scenario
  files, with outputs in Arrow and Parquet.
  DESIGN 16.1: "A Python package builds the pack" becomes the `ceteris-pack` crate.
- ADR 0050: `check_sources.py --profile` becomes `ceteris pack check-sources --profile`, with the
  same rules.
- ADR 0003 (PyPI name) is dormant until bindings are built.
- ADR 0096: the JAX relaxations are gone. The finite-difference test of the dual numbers uses
  `ceteris-reference`.

## Tests

- Parity harness per module (above). The Python side runs only until the module's Python is
  deleted.
- Each pytest moves to a Rust test in the crate that replaces it, and hypothesis properties move
  to proptest. `test_p2_gate`, `test_steady_state` and `test_calibrate` keep their assertions.
- Reference independence: the `cargo metadata` dependency test.
- Pack determinism: two builds of the fixture pack, equal digests.
- `cargo deny check licenses` in CI.

## Consequences

- One toolchain, one test runner and one dependency audit. A release can fetch and build the
  restricted part of the pack on the user's machine with no Python runtime.
- About 6,700 lines of Python and 4,100 lines of tests to port. The risky parts are polygon
  operations without GEOS, GeoTIFF georeferencing, and the Newton-Krylov solver. No time estimate
  is given.
- The reference model loses some independence by sharing a language with the core. The
  dependency test and the rule of writing it from the spec are the guard.
- Exploratory analysis loses numpy and scipy inside the repo. Analysts read the Arrow and Parquet
  outputs from outside, and drive runs through the CLI rather than from a notebook.
- No wheels, maturin builds or PyPI releases to maintain.
- One re-pin of golden hashes, at the end of step 4.

## As built

- Step 0 (foundation): crates `ceteris-pack`, `ceteris-reference`, `ceteris-calibrate` and
  `ceteris-cli` added. The `ceteris` binary moved from `ceteris-econ` to `ceteris-cli` (name
  unchanged, so `cargo run --bin ceteris` is unaffected); `ceteris-calibrate` depends on
  `ceteris-econ`, so the binary could not stay there. `ceteris pack compare A B [--iterative]
  [--table NAME]` compares two directories of Arrow tables at 1e-12 (1e-9 with `--iterative`).
  `ceteris_pack::table::write` writes a table with `pack_format = "3"` and returns its SHA-256;
  it refuses extra schema metadata, because Arrow serialises metadata in hash-map order and the
  bytes would stop being deterministic. `deny.toml` carries the licence allowlist and CI runs
  `cargo deny check licenses`. `tiny-keccak` 2.0.2 (CC0-1.0, via `arrow-array` -> `ahash` ->
  `const-random`) is outside the allowlist and is allowed by a per-crate exception in `deny.toml`.
- Step 1b (fetch): `ceteris pack fetch [--record]` uses `reqwest` (blocking) with `native-tls`, the
  operating system's TLS stack (Security.framework, SChannel, OpenSSL); Linux builds need system
  OpenSSL. The `rustls` features fail the licence check: `webpki-root-certs` is
  CDLA-Permissive-2.0, reached through a wasm32-only dependency of `rustls-platform-verifier`.
  SHA-256 is the pure-Rust `sha2`: verifying the 5.9 GB cache takes about 11 s (2.7 s with the
  `asm` feature, which needs a C compiler and was declined).
- Step 2 (econ tables): `ceteris pack build --econ-only --pack DIR --out DIR` writes the seven
  tables of the Python econ build (`ceteris_pack::econ`, over the source readers of
  `ceteris_pack::read`) with the same columns, row order and schema metadata. It prints the three
  summaries to stderr and the table digests and build stats as JSON to stdout. `downscale`, listed
  under step 3 above, moved here, because the econ build reads it. Parity on the full cache and
  pack (`scripts/parity/step2.sh`): all 101 columns equal (largest relative difference 0, rows in
  file order, no sort), and the schemas, summaries and stats are equal. The same holds with each
  optional source the build reads left out of the cache (the two GEM trackers, the Kummu
  GeoTIFF). Every source reader matched byte for byte as canonical JSON, except the theta class
  bands, which agree within 1e-12 (numpy's BLAS matrix product, about one ulp). The P1 `build.py`
  and `calibrate/gravity.py` call the binary (`pack build --econ-only`, `pack dump-reader`) until
  steps 3 and 4. `python/ceteris/packs/econ/` and its four test files are deleted. Deviations:
  - DOSE region sums run in the order the source lists the regions (a `Vec` in first-row
    order, not a sorted map), as Python's dict iteration does.
  - DOSE and admin-1 names are matched after NFC normalisation and full Unicode case folding
    (`icu_normalizer`, `icu_casemap`, Unicode-3.0), as Python's `normalize` and `casefold` do.
  - calamine decodes the OOXML escape `_x0008_` in one GEM coal mine name, which the Python
    reader kept as raw text. No reader reads that column.
  - BACI sums are taken per batch, the rows ending in one 1 MiB block of the decompressed
    member, then across batches, as pyarrow's streaming CSV reader and `group_by` add them.
  - Kummu gridded GDP is read from a GeoTIFF of the year-2000 band, converted once from the
    NetCDF (`fetch.toml` `derived_from`, `scripts/convert_kummu.py`), so the user-side build
    reads no HDF5.
  - GeoTIFF predictors 2 and 3 are not read yet: no econ raster uses one. ETOPO needs
    predictor 3 in step 3.
  - The manifest's econ stats come back from the binary with their keys sorted.
- Step 4 (reference model and calibration): `ceteris-reference` holds the reference model
  (`io`, `demand`, `sinkhorn`, `uniqueness`, `model`, `solve`, `output`, `ppml`, `linalg`),
  read from the `ceteris export` directory; a workspace test reads `cargo metadata` and fails on
  any edge from it to `ceteris-core`, `-geo`, `-trade` or `-econ`. `ceteris-calibrate` holds
  the steady-state calibration (`steady`, `summary`, `write`), the gravity estimation (`dta`,
  `yotov`, `costs`, `gravity`) and the stability table (`stability`); it exports a pack in
  process, with no cargo subprocess. The commands are `ceteris reference solve --inputs DIR --out DIR`, `ceteris
  calibrate --pack DIR | --inputs DIR [--out DIR]`, `ceteris calibrate stability
  [FIXTURE_DIR]` and `ceteris gravity [--pack DIR] [--write] [--out FILE]`, with the arguments
  and output of the Python entry points. The HANK reference, SBI and Sobol analysis are built
  in Rust as well, in `ceteris-reference` and `ceteris-calibrate` (ADR 0207). Parity on the
  fixture and the full cache and pack (`scripts/parity/step4.sh`), worst relative gap and
  timings (Python vs Rust, release, arm64):
  - 4a inputs, demand, kernels, Sinkhorn, A.3 bound and model evaluation: 4.88e-15.
  - 4b equilibrium: 1.47e-10 on the pack, solved in 72 s against 237 s.
  - 4c PPML: 1.65e-10 on the BACI designs.
  - 4d calibration tables: 7.3e-11 on the pack, 189 s against 466 s.
  - 4e gravity: estimates 2.7e-13 (the rail cost ratio), the Yotov fits 2.5e-13, the country
    costs of the least-cost trees 8.5e-16; the printed report and the stability table are equal
    byte for byte. The estimate takes 81 s against 230 s, the Yotov fits 35 s against 71 s.
  - 4f, the regeneration of the pack and calibration tables from Rust and the re-pin of the
    goldens, is pending.

  Deviations:
  - Dense linear algebra is in-crate (`ceteris_reference::linalg`): Householder QR, one-sided
    Jacobi SVD and Cholesky, with every sum in a fixed order. `faer` was rejected because it
    picks its kernels by CPU feature at run time, so results would depend on the machine.
  - GMRES is restarted GMRES without LGMRES's augmentation vectors, so the inner step counts
    differ from scipy's; the solution agrees to 1e-9.
  - A non-finite Jacobian product ends the Newton run and falls back to tatonnement, which
    reports; Python raised scipy's error.
  - The mixed tolerance |a - b| <= 1e-9 max(1, |a|, |b|) is used only where a value is rounding
    noise around 0: every column of the absorbed-regressor PPML design, and the Gauss-Newton
    step coefficients of the mode cost fit (`d_rail`, `d_sea`, `d_air`), which go to 0 as the
    ratios converge.
  - The HS to CPA key also lists EU aggregate lines (such as `12EU20`), on which Python's
    `baci_flows` failed (`int()`); they match no BACI line and are skipped.
  - Least-cost trees reproduce scipy's Dijkstra: a binary heap of (-distance, node), so ties
    between equal-cost paths go to the larger node index, and path quantities summed by
    pointer jumping.
  - Calibration passes are not compared for equality: a pass count can move by one with the
    tables still within 1e-9.
  - The comment heading `gravity_2000.toml` names `ceteris gravity --write` as the command that
    writes it; the rest of the file is the Python layout.
  - `gravity --write` still defaults to `python/ceteris/packs/gravity_2000.toml`, beside the
    pack configuration, until step 5 moves the configuration.
  - `python/ceteris/reference/` (with the JAX model and `reduced.py`) and
    `python/ceteris/calibrate/` and their tests are deleted; `scripts/parity/step4.sh` runs the
    Python side from a checkout before that commit.
