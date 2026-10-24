# 0052. Forecast scorecard: real-time vintages, hard benchmarks and proper tests

Status: proposed
Date: 2026-10-24
Design sections: 15.3, 15.4, 15.5, 15.7
Phase: P9 (scorecard), with the conditional-forecast mode in P3

## Context

The scorecard (15.4) compares out-of-sample forecasts with AR(1), VAR and IMF WEO forecasts and
scores conflict against ViEWS. Three practices from forecasting research are missing.
Comparisons with institutional forecasts are only fair on the data available at the time, which
differ from revised data. Simple benchmarks are hard to beat: in the 2023-24 ViEWS prediction
challenge the strongest benchmark used each country's last twelve months as its predictive
distribution, and most country-months have zero fatalities. The M-competitions show that
combinations beat single models and that probabilistic scores need calibration checks. The best
data-driven macro ABM (Wiese et al. 2024) beat AR(1) in 33 of 38 OECD countries and is itself a
benchmark.

## Options

1. Status quo.
2. A scorecard with real-time vintages, a fixed set of hard benchmarks, proper scores plus
   calibration diagnostics, formal comparison tests, and live pre-registered forecasts.

## Decision

Option 2.

- Vintages: each rolling origin uses data as published then (ALFRED, OECD Economic Outlook and
  IMF WEO vintages); scoring uses the latest release.
- Benchmarks: random walk, AR(1), a small VAR, IMF WEO, a combination of these, the published
  Wiese et al. forecasts where they overlap, and BeforeIT.jl runs for Austria and Italy as a
  reference (it is AGPL-3.0, so it is run separately and never embedded). Conflict: the ViEWS
  conflictology benchmark and the ViEWS ensemble.
- Metrics: CRPS, log score (ignorance), interval coverage and the mean interval score, PIT
  histograms; Diebold-Mariano and Giacomini-White tests per variable and horizon.
- Conditional-forecast mode: runs can condition on the observed paths of policy rates, fiscal
  stance or commodity prices (as in Poledna et al. 2023), separating mechanism error from
  exogenous-input error.
- Live pre-registration: each January, sealed forecasts from the current release are deposited
  with a DOI and scored a year later.
- Stylized facts added to V2: price-change frequency and size (Nakamura and Steinsson 2008),
  firm age and size dynamics (US Business Dynamics Statistics; Axtell 2001), labour flows,
  intertemporal MPCs, a flat regional Phillips curve (Hazell et al. 2022), exchange-rate
  disconnect (Itskhoki and Mukhin 2021), home bias, procyclical capital flows (Kaminsky,
  Reinhart and Vegh 2004), house-price momentum, trade and business-cycle synchronization
  (Frankel and Rose 1998), recent unconditional convergence (Patel, Sandefur and Subramanian
  2021), slowing regional convergence (Kleinman, Liu and Redding 2023), demographic transition
  timing, democratization and autocratization waves (V-Dem ERT) and conflict persistence.
- Out-of-sample targets beyond the economic core (2011-2024): Arab Spring diffusion and leader
  exits; the global homicide decline with Latin America's plateau; fertility below replacement;
  internet adoption S-curves; democratic backsliding and Russia's autocratization; the 2015
  migration surge; the rise of Chinese development finance; the 2016 populist wave; Pentecostal
  growth; African urbanization; the US overdose epidemic and its 2024 decline; the Sahel coup
  cluster; monthly conflict fatalities scored against ViEWS. Each names the ADR whose module it
  tests (0055 to 0087).
- Subnational track: synthetic satellite observables scored against VIIRS, Sentinel-5P, GHSL
  and shipping density (ADR 0087).

## Migration

- P3: conditional-forecast mode in the experiment API.
- P9: vintage-aware origins in the pipeline; scorecard code in python/ceteris/analysis.

## Tests

- The scorecard reproduces the published ViEWS benchmark scores on the challenge data before
  scoring the model.
- Vintage handling: a forecast at a 2010 origin reads only data released by then (asserted from
  release dates).

## Consequences

The scorecard becomes credible to forecasters, and failures are attributed to mechanisms or
inputs rather than hidden in revised data.
