# 0076. Groundwater, shared river basins and glacier water towers

Status: proposed
Date: 2026-10-24
Design sections: 5.1, 7.2, 11.2, 13.2, ADR 0038
Phase: P8

## Context

Water stress (ADR 0038) is a static baseline. Three dynamic water stocks matter for 2000-2100.
Groundwater is being mined faster than it recharges in northwest India, the North China Plain,
the Middle East and the US High Plains (Rodell et al. 2009, GRACE), with irrigated output at
stake when it runs down. Shared river basins (Nile, Indus, Mekong, Tigris-Euphrates) pit
upstream dams against downstream users, and basin treaties shape how that conflict is managed.
Glaciers in high mountain Asia and the Andes feed seasonal flows that rise with melt and then
decline after "peak water" (Huss and Hock 2018; Immerzeel et al. 2020).

## Options

1. Status quo: water stress as a fixed index.
2. An aquifer stock per province, basin-level allocation between riparian states as a
   bargaining issue, and glacier mass per basin feeding seasonal flow.

## Decision

Option 2.

- Aquifer stock per province: recharge from the climate emulator, extraction from irrigation
  and urban demand; depletion raises pumping costs and finally caps irrigated yields (P129). This is the only
  cap on irrigated yields; ADR 0038's Aqueduct stress initializes it (owner, 2026-10-25).
- Basin allocation: upstream reservoirs and withdrawals reduce downstream flow; allocation is a
  bargaining issue between riparian states in the Fearon module (11.2), constrained by existing
  treaties (P130).
- Glacier mass per basin from regional temperature (0037) adds meltwater to seasonal flow, with
  peak water then decline.

## Migration

- P8, in ceteris-climate.
- Data: GRACE groundwater trends (NASA, public domain), Aqueduct (CC BY 4.0), Transboundary
  Freshwater Dispute Database (terms to audit), glacier projections (terms to audit).

## Tests

- Simulated groundwater depletion 2003-2016 matches GRACE trends in the main depletion hotspots
  within tolerance.

## Consequences

Food security and interstate tension respond to water stocks that run down, not only to
rainfall.
