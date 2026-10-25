# 0062. Chronic disease risk and antimicrobial resistance

Status: proposed
Date: 2026-10-24
Design sections: 6.2, 8.6, 13.3, ADR 0033
Phase: P4 (NCD risk), P8 (AMR)

## Context

The epidemiological transition shifts mortality toward non-communicable disease as incomes
rise, with obesity and diabetes rising fast in middle-income countries; diet prices,
urbanization, tobacco and alcohol policy move the risk. Antimicrobial resistance is a growing
burden: the GRAM study (Lancet 2024) estimates 1.14 million deaths directly attributable to
resistance in 2021 and forecasts about 1.9 million a year by 2050 without new policy, with the
largest burden in South Asia. Resistance rises with human and livestock antibiotic use and
falls with stewardship and new drugs.

## Options

1. Status quo: non-communicable and communicable trends from income and spending only (0033).
2. A non-communicable risk index per pop and an AMR prevalence stock per region, each driving
   its cause group.

## Decision

Option 2.

- Non-communicable risk per pop (a body-mass and risk-factor index) rising with income and
  urbanization, moved by the relative price of food groups from the food market (8.6), and by
  tobacco, alcohol and sugar taxes (P106). It shifts the non-communicable cause group.
- AMR prevalence per region rises with antibiotic consumption (human and livestock) and falls
  with stewardship policy and stochastic new-drug arrivals on the hazard tape (P107). It
  multiplies case fatality of bacterial infections in the communicable group.

## Migration

- P4 and P8.
- Data: NCD-RisC estimates (terms to audit); IHME GBD and GRAM files are non-commercial and
  stay in the research pack (ADR 0050); published GRAM parameters can anchor priors.

## Tests

- Simulated obesity and diabetes prevalence rise 2000-2020 in the order observed by region.
- AMR deaths 2000-2021 within GRAM's back-series bands; 2050 forecasts are a scenario anchor,
  not a target.

## Consequences

Health futures depend on food prices, taxes and antibiotic policy, not only on income.
