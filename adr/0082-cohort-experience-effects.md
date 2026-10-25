# 0082. Cohort experience effects on expectations and risk-taking

Status: proposed
Date: 2026-10-24
Design sections: 12.2, 9.13, 8.6, ADR 0055
Phase: P5

## Context

People weight their own lived experience when forming expectations. Malmendier and Nagel (2016)
show that inflation expectations differ by age in line with the inflation each cohort has lived
through, with recent experience weighted more; cohorts that lived through poor stock returns
take less financial risk for decades (Malmendier and Nagel 2011). The design's heuristic
expectations (12.2) are the same for all ages, although pops already carry age bands.
Experience effects explain why the same shock moves expectations differently in an old and a
young society, and why post-crisis generations stay cautious.

## Options

1. Status quo: one expectation rule per market.
2. A cohort-specific experience heuristic: each age band's expectation of inflation and returns
   is a weighted average of what it has experienced, with weights declining into the past.

## Decision

Option 2, as one more heuristic in the Brock-Hommes set (12.2), per age band:

- Experienced inflation and asset returns are tracked per age band as exponentially weighted
  averages over the band's lifetime, with the decay estimated by Malmendier and Nagel (P135).
- Household inflation expectations and stock-market participation in the demand system (9.9)
  use the band's experience weight; the population average aggregates them by wealth.

## Migration

- P5; the state is two running averages per age band and region.
- Validation: age gradients in survey inflation expectations (University of Michigan survey,
  public) and age differences in equity participation from household finance surveys (research
  pack only where restricted).

## Tests

- After a simulated high-inflation episode, younger cohorts revise expectations more and older
  cohorts less, matching the Malmendier-Nagel weighting.

## Consequences

Ageing changes how economies respond to shocks, and inflation episodes leave a generational
imprint.
