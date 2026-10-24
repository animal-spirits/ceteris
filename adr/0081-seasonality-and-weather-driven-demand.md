# 0081. Seasonality and weather-driven demand

Status: proposed
Date: 2026-10-24
Design sections: 4.3, 7.1, 7.7, 8.2, 13.1, ADR 0034, ADR 0073
Phase: P2 (calendars), P8 (weather-driven demand)

## Context

The tick is monthly, but the economy has almost no months: harvests realize once a year (4.3)
and nothing else is seasonal. Energy demand follows heating and cooling degree days, hydropower
follows snowmelt and monsoon, crop harvests differ by hemisphere and crop, construction and
tourism are seasonal, and Arctic shipping is open only in summer. Much of the observed data
used for validation is not seasonally adjusted, and seasonal stress (a cold winter with low gas
storage, as in 2021-22) is where crises start. The monthly tick makes seasonality nearly free.

## Options

1. Status quo: annual harvests only.
2. Calendars and weather-driven demand: crop calendars by region and crop, heating and cooling
   demand from degree days, seasonal hydro inflow, and seasonal factors for construction,
   tourism and shipping lanes.

## Decision

Option 2.

- Crop calendars per province and crop group (planting and harvest months) from global
  crop-calendar data; storage (0029) carries supply between harvests.
- Heating and cooling energy demand from monthly degree days, historical from ERA5 and future
  from the climate emulator (P134 holds the demand elasticities by region); the power sector
  (0034) sees seasonal load.
- Hydro inflow by month from precipitation and snowmelt (and glacier melt, 0076).
- Seasonal factors for construction, tourism and agricultural labour demand; Arctic routes open
  seasonally (0073).

## Migration

- P2: crop calendars and seasonal factors (data only, plus the harvest step).
- P8: degree-day demand and hydro from the climate module.
- Data: ERA5 (in the manifest), crop calendars from SAGE or MIRCA (licences to audit).

## Tests

- Simulated monthly gas and power demand in Europe show the observed winter peak; simulated
  monthly food prices show post-harvest troughs.

## Consequences

Seasonal stress, storage and energy security become real, and monthly validation series can be
used as they are published.
