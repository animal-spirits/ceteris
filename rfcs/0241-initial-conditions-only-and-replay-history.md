# 0241. Initial conditions only: calendar-blind runs and replay history

Status: accepted (owner, 2026-10-29)
Date: 2026-10-29
Design sections: 4.4, 13, 14.1, 14.4, 15.2, 15.4, 15.5, 15.7, 17.2, 22; ADRs 0051, 0052, 0103, 0107, 0118, 0178, 0206, 0210, 0229, 0230, 0233
Phase: P3 (year-relabel test, replay loader), P8 (hazards), P9 (calibration and scoring)

## Context

The owner wants the Paradox convention: only the state on 1 January 2000 is data, and everything
after it is simulated, with no railroading. Pandemics and other shocks must be able to emerge
without being scheduled, and continuing a run past 2024 must be the same problem as continuing it
past 2000. The design does not do this today. 13.1 replays observed 2000-2024 shocks as the
validation default and switches to fitted generators "after 2024". ADR 0210 defaults to
`tape-then-rules`, and 0210, 0229 and 0233 put tariffs, capital controls, regime changes and
prudential settings on a tape to 2024 and hold them after. So every default run has a seam at the
end of the data, and a hindcast follows history because it is told to. Brainstorm

Three questions: what a default run may read after t0 (D1), what counts as initial state beyond
levels (D2), and how to calibrate and validate without a tape (D3).

## Options

D1, what a default run reads after t0:

1. Calendar-blind: state, parameters and random streams only; history kept as an optional replay
   set.
2. Status quo: tape to 2024, then rules and generators.
3. Calendar-blind with no replay at all.

D2, initial state beyond levels:

