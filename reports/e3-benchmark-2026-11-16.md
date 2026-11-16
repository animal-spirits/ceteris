# e3 benchmark on the full pack, 16 November 2026 (#561 step 1)

Run: `CETERIS_PRICE_ANCHOR=e3 cargo test --release -p ceteris-calibrate --test benchmark -- --include-ignored --nocapture`
(573 s; log `~/ceteris-logs/e3-bench-20261116T0415Z.log`) at main 2060a4b, on the Nov 4 calibrated 2000
pack. The pack was not recalibrated: current `calibrate` starts the epsilon fit, which this
task excludes. Result: 13 pass, 3 fail, as in the 12 November assessment.

## (a) usize underflow, fixed

`benchmark_cases_are_reported` and `acr_formula_holds_under_costlier_borders` panic at
`acr_stats` (`ratio.len() - 1` with an empty `ratio`). Cause: the province form has no gradable country
(every ICIO country has NaN province welfare or ACR), so `graded` is empty. `acr_stats` now returns
(0, NaN, NaN) for no pairs; a test fails without it (`acr_stats_of_no_graded_country_are_nan_not_a_panic`).
The strict test still fails on its median/correlation assertion, which is correct: that is the
benchmark miss below, not a harness bug. No pass criterion changed.

## (b) openness outside 1.5x of ICIO (ADR 0207, recorded, not retuned)

| class group | model | ICIO | ratio |
|---|---|---|---|
| [3, 5] | 0.3797 | 0.1544 | 2.459 |
| [6] | 0.8568 | 0.3085 | 2.777 |
| [0, 4] | 0.6286 | 0.1391 | 4.518 |
| [1, 2] | 0.3845 | 0.2964 | 1.297 |
| all traded | 0.7050 | 0.2115 | 3.333 |

Band [1/1.5, 1.5]: three groups and the pooled share fail; [1, 2] passes.

## Context for the misses (same run)

- The base run is not settled under e3: base max |dln p| over the last 50 ticks is 4.998, and every
  shocked run ends at about 5.0 (the cap), so the shock outcomes (Brexit GBR -3.05%, JPN -13.9% at
  border x1.1, US-China pass-through -4.0) are of an unconverged baseline and not comparable to the
  RFC 0350 bands. Openness above is measured on that baseline.
- ACR x1.1 plain: 7 ICIO countries gradable (71 NaN), 3 wrong sign, median 0.193, corr 0.303;
  input-output: 2 wrong sign, median 1.057, corr 0.333; province: none gradable.
- The fitted border nu targets these shares at the data state (calibration), so a 2.5x to 4.5x
  miss in the running model points to the e3 price rule moving the run away from the calibrated
  state rather than to the fit. Not diagnosed here.
