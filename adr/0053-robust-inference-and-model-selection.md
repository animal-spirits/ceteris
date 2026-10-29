# 0053. Robust inference and model selection

Status: proposed
Date: 2026-10-24
Design sections: 15.2, 15.6, 15.7, 17.2
Phase: P9
Paths (ADR 0113, 2026-10-29): `python/` is gone; the calibration harness is `ceteris-calibrate`.

## Context

With up to 60 free behavioural parameters across many modules, a 2000-2010 calibration window
and a 2011-2024 holdout, the main risk is misspecification and weak identification rather than
sampling noise. Neural posterior estimation can be confidently wrong under misspecification,
and posteriors are uninformative without coverage checks. The ablation rule (15.7) decides
which mechanisms stay, but on point skill alone. Gradient-based calibration (0096) helps for
smooth blocks but is fragile for discrete ones (Quera-Bofarull et al. 2023).

## Options

1. Status quo: NPE or ABC-SMC, history matching, Sobol indices.
2. Add simulation-based calibration checks, learned summary statistics, a
   misspecification-robust variant, Bayesian stacking over structural switches, neural
   surrogates, causal validation of the estimators used in natural experiments, and regime
   scans.

## Decision

Option 2.

- Simulation-based calibration (Talts et al. 2018) for every posterior: coverage of rank
  statistics on simulated data with known parameters.
- Learned summary statistics (summary networks in the BayesFlow style, or signature features of
  time series) alongside hand-picked moments.
- Misspecification-robust inference: generalized Bayes or robust NPE with an explicit
  discrepancy (consistent with Kennedy-O'Hagan in 15.7).
- Ablation and switch choices by Bayesian stacking of holdout predictive densities (Yao et al.
  2018), not point skill alone; lanes that the data cannot weigh (climate damages, ADR 0036)
  are excluded from stacking and reported separately.
- Neural surrogates of annual aggregates, trained on the overnight batches, inside history
  matching; Rust inference with burn or candle if Python round trips cost too much.
- Causal validation: in simulated natural experiments (China shock, tariffs), the empirical
  estimators (difference in differences, synthetic control, shift-share) must recover the
  model's known causal effect; when they do not, the gap is reported with the comparison to the
  literature.
- Regime scans: classify runs into regimes (stable, cycles, crisis-prone) across parameter
  space, as in the Mark-0 phase diagrams (Gualdi et al. 2015), so posteriors near regime
  boundaries are flagged.
- Forward-mode derivatives (0096) are used for smooth blocks only; discrete blocks use
  simulation-based methods.

## Migration

- P9: python/ceteris/calibrate gains the SBC harness, stacking and the causal-validation
  runner.

## Tests

- SBC on a toy model with known posterior shows uniform ranks.
- The causal-validation runner recovers a planted effect in a toy panel.

## Consequences

Posteriors and mechanism choices come with diagnostics a reviewer can check, and
natural-experiment comparisons test the estimators as well as the model.
