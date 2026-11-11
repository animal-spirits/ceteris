# 0234. Late-game flatness gate

Status: proposed
Date: 2026-10-29
Design sections: 19.2, 21.2; ADRs 0207, 0212, 0236

## Context

Paradox and Civ games slow down as a campaign runs: pops split, characters and units accumulate,
scripts run per entity. Ceteris avoids most of these causes by design, but nothing measures it.
The P2 benchmarks time tick 1 from steady state (econ SPEC, Task 19); none compares a late tick
with an early one. Assessment

## Options

1. Per-tick criterion benchmarks at tick 1 only (status quo).
2. A flatness check once, before release R.
3. A flatness gate from P3, kept by every later phase.

## Decision

Option 3 (owner, 2026-10-29).

- **Gate.** A release-build run of 100 simulated years on the full pack, default scenario. The
  median tick time over years 96 to 100 must not exceed the median over years 1 to 5 by more than
  25%. A miss fails the phase's definition of done (21.2), like a missed 19.2 budget.
- **Row counts.** The same run logs the row count of every state table at each decade. A table
  that grows past the bound 0236 requires fails the gate even if time stays flat.
- **Memory.** Peak resident memory is logged with the row counts and checked against 19.2 and the
  retention tiers of 0212.
- **Where it runs.** On the owner's machine (19.2 hardware), as a named benchmark with a
  checked-in report; CI runs a 10-year version for row counts only.

## Tests

- The gate report names the median tick time of both windows, their ratio, and the decade row
  counts per table.
- A synthetic table that appends one row per tick, injected in a test build, fails the gate.

## Consequences

Late-game slowdown is caught in the phase whose system causes it. The full gate costs a few
minutes of owner time per phase.

## Migration

- P3: the benchmark and report; DESIGN 21.2 definition of done gains the gate.
