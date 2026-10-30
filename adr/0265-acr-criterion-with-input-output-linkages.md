# 0265. The ACR criterion with input-output linkages

Status: proposed
Date: 2026-10-30
Design sections: 15.2, 21 (pre-P3 benchmark); amends ADR 0207 (pre-P3 benchmark, "the ACR
formula holding"); ADRs 0200, 0257

## Context

ADR 0207 ends pre-P3 with a rerun of the 28 October benchmark, one of whose criteria is "the ACR
formula holding". The pre-P3 PLAN made it checkable with the multi-sector formula of Arkolakis,
Costinot and Rodriguez-Clare (2012), d ln W_c = -sum over traded g of Omega_cg / theta_g
d ln lambda_cg: under every crossing tau x 1.1, every ICIO country with |ACR| above 1e-4 has the
ACR sign, the median model / ACR ratio is in [0.8, 1.25] and the correlation across countries is
at least 0.9.

That formula has no input-output linkages. The model has Leontief intermediates (ADR 0020,
`econ_io`), with a median column sum of A about 0.55 on the pre-P3 scratch pack `k263/pack`.
For a model with roundabout production the literature's prediction is

d ln W_c = -Omega_c' (I - A_c')^-1 diag(1/theta) d ln lambda_c,

Costinot and Rodriguez-Clare (2014, Handbook of International Economics vol. 4 ch. 4, section
3.4, eq. (28) of NBER WP 18896 under perfect competition, delta_k = 0, where the adjusted
Leontief inverse is the plain one) and Caliendo and Parro (2015), with A_c the country's cost
shares a_kg of input k in sector g and d ln lambda 0 for local goods. With one good using
itself at cost share a it is 1 / (1 - a) times the plain formula (fn. 20 there). The B2b review
estimated that amplification on `k263/pack` at 2.34 at the median (p10 1.71, p90 2.83); the
benchmark measured a median model / ACR ratio of 2.45 against the plain formula.

Measured with `ceteris_calibrate::benchmark` under x1.1 on `k263/pack` (1,200 ticks
frictionless from the calibrated state), the 80 ICIO countries with |ACR| above 1e-4:

| ACR form | median model / ACR | p10 | p90 | correlation | wrong sign |
|---|---|---|---|---|---|
| plain (`acr`) | 2.453 | 1.849 | 3.025 | 0.847 | 0 |
| input-output (`acr_io`) | 1.064 | 0.941 | 1.306 | 0.865 | 0 |

The baseline on that pack does not settle (largest |d ln p| over the last 50 ticks 0.17, the
Sudan drift ADR 0256 removes), so these are a record, not the gate.

## Options

- **A. Grade the input-output formula** (`acr_io`), same band and correlation. The benchmark
  then tests the model against the prediction for the model it is.
- **B. Keep the plain formula and widen the band** by ADR. The band would have to absorb an
  amplification that varies by country with its input shares, so the correlation criterion
  would still fail for a reason that is not a model defect.

## Decision

Option A (owner, 2026-10-30). ADR 0207's ACR criterion uses d ln W_c = -Omega_c' (I - A_c')^-1
diag(1/theta) d ln lambda_c (`ceteris_calibrate::benchmark::acr_io`, A_c from `Countries::io`,
which the calibrated coefficients reproduce at the calibrated prices). The rest stands: under
every crossing tau x 1.1, every ICIO country with |ACR| above 1e-4 has the ACR sign, the median
model / ACR ratio is in [0.8, 1.25], and the correlation across countries is at least 0.9. The
plain formula stays computed and reported beside it. C7 asserts the criterion at B8 on the final
pack; until then the report test (`benchmark_cases_are_reported`) prints both forms and whether
the input-output one is in the band, and asserts nothing on it.

## Consequences

- PLAN (pre-P3, ADR 0207 criteria) and the econ SPEC (Calibration, Benchmark) name the
  input-output formula as the graded one.
- On `k263/pack` the median passes (1.064) and the correlation does not (0.865 < 0.9); whether
  the correlation holds on a settled baseline is C7's question at B8. A miss there is a tier 1
  item under ADR 0257.
- The cost shares are the data's, not the run's: the formula is first order, as the welfare
  measure's buyer price and Omega are.
