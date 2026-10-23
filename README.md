# Ceteris Paribus

A deterministic, forkable, agent-based world simulator starting on
1 January 2000. The design is in [specs/DESIGN.md](specs/DESIGN.md), a snapshot of the
living design document.

## Status

Phase P0 (Foundations) of eleven is complete. The core machinery exists; there is no
economics, map or data pipeline yet. The P0 gate (Section 21.1) passes: a 10-province
toy world runs 1,200 ticks with identical state hashes across runs, across 1 and 8
threads, and after restore-and-replay from any snapshot. The data licence audit is in
`python/ceteris/packs/sources.toml` (ADR 0005).

## Layout

```
specs/          DESIGN.md, module specs (TEMPLATE.md, core/SPEC.md)
adr/            architecture decision records
crates/
  ceteris-core/ entity IDs, scheduler, Philox RNG, ledger, event log, snapshots,
                deterministic reductions, and the P0 toy world
python/ceteris/packs/
                data-source licence manifest and its validator
tests/golden/   stored state hashes of golden scenarios
```

Further crates (`ceteris-geo`, `ceteris-econ`, ...), the Tauri app and the Python
package arrive in the phases that need them (Section 19.3).

## Develop

```
cargo test --workspace
cargo run --release -p ceteris-core --example toy -- 1200 10 42
cargo bench -p ceteris-core
python3 -I python/ceteris/packs/check_sources.py
```

Golden hashes change only deliberately: `CETERIS_BLESS=1 cargo test --test golden`,
then explain why in the commit.

The rules for contributors and coding agents are in Section 21.2 of the design: spec
first, tests before implementation, one system per task, golden runs, no silent
tuning, decisions recorded as ADRs, performance gates.

## Licence

Closed source for now (ADR 0005): some data sources permit no redistribution, and the
project opens once they are replaced or cleared. The intended licence on opening is MIT
or Apache-2.0 for code; bundled data keeps its source licence.
