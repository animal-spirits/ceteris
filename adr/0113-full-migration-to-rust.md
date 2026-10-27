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
  `const-random`) is outside the allowlist and awaits a ruling.
