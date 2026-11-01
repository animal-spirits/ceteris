# 0032. Cohort demography: education transitions, fertility timing and households

Status: proposed; amended per ADR 0095 (2026-10-26)
Date: 2026-10-24
Design sections: 6.1, 6.2, 6.3, 8.6, 8.7 (land and housing), 17.1
Phase: P4; age bands arrive in P4 (P2 pops are province by education)

## Context

Pops already carry size by five-year age band and sex (6.1). What is missing is how education
moves through cohorts and how fertility timing works. Education attainment is mostly fixed by
about age 30 and then carried by the cohort for life, and education composition dominates
long-run human capital: the Wittgenstein Centre multistate projections (K.C. et al., WCDE v3,
2023-24) model population by age, sex and education for about 200 countries to 2100 with
education-specific fertility and mortality. Period total fertility misleads when births are
postponed (Bongaarts and Feeney 1998), which matters for the East Asian and European
low-fertility episodes that the model must reproduce. Housing demand depends on the number of
households, not people: headship rates by age.

## Options

1. Status quo: education levels per pop with enrollment rates (6.3); period TFR (6.2).
2. Multistate cohort-component demography: education progression ratios by cohort and sex,
   education-specific fertility and mortality differentials, a fertility tempo component, and
   headship rates.
3. Individual life-course microsimulation.

## Decision

Option 2.

- Education: each cohort aged 15 to 34 moves up education levels by progression ratios that
  respond to education spending and household income (P62), with WCDE schedules as priors and
  the Wittgenstein reconstruction (already in the manifest, wic-reconstruction) as the 2000
  initial state.
- Fertility: quantum (TFR from 6.2) and tempo (mean age at childbearing) are separate; the
  tempo shift responds to female education and housing costs (P63). Age-specific fertility
  follows a parametric schedule shifted by tempo.
- Mortality: Brass level and slope by education group, with education-specific differentials
  from WCDE life tables.
- Households: headship rates by age, sex and education turn pops into households for housing
  demand and for the consumption rule's household size.
- Ageing is a shift of age bands each year (19.4 already plans this).

## Migration

- P4: demography module, which brings age bands and age-specific participation to labour supply.
  P2 pops are province by education (ADR 0095).
- Data: WCDE v3 transition schedules and life tables (licence to audit, ADR 0050), UN WPP
  (already listed), WIC reconstruction.
- Register P62, P63. Nesting benchmark: with WCDE scenario inputs held fixed, the module
  reproduces WCDE SSP2 population by age, sex and education to 2100 within tolerance.

## Tests

- Population conservation by cohort cell holds every year (exact integers).
- The nesting benchmark above.
- Tempo: a simulated postponement lowers period TFR while completed cohort fertility stays
  constant, as in Bongaarts-Feeney.

## Consequences

Pensions, labour supply, conscription pools, life-cycle saving and housing demand all key on
the same cohort structure. P7's manpower and P5's pensions (0044) depend on this ADR.
