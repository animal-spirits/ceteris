# 0138. Natural capital as extended-profile minors

Status: proposed
Date: 2026-10-28
Design sections: 3.4, 13.2, 15.7, 17, 17.2 (S10), 19.2; ADRs 0029, 0036, 0037, 0038, 0076, 0135, 0136
Phase: after P8, as 2.x and 3.x minors

## Context

"Nature moves" was a 5.0 candidate under ADR 0136. Most of it is already before 1.0: emissions
drive FaIR and MESMER (0037), damages run on three lanes (0036), and water, air, land, aquifers
and food are P8 systems (0029, 0038, 0076). Climate is therefore a 1.0 feature, so it fails the
first test of 0136.

What stays frozen: impacts are regressions fitted on 2000-2024 variation, and natural capital
(fish biomass, standing forest, soil carbon, pollinators, intactness) has no stocks, so
overharvest, collapse and recovery cannot happen. These variables move a few percent per decade.
The 2011-2024 holdout cannot separate them from the regression lane, so they would fail
ablation (15.7). Fisheries are the exception: stock assessments give biomass series, and
collapses since 2000 are observed. Feasibility study:
https://claude.ai/artifact/WS9DBVdJaBuEba8aVVgqZ5

## Options

1. Status quo: no natural-capital stocks.
2. Natural capital as a lab-default major (5.0).
3. Natural capital in the extended profile as minors. Fisheries go into the lab as opt-in on
   their own scorecard, and process-based yields become a fourth damage lane.

## Decision

Option 3. This ADR does not make natural capital a major.

- Process yields and runoff: the GGCMI Phase 2 crop-model emulators (Franke et al. 2020) give
  yield per 0.5 degree cell as a polynomial in CO2, temperature, water and nitrogen; it is
  aggregated to H3 cells. Runoff comes from ISIMIP hydrology emulators. Both form lane 4 of
  S10, next to the three lanes of 0036 and never averaged with them. Refit to 2000-2024, lane 4
  nests the local-level lane (3.4).
- Fisheries: Gordon-Schaefer logistic biomass per stock and sea zone; open access, quotas and
  ITQs as policy (Costello et al. 2016); carrying-capacity drift from the FishMIP ensemble
  (Tittensor et al. 2018). Lab profile, opt-in, once it passes ablation on its own targets.
- Forests and soils: standing timber and soil organic carbon per land class in ADR 0038's
  allocation, with harvest, degradation and recovery rates. Each enters production as a factor
  input valued per SEEA Ecosystem Accounting. Extended profile.
- Biodiversity: a Biodiversity Intactness Index per cell (Newbold et al. 2016) driven by land
  use. Pollination-dependent crop shares scale yields. Extended profile.
- Budget: under 2 ms per tick, mostly annual; about 50 MB of state; 8 to 14 new free
  parameters, in the X-register (23.1) while extended. Emulator coefficients are data.
- Coupling (0135): exchanged variables use ISIMIP names and units, so outside impact models
  can replace an emulator without a schema change.

## Migration

- Now: ISIMIP variable names and units in the 0135 coupling schema.
- After P8: lane 4 and fisheries. 2.x and 3.x: forests, soils and biodiversity as minors.
- Data: ISIMIP outputs and GGCMI emulators (terms per dataset to audit), RAM Legacy Stock
  Assessment Database and Sea Around Us (terms to audit), BII (terms to audit). LPJmL is AGPL,
  so use its outputs, not its code. Non-commercial terms are acceptable.

## Tests

- With every module off, golden hashes are unchanged.
- Lane 4 refit to 2000-2024 reproduces the local-level lane within tolerance in-sample.
- Fisheries reproduce 2000-2020 biomass trends in RAM Legacy stocks out of sample, including
  the direction of collapses and recoveries.
- Natural-capital stocks satisfy their own mass balance every tick (stock change equals growth
  minus harvest minus loss).

## Consequences

Commons, collapse and recovery become playable and testable without claiming lab skill the data
cannot show. Process yields add structural uncertainty where warming leaves the historical
record, as a lane rather than a replacement. If a later hindcast lets these stocks pass ablation,
they move into the lab as opt-in minors, and the default changes only at a major (0136).
