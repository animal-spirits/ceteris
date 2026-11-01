# 0070. Development finance actors

Status: proposed
Date: 2026-10-24
Design sections: 5.5, 9.4, 11.1, ADR 0043, ADR 0045
Phase: P5 (finance), P6 (strategic allocation)

## Context

Development finance shaped 2000-2024 infrastructure, debt and influence. China became the
largest official creditor to developing countries: AidData's Global Chinese Development Finance
Dataset 3.0 records 20,985 projects in 165 countries worth $1.34 trillion over 2000-2021, with
a geocoded version (ODC-By). Western donors and multilateral banks allocate by need, strategic
and commercial motives, and Chinese aid and lending have measurable growth effects (Dreher et
al. 2021). The rise of China's global influence is a named validation target, and the debt side
connects to restructuring (0043).

## Options

1. Status quo: aid as a fiscal transfer if at all.
2. Donor and lender actors (DAC donors, China's policy banks, multilateral development banks)
   choosing project portfolios by need, strategic and commercial motives; projects add
   infrastructure edges and capital, create debt with terms, and build influence.

## Decision

Option 2.

- Each lender allocates an annual budget across recipient states and project types (transport,
  energy, social) by a logit on need, alignment, resource access and commercial return (P119),
  estimated on 2000-2010 data.
- Projects add transport-graph edges, power capacity or public capital (0042) after
  construction lags, with efficiency from the recipient's capacity; loans enter the who-to-whom
  matrix with their terms and collateral (resource-backed loans).
- Influence: aid and lending raise alignment and soft power (0072) in the recipient.

## Migration

- P5 (debt and projects), P6 (strategic allocation).
- Data: AidData GCDF 3.0 (ODC-By; OSM-derived geometry under ODbL), OECD Creditor Reporting
  System (terms to audit), World Bank IDS (in the manifest).

## Tests

- Calibrated on 2000-2010 allocations, the model reproduces the 2013-2021 surge in Chinese
  lending and its regional distribution out of sample, within wide bands.

## Consequences

Infrastructure, debt and influence in developing regions acquire their main external driver.
