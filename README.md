# Ceteris Paribus

A deterministic, forkable, agent-based world simulator starting on 1 January 2000 and
running open-ended. The economic core is a province-level production-and-trade model; the
app is a native desktop map. Any state variable or parameter can be edited before a run
or at any tick. The design is in [spec/](spec/README.md); decisions are rows in
[spec/rows.toml](spec/rows.toml) with proposals in [rfcs/](rfcs/README.md).

## Status

Pre-P3. P0 (foundations), P1 (world and map), P2 (economic core) and the native app shell
are done; the pre-P3 audit is in progress, with the C6 recalibration and the C7 benchmark
rerun still to pass before P3. The order of phases is in Section 21 of the design.

- Rust only: the data pipeline, reference model and calibration are crates driven by the
  `ceteris` CLI (ADR 0113). No Python or web code remains.
- App (ADRs 0011, 0088): `app/` is Slint panels over a wgpu globe, running the economy on a
  sim thread. Tauri and the web map are gone.
- World: 4,796 provinces at 2000 borders and 53,084 counties, built into a pack from pinned
  sources; each user fetches the source data themselves.
- Determinism: golden hashes across runs, thread counts and snapshot replay.
- CI: GitHub Actions on a self-hosted macOS runner. A nightly full-pack gate runs there
  ([docs/pack-runner.md](docs/pack-runner.md)). Pack-dependent goldens move with each
  pack rebuild and re-bless.

Source licences are in `crates/ceteris-pack/config/sources.toml` (ADRs 0005, 0050).

## Layout

```
spec/           the design, one file per section; rows.toml decision registry
specs/          module specs (SPEC.md) and plans (PLAN.md); DESIGN.md is a frozen stub
rfcs/           proposals and their decisions, incl. the former ADRs 0001-0281
docs/research/  research index
tools/rows/     registry check (cargo run -p ceteris-rows -- check)
crates/
  ceteris-core/ entity IDs, scheduler, Philox RNG, ledger, event log, snapshots,
                deterministic reductions, and the P0 toy world
  ceteris-geo/  pack loading, transport graph, least-cost paths and their cache
  ceteris-trade/, ceteris-econ/
                province markets and trade kernel; production, labour, demand and
                the monthly economic tick
  ceteris-pack/ fetch, licence manifest check, source readers, the pack build and the
                table comparison; config/ holds the pack configuration: pinned sources,
                licence manifest, 2000 borders, chokepoints
  ceteris-geos-coverage/
                GEOS coverage simplification for the owner-side map build
  ceteris-reference/, ceteris-calibrate/
                independent reference model; steady-state calibration and gravity
  ceteris-cli/  the `ceteris` binary: export, run, pack, reference, calibrate
app/            native desktop app (Slint, wgpu), its own cargo workspace
tests/golden/   stored hashes of golden scenarios
scripts/        clippy bans check, nightly pack gate
```

Further crates arrive in the phases that need them (Section 19.3).

## Develop

Build the world pack first. It needs [tippecanoe](https://github.com/felt/tippecanoe)
and GEOS 3.15 (`brew install tippecanoe geos`), downloads about 920 MB of pinned sources
into `data/cache/` once, then builds `data/packs/2000/` in about 2.5 minutes:

```
cargo run --release --features ceteris-cli/geos --bin ceteris -- pack build
```

The goldens pin this build together with its calibration tables, written by
`cargo run --release --bin ceteris -- calibrate --pack data/packs/2000 --out DIR` and copied
into the pack (ADR 0113, step 4f).

Then:

```
cargo test --workspace
cd app && cargo run --release
```

The app reads `data/packs/2000` unless `CETERIS_PACK` names another pack. To benchmark the
map (frame times over a camera flight, and the cost of recolouring every county), run
`CETERIS_BENCH=1 cargo run --release` in `app/`. Results print and the app exits. The display
must be awake: a sleeping display stops vsync.

Other entry points: `cargo run --release -p ceteris-geo --example costs -- NLD-3485 CHN-1819`
prints least costs from a province; `cargo bench -p ceteris-core -p ceteris-geo` runs the
benchmarks; `ceteris pack check-sources --profile research|release` validates the licence manifest.
Tests that need the pack skip when it has not been built, as in CI.

Golden hashes change only deliberately: `CETERIS_BLESS=1 cargo test --test golden`, or
`CETERIS_BLESS=1 cargo test -p ceteris-pack --test build --test econ_build` for the pack's
digests, then explain why in the commit.

The rules for contributors and coding agents are in [CLAUDE.md](CLAUDE.md) and Section 21.2
of the design: spec first, tests before implementation, one system per task, golden runs,
no silent tuning, decisions recorded as RFCs and rows, performance gates. Rust is pinned in
`rust-toolchain.toml`; checks are `cargo fmt --all --check`, `cargo clippy --workspace
--all-targets -- -D warnings`, `scripts/check_clippy_bans.sh` and `cargo test --workspace`,
and the same in `app/`.

## Licence

Open source and non-commercial (owner ruling 28 October 2026, superseding the closed-source
fallback of ADR 0005). Code is dual-licensed MIT or Apache-2.0
([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)). Data is never bundled
when its source bars redistribution; each user's pipeline downloads it under the source's
own licence.
