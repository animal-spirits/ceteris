# 0151. Naval forces and sea control

Status: proposed
Date: 2026-10-28
Design sections: 5.3, 11.5, 11.8; ADRs 0145, 0150
Phase: P7 (2.0-beta)

## Context

11.5 lets sea control set blockade risk on transport edges. The transport graph already has 725
sea zones and explicit chokepoints. Since 2000, sea denial from land (Black Sea from 2022) and
attacks on shipping (Red Sea from late 2023, when most container lines rerouted round the Cape)
moved trade more than fleet battles did.

## Options

1. Sea control as a fixed modifier per chokepoint.
2. Task groups on sea zones with salvo and search models, and sea control priced into shipping
   edges.

## Decision

Option 2.

- **Task groups** patrol, escort, blockade, strike ashore, lay and clear mines, and land troops,
  per sea zone.
- **Surface combat.** Hughes salvo model: offensive salvo minus defensive interception, scaled by
  staying power. Missile magazines are finite; reloading needs a port.
- **Submarines.** Detection by Koopman random search, p = 1 − exp(−W·v·t / A), with sweep width
  from sonar against signature. SSBNs are the second-strike force of 0152.
- **Sea denial from land.** Anti-ship missiles, drones and uncrewed surface vessels based ashore
  contest zones without a fleet.
- **Shipping coupling.** Attack risk per zone raises insurance and freight costs on its edges;
  above a threshold carriers reroute through the trade module. A blockade is sea control plus a
  declared target; flows to the target's ports through the zone are cut.
- **Amphibious operations** are limited by lift, landing-port capacity, sea control and air
  superiority.

## Migration

- P7 stage W7, in `ceteris-war` (naval). Data: Marine Regions sea areas (licence to check), NGA
  World Port Index (public domain).

## Tests

- Red Sea 2023-24 as a hold-out: simulated rerouting via the Cape and freight-rate response within
  the observed range.
- A blockade cuts the target's seaborne imports through the zone and raises land and neutral
  routes.

## Consequences

Naval war acts on the economy through shipping costs and routes, the channel that mattered most in
the period.
