# 0180. Endogenous shipping costs and water-limited waterways

Status: proposed
Date: 2026-10-28
Design sections: 8.3, 15.4; ADRs 0007, 0022, 0076, 0081, 0102
Phase: P8, 1.x

## Context

Transport costs per mode are fixed (P04, ADR 0102). Shipping has scale economies, so busier
routes are cheaper (Ganapati, Wong and Ziv 2024), and ships run empty on backhaul legs, so
imbalanced trade lanes have asymmetric freight rates (Brancaccio, Kalouptsidi and Papageorgiou
2020). Inland waterways (ADR 0007) are not limited by water. Low water on the Rhine in 2018 cut
barge capacity and German industrial output; the Mississippi in 2022 and the Panama Canal drought
of 2023-24 cut capacity the same way.
Gap audit (2026-10-28): https://claude.ai/artifact/HRDQVbjJz8MrcDuPphPyYs

## Options

1. Status quo: fixed costs.
2. Scale economies and backhaul on shipping lanes; draft limits on waterways from runoff.

## Decision

Option 2.

- Shipping cost per lane falls with lane volume by an elasticity estimated in the published
  work. Concave costs can make equilibria non-unique, so ADR 0022's uniqueness check must pass
  before this enters the lab profile.
- Freight rates on each direction of a lane depend on the imbalance of flows.
- Waterway capacity on the Rhine, Mississippi, Yangtze and Panama Canal falls with runoff from
  the hydrology module (0076) and its seasonal factors (0081), which caps barge loads.

## Migration

- P8: lane-volume state in the trade kernel; draft limits in the waterway network.
- 1.x: backhaul asymmetry.
- Data: UNCTADstat LSCI and port throughput (manifest), GRanD and GloFAS (ADR 0182).

## Tests

- With costs frozen, golden hashes are unchanged.
- 0022's uniqueness check passes with scale economies on.
- A 2018 Rhine runoff replay lowers barge capacity and German chemical output within tolerance.

## Consequences

Trade-lane dynamics and drought effects on logistics become endogenous. Register: 2 entries.
