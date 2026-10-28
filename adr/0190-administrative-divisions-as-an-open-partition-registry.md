# 0190. Administrative divisions as an open partition registry

Status: proposed
Date: 2026-10-28
Design sections: 5.1, 5.2, 10.3, 14.2, 20.1, 23.2; amends ADR 0115

## Context

The owner wants to change administrative structure freely: redraw states, create or remove
tiers in whole or in part, mix unit types within one country (special economic zones, directly
administered cities, autonomous regions), and see the result as a tree. ADR 0115 made territory a
set of partitions over a fixed H3 resolution 6 grid, but the set is closed (provinces, countries,
control, admin regions, counties, ethnic areas), admin regions are only "a relabel for politics
and the map", and province geometry is fixed during a run. Real administration is deeper and
uneven: Brazil in 2000 had 27 states and 5,507 municipalities, China's four municipalities sit
directly under the centre, India's union territories have no state, and US congressional and
school districts do not nest in counties.

Design brainstorm with diagrams: https://claude.ai/artifact/9DHJmPTEUEbthJhMoRZw5W

The government side of that brainstorm (bodies with membership, decision rule, agenda,
competences and scope; lever composition across tiers; appointment policies; integration of
other states; reforms passed as bills) waits for P6 by the owner's choice and is not decided
here.

## Options

1. Keep 0115's closed list; admin structure changes only in the pack.
2. An open registry of partitions that the pack seeds and the user edits, ex ante or at any tick,
   with only the partitions the engine reads by role able to move economic state.
3. As 2, plus the government primitive now.

## Decision

Option 2. Option 3 waits for P6.

- **Registry.** A run holds any number of partitions. The pack seeds owner, controller, provinces
  (0114), counties (0009) and the 2000 administrative tiers; the user adds, renames, edits and
  deletes any of them. Each partition has an id, a localized name (0143), a kind (admin,
  electoral, statistical, religious, custom), an optional scope partition (drawn inside each unit
  of another, usually owner), an optional parent, a nesting rule and per-unit metadata (name,
  seat cell, creation tick, status).
- **Roles.** The engine reads three partitions by role: owner, controller and the economic
  overlay of provinces and owners (0115). All others are referenced by id from map modes,
  statistics, peace terms and, from P6, bodies.
- **Tiers.** A tier is a partition with a parent. Nesting is checked per partition: strict
  refuses an edit that breaks it, loose reports it, none never checks.
- **Partial tiers.** Label 0 means no unit at this tier. Cells with no unit fall through to the
  tier above, so a tier can exist in some regions and not others, and can be added or removed in
  part.
- **Statuses.** A status is a named tag on a unit or on a painted set of cells (special economic
  zone, direct administration, autonomous region, special administrative region, military zone,
  traditional authority, user-defined). A status painted across existing units becomes its own
  partition with nesting none. In this ADR a status is metadata for the map, the tree and
  statistics; the lever overrides each status carries are defined with the P6 government ADR.
- **No state on units.** Units hold metadata only. A unit's figures (population, output, sector
  mix, ethnic and religious shares) are always aggregated from cells or economic units with
  0115's operator, so any non-economic partition can be redrawn without imputation.
- **Edits.** Painting labels, creating or deleting partitions and tiers, re-parenting, setting
  nesting, applying statuses and moving seats are intervention types (14.2), valid ex ante or at
  any tick, logged as god-mode edits until P6 gives them an in-world path.
- **Province redraws during a run (amends 0115).** Province labels may change at any tick,
  through the same split and merge as a sovereignty change: recompute the overlay, split stocks
  by cell weights or merge by summing, rebuild markets and transport edges of changed units. The
  solver log marks the imputed within-province distribution. Province geometry is no longer fixed
  in-run.
- **Storage.** A partition's labels are stored in two levels: a county-to-unit table plus a
  sparse map of exception cells for units that cut a county. A county-aligned partition costs
  about 100 KB rather than 8 MB. Lookups read the exception map first, then the county table.
  Snapshots carry only partitions changed since the parent snapshot.
- **Views.** A map mode per partition, and a tree view of the tiers in which each unit shows its
  aggregated figures and status; selecting a node highlights its cells. The org chart with
  office holders comes with the P6 bodies.
- **Data.** 2000 boundaries for the admin tiers from FAO GAUL, whose yearly vintages run 1990 to
  2014 and so also date the post-2000 reforms as validation events; deeper levels from
  geoBoundaries (0009). GAUL's non-commercial terms are acceptable under the 2026-10-28
  decision (each user fetches their own data); licence to audit before the build.
- **Build.** In `ceteris-pack` with migration Task 3f (ADR 0113), which already rewrites the
  cell-to-province and cell-to-county maps, so the pack format and goldens change once.

## Tests

- A null partition edit returns the input state bit for bit.
- Painting labels on any partition other than owner and provinces changes no economic state
  hash.
- Province split then merge at the same tick returns exact province totals.
- Two-level storage returns the same label as a full cell array for every cell of every seeded
  partition.
- A strict partition refuses an edit that breaks nesting; a loose one applies it and reports it.

## Consequences

- Any administrative map, real or invented, is expressible as data, including uneven depth and
  zones that cut existing units.
- Task 3f grows by about 3 to 5 part-time weeks (a guess); the per-tick cost is zero when nothing
  is edited.
- Province redraws join sovereignty changes as the second point where stocks are imputed below
  the data grain.
- P6 inherits a registry for bodies, competences, statuses with lever effects, appointment
  policies and reforms passed as bills; that ADR is still to be written.
