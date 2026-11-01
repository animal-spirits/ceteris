# 0064. Child nutrition and the care burden of ageing

Status: proposed
Date: 2026-10-24
Design sections: 6.1, 6.3, 8.7, 9.6, ADR 0059
Phase: P4

## Context

Two slow channels link today's shocks to future labour. Child stunting, driven by food prices,
conflict, water and sanitation, lowers cognitive skills and adult earnings (Hoddinott et al.
2013), so climate and food shocks reach human capital with a 15 to 20 year lag. Ageing raises
care needs, much of it supplied by unpaid household labour, mostly women's, and by migrant care
workers; that links ageing to female labour supply, migration and public spending.

## Options

1. Status quo.
2. A stunting rate per province feeding learning quality with a lag, and a care-need stock by
   age band drawing on household labour, migrant labour and public spending.

## Decision

Option 2.

- Stunting per province from food prices relative to income of the poorest pops, conflict,
  water stress (0038) and sanitation coverage; it lowers the learning quality of the cohorts it
  affects (0059) (P109).
- Care need by age band from age and health; met by household time (lowering female
  participation in 0057), migrant care workers (a labour demand in the migration model) and
  public long-term care spending (P110).

## Migration

- P4.
- Data: UNICEF-WHO-World Bank Joint Malnutrition Estimates (terms to audit); OECD long-term
  care statistics (terms to audit).

## Tests

- Simulated stunting 2000-2020 falls in the order observed by region.
- Raising public long-term care spending in a toy ageing state raises female participation with
  a plausible sign and size.

## Consequences

Climate and food shocks gain a human-capital echo, and ageing gains its care economy.
