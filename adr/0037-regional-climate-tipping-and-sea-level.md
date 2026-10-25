# 0037. Regional climate emulation, tipping elements and sea level

Status: proposed
Date: 2026-10-24
Design sections: 13.2, 5.1, 17.2
Phase: P8

## Context

Section 13.2 uses FaIR for global response and pattern scaling for regional climate. Pattern
scaling is adequate for mean temperature but not for variability, precipitation and extremes,
which drive yield and labour damages. MESMER (Beusch et al. 2020; v1.0.0 in Geoscientific Model
Development, 2026) is a maintained emulator that produces local annual and monthly temperature
(MESMER-M), monthly precipitation (MESMER-M-TP) and annual extremes (MESMER-X) from global mean
temperature, with pre-calibrated parameters for many climate models, in minutes. Tipping
elements are absent: Armstrong McKay et al. (2022) identify sixteen, several possible near
current warming, and Dietz et al. (2021) find that tipping points raise the social cost of
carbon by about a quarter, with a fat right tail. Sea-level rise needs regional projections
with an ice-sheet tail.

## Options

1. Status quo: FaIR plus pattern scaling.
2. FaIR global driver plus MESMER regional emulation, stochastic tipping events, and AR6
   regional sea level.
3. An intermediate-complexity climate model.

## Decision

Option 2.

- FaIR stays the global driver. MESMER generates province-level monthly temperature and
  precipitation and annual extremes from its global mean, drawn per run so ensembles carry
  natural variability. Precomputed MESMER parameter sets (by climate model) are a structural
  ensemble dimension.
- Tipping elements as threshold events with hazards rising in global mean temperature
  (Armstrong McKay et al. ranges; P71 scales the hazards): AMOC weakening shifts regional
  patterns; Greenland and West Antarctic commitment adds to sea level; permafrost thaw and
  Amazon dieback add emissions. Switch S17 turns them off.
- Sea level from IPCC AR6 regional projections (FACTS framework) with a low-probability,
  high-impact ice-sheet tail; coastal exposure from the ETOPO elevation raster removes land and
  raises flood hazard.

## Migration

- P8: MESMER parameters as pack data (Python pipeline), the emulator step in ceteris-climate
  (annual and monthly), tipping hazards on the shock tape.
- Data: MESMER (open-source software; licence to audit), CMIP6-derived parameters, AR6
  sea-level projections (licence to audit).

## Tests

- With S17 off and MESMER noise set to zero, regional means equal pattern scaling within
  tolerance.
- Emulated 2000-2024 regional temperature and precipitation variability matches ERA5
  statistics.
- Tipping hazards reproduce the Armstrong McKay et al. likelihood bands at given warming
  levels.

## Consequences

Damages respond to variability and extremes, not only to means, and long runs carry tail risk
from tipping elements.
