# 0150. Air power, drones and air defence

Status: proposed
Date: 2026-10-28
Design sections: 5.3, 11.5, 23.7; ADRs 0075, 0145, 0149, 0152
Phase: P7 (2.0-beta)

## Context

11.5 lets air superiority set strike risk on edges and infrastructure. Since 2000, air campaigns
have been bound by sortie generation and munitions more than by aircraft counts, and 2022 showed
mutual air denial by layered air defence. Drones range from strategic ISR to FPV.

## Options

1. Air superiority as a theatre modifier only.
2. Sorties from bases into air zones, layered air defence, and drones split by role.

## Decision

Option 2.

- **Bases and zones.** Aircraft live at airbases (OurAirports nodes plus military fields) and fly
  into air zones, clusters of cells drawn per theatre.
- **Sortie generation** per wing per day = available airframes × ready crews × turnaround rate,
  capped by runway capacity, fuel and munitions at the base.
- **Missions.** Air superiority, suppression and destruction of air defences, interdiction against
  network edges and depots, close support to segments, strategic strike, ISR, airlift, tanking and
  AEW, allocated daily by an air tasking order (written by player or planner).
- **Air defence.** Each zone holds layered radars and SAMs with interceptor stocks; probability of
  kill per shot falls with EW, low altitude, standoff weapons and suppression.
- **Air superiority** per zone and side: σ(log of effective sortie ratio minus enemy air defence
  plus own air defence), from denied to supremacy; both sides can be denied at once. It feeds E in
  0149, interdiction effectiveness and ISR coverage.
- **Drones.** Strategic ISR and armed MALE drones fly as aircraft. FPV and loitering munitions
  enter 0149 as fires with their own ammunition class. One-way attack drones and cruise missiles go
  through 0152 against infrastructure and into the cascades of 0075.
- **Quality** (generation, radar, missiles, AEW, training) is a moddable table with uncertain
  ranges.

## Migration

- P7 stage W6, in `ceteris-war` (air).

## Tests

- 2022: neither side reaches superiority over the front in ensembles with 2022 inventories.
- Kosovo 1999 as an out-of-period check: a large quality gap with strong suppression reaches
  superiority.

## Consequences

Air power is limited by bases, crews and munitions, as in the period. Air-to-air quality has thin
data since 2000; its ranges are wide.
