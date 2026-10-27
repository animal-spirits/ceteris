# Rust Migration Implementation Plan (ADR 0113)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace all Python under `python/` (pack pipeline, GE reference model, PPML and calibration) with Rust crates that produce the same tables, then delete `python/`, before pre-P3 branches B and C (which are then built in Rust).

**Architecture:** Three new crates, `ceteris-pack`, `ceteris-reference`, `ceteris-calibrate`, plus `ceteris-cli`, which takes over the `ceteris` binary so it can depend on all of them without a cycle. Each module is ported behind a parity harness that builds the same Arrow tables with Python and Rust and compares them column by column; the Python is deleted only after its parity passes. Five branches, one per ADR step, each its own PR.

**Tech Stack:** Rust 2024 (arrow 60, csv, calamine, zip, reqwest + rustls, sha2, toml/serde, tiff, geo + i_overlay, spade, rstar, h3o, shapefile, geojson, earcutr, faer, statrs), cargo-deny. Python only as the parity oracle until each step deletes it.

**Spec:** `adr/0113-full-migration-to-rust.md`; inventory `.superpowers/sdd/MIGRATE/inventory.md` (worktree scratch); geo and econ SPECs for the tables' meaning.

## Owner decisions (2026-10-27)

1. All five ADR 0113 steps land before pre-P3 B; B and C are then built in Rust.
2. ADR 0113 is accepted now; its As built section is filled as steps land.
3. Kummu GDP (NetCDF-4/HDF5) is converted once, owner-side, to a GeoTIFF of the year-2000 band, pinned by sha256 as a derived source with its conversion command recorded; Rust reads only the GeoTIFF.
4. Geometry: a time-boxed pure-Rust spike first; any operation that fails parity uses the `geos` crate behind an owner-side cargo feature. The user-side build stays pure Rust.
5. Solver: Newton-Krylov with restarted GMRES, step caps and the Anderson tatonnement fallback; parity at the solution (1e-9), not on iterates.
6. Tables: arrow-rs arrays and typed Rust structs with `BTreeMap`/`Vec` groupings; no Polars (determinism, size).

## Controller rulings (cost if wrong in brackets)

- The `ceteris` binary moves from `ceteris-econ` to a new `ceteris-cli` crate (bin name unchanged). `ceteris-calibrate` depends on `ceteris-econ`, so the bin cannot stay in econ without a cycle. [one crate move]
- The parity harness is a permanent dev command, `ceteris pack compare A B [--iterative]`, comparing two directories of Arrow tables with ADR 0113's tolerances; the Python side is invoked by `scripts/parity/<step>.sh` until step 5 deletes them. [none]
- Summation order: Rust ports reproduce the Python order where it is deterministic and cheap to copy (sequential loops, `np.bincount` per block, numpy pairwise `np.sum` reimplemented in `ceteris-pack::numeric::pairwise_sum`). A column that still misses 1e-12 stops its task for a ruling with the measured gap; Python is never changed to make parity pass. [a ruling per column]
- `reference/reduced.py` and `test_jax_model.py` are deleted with `jax_model.py`; `calibrate/stability.py` moves to `ceteris-calibrate`; `counties.py --record` becomes `ceteris pack record-counties`. [none]
- Pack format stays 3: schemas and metadata are unchanged; only file bytes differ (pyarrow vs arrow-rs), so the pinned digests in `tests/golden/p1-pack.txt`, `p2-pack.txt` and `manifest.json` are re-pinned once, at step 4f, with the core goldens, as ADR 0113 says. [none]
- `ceteris-reference` reads the `ceteris export` Arrow directory (export format 9), never core structs; a workspace test reads `cargo metadata` and fails on any edge from it to `ceteris-core`, `-geo`, `-trade` or `-econ`. [none]
- Fill files (`web/*.fill`, read by the app) are compared by decoded triangle sets and per-province area; byte identity is required only if `earcutr` reproduces `mapbox_earcut`'s order. [app reader unchanged either way]

## Global Constraints

