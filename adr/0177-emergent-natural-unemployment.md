# 0177. Natural unemployment rate from matching

Status: proposed
Date: 2026-10-28
Design sections: 6.3, 9.3, 15.4; ADRs 0089, 0105, 0169
Phase: P4 or P5

## Context

ADR 0089 sets each country's natural unemployment rate u* as the ILOSTAT 1995-2005 mean and
holds it as a wedge. It therefore cannot move with demography or institutions. Euro-area
unemployment persistence in 2011-2016 and the fall in German u* after the Hartz reforms
(Krause and Uhlig 2012) are outside reach, and the wedge was meant to be retired once the labour
block could produce u*.
Gap audit (2026-10-28): https://claude.ai/artifact/HRDQVbjJz8MrcDuPphPyYs

## Options

1. Status quo: fixed u*.
2. A tape of OECD NAIRU estimates.
3. u* emerges from matching efficiency, separations, the youth share, employment protection and
   benefits; the ILOSTAT mean becomes the t = 0 calibration target.

## Decision

Option 3.

- Matching efficiency per country is calibrated so that steady-state unemployment in 2000
  equals the ILOSTAT mean. Separations come from the wage-segment turnover (0105).
- Employment protection lowers separations and hiring; benefit replacement and duration lower
  search intensity (OECD indicators already in the manifest). Each enters with a published
  elasticity, not a free parameter.
- The youth share comes from demography, so ageing lowers u*.
- The Taylor rule's output gap uses the estimated u*, read from published statistics (0065).

## Migration

- P4 or P5: remove 0089's wedge; add the institutional terms to the matching step.

## Tests

- At t = 0 steady-state unemployment equals the ILOSTAT mean per country.
- A Hartz-style benefit cut in a toy Germany lowers u* by the published range.
- Euro-area unemployment 2011-2016 is closer to history than with the fixed wedge.

## Consequences

Labour-market reforms become experiments, and one wedge from 0089 is retired. Register: none;
matching efficiency is a calibrated initial condition.
