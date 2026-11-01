# 0048. Grounding the war module in modern evidence

Status: proposed
Date: 2026-10-24
Design sections: 11, roadmap P7 (military and war), 6.1
Phase: P7

## Context

The P7 design (roadmap section "Military and war") is HOI-like but simulation first. Several of
its parameters need modern calibration data rather than WWII-era defaults, and a few mechanisms
need explicit links to the economy:

- Munitions consumption and production rates are what bind in long wars; the 2022- war in
  Ukraine gives modern artillery and drone consumption data, and the 2022-25 ramp of shell
  production in Europe and the United States shows how slowly industrial capacity converts.
- Manpower depends on age-sex cohorts (ADR 0032), with the labour-market cost of mobilization.
- Effectiveness differs across armies far more than equipment counts suggest; force employment,
  training and cohesion explain much of it (Biddle 2004).
- Military spending follows action-reaction dynamics between rivals (Richardson models,
  estimated on SIPRI data).
- War severity follows a heavy-tailed distribution with no significant trend (Clauset 2018),
  which onset and termination models must reproduce in ensembles.
- Territorial claims seed disputes (ICOW).

## Options

1. Calibrate on historical WWII-era sources (Dupuy).
2. Calibrate consumption, conversion and effectiveness on post-1990 and 2022- data, and add the
   economic links explicitly.

## Decision

Option 2.

- Consumption rates per formation and posture from modern open-source assessments (P89 as data
  with ranges).
- Industrial conversion: defense output is putty-clay capacity (ADR 0020) that retools with
  lags and learning curves; surge capacity is limited by inputs and skilled labour.
- Manpower pools from cohorts; mobilization removes workers from their pops' labour supply.
- Morale and cohesion depend on legitimacy (10.7), casualties, supply state and leadership
  skill (P90); the Dupuy-style combat model uses them as multipliers.
- Military spending: Richardson-type reaction functions between rivals as the Tier 1a prior
  (P91), estimated on SIPRI.
- Claims from ICOW seed the disputes of 11.2.
- Validation: war severity distribution in ensembles against COW; ensemble war durations
  against 1946-onward data; the Ukraine 2022 hold-out case for consumption and front movement.

## Migration

- P7, in ceteris-war and ceteris-econ (defense sectors).
- Data: SIPRI military expenditure (already in the manifest, sipri-milex; terms to audit for
  release), ICOW (licence to audit), open-source consumption assessments (citation only; values
  entered as priors).

## Tests

- Ensemble war severities follow a power law with exponent within the COW estimate's interval.
- A mobilization in a toy state lowers civilian output by the removed labour share times its
  marginal product.

## Consequences

P7 inherits modern, economically consistent foundations; munitions and industrial capacity, not
abstract supply points, decide long wars.
