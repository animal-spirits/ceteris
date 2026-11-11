# 0236. Bounded state tables

Status: proposed
Date: 2026-10-29
Design sections: 4.4, 9.8, 19.2, 19.4; ADRs 0145, 0183-0187, 0202, 0212, 0223-0226, 0234

## Context

Entity growth is the main reason strategy games slow down late. Ceteris fixes provinces, pop
cells and firm cells, but several tables can grow over a long run with no rule: financial claims
(19.2 budgets 1M; loans and bonds accumulate), characters (0183-0187), pop cells created by
migration and identity change (0223-0226), and units in war. History is bounded by 0212.
Assessment

## Options

1. Each system's SPEC decides when it is built.
2. A design-wide rule in 19.4 that each SPEC must meet.

## Decision

Option 2 (owner, 2026-10-29).

- **Rule (19.4).** Every state table is either fixed-size for the run or names a bound and the
  rule that keeps it: a cap, an aggregation or an archive. The module SPEC states the bound and
  its expected size at 2150.
- **Archive.** Rows that no system reads any more (dead characters, matured claims, dissolved
  bodies) move to an archive table outside the tick's working set. The archive is history under
  0212, read by panels and the chronicle, never by a system.
- **Initial bounds** (each owning SPEC confirms or replaces them):
  - Financial claims: claims older than a year aggregate into cohorts by (holder class, issuer,
    instrument, issue year); matured claims are archived.
  - Characters: dead characters are archived at death; living characters are bounded per state by
    the office and pool sizes of 0183.
  - Pop cells: a cell below a size floor merges into the nearest cell of the same province and
    education by group similarity; the key space bounds the rest.
  - War units: disbanded or destroyed units are archived; live units are bounded by mobilization
    (0145).
- **Determinism.** Aggregation and archiving run at quarter boundaries (the 19.4 re-sort) in
  stable ID order, so they are part of the deterministic tick.

## Tests

- The 0234 gate fails on any table without a stated bound, and on a table whose decade row
  counts exceed it.
- Aggregating claims into cohorts preserves the SFC matrices exactly (9.8).

## Consequences

The one Paradox slowdown cause the design still shared is closed by rule, at the cost of an
aggregation or archive step in each growing system.

## Migration

- DESIGN 19.4: the rule. Each module SPEC from P3 on names its bounds.
