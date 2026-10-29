# 0128. Distribution: runs as hashes, reproducible builds, annual report, engine crate

Status: proposed
Date: 2026-10-28
Design sections: 2.5 (principle 7), 14.3, 19.1
Phase: P3 (run hashes, reproducible builds); 1.0 and later (annual report, engine crate, wasm)

## Context

Snapshots are content-addressed (14.3) and runs are deterministic, so a run is fully described
by a hash. Distribution can lean on that instead of hosting. Brainstorm:
https://claude.ai/artifact/N9zi5kznSaqvjg9ztw4WMT (ideas 29 to 32).

## Options

1. Status quo: share files.
2. Make the run hash the unit of sharing, prove determinism across platforms, and add a yearly
   release ritual and a reusable engine crate.

## Decision

Option 2.

- Runs as hashes: `ceteris share` prints a run's manifest hash; `ceteris fetch <hash>`
  recomputes it locally or pulls chunks from any mirror or peer. No project-run server.
- Reproducible builds: a pinned build (Nix flake or equivalent) and a CI test that macOS, Linux
  and Windows binaries give the same golden hash. A wasm build that matches is a stretch goal
  after 1.0.
- Annual State of the World: each year, rebuild the pack on the newest data, publish an open
  forecast report with fan charts, and score last year's forecast against reality.
- Engine crate: publish the trade core as a headless crate with a small stable API (provinces,
  goods and costs in; prices and flows out) for other games and tools.

## Migration

- P3: share and fetch, reproducible builds. 1.0: annual report. After 1.0: engine crate once
  the API settles, wasm.

## Tests

- `fetch` of a shared hash reproduces the run's state hashes on another platform.
- The cross-platform golden test passes on all three operating systems.

## Consequences

Sharing needs no infrastructure, and reproducibility becomes something anyone can check. The
annual report is a recurring commitment.
