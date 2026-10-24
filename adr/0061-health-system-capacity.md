# 0061. Health-system capacity

Status: proposed
Date: 2026-10-24
Design sections: 6.2, 9.6, 13.3, ADR 0033
Phase: P4 (with pandemics in P8)

## Context

Mortality responds to health spending (6.2, ADR 0033), but there is no capacity: beds,
physicians, nurses and service coverage. The pandemic model therefore cannot saturate
hospitals, which drove case fatality and lockdown decisions in 2020-21, and maternal and child
mortality and treatment of non-communicable disease have no supply side. Health spending is
also a large and growing fiscal category in ageing states.

## Options

1. Status quo: spending lowers mortality directly.
2. A capacity stock per province (beds, physicians, nurses) and a service-coverage index, built
   by spending with lags and depleted by emigration of health workers.

## Decision

Option 2. Capacity per province is built from public and private health spending with
construction and training lags, and lost to depreciation and emigration of health workers. Case
fatality in epidemics rises when patients exceed capacity (P105); coverage lowers maternal,
child and treatable non-communicable mortality in the cause groups of 0033. Health-worker
migration runs through the migration module and is a lever (recruitment agreements).

## Migration

- P4; the saturation term joins the SEIR model in P8.
- Data: World Bank re-publications of WHO capacity indicators (CC BY 4.0) where available.
  WHO's own data portal shows CC BY-NC-SA 3.0 IGO, which is non-commercial; see ADR 0050.

## Tests

- A toy epidemic above capacity shows the case-fatality rise of the chosen function; below
  capacity, none.
- Simulated physician density and UHC coverage 2000-2020 within tolerance by region.

## Consequences

Pandemic policy, health spending and health-worker migration act through capacity, as they do
in practice.
