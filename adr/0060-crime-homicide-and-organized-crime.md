# 0060. Crime, homicide and organized crime

Status: proposed
Date: 2026-10-24
Design sections: 6, 10.7, 11.6, 5.3, 8.5
Phase: P6 (homicide and organized crime), P7 for state capture

## Context

Violence in the design is war and civil conflict. Intentional homicide kills far more people:
UNODC's Global Study on Homicide 2023 estimates about 440,000 homicides a year in 2019-2021
against about 94,000 conflict deaths, and homicide was still about twice the conflict toll in
2022. The global rate fell from 6.9 to 5.8 per 100,000 between 2000 and 2021, steeply in Europe
and Asia, while Africa's stayed flat and Latin America's stayed high. Organized crime accounts
for about a fifth of homicides worldwide and half in the Americas. Drug-trafficking rents
follow transport routes, and interdiction shifts violence along them (Dell 2015). Cartel and
armed-group control of territory shapes Mexico, Central America, Ecuador, the Sahel and
Myanmar.

## Options

1. Status quo: no crime.
2. Homicide per province with interpersonal and organized-crime components; drug-trade rents as
   flows on the transport graph; organized-crime control of territory as a stock.
3. Individual offenders and police.

## Decision

Option 2.

- Interpersonal homicide rate per province as a function of the young-male share, inequality,
  urbanization, the marriage squeeze (0057) and policing capacity (P102).
- Illicit drugs as goods produced in a few regions (coca, opium, synthetic precursors) and
  shipped over the same route-choice graph (0017) to consumer markets; interdiction is a policy
  wedge on links. Route rents fund criminal groups, and their violence rises with rival groups
  and enforcement shocks (P103, P104).
- Organized-crime control: a stock per province of territory controlled by armed non-state
  actors, which taxes local output, corrupts officials (0046) and lowers state capacity (0067).
- Homicide victimization lowers trust (0056) and enters the mortality causes (0033).

## Migration

- P6: crime and drug-route modules; P7 links capture to insurgency (0069).
- Data: UNODC homicide statistics (terms restrict commercial reuse; check the data portal, ADR
  0050); UCDP (CC BY 4.0) for socio-political violence.

## Tests

- The simulated global homicide rate falls 2000-2021 with the regional pattern above.
- A toy interdiction on one route shifts flows and violence to the next-cheapest routes, as in
  Dell (2015).

## Consequences

The model's violence budget is no longer inverted, and drug policy, policing and state capture
become experiments.
