# 0084. Price controls, subsidies, parallel markets and currency substitution

Status: proposed
Date: 2026-10-24
Design sections: 8.2, 9.2, 9.9, 9.12, 9.6
Phase: P5

## Context

Many 2000-2024 economic crises in emerging markets ran through distorted prices: fuel and food
subsidies that strained budgets (and fell with the 2014 oil drop, rose in 2022), price controls
that produced shortages and queues (Venezuela), official exchange rates defended with capital
controls while parallel rates diverged (Argentina, Venezuela, Iran, Nigeria, Egypt), and flight
from domestic money into dollars as inflation rose, up to hyperinflation (Zimbabwe 2007-08,
Venezuela 2016-19). Money demand collapses as expected inflation rises (Cagan 1956), and
dollarization is persistent once established (Reinhart, Rogoff and Savastano 2003). The design
has pegs, controls and money, but no capped prices, no parallel markets and no currency
substitution.

## Options

1. Status quo.
2. Price ceilings and subsidies as policies on goods; parallel markets for goods and foreign
   exchange where official prices are capped; currency substitution in household and firm
   portfolios as a function of expected inflation.

## Decision

Option 2.

- Subsidies and ceilings per good and state: subsidies are a fiscal cost at the gap between
  world and domestic price; a binding ceiling triggers the rationing of 8.2 at the official
  price, and a parallel market clears the excess demand at a higher price with a leakage share
  and an enforcement cost (P139).
- Parallel foreign exchange: under controls with an official rate, excess currency demand from
  the demand system (9.9) clears in a parallel market; the parallel premium is reported and
  feeds expectations.
- Currency substitution: the weight of foreign-currency deposits and cash in domestic
  portfolios rises with expected inflation and depreciation, with hysteresis (P138); money
  demand falls accordingly, so high inflation can accelerate into hyperinflation when deficits
  are monetized.

## Migration

- P5, in ceteris-finance and ceteris-trade.
- Data: IMF energy subsidy estimates and AREAER exchange arrangements (terms to audit),
  parallel exchange rate series (Reinhart-Rogoff, research use), dollarization ratios from IMF
  data (in the manifest under imf-data).

## Tests

- A toy economy with monetized deficits and rising expected inflation shows falling real money
  demand and accelerating inflation, as in the Cagan model.
- A binding ceiling produces a parallel price above the official one and fiscal costs equal to
  the subsidy gap.

## Consequences

Emerging-market crisis dynamics (subsidy reform, parallel premia, dollarization,
hyperinflation) become reproducible, and so do their policy fixes.
