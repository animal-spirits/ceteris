# 0147. Command, control and delegation

Status: proposed
Date: 2026-10-28
Design sections: 12.3, 12.6, 23.5, 23.8; ADRs 0066, 0145
Phase: P7 (2.0-beta), after P6 characters

## Context

HOI3 made headquarters matter: range, leadership and theatre AI. 23.5 sets a command spectrum from
micromanagement to delegation, and 23.8 a strategic AI with war planning. Neither defines orders,
how long they take, or what a delegated planner may decide.

## Options

1. Instant orders and a flat command tree (HOI4).
2. HQs with staff capacity and order latency, typed orders written by player and AI alike, and
   delegation per HQ.

## Decision

Option 2.

- **Headquarters** have a commander, staff capacity (orders and plans per day, a trained stock
  that falls with HQ losses and displacement), communications quality (from equipment, EW
  pressure and distance) and a location on a cell, where they can be targeted.
- **Span of control.** Nominal three to six subordinates per echelon; above it, decision cycles
  lengthen and coordination penalties rise.
- **Orders** are typed objects: a mission verb (attack, defend, delay, screen, hold, encircle,
  pursue, withdraw, reserve, relieve), an objective as cells or a line, boundaries, phase lines,
  H-hour, risk tolerance, and priorities for fires, air and supply.
- **Latency** per echelon: base × (1 + load / capacity) / comms × doctrine. Mission command
  shortens it and lets subordinates apply their own judgement; detailed command keeps plans exact
  and slow (P-numbers in the register). A destroyed HQ leaves subordinates on their last orders.
- **Delegation levels**, set per HQ: L0 brigade orders; L1 division objectives; L2 front lines
  and axes; L3 a theatre goal and risk tolerance given to a general; L4 national war aims and
  theatre priorities; L5 nothing military (the quartermaster seat, 0130). Direct control of a
  formation does not bypass latency.
- **Generals** as in 23.5: latent operational skill, logistics skill, initiative and risk
  aversion, seen through dossiers; loyalty against competence in appointments (Egorov and Sonin
  2011). A general may modify an order that clashes with their risk aversion and reports why.
- **Theatre planner.** Daily, from its side's belief state (0153): score front segments by force
  ratio, terrain and supply, choose a main effort, assign reserves, write orders. Weekly, a fixed
  number of rollouts of the 0149 resolver on beliefs, seeds keyed by (theatre, day), within 23.8's
  10% budget. Corps planners handle brigades; the logistics planner only sets priorities.

## Migration

- P7 stage W5, in `ceteris-war` (command); needs P6 characters.

## Tests

- An order issued at L0 and the same order written by a planner produce the same state.
- Delegated theatres reproduce the W4 outcome distributions (nesting, 0145).
- The theatre planner beats a scripted baseline (hold the line, attack the weakest segment) in
  paired runs.

## Consequences

HQ placement, staff and doctrine become decisions with visible costs. Whether players delegate
depends on planner quality, which makes the theatre planner the critical path of the command
layer.
