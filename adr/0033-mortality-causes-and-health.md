# 0033. Mortality causes and health

Status: proposed
Date: 2026-10-24
Design sections: 6.2, 9.6, 13.2, 13.3
Phase: P4, with hazards added in P8

## Context

The Brass level of 6.2 responds to income (Preston curve), health spending, epidemics, conflict
and heat. Several large, documented mortality shocks of 2000-2024 come from other channels:
non-communicable diseases and their risk factors, air pollution, and "deaths of despair" in the
United States (Case and Deaton 2020). Air pollution alone is among the largest global mortality
risk factors (Burnett et al. 2018). Without cause groups, the model cannot separate a pandemic
from an opioid wave or a pollution policy, so counterfactuals on health spending, pollution or
pandemics have no channel to act on.

## Options

1. Status quo: one Brass level per pop.
2. Cause-group hazards (communicable, non-communicable, injuries, plus pollution, heat,
   conflict and famine) added on the Brass level, each driven by its own determinants.
3. A health system model with providers and insurance.

## Decision

Option 2.

- The Brass level becomes the sum of cause-group components, initialized from WHO Global Health
  Estimates by country, age and sex. The non-communicable component trends with income and
  ageing; communicable with income, health spending and urbanization; injuries with conflict
  and road transport.
- Additive hazards: PM2.5 exposure (ADR 0038), heat (13.2), epidemics (13.3), conflict (11.7),
  famine (ADR 0029), and a scenario hazard for deaths of despair (P65 sets its default to zero;
  it is a scenario lane until a mechanism passes ablation).
- Health spending is a fiscal category (9.6) and a household expenditure; it lowers
  communicable and part of non-communicable mortality with diminishing returns. No hospital
  system is modeled.

## Migration

- P4: cause-group decomposition; P8: pollution and heat hazards.
- Data: WHO Global Health Estimates (terms to audit; preferred to IHME GBD, whose licence is
  non-commercial, ADR 0050).

## Tests

- With all added hazards at zero, the Brass level equals the status quo.
- Cause-of-death shares by age for 2000-2019 within tolerance of WHO GHE for the calibration
  window.

## Consequences

Pandemic, pollution, heat and health-spending counterfactuals act on separate, measurable
channels.
