# 0165. Mobilization and the war economy

Status: proposed
Date: 2026-10-28
Design sections: 6.5, 8.2, 9, 11.7, 23.7; ADRs 0020, 0032, 0043, 0048, 0086, 0105, 0109, 0130, 0145
Phase: P7 (2.0-beta), lab profile

## Context

11.7 and 0048 tie war to the economy: mobilization removes workers, procurement shifts demand,
defence output is putty-clay capacity with learning, war finance runs through debt and money.
These links are the lab half of the war model and need one statement.

## Decision

- **Manpower.** Pools from age-sex cohorts (0032) by fitness and education, split by skill group
  (0105). Recruitment law: volunteer, selective service, partial or general mobilization, each with
  eligible cohorts, exemptions and a legitimacy cost. Draft flight through 0109.
- **Training pipelines** by tier (0146) with lags of months to years; shortened training lowers
  quality.
- **Casualties** return to cohorts: deaths, long-term disability lowering labour supply, prisoners
  returning at peace.
- **Procurement** is a final-demand vector into the IO table across arms, aerospace, shipbuilding,
  electronics, chemicals, steel and fuel.
- **Production lines** per equipment type are putty-clay capacity (0020) with Wright's-law
  learning; switching a line resets learning. Converting civilian capacity takes 12 to 24 months,
  capped by bottleneck inputs (explosives precursors, chips, specialty steel, skilled labour).
- **Imports and aid** move equipment stocks and create spare-parts dependence (23.6 arms deals).
- **Finance.** Taxes, debt and money creation through the finance module and 0043; sanctions
  through trade and finance (11.8); rationing, price controls and export bans as 23.7 levers.
- **Damage.** Capital and housing in contested cells destroyed by engagement intensity and urban
  share; infrastructure service through 0075; reconstruction backlog through 0086.
- **Displacement** by danger and network access (6.5); war weariness into approval (P6).

## Migration

- P7 stage W1, in `ceteris-war` (mobilize) and `ceteris-econ` (defence sectors in the IO pack,
  prepared in W0).

## Tests

- A mobilization in a toy state lowers civilian output by the removed labour share times its
  marginal product (0048).
- The European 155 mm ramp of 2022-25 is reproduced within its published range by a conversion
  order with 2022 capacity.

## Consequences

Every military choice carries a measured economic cost, which is the part of the war model no war
game has and the part the quartermaster seat (0130) plays.