1. Levels plus commitments in force at t0, each with a completion hazard.
2. Levels only.
3. Option 1 plus items under negotiation at t0 (China's WTO accession, EU 2004 enlargement).

D3, calibration and validation:

1. Hazards fitted on long catalogues; mechanisms estimated in replay; 2011-2024 scored both
   unconditionally and in replay.
2. Unconditional only (simulation-based inference on distributional moments).
3. Replay only (status quo).

## Decision

Option 1 for each (owner, 2026-10-29).

**Calendar-blind runs (D1).** After t0 a default run reads only state, parameters and
counter-keyed random draws (4.5). No input is indexed by calendar date. Three things that look
like dates are state: month of year (seasons, harvests, weather), ages and durations (cohorts,
maturities, a peg's age), and the commitments of D2. Events come from hazards whose rates depend on
simulated state (13; the hazards themselves are in ADR 0242). A run started from a 2024 pack uses
the same code as a run from 2000 that passes 2024; only initial conditions differ.

**Replay history.** The historical shock and policy tapes become one optional set of
interventions (14.2) named `replay-2000-2024`: observed weather (ERA5), pandemic seedings,
disasters, chokepoint closures, policy rates and fiscal stances, tariff and regime changes,
prudential settings, named projects, discoveries. The existing tape tables and reader are kept;
the reader emits interventions, which are logged like any user edit. Replay may be applied in
whole or by family (shocks only, policy only). Replay is a lab instrument for calibration,
natural experiments (15.5) and the conditional scorecard (0052); it is never part of a default run.
13.1's three modes collapse into one default plus interventions; switch S07 (17.2) becomes
`replay: off | shocks | policy | all`.

**Commitments in force at t0 (D2).** The pack records, as state, schedules that were legally
adopted by 1 January 2000 and projects under construction then: tariff phase-ins under the
Uruguay Round and regional agreements in force, the end of textile quotas in 2005, euro
banknotes in 2002, the 0206 construction pipeline. Each item carries a completion hazard (delay or
reversal), fitted where a reference class exists and zero otherwise. Agreements still under
negotiation at t0 are not state, so the default 2000 run has no Chinese WTO accession (signed
December 2001); the China shock comes from Chinese productivity and capital deepening (7.5, ADR
0103). A pack built for another origin records the commitments in force on its own start date.

**Calibration and validation (D3).**

| Job | Method | Data |
| --- | --- | --- |
| Hazard rates and severities | Fitted outside the simulation (as 0022 does for gravity): frequency on state covariates, generalized Pareto tails, tail indices drawn per member (0051) | Long catalogues, mostly before 2000; 2011-2024 frequencies held out |
| Mechanism parameters | Estimated in replay on 2000-2010 (15.2), so that shock size and propagation are separately identified | As 15.2 |
| Validation, unconditional | Runs from the 2000, 2005 and 2010 origins with replay off. CRPS, PIT and interval coverage on paths; counts and severities of recessions, banking crises, sudden stops, pandemics and disasters per decade against catalogues; history's percentile (0118) | 0107 split: select on 2011-2018, sealed 2019-2024 |
| Validation, conditional | Same origins with replay on; 15.5 natural experiments as replay subsets | Same |

The scorecard reports both, and the gaps: unconditional against conditional is shock luck; replay
with and without the policy family is the policy error 0210 asks for. Unconditional scores are
compared with the WEO and other benchmarks (0052) at the horizons where those forecasts also did
not know the shocks.

**Common random numbers.** State-dependent hazards draw by thinning: one uniform per (hazard,
cell, tick) from the counter RNG, and the event fires when the uniform falls below the rate.
Branches share the uniforms, so an event occurs in both unless an intervention moved its rate
across the draw (14.4). Removing an event from a run ("no pandemic") is an edit of that run's
event, not of history.

**Test.** Relabelling the start year of the same pack (2000 to 2100) leaves the golden hash
unchanged apart from labels. This test runs in CI from P3.

## Migration

- DESIGN 13 intro and 13.1 (one default, replay as interventions), 13.4 (discoveries per 0178),
  14.1 (scenario carries an optional replay set, not a tape), 15.5 and 15.7 (natural experiments
  and the policy tape run as replay), 17.2 S07, and the 22 railroading row are edited when this
  ADR is accepted.
- ADR 0210: the default mode is `rules` from t0, and `tape` becomes the policy family of replay.
  Tariffs, trade wedges and capital controls hold their t0 values, plus D2 schedules; regime
  changes and post-crisis prudential changes follow ADR 0242. Its test "a 2150 run under rules"
  is unchanged.
- ADR 0229: countercyclical buffer follows the Basel gap guide from t0; LTV and DTI caps, the
  deposit-insurance limit, the resolution regime and the swap-line list hold their t0 values and
  change through ADR 0242; iMaPP, BRRD and swap-line history are replay.
- ADR 0233: post-2022 LDI buffers are replay; in a default run they change through ADR 0242.
- ADR 0230, 0051, 0035, 0062: "drawn on the shock tape" reads "drawn from the run's random
  streams and written to its event log". In replay, 0230's latent demand is filtered from observed
  holdings.
- ADR 0206: projects under way at t0 are D2 state; the project tape is replay.
- ADR 0052: the conditional-forecast mode is replay. Real-time vintages are still used to build
  origin packs.
- ADR 0028's oil-price hindcast test runs in replay; an unconditional test of oil-price volatility
  is added.
- P3: the year-relabel test and the replay loader in the experiment API. P9: the two-way
  scorecard.

## Tests

- Year relabel: identical hashes for start labels 2000 and 2100 on the same pack.
- With replay `all`, the hindcast reproduces the observed lever paths exactly (0210's tape test).
- No pack table read after t0 is keyed by calendar date (asserted by the pack loader).
- A D2 schedule with a zero completion hazard executes on its scheduled offset from t0.
- Two branches differing only in an edit that does not touch a hazard's inputs share every
  event of that hazard.

## Consequences

Runs are not railroaded, and there is no seam at 2024. Unconditional hindcast scores will be worse
than replay scores, with wide bands around 2008 and 2020; that is the honest uncertainty and is
reported beside the conditional scores. Rare-event frequencies need the large ensembles of 0107.
No new free behavioural parameters: hazard and completion parameters are estimated on catalogues
and enter the register (17) as data-estimated.
