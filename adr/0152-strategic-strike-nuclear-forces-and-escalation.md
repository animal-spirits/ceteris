# 0152. Strategic strike, nuclear forces and escalation

Status: proposed
Date: 2026-10-28
Design sections: 11.2, 11.4, 13.2, 23.7; ADRs 0029, 0073, 0075, 0108, 0110, 0132, 0145
Phase: P7 (2.0-beta)

## Context

Nuclear status is a state attribute (11.4) and deterrence a lane (0110); proliferation follows
0073. Conventional long-range strike against energy and transport has become central since 2022.
There is no data on escalation past conventional war between nuclear states since 1945.

## Options

1. Nuclear war as a scripted end state.
2. Arsenals, an escalation ladder with hazards as lanes, and computed effects.

## Decision

Option 2.

- **Conventional strike.** Ballistic and cruise missiles and long-range drones hit targets with
  economic identity (power plants, refineries, rail junctions, bridges, ports, factories, depots,
  HQs). The targeting planner scores targets by IO centrality and 0075 dependencies. Missile
  defence uses finite interceptor stocks, so cost exchange becomes a production problem.
- **Arsenals.** Warheads by yield and delivery (ICBM, SLBM, bombers, dual-capable aircraft,
  tactical missiles), alert posture and C2 survivability.
- **Ladder.** Rungs 0 to 5: peace or proxy conflict; conventional war in a nuclear dyad; explicit
  threats and raised alert; demonstration or limited use; counterforce; countervalue. Each step
  has a hazard in stakes, conventional defeat on core territory, leader risk tolerance, balance of
  resolve, regime type and use-or-lose pressure. Every hazard is a prior run as a lane (0108),
  reported as sensitivity, never as a forecast.
- **Effects** computed per cell: blast, thermal and radiation casualties from population and yield
  (Glasstone and Dolan 1977), destroyed capital and infrastructure, fallout by wind, and for large
  exchanges stratospheric soot into the climate emulator with crop losses after Xia et al. (2022),
  carried by the food system (0029).
- **No nuclear victory condition.** AI restraint comes from estimated costs, not a rule.
- **Pacing.** Crises from rung 2 may switch to 0132's real-time mode.

## Migration

- P7 stage W9, in `ceteris-war` (strategic). Data: FAS and Nuclear Notebook as cited priors.

## Tests

- No proliferation cascade under strong alliance credibility (0073).
- Effect tables reproduce the source curves for reference yields.
- Ensembles with default lanes reproduce zero nuclear use over 2000-2024 in at least the share the
  lane's prior implies.

## Consequences

Nuclear weapons shape bargaining and can be used, at computed cost. Escalation results are always
presented as conditional on uncertain priors.
