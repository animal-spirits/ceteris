# 0107. Holdout protocol, Monte Carlo error and parameter lanes

Status: proposed
Date: 2026-10-26
Design sections: 14.4, 15.4, 15.6, 15.7, 19.2; amends ADRs 0024, 0052, 0053, 0063, 0066, 0071
Paths (ADR 0113, 2026-10-29): `python/` is gone; the holdout config and Monte Carlo error columns go in the Rust crates and the run outputs.

## Context

- **The holdout is used to select structure.** Ablation and switch choices are scored on
  2011-2024 (15.6; 0053 by stacking). Every such choice is a fit to the holdout; after dozens,
  the reported holdout skill is optimistic and nothing out of sample remains. 0052's live
  pre-registration helps only from the first sealed January onward.
- **Ensembles are small.** K = 32 members. Monte Carlo standard error on a 5% event is about
  4 points, so rare-event probabilities and tail quantiles are mostly noise, and reports do not
  say so.
- **Literature parameters enter as points.** Parameters fixed from the literature carry
  published uncertainty that is absent from ensemble spread, so intervals are too narrow.

## Options

1. Keep the protocol; rely on stacking.
2. Split the holdout into a selection period and a sealed period; report Monte Carlo error
   always; draw literature parameters per member.
3. Third-party blind scoring.

## Decision

Option 2.

- **Split.** Structure selection, ablation and stacking (0053) use 2011-2018 only. 2019-2024 is
  sealed: the scoring script and target data are committed with their hash; the sealed score
  is computed once per phase gate and recorded in the changelog, never used to choose.
  Selection within 2000-2010 adds leave-one-region-out validation (seven regions).
  Targets dated in the sealed period are sealed-gate targets only: the 2024 US overdose decline
  (ADRs 0052, 0063), the Sahel coup cluster (0052, 0066), and the 2019-2024 years of the
  internet-adoption and leader-exit tests (0066, 0071). Module tests use 2011-2018.
- **Monte Carlo error.** Every reported probability or quantile carries its Monte Carlo
  standard error (binomial for probabilities, batch means for quantiles). Scenario comparisons
  use common random numbers across lanes (0024, already free with the counter RNG) and report
  the paired difference's error. Any question about an event below 10% runs in the lab profile
  with K >= 256 or reports that it is unresolved.
- **Parameter lanes.** Each literature parameter has a published range in the register; each
  ensemble member draws all of them by a scrambled Sobol sequence keyed on the member index,
  deterministic. A switch fixes them at centre values for golden tests.

## Migration

- python/ceteris/analysis: holdout period config with the sealed hash check; MC error columns in
  every report table.
- Register: each Literature parameter gets a range column if it lacks one.
- core: parameter draw per lane from the Sobol stream.

## Tests

- The scorer refuses to read 2019-2024 targets unless invoked with the sealed-gate flag, and the
  flag writes a changelog entry.
- A Bernoulli(0.05) toy at K = 32 reports a standard error within 10% of 0.0385.

## Consequences

- Selection has less data (8 years); sealed scores are honest.
- Ensemble spreads widen by the parameter uncertainty that was missing.
- Rare-event claims are fewer and labelled.
