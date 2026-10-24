# 0059. Learning quality, not only years of schooling

Status: proposed
Date: 2026-10-24
Design sections: 6.3, 7.2, 7.5, 8.7, 17.1 (P42)
Phase: P4 (dynamics); P2 can read the 2000 values

## Context

Education in the design counts attainment (WCDE levels, ADR 0032) and maps each level to an
efficiency (P42). A year of school buys very different skills across countries: the Harmonized
Learning Outcomes database (Angrist et al. 2021, Nature; 164 countries, 98% of world
population) shows enrolment rising after 2000 with limited learning gains in many low-income
countries. Learning-adjusted years of schooling (LAYS) are expected years times harmonized
learning outcome over 625. Without a quality term, the model overstates human capital where
schooling expanded without learning, and so overstates 2050-2100 growth there, through labour
efficiency and the catch-up rate (7.5).

## Options

1. Status quo: efficiency by level only.
2. A learning-quality stock per province that scales the efficiency of each education level for
   the cohorts it schools.

## Decision

Option 2. Each province carries a learning-quality index (harmonized learning outcome units)
that moves with teacher supply, spending per pupil, child nutrition (0064) and school closures
from conflict and pandemics (P101). Each cohort's effective skill is its attainment times the
quality it was schooled under, so quality changes reach the labour force with a lag of years,
as in the data. Emigration of the skilled lowers the stock where it is severe (brain drain),
and returning migrants raise it.

## Migration

- P2: initialize education efficiency (P42) by country from 2000 learning outcomes, so P2
  labour already reflects quality.
- P4: the dynamic stock.
- Data: World Bank Harmonized Learning Outcomes and Human Capital Index (CC BY 4.0).

## Tests

- The simulated cross-section reproduces "schooling without learning": attainment and learning
  diverge in low-income countries 2000-2017.
- Simulated COVID school closures produce learning losses of the published order.

## Consequences

Long-run growth in low-income countries stops being overstated, and education policy gains a
quality lever.
