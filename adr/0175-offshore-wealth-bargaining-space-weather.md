# 0175. Offshore household wealth, bargaining coverage and space weather

Status: proposed
Date: 2026-10-28
Design sections: 6.4, 6.3, 9.6, 13.4; ADRs 0051, 0075, 0138
Phase: 1.x minors

## Context

Three small items have no place in the design. Households hold about 10% of world GDP in
offshore wealth, concentrated at the top (Alstadsaeter, Johannesen and Zucman 2018), so the
wealth distribution and tax bases are wrong at the top. Wage rigidity (P40) is one value per
country, but collective-bargaining coverage ranges from under 10% to over 90% across OECD
countries. Space weather (a Carrington-scale storm) is a low-probability hazard to grids and
satellites that is missing from 0051's hazard list.
Gap audit (2026-10-28): https://claude.ai/artifact/HRDQVbjJz8MrcDuPphPyYs

## Options

1. Leave all three out.
2. Add each as its own minor.

## Decision

Option 2, each as a 1.x minor.

- Offshore wealth: an accounting split of top-decile wealth into domestic and offshore shares
  by country from the published estimates. Offshore wealth is untaxed and earns the world
  rate. No new behaviour; tax evasion levers stay out.
- Bargaining coverage: P40's rigidity scales with coverage from the OECD/AIAS ICTWSS database,
  read with the labour institutions of 1.3. No new free parameter.
- Space weather: an extended-profile hazard in 0051 that damages the grid layer (0075) by
  geomagnetic latitude, following ADR 0138's extended-minor pattern.

## Migration

- Each ships independently in 1.x; none touches the lab default except bargaining coverage.

## Tests

- Each off by default leaves golden hashes unchanged.
- Top 1% wealth shares in Europe within the published offshore-inclusive band.

## Consequences

Small accuracy gains at the top of the distribution and in wage rigidity; one more tail hazard
for scenarios. Register: none.