- Determinism (ADR 0097): no rayon `sum`/`reduce`/`fold`, no `HashMap`/`HashSet` reaching output; two builds of the fixture pack give identical bytes (CI test).
- Parity (ADR 0113): integer, string, boolean, categorical columns equal; float columns relative difference at most 1e-12 per element, 1e-9 where an iterative solve sets the value (reference equilibrium, PPML, calibration tables); geometry: the same province and county for every H3 cell, area per province within 1e-9 relative.
- A module's Python and its pytest are deleted only in a commit after its parity passes on the full cache and pack, recorded in the ledger with the compare output.
- Every pytest moves to a Rust test in the replacing crate; hypothesis properties become proptest; data-dependent tests skip without `data/cache` or the pack, as today. `test_p2_gate`, `test_steady_state`, `test_calibrate` keep their assertions and tolerances.
- Licences: `deny.toml` allowlist MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib, Unicode-3.0/Unicode-DFS-2016, MPL-2.0; `cargo deny check licenses` in CI. No GPL/AGPL. `geos` only behind the owner-side feature `geos`, off by default.
- Config schemas unchanged: `sources.toml`, `fetch.toml` (including `optional` and `manual`), `validation.toml`, `pack.toml`, `borders_2000.toml`, `chokepoints.toml`, `counties.lock.toml`. They move from `python/ceteris/packs/` to `crates/ceteris-pack/config/` in step 5 (paths in Rust read from a constant until then).
- CLI arguments and outputs match today's entry points (inventory section 1).
- `CETERIS_PACK` absolute; the pack is `data/packs/2000`; never modify it except at step 4f's regeneration, with replaced files moved to the Trash.
- `unsafe_code = "forbid"` stays workspace-wide.
- Conventional Commits, no attribution, no em dashes; comments only for counterintuitive logic; no process references in code.

## Review Focus

