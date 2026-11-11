# 0174. Temporary labour migration

Status: proposed
Date: 2026-10-28
Design sections: 6.5, 15.4; ADR 0109
Phase: P4

## Context

ADR 0109 models migration as permanent admissions under a quota. Much labour migration is
temporary by contract: workers in the Gulf states under the kafala system, seasonal farm work in
Europe and North America, and EU posted workers. Non-citizens are about 88% of the UAE's and
Qatar's populations, and remittances from these workers are a large share of GDP in Nepal, the
Philippines and Kerala. The permanent channel gets the stocks wrong in the Gulf and makes
remittances too small (family joins, and ties weaken).
Gap audit (2026-10-28)

## Options

1. Status quo: permanent migration only.
2. A second, contract channel with fixed tenure.

## Decision

Option 2.

- Contract pops carry an origin and a tenure; they return when it ends unless renewed.
  Families stay behind, so remittance shares are high and do not decay as in the permanent
  channel.
- Quotas come from destination labour demand in the sectors that use contracts (construction,
  domestic work, agriculture), so the channel shrinks in a downturn (Gulf 2015-16, 2020).
- Destination policy levers: quota size, sponsor rules and the right to switch employer.

## Migration

- P4: a tenure and an origin field on migrant pops; the contract admission rule next to 0109.
- Pack: 2000 contract stocks from UN DESA migrant stock and Gulf labour statistics; corridors
  from KNOMAD (ADR 0182).

## Tests

- With the channel off, results equal 0109.
- Gulf non-citizen shares 2000-2015 and remittances to Nepal and the Philippines within
  tolerance when oil revenue follows history.

## Consequences

Gulf demography and South Asian remittances follow the observed paths, and the 2015 and 2020
returns become endogenous. Register: none new; tenure is data.
