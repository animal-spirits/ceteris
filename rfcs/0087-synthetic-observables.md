# 0087. Synthetic observables for validation

Status: proposed
Date: 2026-10-24
Design sections: 15.3, 15.4, ADR 0052, ADR 0065
Phase: P3 (output layer), P9 (scoring)

## Context

Subnational validation is limited by official statistics, which are missing for many regions
and manipulated in some states (0065). Satellites observe proxies everywhere at fine
resolution: night lights (VIIRS), nitrogen dioxide and aerosols (Sentinel-5P, MODIS), built-up
area (GHSL), shipping density (AIS-based), gas flaring and vegetation greenness. If the
simulator emits what those sensors would see, it can be scored against them directly, province
by province, and the same signals become the independent measures that players and AI states
use to see through manipulated statistics.

## Options

1. Validate only against official statistics.
2. Observation operators that map simulated state to synthetic satellite observables,
   calibrated in-sample and scored out of sample.

## Decision

Option 2. Observation operators per observable (P142): night lights from output,
electrification and the settlement mix (0074); NO2 and aerosol columns from combustion and
pollution (0038); built-up area from housing and urban growth; shipping density from trade
flows on sea lanes; flaring from oil output; vegetation greenness from yields. Operators are
fitted on 2000-2010 and frozen. The scorecard (0052) adds a subnational track scored against
these observables; published-statistics manipulation (0065) is detected where official and
synthetic-observable signals diverge.

## Migration

- P3: observable tables in the output schema; P9: operator calibration and the subnational
  track.
- Data: VIIRS, DMSP and Black Marble (in the manifest), Sentinel-5P (Copernicus, free and
  open), GHSL, World Bank shipping density (in the manifest), VIIRS Nightfire, MODIS (public
  domain).

## Tests

- Operators fitted on 2000-2010 predict 2011-2020 observables better than a province fixed
  effect with a country trend.

## Consequences

The model can be validated where statistics are poor, and the truth-versus-reported layer gets
its independent signals.