1. **Row order is table identity.** Tables whose row order comes from Python set/dict iteration, STRtree query order or h3 child order (nodes, edges, cells, counties, gateways) must have the same order in Rust, or a documented canonical sort applied on both sides before compare (tests: each table's compare runs without `--sort`; a sorted compare is a stop for a ruling).
2. **Pixel-to-cell assignment at raster edges.** WorldPop, ETOPO and MapSPAM pixels on province or antimeridian boundaries must land in the same H3 cell as rasterio + h3 put them (test: `pixel_cells_match_python_on_border_windows` on fixed windows around the antimeridian, a pole-ward row and a MapSPAM 0.083333 edge).
3. **calamine date and number semantics.** Dates come back as serials (tariffs uses 36526), integers stored as floats stay floats, shared vs inline strings both read (test: `xlsx_cells_match_python_reader` over every sheet the pack reads).
4. **Missing optional or manual cache files.** `fetch` with `optional = true` or `manual = true` and the file absent: the build degrades exactly as Python does (falls back, flags) and does not download (test: `missing_optional_source_falls_back` per optional entry).
5. **Solver failure paths.** Newton-Krylov hitting MAX_NEWTON or a non-finite residual falls back to tatonnement and reports, as `solve.py` does; calibration that does not settle reports passes and residual instead of writing tables (test: `nonconverging_solve_reports_and_falls_back` on a perturbed fixture).

---

## Step 0: `migrate/foundation` (prerequisites)

### Task 0: Crates, binary move, licences, compare harness

**Files:**
- Create: `crates/ceteris-pack/`, `crates/ceteris-reference/`, `crates/ceteris-calibrate/`, `crates/ceteris-cli/` (each `Cargo.toml` with `[lints] workspace = true`), `deny.toml`, `crates/ceteris-pack/src/compare.rs`, `crates/ceteris-pack/src/table.rs` (Arrow write helper), `crates/ceteris-reference/tests/independence.rs`, `scripts/parity/README.md`
- Move: `crates/ceteris-econ/src/bin/ceteris.rs` -> `crates/ceteris-cli/src/main.rs` (subcommands unchanged)
- Modify: `.github/workflows/ci.yml` (cargo deny step), `adr/0113` (status accepted, As built section stub)

**Interfaces:**
- Produces: `ceteris_pack::table::write(path: &Path, batch: &RecordBatch, metadata: &BTreeMap<String,String>) -> io::Result<[u8; 32]>` (sha256 of bytes, `pack_format = "3"` in schema metadata); `ceteris_pack::compare::tables(a: &Path, b: &Path, tol: Tolerance) -> Report` with `Tolerance { rel: f64 }` (1e-12 default, 1e-9 iterative) and `Report { tables: Vec<TableDiff>, ok: bool }`; CLI `ceteris pack compare A B [--iterative] [--table NAME]`, exit 1 when not ok, one line per column with max relative difference.

- [ ] **Step 1:** Failing tests: `compare_flags_float_beyond_tolerance`, `compare_requires_equal_ints_and_strings`, `compare_reports_missing_column`, `table_write_is_byte_deterministic`; `independence.rs`: `reference_does_not_depend_on_core_crates` (reads `cargo metadata --format-version 1`).
- [ ] **Step 2:** Run, expect failures. **Step 3:** implement; move the bin; existing CLI tests (`history_cli.rs` and others invoking `ceteris`) follow the move.
- [ ] **Step 4:** `cargo test --release --workspace` with `CETERIS_PACK`, clippy, bans script, fmt, `cargo deny check licenses` (install `cargo-deny` if missing; record its version).
- [ ] **Step 5:** Commit `build: add migration crates, move the ceteris binary and add licence checks (adr 0113)`; mark ADR 0113 accepted (owner, 2026-10-27) in the same branch.

---

## Step 1: `migrate/fetch`

### Task 1a: Source manifest check

**Files:** Create `crates/ceteris-pack/src/sources.rs`; CLI `ceteris pack check-sources --profile research|release`; port `python/tests/test_check_sources.py` to `crates/ceteris-pack/tests/check_sources.rs`.
**Interfaces:** `sources::problems(root: &Path, profile: Profile) -> Vec<Problem>`; same summary line on stdout and problem lines on stderr as `check_sources.py`.

- [ ] Failing ports of the 8 tests plus one mutation test per REQUIRED rule; implement; parity: both profiles on the real `sources.toml` give the same stdout line, stderr lines and exit code as Python (diff the outputs in `scripts/parity/step1.sh`); replace the CI `rust` job's `python3 -I .../check_sources.py` line; delete `check_sources.py` and its test after parity. Commit `feat(pack): port the source manifest check to rust (adr 0113)`.

### Task 1b: Fetch and record

**Files:** Create `crates/ceteris-pack/src/fetch.rs`; CLI `ceteris pack fetch [--record]`.
**Interfaces:** `fetch::entries(root) -> Vec<Entry>` (`Entry { source, name, sha256: Option<String>, bytes: Option<u64>, url, optional, manual }`), `fetch::fetch(entry, cache: &Path) -> Result<PathBuf, FetchError>` (stream to `.part`, verify sha256 and bytes, rename; User-Agent `ceteris-packs`; never downloads `manual` entries), `fetch::path_of(name) -> PathBuf`, `CACHE` = `<repo>/data/cache`.

- [ ] Failing tests against a local HTTP server (std `TcpListener` in the test): download and rename, sha mismatch error and `.part` cleanup, `manual` never fetched, `optional` absent tolerated, `--record` prints sha256/bytes for unpinned entries. Parity: `ceteris pack fetch` verifies all 50 cached files as Python does (no network when all present). Delete `fetch.py` only in step 5 (econ and geo Python still import it). Commit `feat(pack): port fetch to rust (adr 0113)`.

---

## Step 2: `migrate/econ-pack`

Reads the P1 tables (provinces, cells, nodes, edges, countries) from the existing pack until step 3 replaces their producer.

### Task 2a: Readers

**Files:** `crates/ceteris-pack/src/read/{xlsx,csv_sources,wdi,ilostat,wic,barro_lee,swiid,pwt,icio,baci,cepii,dta_xlsx,dose,seds,gem,admin0}.rs` (one reader per source family; group small ones).
**Interfaces:** each reader returns typed rows (`Vec<Struct>` or `BTreeMap<key, Struct>`); `xlsx::sheet(path, name) -> impl Iterator<Item = BTreeMap<String, Cell>>` with `Cell::{Number(f64), Text(String), Empty}` (dates as serial numbers).
- [ ] Parity by dumping each reader's output as canonical JSON from both sides (`scripts/parity/readers.py` writes Python's; `ceteris pack dump-reader NAME` writes Rust's); equal for every source the econ build reads. Includes Review Focus 3 test. Commit per family group.

### Task 2b: Kummu GeoTIFF (owner decision 3)

- [ ] Using the current Python env (rasterio), write `scripts/convert_kummu.py` that extracts the band tagged `NETCDF_DIM_time == "2000"` to `data/cache/kummu_gdp_ppp_2000_5arcmin.tif` (Float32, same transform, nodata -9, deflate); add the derived entry to `fetch.toml` (`manual = true`, `derived_from = "GDP_PPP_1990_2015_5arcmin_v2.nc"`, `command = "..."`, sha256, bytes) and a `sources.toml` note; switch the Python `downscale.gridded_gdp` to read the GeoTIFF too, and confirm the Python econ tables are byte-identical before and after (same values). The `.nc` stays pinned as the provenance source. Commit `build(pack): pin the kummu 2000 band as a derived geotiff (adr 0113)`.

### Task 2c: Rasters

**Files:** `crates/ceteris-pack/src/raster/{geotiff,population}.rs`.
**Interfaces:** `geotiff::open(path) -> Raster` (tiled or striped, LZW with predictor, deflate; ModelPixelScale + ModelTiepoint or ModelTransformation; nodata), `Raster::window(row0, rows) -> Vec<f32>`; `population::province_population(raster, cells, block_rows = 512) -> Vec<f64>` reproducing the Python pixel-to-cell rule and per-block summation.
- [ ] Review Focus 2 test; parity of `people.province_population` totals for WorldPop, MapSPAM (summed `*_A.tif` from the zip) and Kummu at 1e-12. Commit.

### Task 2d: Econ tables

**Files:** `crates/ceteris-pack/src/econ/{people,icio,goods,theta,tariffs,gateways,downscale,build}.rs`; CLI `ceteris pack build --econ-only --pack DIR --out DIR`.
**Interfaces:** `econ::build(pack: &Path, out: &Path, cache: &Path) -> BuildReport` writing the 7 tables of `econ/build.py` `TABLES` with the same columns and metadata; k-medoids with the Python init and tie rules.
- [ ] Port order people -> icio -> goods/theta -> tariffs -> gateways -> downscale -> build, each with its pytest ported (test_people, test_icio, test_econ_pack, test_downscale) and its table's compare passing; `scripts/parity/step2.sh` builds both into scratch and runs `ceteris pack compare`. Review Focus 1 and 4 tests here. Delete `python/ceteris/packs/econ/` and the four test files after the full compare passes; `build.py` (P1) keeps calling the Rust econ build through the CLI until step 3. Commits per module.

---

## Step 3: `migrate/geo-pack`

### Task 3a: Geometry spike (time box about one day; owner decision 4)

- [ ] On the real cache, try pure Rust for `make_valid`, `union_all` of `ne_10m_land`, `buffer(0.005/0.01)` as used in `transport.adjacency`, `voronoi_polygons` (spade, with Python's `extend_to` envelope), polygon `difference`, and `coverage_simplify(0.01)`; compare against shapely outputs (areas 1e-9, same province per H3 cell, adjacency sets equal). Write `specs/rust-migration/geometry-spike.md` with a per-operation verdict: pure Rust, or `geos` feature. No production code is kept from the spike except what its verdicts adopt. Commit the report.

### Task 3b: Geo primitives, terrain, resources, fills

**Files:** `crates/ceteris-pack/src/geo/{sphere,raster_terrain,resources,fills}.rs`.
- [ ] Ports of `geo.py`, `raster.py` (ETOPO deflate tiled, H3 child sampling, h3o `area_km2` checked against h3 to 1e-12), `resources.py` (point-in-polygon with boundary-outside semantics), `fills.py` (CFIL v1 writer, MAX_EDGE 0.004, MAX_LAT 85.0511287798066); parity per column and decoded fills; tests from test_packs.py. Commit per module.

### Task 3c: Provinces and counties

**Files:** `crates/ceteris-pack/src/geo/{provinces,counties}.rs`; CLI `ceteris pack record-counties`.
- [ ] Port with the spike's verdicts; parity: same province and county for every H3 cell, area per province 1e-9; ports of the province and county tests. Commit.

### Task 3d: Transport graph

**Files:** `crates/ceteris-pack/src/geo/transport.rs`.
- [ ] Port; parity: nodes, edges, sea_zones, chokepoints equal in order (Review Focus 1). Commit.

### Task 3e: P1 build and tiles

**Files:** `crates/ceteris-pack/src/build.rs`; CLI `ceteris pack build --out DIR` (runs fetch, P1, tiles via `tippecanoe` with the same flags, fills, JSON, then the econ build).
- [ ] Parity: every Arrow table, `provinces.json`, `counties.json`, manifest `tables` (GeoJSON layer hashes for the tiles); fixture-pack determinism test (two builds, equal digests). Delete `python/ceteris/packs/*.py` (all but `__init__`) and `test_packs.py` after parity. Commit.

---

## Step 4: `migrate/reference`

### Task 4a: Reference inputs, demand, kernels

**Files:** `crates/ceteris-reference/src/{io,demand,sinkhorn,uniqueness}.rs`.
- [ ] `io::load(dir) -> Inputs` for export format 9 with `validate`; PIGL demand with `statrs` erf_inv checked against scipy to 1e-15 on a grid; dense Sinkhorn; spectral bound (power iteration on the nonnegative matrix, checked against numpy eigvals). Parity: residual of `model.evaluate` at fixed x is in 4b; here, unit parity on the fixture export. Commit.

### Task 4b: Model and solver

**Files:** `crates/ceteris-reference/src/{model,solve}.rs`; CLI `ceteris reference solve --inputs DIR --out DIR`.
**Interfaces:** `model::evaluate(x: &[f64], inp: &Inputs, a: &A) -> Evaluation`, `solve::equilibrium(inp, x0, tol = 1e-10, deadline) -> Result<Equilibrium, NoConvergence>`; Newton-Krylov with restarted GMRES (restart 100, MAX_NEWTON 200, STEP_CAP 0.3), Anderson tatonnement fallback (memory 5); same five output tables and `solution.json`.
- [ ] Parity: residual at fixed x 1e-12 (fixture and pack export), equilibrium 1e-9 on the fixture and the pack reference point; routing vs Rust core on the pack 1e-12 (the A3 test moves here); Review Focus 5 test; port test_reference. Commit.

### Task 4c: PPML

**Files:** `crates/ceteris-reference/src/ppml.rs`.
- [ ] IRLS, HDFE alternating projections (DEMEAN_TOL 1e-13, 10,000 sweeps), clustered SEs, `lstsq`/`pinv` via faer with numpy's rcond (eps * max(m, n)); parity 1e-9 on BACI samples; Yotov 2016 reproduction keeps its tolerances (needs 4e's `.dta` reader; order 4e's reader first if needed). Commit.

### Task 4d: Calibration

**Files:** `crates/ceteris-calibrate/src/{steady,write}.rs`; CLI `ceteris calibrate --pack DIR | --inputs DIR [--out DIR]` (default `target/calibration/<pack>`).
- [ ] In-process export through `ceteris-econ` (no cargo subprocess); six tables with `calibration_format` "3" and the same `static_digest`; parity 1e-9 on the fixture and the full pack; `EconInputs::from_pack` loads the Rust tables; ports of test_steady_state, test_calibrate (non-gravity), test_p2_gate. Commit.

### Task 4e: Gravity and stability

**Files:** `crates/ceteris-calibrate/src/{dta,gravity,stability}.rs`; CLI `ceteris gravity [--pack] [--write] [--out]`, `ceteris calibrate stability [FIXTURE]`.
- [ ] Stata `.dta` reader (formats 251-255, strings), Dijkstra with the same tie-breaking as `scipy.sparse.csgraph.dijkstra`, theta/border/mode fits; parity 1e-9 on estimates; stability table text equal. Commit.

### Task 4f: Regenerate and re-pin

- [ ] Rebuild the pack and the calibration from Rust into scratch, compare against the installed ones (ADR tolerances), install (old files to the Trash), re-pin `tests/golden/p1-pack.txt`, `p2-pack.txt` and the core goldens (`golden`, `p0_acceptance`, `p2_acceptance`) in one commit recording old and new digests; re-run the P2 gate (V1, H2 distance, H2 per class, 100 years, threads) and record the numbers in the econ SPEC. Delete `python/ceteris/reference/`, `python/ceteris/calibrate/` and their tests. Commit `test(golden): re-pin pack and calibration digests built in rust (adr 0113)`.

---

## Step 5: `migrate/remove-python`

### Task 5: Remove Python and update records

- [ ] Delete `python/` (pyproject, uv.lock, remaining tests), the CI `python` job; move the TOML configs to `crates/ceteris-pack/config/`; update DESIGN 14.8, 16.1, 19.1, 19.3, 22.1 (snapshot and the Claude Doc), ADRs 0003, 0050, 0096 per ADR 0113 Migration, READMEs and specs that name `uv run` commands (grep `uv run`, `python -m ceteris`); ADR 0113 As built completed. Full workspace tests, determinism test, `cargo deny`. Commit `chore: remove python after the rust migration (adr 0113)`.

---

## Order

Branches in order, each a PR for the owner: foundation, fetch, econ-pack, geo-pack (spike first), reference, remove-python. A branch may start stacked on the previous one while its PR waits. After step 5, the pre-P3 plan's branches B and C are rewritten against the Rust crates before execution.
