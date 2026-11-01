# 0075. Infrastructure interdependence and cascading failures

Status: proposed
Date: 2026-10-24
Design sections: 5.3, 13.4, 9.11, 19.4, ADR 0034
Phase: P8 (P7 for war damage)

## Context

Hurricanes, heatwaves, cyberattacks and war damage cascade across infrastructure: power loss
stops water pumping and telecoms, which stop payments and logistics. Recovery time, not initial
damage, dominates total loss. Interdependent networks fail more abruptly than isolated ones
(Buldyrev et al. 2010; Rinaldi, Peerenboom and Kelly 2001). The design has a transport graph, a
power sector (0034) and financial contagion, each with its own propagation, but no dependencies
between them, and no communications layer (undersea cables, satellites, positioning).

## Options

1. Status quo: independent systems.
2. A multi-layer dependency network per province (power, water, telecoms, transport, payments),
   with deterministic failure propagation and repair capacity; undersea cables and satellite
   services as edges and nodes of the telecoms layer; cyber operations as hazards on nodes.

## Decision

Option 2.

- Each province has service levels for five layers and a small dependency matrix between them.
  A shock (disaster, attack, war damage) lowers a layer's service; dependents lose service in
  proportion; repair restores it at a rate limited by construction capacity and crews (P128),
  shared with reconstruction (0086).
- Undersea cables and satellite constellations are telecoms edges and nodes with chokepoints
  (Red Sea, Luzon Strait); cuts and anti-satellite debris come from the hazard library (0051).
- Cyber operations: states hold a capability stock; attacks are hazards against infrastructure
  and financial nodes; attribution is uncertain and appears in dossiers as signals.
- The same propagation code serves financial contagion and supply-chain rationing where
  possible, so channel shutdown and Shapley attribution see cascades across layers.

## Migration

- P8 (P7 for war damage and cyber).
- Data: submarine cable maps (TeleGeography's public map has restrictive terms; use open
  alternatives or derived topology, ADR 0050), OSM power and telecom features (ODbL, in the
  manifest).

## Tests

- A toy power outage propagates to water and telecoms in the dependency order and recovers on
  the repair-capacity schedule.
- The Hurricane Maria (2017) replay shows a recovery curve of the right length for Puerto
  Rico's grid within wide bands.

## Consequences

Disasters and attacks cost what they cost through cascades and slow recovery, and resilience
investment becomes a lever.
