# 0007. Transport graph construction

Status: accepted
Date: 2026-10-23
Design sections: 5.3, 5.4, 19.1 (Graphs), 19.4

## Context

Section 5.3 specifies nodes at province centroids, ports, airports and sea-lane waypoints,
and six modes. The open sources available after ADR 0005 are Natural Earth (railways, ports,
land), OurAirports and the Eurostat searoute network, whose underlying ORNL lane data has an
unverified licence. No open source records navigability of rivers or 2000-vintage pipelines.

## Options

1. Sea lanes from searoute (fetch-only, licence unclear).
2. Sea lanes from an H3 grid over open water, with explicit links where the grid is too
   coarse to see a passage.

## Decision

Option 2, and a layered graph:

- Province nodes are the anchor of each province (largest part's centroid, moved inside if
  needed). Road edges join provinces sharing a land border of at least 1 km (or a quarter of
  the smaller perimeter, for enclaves such as the Vatican).
- A rail node per province with rail; rail edges where a Natural Earth railway crosses the
  shared border; access edges join the rail node to its province.
- Sea nodes are H3 resolution-3 cells whose centre is not on Natural Earth land (29,557
  cells); edges join neighbouring cells. Ten chokepoints tag the cells within a radius, so
  a shock can close them. Five links bridge passages the grid cannot see: Suez and Panama
  (canals, with a fee), the Turkish Straits, Gibraltar and Bab-el-Mandeb. Components without
  ports (fjords, lagoons) are dropped; the Caspian is kept.
- Sea zones are the resolution-1 parents of sea cells (725).
- Ports: 1,001 Natural Earth ports within 150 km of the sea grid, and a minor port, with a
  higher handling fee, for each of 1,276 coastal provinces without one.
- Airports: 1,154 large airports with scheduled service, each linked to its 12 nearest and
  to the 3 nearest on every other continent (24,849 links), not a complete graph.
- The pack stores physical attributes only; `ceteris-geo` prices edges from parameters, so
  calibration never rebuilds the pack.

## Consequences

- Inland waterways and pipelines are deferred: Rhine, Danube and Great Lakes traffic moves by
  road, rail or sea. Pipelines arrive with energy assets (P8).
- Rail, ports and airports are current vintages. Gauge breaks, ferries, bridges and tunnels
  across straits are not modelled; Denmark's islands connect by sea.
- Chokepoint closure is a cost change, so the path cache recomputes only after shocks.
