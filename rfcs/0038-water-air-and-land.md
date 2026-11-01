# 0038. Water, air pollution and land use

Status: proposed
Date: 2026-10-24
Design sections: 5.1, 6.2, 7.2, 7.7, 13.2, 13.4
Phase: P8

## Context

Three environmental channels with large, well-measured stakes are missing. Water stress and
drought constrain agriculture and industry and are worsening in many regions (WRI Aqueduct).
Fine particulate pollution from fuel combustion is one of the largest global mortality risk
factors, with exposure-response functions estimated across cohorts (Burnett et al. 2018, GEMM).
Land is fixed by use in the design (land by use sums to area, 4.6), so bioenergy, deforestation
and food-versus-energy competition cannot be studied; integrated assessment models allocate
land by logit sharing across uses (GCAM).

## Options

1. Status quo.
2. Reduced-form modules: water stress and drought as yield and industrial-capacity shifters,
   PM2.5 from combustion with an exposure-response mortality function, and logit land
   allocation across uses.
3. Hydrological and atmospheric chemistry models.

## Decision

Option 2.

- Water: baseline water stress per province from Aqueduct; droughts (SPEI, already in the
  manifest) and floods (Global Flood Database, already listed) as hazards; water stress caps
  water-intensive industrial output, relieved by adaptation investment. Aqueduct also sets the
  initial water state of ADR 0076, whose aquifer and basin stocks are the only cap on irrigated
  yields (owner, 2026-10-25).
- Air: PM2.5 concentration per province from combustion by sector and fuel (7.7 already
  computes fuel use) with a source-receptor coefficient per province, plus a background;
  mortality through the GEMM exposure-response (P73) as an additive hazard on the Brass level
  (ADR 0033); abatement policy and income-related controls lower emission factors.
- Land: each province allocates land among cropland, pasture, managed forest, bioenergy and
  natural land by logit on returns (P72), with conversion costs; land-use change emits carbon.
  The land invariant of 4.6 still holds.

## Migration

- P8: three small systems in ceteris-climate and ceteris-econ.
- Data: WRI Aqueduct (CC BY 4.0), van Donkelaar satellite PM2.5 for calibration (licence to
  audit), land cover already in the raster, FAOSTAT land use.

## Tests

- With modules off, golden hashes equal the status quo.
- Simulated PM2.5 by province in 2000-2019 against satellite estimates; attributable mortality
  against published GEMM estimates.
- Land allocation reproduces 2000-2020 cropland and forest change by region within tolerance.

## Consequences

Energy-transition and climate counterfactuals gain their health co-benefits and their land and
water constraints.
