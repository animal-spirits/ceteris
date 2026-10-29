# 0066. Leaders, tenure and succession

Status: proposed
Date: 2026-10-24
Design sections: 10.6, 10.8, 13.5, 2.4
Phase: P6

## Context

Politics in the design runs at the level of institutions, groups and parties; leaders exist
only as stochastic attributes (2.4). Many political events happen at the leader time scale:
irregular exits (coups, uprisings, assassinations), succession crises in personalist
autocracies, term-limit evasion, and leaders who fear punishment after office clinging to
power. Archigos codes how leaders entered and left office and their fate afterwards (Goemans,
Gleditsch and Chiozza 2009); REIGN covers more than 2,300 leaders monthly since 1950 with
election dates. Forecasting work models irregular leadership change with split-population
duration models (Beger et al. 2014).

## Options

1. Status quo: leaders as attributes, coups as state-level hazards.
2. A generated leader per state (no real names in v1) with age, tenure, manner of entry and an
   irregular-exit hazard depending on regime type, selectorate structure, economic shocks,
   coup-proofing, protest and age; succession rules and crises as explicit states.

## Decision

Option 2.

- Each state has a leader with age (mortality from 13.5), tenure and entry manner. The
  irregular-exit hazard (P113) follows a split-population duration model estimated on Archigos
  and REIGN covariates; regular exits follow elections and term limits, which leaders in
  weak-constraint states may try to evade.
- Post-tenure fate (exile, prison, death) enters the leader's planner weights: a high expected
  penalty raises the value of staying, as in the literature.
- Succession in personalist autocracies: when the leader exits, a crisis state raises coup,
  purge and policy-reversal hazards until consolidation.
- Leaders are generated, not named, consistent with 2.4; the chronicle (0078) gives them
  generated names and biographies. Amended by ADR 0183 (28 Oct 2026): the 2000 roster carries
  real names; successors are generated.

## Migration

- P6, in ceteris-polity.
- Data: Archigos and REIGN (licences to verify, ADR 0050).

## Tests

- Simulated distributions of leader tenure and the share of irregular exits 2011-2024 within
  tolerance by regime type, including the Sahel coup cluster as a pattern.

## Consequences

The chronicle gets its main characters and politics gets the leader time scale where much of
2011-2024's instability happened.
