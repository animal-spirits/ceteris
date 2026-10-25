# 0036. Climate damage lanes

Status: proposed
Date: 2026-10-24
Design sections: 13.2, 15.7, 17.2 (S10), 18 (H11)
Phase: P8

## Context

Section 13.2 applies Burke, Hsiang and Miguel (2015) with a level-versus-growth switch (S10).
The damage literature has moved, and no single specification is defensible as a default:

- Kotz, Levermann and Wenz (Nature 2024), the most cited recent estimate, was retracted in
  December 2025 after its results proved sensitive to inaccurate 1995-1999 data for Uzbekistan;
  the authors say a corrected paper will follow. It cannot anchor anything until then.
- Nath, Ramey and Klenow (NBER w32761) find persistent but not permanent growth effects,
  because technology diffusion tethers growth rates across countries; their projections lie
  between level-effect and permanent-growth estimates.
- Bilal and Kanzig (QJE 2026) estimate that a 1 degree global temperature shock lowers world
  GDP by about 12% at peak, several times local-temperature estimates, from a time series with
  few effective observations.

Damage estimates differ by up to an order of magnitude, the 2000-2024 sample cannot
discriminate between them, and they dominate every run past 2050.

## Options

1. One default (BHM growth or level) with a switch (status quo).
2. A probability-weighted average damage function.
3. Three structural lanes, run as separate ensemble members, with counterfactuals reported
   conditional on each lane.

## Decision

Option 3. S10 is redefined with three lanes:

1. Local level effects: damages from local temperature and precipitation deviations on output
   levels, yields (Schlenker-Roberts style) and labour productivity, with adaptation rising in
   income.
2. Persistent effects (Nath, Ramey and Klenow): local shocks lower growth for several years and
   decay as catch-up (7.5) pulls productivity back toward the frontier; the decay rate is P70.
3. Global temperature effects (Bilal and Kanzig): a global-mean-temperature term on top of
   local effects, capturing channels that local regressions miss (extreme events, global trade
   and demand spillovers).

Lanes are never averaged: the 2000-2024 data cannot weight them, so stacking (0053) does not
apply. Every long-horizon result is reported for each lane.

Damages are drawn stochastically at the cell level from the lane's distribution, as in the DSK
model (Lamperti et al. 2018), rather than applied as a smooth aggregate shifter, so they
interact with finance and firm exit.

## Migration

- P8: damage functions per lane in ceteris-climate; the shock tape records draws.
- Carleton et al. (2022) mortality damages are common to all lanes.
- The lanes own all heat mortality and heat labour losses (owner, 2026-10-25). Heat exposure
  (heatwave days, the urban multiplier) comes from ADR 0077, and deaths enter the cause groups of
  ADR 0033; neither adds a separate heat damage.
- The H11 test runs per lane.

## Tests

- Each lane reproduces its source's headline response in a toy single-region run (local level
  and persistent lanes against their regression specifications; the global lane against
  Bilal-Kanzig's impulse response).
- Lane switch changes no pre-2025 calibration target beyond tolerance (damages are small
  in-sample).

## Consequences

Climate counterfactuals become honest about structural uncertainty. The UI's comparison mode
shows the lane spread; no single "the cost of climate change" number is produced.
