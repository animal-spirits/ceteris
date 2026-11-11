# 0163. State reach

Status: proposed
Date: 2026-10-28
Design sections: 5.4, 9.6, 10.1, 17; ADRs 0067, 0068, 0083
Phase: P6 (2.0-alpha)

## Context

Europa Universalis V makes a location's control fall with its travel distance from the capital,
which caps what can be extracted there and makes roads and administrative seats matter beyond
trade. The evidence agrees: national institutions matter less far from the capital
(Michalopoulos and Papaioannou 2014), and state power thins with distance where infrastructure is
poor (Herbst 2000). ADRs 0067 and 0068 have no distance term. Institutions are held fixed in 1.0
(ADR 0136), so this belongs to 2.0. Survey of other games' mechanics (2026-10-28)

## Options

1. Uniform state capacity within a country.
2. Reach that falls with generalized trade cost from the capital and regional seats.

## Decision

Option 2, as an extended mechanism that may join the lab profile if it passes ablation (15.7).

- Reach per province: a decreasing function of generalized trade cost (5.4) from the nearest
  administrative seat, weighted by the seat's rank (capital, regional capital).
- Reach scales tax collection efficiency (9.6) and the implementation rate of 0083 in that
  province, and enters 0067's state capacity as a spatial weight.
- Infrastructure that lowers trade cost raises reach, so a road built for trade also extends the
  state.
- The decay parameter is a new section 17 entry, fitted to the distance gradient of night lights
  across borders and of tax collection where subnational data exist.

## Tests

- With the decay parameter at zero, every province has reach one and 0067 is unchanged bit for
  bit.
- Moving a capital (an edit) changes reach only through trade costs.

## Consequences

Geography constrains power, and infrastructure gains a political return. Adds one or two free
parameters. Cost about 3-5 part-time weeks after P6.
