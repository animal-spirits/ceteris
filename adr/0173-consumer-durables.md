# 0173. Consumer durables

Status: proposed
Date: 2026-10-28
Design sections: 6.4, 7.1, 15.4, 15.5; ADR 0035
Phase: P5

## Context

Housing is the only durable a pop owns. Cars, appliances and electronics are bought as flows,
so their purchases do not bunch or postpone. Durable spending falls far more than other
consumption in recessions, and purchase subsidies move timing: the 2009 Cash for Clunkers
programme pulled sales forward and reversed them within a year (Mian and Sufi 2012). In 2020-21
spending shifted from services to goods, which is part of the COVID case of 15.5. The car stock
also drives fuel demand and the vehicle-technology diffusion of ADR 0035.
Gap audit (2026-10-28): https://claude.ai/artifact/HRDQVbjJz8MrcDuPphPyYs

## Options

1. Status quo: durables are flows.
2. Durable stocks per pop with a stock-adjustment demand rule.

## Decision

Option 2.

- Pops hold stocks of vehicles and of other durables, depreciating at published rates. Demand
  for each is replacement plus a partial adjustment of the stock toward a target that depends
  on permanent income and the user cost (price, interest rate, depreciation), which makes
  purchases sensitive to credit (8.5) and uncertainty.
- A purchase subsidy lever changes the user cost for a time-limited window.
- The vehicle stock per province feeds fuel demand and ADR 0035's vehicle-technology diffusion.

## Migration

- P5: two stock arrays per pop in the household block; the adjustment rule in the demand step.
- Pack: 2000 vehicles per capita (WDI, OICA) and durables shares from household surveys.

## Tests

- With the adjustment speed at one, durables behave as flows and results equal the status quo.
- In a toy recession the fall in durable purchases is a multiple of the fall in nondurables.
- A Cash for Clunkers replay pulls purchases forward and reverses within 12 months.

## Consequences

Recession dynamics and the 2020-21 goods boom gain a timing channel. Register: 1 entry
(adjustment speed).
