# 0065. True and reported statistics

Status: proposed
Date: 2026-10-24
Design sections: 2.1, 12.3, 12.6, 15.1, 15.4, 20, roadmap P7 (dossiers)
Phase: P3 (data layer), P5 and P6 (manipulation incentives), P9 (calibration)

## Context

The design gives players dossiers of noisy institutional signals, but the noise has no
generative model, and the calibration pipeline treats official statistics as truth. Official
statistics are not truth: Martinez (2022, JPE), comparing reported GDP growth with night-lights
growth across 184 countries, finds that autocracies overstate annual growth by about 35% (a
true 2% is reported as about 2.7%), concentrated in components with no third-party check such
as public investment and government consumption. Statistical capacity also varies widely, so
reported data are noisier where states are weaker. A model calibrated on 2000-2010 official
data without this absorbs exaggeration into its truth, and AI states and the Steward player
decide on numbers they could not actually see.

## Options

1. Status quo: one truth, dossiers with unstructured noise.
2. Two layers per state: the simulated truth and the published statistics, with published =
   truth distorted by manipulation (regime incentives, election proximity, component
   verifiability) plus measurement noise scaled by statistical capacity. Decisions use
   published data; ground truth stays visible in observer and lab modes.

## Decision

Option 2, behind switch S21 (off: published equals truth).

- Each state's statistical office produces the national accounts, inflation, unemployment and
  population from the true state with lags, revisions and noise scaled by statistical capacity
  (initialized from the World Bank statistical performance indicators), and with manipulation
  whose intensity rises with autocracy, low media freedom and election proximity, concentrated
  in unverifiable components (P111, P112).
- State AI (12.3) and Steward mode decide on published data. Independent measures (night
  lights, trade partners' mirror statistics, IMF assessments) appear as alternative signals
  with their own noise, so a careful player can triangulate.
- Calibration (P9) targets manipulation-adjusted data where independent measures exist and
  down-weights unverifiable components; the scorecard scores against the same adjusted targets.

## Migration

- P3: published-statistics layer in the output schema (a second set of tables) and a UI toggle
  "what the government says".
- P5 and P6: manipulation incentives from regime and electoral state.
- Data: World Bank Statistical Performance Indicators (CC BY 4.0); VIIRS and DMSP night lights
  (already in the manifest).

## Tests

- With S21 off, golden hashes are unchanged.
- On simulated data, the Martinez regression (reported growth on night-lights growth interacted
  with regime) recovers the planted autocracy gradient.

## Consequences

The fog of war extends to the economy, AI states face the dictator's information problem, and
calibration stops treating propaganda as data.
