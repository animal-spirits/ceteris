# 0246. Rear security: computed demand and key-node orders

Status: accepted (owner, 2026-10-30)
Date: 2026-10-30
Design sections: 11.5, 11.6; amends ADRs 0147, 0148, 0154 and 0245
Phase: P7 (2.0-beta), stage W2

## Context

Under ADR 0245 a cell taken by an advance keeps its controller label until enemy formations come
within the zone-of-control radius. Nothing ties keeping control behind the front to troops being
there: 0154 makes garrisons a cost and resistance a hazard in repression, exclusion and duration,
but not in force presence. Cities matter only as terrain that slows an advance (0149), and supply
lines in the rear are protected by nothing (0148). Counterinsurgency research ties outcomes to
force density per population (0154; the often cited 20 to 25 per 1,000 inhabitants is a contested
rule of thumb, not a law).

## Options

1. Computed security demand per occupied province, a shortfall that feeds the 0245 insurgent shares
   and supply losses, and hold orders on key nodes.
2. Garrison units placed by player or planner, each holding cells within a radius.
3. A fixed troop cost per cell taken, scaled by population.
4. No change.

## Decision

Option 1 (owner, 2026-10-30).

- **Demand.** For each (province, occupier) with control share above zero on cells the occupier
  does not own, and for each owner's own cells with active irregular war, security demand in
  personnel = population held × base density × hostility × urban factor. Hostility rises with
  the local group's exclusion and kin ties to the enemy (EPR, 0058), repression, civilian harm
  (0154) and time held; it falls with local recruitment and governance spending. Base density and
  the factors are register parameters, fitted on the 0154 calibration cases.
- **Supply.** Security troops are formations in rear posture assigned to a province, plus police,
  gendarmerie and local auxiliaries counted at a quality weight. The theatre planner (0147)
  assigns them by default at delegation L2 and above; the player can override.
- **Shortfall.** Coverage = supply / demand, capped at 1. Below 1, each month the occupied cells'
  irregular share field (0245) opens if closed and insurgent share grows at a rate in the shortfall
  and hostility; when an insurgent share becomes the largest, the cell's label changes (0245).
  Shortfall also raises last-leg supply losses (0148) and lowers the tax the occupier collects
  (0154).
- **Key nodes.** Cities above a population threshold, bridges, rail junctions, railheads, ports,
  airfields and depots are nodes. A hold order (0147) pins a formation on a node: the node's cell
  and its transport edges stay with the holder regardless of the shares around them, and partisan
  losses on those edges fall. Node garrisons count toward their province's supply.
- **Undefended cities.** A city cell is relabelled by a sweep or the zone of control like any
  other cell, but a city with a hostile population and no holder starts with its irregular share
  field open.
- **Granularity.** Demand and supply are per province, never per cell; nothing is placed with an
  area of effect.

## Consequences

- Fast advances leave rear areas that drift toward insurgents unless security troops follow, so
  occupation costs manpower in proportion to population and hostility.
- The player sees one coverage figure per occupied province and a list of key nodes, not a
  per-cell garrison map; the planner handles the rest.
- New register parameters: base density, hostility weights, urban factor, insurgent growth rate,
  auxiliary quality weight, city threshold.
- Tests in W2: coverage of 1 or more never grows insurgent shares through this channel; a held node
  keeps its label and edges with all surrounding cells hostile; demand scales linearly with
  population at fixed hostility.
