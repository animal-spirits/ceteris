# 0139. Narrative state: endogenous belief content as a major candidate

Status: proposed
Date: 2026-10-28
Design sections: 9.13, 10.5, 10.7, 12.2, 12.5, 14.2, 15.7, 17, 19.2; ADRs 0046, 0055, 0071, 0119, 0123, 0125, 0126, 0136
Phase: cheap steps in P4 and P5; experience kernel as an opt-in minor after 1.0; module as a 5.0 or 6.0 candidate

## Context

Expectations switch among a fixed menu of heuristics (12.2), finance adds a diagnostic rule
(9.13), ideology diffuses by bounded confidence (10.7), values form by cohort (0055), and
information exposure is a scalar per pop (0046, 0071). These mechanisms move how much agents
believe, never what they believe. No agent holds a causal claim ("imports cost jobs", "deficits
cause inflation", "the peg is safe"), so no claim can spread, compete or be discredited. A
cohort that lived through hyperinflation forecasts like one that did not.

LLM agents as the carrier fail on cost and validity. At about 200 states deciding quarterly, a
50-year run needs about 40,000 calls, which is days of local inference per seed. The hash cache
of 12.5 misses on every new branch, which is exactly the counterfactual case. Anonymization
conflicts with the real map, and LLM expectations are more homogeneous than humans' (Lin et al.
2025). Section 15.7 already excludes LLM agents. Feasibility study

## Options

1. Status quo.
2. LLM agents as the belief layer.
3. A structured narrative state per pop cell, with contagion, experience weighting and learning.
   It nests 12.2, so it can become a lab default at a major.

## Decision

Option 3. LLM agents remain extended-profile seats and presentation (0125, 0126).

- Narrative library: K narratives, about 8 to 16, each a small causal graph over model variables
  (Eliaz and Spiegler 2020). The library and graphs are fixed and preregistered (0119) before
  calibration. A narrative sets how a pop maps observed variables to expected outcomes.
- State: an adoption share per narrative per pop cell. Adoption enters expectations (12.2,
  9.13), positions in the policy space (10.5) and protest thresholds (10.8).
- Contagion: an SIR-type process over the migration, trade-contact and media layers (0046,
  0071), with forgetting. This follows Shiller (2017) and the epidemiological expectations of
  Carroll (2003).
- Experience: each cohort weights lived inflation and unemployment with the declining-weight
  kernel of Malmendier and Nagel (2016). Lived crises raise adoption of narratives that explain
  them.
- Learning: constant-gain updating of narrative coefficients (Evans and Honkapohja 2001).
- Nesting: with adoption frozen at the 2000 calibration, experience weight zero and gain zero,
  the module reproduces 12.2 bit for bit.
- Edits: a `belief` edit type in 14.2 (suppress, seed or shift a narrative in a place and
  tick). The schema reserves it now and implements it with the module, so the change is additive.
- Budget: 0.5 to 1 ms per tick (100k pops x 16 shares over sparse graphs, fixed-order
  reductions); about 10 MB of state; 8 to 12 free parameters. These compete for the cap of 60
  (17), so some 12.2 parameters retire when the module becomes the default.
- Release: the module becomes the lab default only at a major (0136), after the scorecard below
  passes ablation. If it passes in the OECD and fails elsewhere, leave-one-region-out (15.7)
  decides the default.

## Migration

- P4: keep per-pop, per-age-band histories of inflation and unemployment.
- P5: add the Malmendier-Nagel cohort test and cross-sectional dispersion of expectations to the
  scorecard as diagnostics (no target, no tuning).
- Now: reserve the `belief` edit type in the intervention schema, unimplemented.
- After 1.0: the experience kernel as an opt-in minor. Narratives, contagion and media follow as
  opt-in minors; the default flips at 5.0 or 6.0.
- Data: Michigan Surveys of Consumers, SPF, ECB Consumer Expectations Survey, firm surveys
  (Coibion, Gorodnichenko and Kumar 2018), EPU (Baker, Bloom and Davis 2016), news-topic series
  (Bybee et al. 2024), GDELT; WVS and EVS offline, derived only (0055). Terms to audit for each.
  Survey coverage outside about 40 countries is thin.

## Tests

- Nesting: frozen adoption, zero experience weight and zero gain give golden hashes equal to 12.2.
- Out of sample: cohorts with little inflation experience update expectations more in 2021-23,
  with the sign and rough magnitude of Malmendier and Nagel.
- Expectation dispersion rises in crises, matching survey dispersion in the 2008 and 2020
  episodes.
- Adoption of preregistered narratives tracks news-topic intensity in shape and timing (for
  example euro breakup 2010-12, trade war 2018-19), with contagion fitted only to text-derived
  series.
- Policy-preference shifts after the China shock match the vote event study of 0046 out of
  sample.

## Consequences

Questions such as "what if this story had never spread?" or "does central-bank communication
change outcomes?" become posable, with real belief heterogeneity instead of a fixed heuristic
menu. The main risk is identification: adoption is latent and can absorb residual error. The
fixed, preregistered library and text-only contagion fitting are the guard.
