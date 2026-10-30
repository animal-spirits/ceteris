# 0242. Endogenous hazards: pandemics, policy regimes and technology jumps

Status: accepted (owner, 2026-10-29)
Date: 2026-10-29
Design sections: 5.3, 7.5, 9.2, 9.11, 13.3, 13.4, 17; ADRs 0051, 0178, 0179, 0210, 0229, 0233, 0241
Phase: P5b (regime and prudential hazards), P8 (pandemics, chokepoints, technology jumps), P9 (fits)

## Context

ADR 0241 makes default runs calendar-blind: after 1 January 2000 events come only from hazards on
simulated state. 13.6 and ADRs 0178 and 0229 already make crises, rescues and discoveries
endogenous, and 0051 gives disasters a hazard library. Three gaps remain. Pandemics are seeded from
the tape (13.1). Levers without a 0210 rule (exchange-rate regime, prudential settings) were taped
to 2024 and held after, so in a calendar-blind run they never move. Frontier growth (7.5) is
smooth, so there are no technology surprises. Politics and war stay in 2.0 (ADR 0136).
Brainstorm: https://claude.ai/artifact/1RphqtMVgze9SF1vtz6ByR

## Options

D4, policy regimes after t0:

1. Held at t0 values.
2. Held at t0, plus regime-switch hazards and post-crisis prudential tightening; trade policy
   held until 2.0.
3. Option 2 plus trade-policy hazards.

D5, pandemic origin:

1. Per-province spillover hazard; pathogen traits drawn from a prior over past pathogens.
2. Global arrivals seeded in proportion to population.
3. Replay only in 1.0.

D6, technology shocks:

1. Split part of 7.5 frontier growth into fat-tailed sector jumps.
2. Smooth frontier growth only.
3. Research-driven arrival rates now (ADR 0179).

## Decision

Option 2 for D4, option 1 for D5 and D6 (owner, 2026-10-29). Every hazard registers with the 0051
library and draws by thinning on shared uniforms (0241).

**Exchange-rate and monetary regimes (D4).** Each state sits in one coarse
Ilzetzki-Reinhart-Rogoff class (hard peg, band or crawl, managed float, free float); inflation
targeting is a flag on the floating classes. Monthly transition hazards are a multinomial logit
on inflation over the past 24 months, reserve cover, a peg break in the past 24 months (0210's
reserve-cover rule), income, and trade share with the anchor currency, fitted on the IRR
classification since 1970. Inflation-targeting adoption is a hazard on the same covariates (Rose
2007). Entering or leaving a monetary union is political and stays at t0 until 2.0, apart from D2
commitments in force (0241).

**Prudential settings (D4).** For each lever held at t0 by 0241 (LTV and DTI caps, the
deposit-insurance limit, the resolution regime, LDI collateral buffers of 0233, the
countercyclical buffer beyond the Basel guide), two hazards:

- Tightening, whose rate rises for some years after a simulated crisis of the matching type in
  that state (banking crisis, house-price bust, bank run, margin spiral) and with the credit gap;
  the size of a step is drawn from the observed steps.
- Loosening, whose rate rises with a negative credit gap and time since the last crisis.

Both are fitted on IMF iMaPP (1990 onward) joined to Laeven-Valencia crisis dates. Tariffs, trade
wedges and capital controls hold their t0 values and D2 schedules until 2.0.

**Pandemics (D5).** Zoonotic spillover runs per province with rate

```
lambda_p = lambda0 * exp(b1 ln livestock_p + b2 forest_edge_p + b3 ln density_p + b4 wildlife_trade_p)
```

with covariates from simulated state where they exist (density, land use, livestock) and pack data
otherwise. The b are the Allen et al. (2017) hotspot estimates; lambda0 is fitted so that the
global frequency of events above each intensity matches Marani et al. (2021). Each spillover draws
a pathogen from a prior built on past pathogens (transmission route, R0, infection fatality by
age, incubation, immunity duration); most draws have R0 below one and fizzle. Those that spread run
in the 13.3 metapopulation SEIR model on the simulated air, trade and migration network, with
behavioural distancing (0051). Two rules, fitted like 0210's: non-pharmaceutical interventions as
a stringency level on deaths per capita, state capacity and neighbours' stringency (OxCGRT), and
vaccine arrival as a lag drawn from a distribution conditioned on pathogen family, followed by
rollout limited by income and production capacity.

**Chokepoints.** Each chokepoint on the transport graph (5.3: Suez, Panama, Hormuz, Bab el-Mandeb,
Malacca, Bosporus, Gibraltar) has a stationary closure hazard with a duration and capacity-cut
distribution. Piracy is a hazard on adjacent coastal provinces scaled by their poverty, adding a
freight-rate premium. Fitted on maritime incident records, the 2021 Suez blockage and the 2023-24
Red Sea disruption. Closures caused by war are 2.0.

**Technology jumps (D6).** The frontier growth of 7.5 is split into a smooth part and jumps:

```
d ln A*_s = (1 - omega_s) g*_s dt + J_s dN_s,   E[J_s] rate_s = omega_s g*_s
```

N_s is a Poisson process per sector, J_s is Pareto with tail index per sector group, and omega_s
is the share of frontier growth that arrives in jumps. Mean growth, and its dependence on research
effort, are unchanged; only the timing becomes lumpy. Catch-up (7.5) spreads each jump across
provinces. omega, rates and tail indices are fitted to the distribution of sector TFP changes in
KLEMS and EU KLEMS since 1970. Directing research effort across sectors stays with 0179 and the 3.0
technology major.

## Migration

- P5b: regime and prudential hazards in `ceteris-ai` beside 0210's and 0229's rules;
  `specs/policy/SPEC.md` gains them.
- P8: pandemic, chokepoint and technology-jump hazards register with the 0051 library in
  `ceteris-climate`; SEIR in the disease module (13.3).
- Pack: livestock density, forest edge and wildlife-trade proxies by province; the pathogen prior;
  chokepoint parameters. Sources to audit for the manifest: FAO Gridded Livestock of the World,
  Hansen Global Forest Change, Allen et al. (2017) supplementary data, Marani et al. (2021), OxCGRT
  (CC BY 4.0), IRR classification, IMF iMaPP, Laeven-Valencia, EU KLEMS and World KLEMS, IMB piracy
  reports.
- P9: all fits outside the simulation; the unconditional scorecard of 0241 checks the resulting
  frequencies on 2011-2024.

## Tests

- With every hazard rate at zero, golden hashes equal the hazard-free run.
- Over 10^4 simulated years with t0 covariates held fixed, the frequency-intensity curve of
  pandemics matches the fitted Marani curve (Kolmogorov-Smirnov), and regime transition
  frequencies match the IRR panel.
- Pandemics start more often in provinces with higher spillover covariates (rank correlation of
  simulated origins with lambda_p).
- With omega_s = 0, technology results equal the smooth frontier; with omega_s > 0, mean frontier
  growth over long runs is unchanged within Monte Carlo error.
- A simulated banking crisis raises the tightening rate in that state, and none fires in a state
  that has no crisis or credit boom.

## Consequences

Pandemics, regime changes, prudential reforms, chokepoint closures and technology surprises all
emerge from simulated state, so counterfactuals that change land use, inflation or credit change
their risk. Parameters are estimated on catalogues outside the simulation and enter the register
(17) as data-estimated; none is free. Trade policy stays frozen in 1.x except for commitments in
force at t0, which is the main remaining stillness in long runs until 2.0 politics.
