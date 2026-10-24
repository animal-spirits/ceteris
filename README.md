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
- Map (ADR 0010): provinces are coloured by a GPU fill layer on a flat map or a globe; the
  desktop app reads the pack from disk through `pack://`. ADR 0011 records the decision to
  rewrite the map in Rust.

The data licence audit is in `python/ceteris/packs/sources.toml` (ADR 0005).

## Layout

```
specs/          DESIGN.md, module specs (TEMPLATE.md, core/, geo/)
adr/            architecture decision records
crates/
  ceteris-core/ entity IDs, scheduler, Philox RNG, ledger, event log, snapshots,
                deterministic reductions, and the P0 toy world
  ceteris-geo/  pack loading, transport graph, least-cost paths and their cache
app/            Tauri desktop app: TypeScript map (MapLibre, React) over the Rust core
python/ceteris/packs/
                pack pipeline: pinned sources, provinces, H3 raster, transport graph;
                licence manifest and its validator
tests/golden/   stored hashes of golden scenarios
```

Further crates (`ceteris-econ`, `ceteris-trade`, ...) arrive in the phases that need
them (Section 19.3).

## Develop

Build the world pack first. It needs [tippecanoe](https://github.com/felt/tippecanoe)
(`brew install tippecanoe`), downloads about 920 MB of pinned sources into `data/cache/`
once, then builds `data/packs/2000/` in about 2.5 minutes:

```
cd python && uv run python -m ceteris.packs
```

Then:

```
cargo test --workspace
cd python && uv run pytest
cd app && npm install && npm run tauri dev
```

To work on the map in a browser instead, run the development engine and Vite:

```
cargo run --release -p ceteris-geo --example engine
cd app && npm run dev
```

To benchmark the map inside the desktop app's own webview (frame times over a camera
flight, and the cost of recolouring every county from the core), run
`CETERIS_BENCH=1 cargo run --release` in `app/src-tauri`. The window comes to the front,
results print as a `CETERIS_BENCH` JSON line, and the app exits. The display must be awake:
a sleeping display stops the webview from rendering.

Other entry points: `cargo run --release -p ceteris-geo --example costs -- NLD-3485 CHN-1819`
prints least costs from a province; `cargo bench -p ceteris-core -p ceteris-geo` runs the
benchmarks; `python3 -I python/ceteris/packs/check_sources.py` validates the licence manifest.
Tests that need the pack skip when it has not been built, as in CI.

Golden hashes change only deliberately: `CETERIS_BLESS=1 cargo test --test golden` or
`CETERIS_BLESS=1 uv run pytest`, then explain why in the commit.

The rules for contributors and coding agents are in Section 21.2 of the design: spec
first, tests before implementation, one system per task, golden runs, no silent
tuning, decisions recorded as ADRs, performance gates.

## Licence

Closed source for now (ADR 0005): some data sources permit no redistribution, and the
project opens once they are replaced or cleared. The intended licence on opening is MIT
or Apache-2.0 for code; bundled data keeps its source licence.
