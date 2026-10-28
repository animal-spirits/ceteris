# 0166. War interaction surfaces and modding

Status: proposed
Date: 2026-10-28
Design sections: 20, 23.3, 23.4, 23.8; ADRs 0049, 0128, 0130, 0132, 0134, 0140, 0145, 0147
Phase: P7 (2.0-beta) onward

## Context

The owner asked for several interaction surfaces and for customization. Surfaces must stay views
that write state only through orders (0145), and mods must keep runs reproducible.

## Decision

- **Surfaces**, each a view plus order verbs, with a default delegation level (0147):
  - Strategic map (L4): fronts on cells, theatres, map modes for control, supply state, air
    superiority, sea control, intelligence confidence, civilian harm and war-economy strain.
  - Theatre view (L2 to L3): segments with estimated force ratios, axes, reserves, phase lines,
    HQs and their latency, supply reach; draw a front, assign objectives, hand to a general.
  - Order of battle editor (any): formation tree, fill and readiness, span warnings, and a template
    designer showing combat values, supply per day, manpower and cost while editing.
  - Production and procurement (quartermaster): lines with learning curves, bottlenecks, order
    books, stockpiles as days of fire, imports and aid.
  - Logistics view (L2 to L4): flows in tonnes per day, utilisation, depots, railheads, truck
    radius, interdicted edges, each shortage traced to its binding edge.
  - Battle planner (L1 to L2): phased operations with objectives, axes, fires, air, deception,
    timing and abort conditions, and an outcome forecast with a band computed on beliefs (0153).
  - Air tasking and naval board (L2 to L3), command console (appointments, levels, risk tolerance,
    deviations), mobilization and war finance, intelligence, and the war ledger (a view only).
- **Event cards** (23.8) for war moments; every response is an ordinary order. Panel titles change
  by regime class under 0140; actions do not.
- **Unit symbology** (owner, 2026-10-28): NATO APP-6 symbols from operational zoom down, with
  Ceteris amplifiers under the frame (supply and cohesion bars, delegation level, order latency);
  at strategic zoom, country-coloured atlas pills with army totals on control-shaded cells.
  Estimates are dashed and faded with a confidence. Wargame counters and a staff-map style may
  ship later as skins. Mockups: https://claude.ai/artifact/C8TXwPJD8uPSoFXkHboCmV. Symbology never
  changes state.
- **Modding.** Data mods in the 23.3 manifest: equipment catalog, templates, doctrines,
  consumption tables, OOBs, air zones and naming grammars. Parameter lanes for every coefficient.
  Model switches in 17.2 for the resolver (0149) and onset model (0110). Mechanism mods (custom
  resolver, planner, targeting) as sandboxed modules under 0134. Scenario editor with logged
  edits. Friction presets (Staff college, Commander, Lab) that change information and friction
  only (23.4); Ironman locks them. Mods that touch state enter the run hash (0128).
- **Seats.** Steward at war (2.x), quartermaster and mediator (0130); a theatre-commander seat
  scored on one theatre against the AI baseline is a candidate minor.

## Migration

- Surfaces arrive with their stages: strategic map W2, logistics and production W1 and W3, OOB
  editor W4, theatre view, command console and battle planner W5, air tasking W6, naval board W7,
  intelligence W8; modding layers W4 (data) and W10 (mechanism mods, presets).

## Tests

- No surface changes a state hash except through an order in the intervention list.
- A modded run replays to the same hash; an unmodded run's hash never includes a mod hash.

## Consequences

The same war can be played from the front, the staff or the factory. Each surface adds UI work
that grows with the number of regime registers under 0140.
