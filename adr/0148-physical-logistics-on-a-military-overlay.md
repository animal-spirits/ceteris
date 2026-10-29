# 0148. Physical logistics on a military overlay

Status: proposed
Date: 2026-10-28
Design sections: 5.3, 8.2, 11.5, 11.7, 23.5; ADRs 0028, 0029, 0048, 0075, 0085, 0086, 0115, 0145
Phase: P7 (2.0-beta)

## Context

11.5 has forces draw fuel, ammunition and food over the transport graph, and 0048 says munitions
and industrial capacity, not abstract supply points, decide long wars. The transport graph (5.3)
has 4,796 province anchors and 2,757 rail nodes: too coarse for railheads, bridges and the last
leg from railhead to brigade, where armies stall (van Creveld 1977).

## Options

1. Supply points flowing from the capital (HOI3).
2. Typed goods in tonnes on the province-level graph.
3. Typed goods in tonnes on a military overlay built at cell grain for active theatres.

## Decision

Option 3.

- **Classes.** Fuel (diesel, jet, naval), ammunition by family (tube artillery, rockets, tank and
  autocannon, small arms, guided missiles, drones), food and water, spares by equipment family,
  medical. Each comes from its econ sector (refined petroleum, ammunition with chemical inputs,
  food system, machinery and electronics, pharmaceuticals), stockpiles (0085), imports and aid.
- **Depots.** National depots, theatre depots, railheads, ports and airheads (placed by orders),
  forward supply points, formations. Trucks carry the last leg; beyond about 100 to 150 km round
  trips eat their own fuel and lift (register parameter).
- **Overlay.** When a theatre opens, build road class and rail line per cell edge, bridges and
  junctions as nodes and rail gauge per segment, from OSM; drop it when the theatre closes.
  Military flows add load to the economic edges they share, so war raises civilian freight costs
  on the same lines.
- **Flow.** Per day and theatre, min-cost flow with capacities reduced by damage and closed where
  the front cuts an edge (0115), priority weights from orders, three aggregated commodity groups,
  solved by an auction algorithm with a fixed iteration count; residual shortfalls rationed in
  formation-ID order. Every shortage is reported with its binding edge.
- **Consumption** per formation per day by posture (idle, defend, attack, pursue) from 0048's
  modern ranges. Shortages cut mobility (fuel) and combat power (ammunition) steeply.
- **Maintenance.** Availability is a stock drawn down by tempo and restored by crews and spares;
  battle damage splits into destroyed, repairable and abandoned (capturable).
- **Interdiction.** Strikes on bridges, junctions and depots cut capacity; partisans and drones
  raise last-leg losses; repair crews are shared with 0075 and 0086.
- **Fallbacks** if the solver exceeds budget, each a switch: weekly solve with daily proportional
  rationing; one commodity group; overlay coarsened to rail and major roads.

## Migration

- P7 stage W3, in `ceteris-war` (supply). Pack: gauge and bridge attributes in the transport
  build (W0).
- Data: OpenStreetMap (ODbL; derived database share-alike), OurAirports and the NGA World Port
  Index (public domain).

## Tests

- Ukraine 2022: simulated artillery consumption and the gap to European production fall within
  the 0048 ranges.
- Each fallback keeps supply shortfalls within a set band of the full solve on a reference war.
- Supply flow per day stays within its 0145 budget on the world-war benchmark.

## Consequences

Logistics becomes physical and tied to the economy. The player never routes a truck: they place
nodes and set priorities. The overlay is the one structure built and dropped during a run.
