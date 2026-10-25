# 0051. A fat-tailed hazard library for rare events

Status: proposed
Date: 2026-10-24
Design sections: 13.1, 13.3, 13.4, 13.5, 11.2, 19.4, 15.4
Phase: P8 (natural and epidemic hazards), P7 (war severity), P9 (calibration)

## Context

Pandemics, large wars, great earthquakes, eruptions and crises dominate the variance of
2000-2100 outcomes, and all have heavy-tailed severities: epidemic intensity (Marani et al.
2021), interstate war deaths with no significant trend over two centuries (Clauset 2018),
earthquake magnitudes (Gutenberg-Richter), eruption sizes (volcanic explosivity index). The
design draws disasters from hazard maps and pandemics as SEIR seeds, and samples next-event
times (19.4), but has no common treatment of tails, clustering or the uncertainty of tail
parameters, which is where ensembles are most sensitive.

## Options

1. Status quo: per-module hazards with thin-tailed or fixed severities.
2. One library: peaks-over-threshold generalized Pareto severities, Poisson or Hawkes timing
   for clustering, hierarchical pooling across regions, and tail indices as uncertain
   parameters drawn per ensemble member.
3. Scenario-only rare events.

## Decision

Option 2. Each hazard type registers a frequency model, a severity model and its pooling
structure; the library draws next-event times and severities from the run's RNG streams and
records them on the shock tape.

- Epidemics: arrival and intensity from the Marani et al. distribution; the SEIR model of 13.3
  gains endogenous behavioural distancing (Eichenbaum, Rebelo and Trabandt 2021), vaccine
  arrival as a stochastic research lag, non-pharmaceutical interventions as policy choices, and
  a post-infection labour-supply hazard.
- Earthquakes: Gutenberg-Richter frequency-magnitude by seismic zone from the USGS catalogue
  (already in the manifest); damage via the capital stocks of 13.4.
- Volcanoes: eruptions by VEI from the Smithsonian Global Volcanism Program; large eruptions
  inject stratospheric aerosol forcing into FaIR.
- Tropical cyclones and floods: from IBTrACS and the flood database (in the manifest), with
  climate-scaled frequency (ADR 0037).
- War severity: the onset and termination models are validated against the COW power law (ADR
  0048).
- Insurance: a protection gap by income sets how much disaster loss is insured, and
  post-disaster fiscal transfers.
- Tail indices (P92, per hazard) are uncertain parameters drawn per ensemble member from their
  posteriors.

## Migration

- ceteris-climate hosts the library; other modules register hazards.
- P9: tail-index posteriors by peaks-over-threshold on historical catalogues.

## Tests

- Draws reproduce the fitted distributions (Kolmogorov-Smirnov on 10^6 draws) and are
  deterministic per seed.
- Ensemble spreads widen as tail-index uncertainty is switched on, by the amount implied
  analytically for a toy loss.

## Consequences

Long-horizon distributions carry their real tails, and the tail assumptions are explicit and
reported.
