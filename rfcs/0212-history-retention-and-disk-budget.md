# 0212. History retention and the disk budget

Status: proposed
Date: 2026-10-28
Design sections: 14.1, 14.3, 14.5, 19.1, 19.2, 19.4; ADRs 0024, 0097, 0211
Phase: P3 (policy and measurement), P8 (the 2150 run within budget)

## Context

The econ SPEC measures 1.88 GB peak memory for 120 ticks with history at interval 1, on 10 goods.
1.0 has 45 goods, runs to 2150 (about 1,800 ticks) and defaults to ensembles of K = 32 (14.1).
Scaled linearly that is tens of GB for one run and terabytes for an ensemble. DESIGN 19.2 budgets
memory for one run (6 GB) and says nothing about disk. Audit gap 3.

## Options

1. Keep everything; let users buy disks.
2. Keep history at a fixed coarse interval for every run.
3. Tiered retention: dense where the user is looking, sparse elsewhere, recompute the rest on
   demand. Exact replay (14.3) makes recomputation cheap and faithful.

## Decision

Option 3.

- **Live branch.** History at the view interval (monthly by default) for the most recent 25
  simulated years; older history kept annually. Memory holds a window; the rest streams to disk.
- **Other branches and ensemble members.** The declared outcome metrics of the experiment (14.1)
  at monthly resolution, and snapshots. Any other series is recomputed from the nearest snapshot
  when asked for, and cached while the branch is open.
- **Snapshots.** Every 12 ticks for the first 25 years, then every 60; content-addressed chunks
  shared across branches (14.3) are unchanged.
- **Budgets** (defaults, editable in settings): one interactive run to 2150 at most 20 GB on disk;
  an ensemble of 32 at most 50 GB; memory as 19.2. When a budget would be exceeded the app says
  which tier it will thin and asks.
- Caches follow ADR 0211: everything here is deletable and recomputable from the scenario file.

The numbers are first estimates; the P3 measurement replaces them, as P0 and P2 benchmarks
replaced 19.2's.

## Migration

- P3: retention tiers in the run store; a benchmark reports bytes per tick per tier.
- DESIGN 19.2 gains a disk row when this is accepted.

## Tests

- A full-pack run to 2150 and an ensemble of 32 to 2050 stay within their budgets.
- A series recomputed from a snapshot equals the one recorded densely, bit for bit.

## Consequences

Long runs and ensembles fit a laptop disk. Opening an old, thinned series costs a replay of the
span asked for.
