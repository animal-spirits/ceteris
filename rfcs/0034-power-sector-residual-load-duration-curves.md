# 0034. Power sector via residual load duration curves

Status: proposed; amended per ADR 0095 (2026-10-26)
Date: 2026-10-24
Design sections: 7.1, 7.5, 7.7, 13.2, 15.7 (excluded: power-grid dispatch), 17.2
Phase: stub in P2; full in P8

## Context

Electricity is where decarbonization, energy security and the 2021-22 gas shock act: gas sets
the marginal power price in many markets, and integrating variable renewables changes the value
of every other plant. The design splits electricity by fossil, nuclear and renewable generation
(7.1) but has no rule for how plants are used, and 15.7 rightly excludes grid dispatch.
Integrated assessment models (REMIND, MESSAGE) solve this with residual load duration curves:
demand minus variable renewable output, sorted by hour and summarized in a few segments, served
by dispatchable plants in merit order (Ueckerdt et al. 2015, 2017). This captures integration
costs, curtailment, capacity adequacy and the price-setting technology without dispatch.

## Options

1. Fixed generation shares by state (status quo in effect).
2. Residual load duration curves with a monthly merit order over generic technology classes.
3. Hourly dispatch with transmission (excluded as bloat in 15.7).

## Decision

Option 2.

- Per state (or market zone for interconnected areas such as continental Europe), demand is a
  load duration curve with 4 to 8 segments, from historical load profiles. Wind and solar
  output, scaled by capacity and resource quality from the H3 raster, is subtracted to give the
  residual curve; curtailment and capacity credit come from its shape (P66).
- About ten generic classes (coal, gas combined cycle, gas peaker, oil, nuclear, hydro, wind,
  solar, storage, biomass) serve residual segments in order of marginal cost, which includes
  fuel prices from the goods markets and carbon prices. The segment's marginal class sets the
  electricity price; inframarginal rents go to plant owners.
- Storage shifts energy from low to high residual segments within round-trip efficiency and
  capacity limits.
- New capacity comes from the investment step through technology diffusion (ADR 0035). Capacity
  is a vintage stock, consistent with putty-clay (0020).
- P2 stub: electricity is produced with fixed 2000 shares by class (S15 = fixed); P8 switches
  to S15 = RLDC.

## Migration

- P2: carry generation capacity by class per state (data only) and the S15 switch.
- Before P8: a state grid market for electricity (or trade in good 7 within a state), since
  electricity is a province-local good in P2 and load duration curves are per state (ADR 0095).
- P8: the RLDC system in ceteris-econ (energy), microseconds per state per month.
- Data: Global Energy Monitor plant trackers (in the manifest; confirm commercial terms, ADR
  0050); Ember or IEA generation statistics (licences to audit); load profiles from public
  system operators; renewables.ninja capacity factors are non-commercial (research pack only),
  so the release pack derives capacity factors from ERA5.

## Tests

- With S15 fixed, golden hashes equal the stub.
- A toy system with rising solar share shows falling capture prices and rising curtailment with
  the shape reported by Ueckerdt et al.
- 2021-22 gas shock replay: power prices in gas-marginal markets move with gas, and less where
  hydro or nuclear set the margin.

## Consequences

Energy security, renewable integration and carbon pricing get a mechanism at negligible cost,
without dispatch.
