# Ceteris Paribus

A deterministic, forkable, agent-based world simulator starting on
1 January 2000. The design is in [specs/DESIGN.md](specs/DESIGN.md), a snapshot of the
living design document.

## Status

Phase P1 (World and map) of eleven is complete; there is no economics yet.

- P0 gate: a 10-province toy world runs 1,200 ticks with identical state hashes across
  runs, across 1 and 8 threads, and after restore-and-replay from any snapshot.
- P1 gate: 4,796 provinces at 2000 borders render at 60 fps; clicking a province shows
  its attributes; the freight-cost mode colours every province by least cost per tonne
  from a chosen origin over road, rail, sea and air.
- County layer (ADR 0009): 53,084 counties from geoBoundaries, nested in provinces and
  drawn from zoom 4; oversized provinces are split along county borders.
- Map (ADRs 0010, 0088, 0250): a native desktop app (winit, Slint panels, a wgpu globe
  drawn into a texture) colours provinces from a per-feature texture and runs the economy
  on a sim thread.

The data licence audit is in `crates/ceteris-pack/config/sources.toml` (ADR 0005).

## Layout

```
specs/          DESIGN.md, module specs (TEMPLATE.md, core/, geo/)
adr/            architecture decision records
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

The rules for contributors and coding agents are in Section 21.2 of the design: spec
first, tests before implementation, one system per task, golden runs, no silent
tuning, decisions recorded as ADRs, performance gates.

## Licence

Closed source for now (ADR 0005): some data sources permit no redistribution, and the
project opens once they are replaced or cleared. The intended licence on opening is MIT
or Apache-2.0 for code; bundled data keeps its source licence.
