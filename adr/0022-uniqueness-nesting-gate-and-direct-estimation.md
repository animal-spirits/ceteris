# 0022. Uniqueness check, static nesting gate and direct estimation

Status: accepted (owner, 2026-10-24, PR 1); implemented on p2-econ (MIGRATION Task 9; the estimation part lands with PLAN Task 18)
Date: 2026-10-24
Design sections: 3.4, 15.2, 15.3 (V1), 17.1, 21.1 (P2); roadmap P2 gate
Paths (ADR 0113, 2026-10-29): the uniqueness check and PPML are `ceteris_reference::{uniqueness, ppml}`, the gravity estimation `ceteris_calibrate::gravity` (`ceteris gravity`), and the gate `crates/ceteris-cli/tests/p2_gate.rs`.

## Context

With agglomeration (A5) and trade, and with migration from P4, the spatial equilibrium can be
non-unique; then "within 1% of the GE reference" is ill-posed, since both models can be right
and sit in different equilibria. Reference models with perfect foresight
(Caliendo-Dvorkin-Parro, Kleinman-Liu-Redding) cannot be matched on transition paths by agents
with heuristic expectations. And theta_g, border frictions and mode costs are identified by
gravity equations the model nests, so estimating them by simulation wastes the run budget.

## Decision

1. Before V1, evaluate the uniqueness condition of Allen, Arkolakis and Li (2024), a
   spectral-radius bound on the matrix of elasticities built from theta, agglomeration and
   (from P4) migration dispersion, at the tested parameters. Where it fails, V1 starts both
   models from the same point and reports which equilibrium each reaches instead of a 1% gap.
2. V1 compares steady states with the custom static GE. Comparisons with CDP and KLR are sign
   and rank only, as the roadmap already states.
3. theta_g (0018), border frictions (P05) and mode cost ratios (P04) are estimated by PPML
   structural gravity on 2000 BACI with the route-choice cost structure, in
   python/ceteris/calibrate, outside simulation-based inference.

## Migration

- Add the spectral check to python/ceteris/reference and call it from the V1 test.
- Add the PPML estimation; its outputs enter the pack with provenance "data", not "calibrated".
- Update the gate text in the P2 SPEC.md.

## Tests

The gate itself; the PPML code reproduces published estimates on a published sample before it
runs on the pack.

## Mapping to P2 as built

- The uniqueness check is `python/ceteris/reference/uniqueness.py` (MIGRATION Task 9), Theorem 1
  of the paper (transcribed in `specs/econ/SPEC.md`, Equilibrium nesting). No example in the paper
  is a multi-sector model with an input-output table, so the check uses the special case of its
  Online Appendix A.3 (Eaton-Kortum with intermediates and quasi-symmetric costs) good by good, at
  theta_g and the good's smallest value-added share, and takes the largest radius; the SPEC lists
  what this leaves out. P2 has no agglomeration (A5 off) and no migration; the function raises for
  either until a phase that adds one derives its matrix. At the P2 defaults the bound is 0.922 (0.932 at the first thetas, before the OECD key, ADR 0023; 0.967
  on the 2000 pack's value-added shares, Task 20), so V1 runs in tolerance mode. It runs before V1 in `python/tests/test_p2_gate.py` (PLAN Task 20);
  its tests are in `python/tests/test_reference.py`.
- V1 already compares steady states with the custom static GE (the reference model).
- PPML on 2000 BACI runs on least-cost (S05) costs between countries, not route-choice costs:
  route choice is deferred past P2 (ADR 0017, Deferral). It reuses
  `python/ceteris/reference/ppml.py` (Task 12, which recovers a known elasticity on synthetic
  data) and first reproduces the published estimates of Yotov, Piermartini, Monteiro and Larch
  (2016) on their sample. Estimation is the first step of PLAN Task 18, since it needs the BACI
  cache and the pack's costs (Task 17); outputs carry provenance "data".
- The P2 gate text in `specs/econ/SPEC.md` is rewritten (MIGRATION Task 9).

## Direct estimation on 2000 BACI (2026-10-24)

PLAN Task 18 Step 1 (`python/ceteris/calibrate/gravity.py`, commit b967494). The estimator is
`ppml.hdfe` (absorbed fixed effects, standard errors clustered by country pair).

**Reproduction.** On the sample of Yotov, Piermartini, Monteiro and Larch (2016; `AdvancedGuideFiles.zip`,
WTO, all rights reserved, test only) the code matches every published PPML estimate and pair-clustered
standard error to the printed three decimals, and every observation count, in Table 1 column 4 (p. 42),
Table 2 columns 2 to 5 (p. 47) and Table 3 columns 2 to 4 (p. 51, column 4 with pair fixed effects)
(`test_gravity_reproduces_yotov_2016`). The R2 of Table 1 column 4 (0.614) is not reproduced (0.946,
squared correlation of flows and fits); the test does not check it.

**Sample.** BACI HS92 V202601, 2000, split over the 7 traded goods by the Task 13 concordance, on the 242
pack countries (218 trade); every connected pair, zeros kept (378,392 rows, 73% zero). Tariffs from
`econ_tariffs.arrow`. Costs: S05 at `TransportParams::default()`, the cheapest access, crossing and access
chain over gateway pairs, averaged over province pairs weighted by exporter VA of the good times importer
VA; the Python legs equal the `ceteris export` crossing legs to 4.4e-16
(`test_s05_costs_match_the_rust_crossing_legs`). Domestic flows for P05: 2000 ICIO output less exports
for the 80 ICIO economies (44,800 rows). Fixed effects exporter x good and importer x good.

**Estimates** (default in brackets; classes and defaults as of the estimate, 2026-10-24):

| Parameter | Estimate (s.e.) | N | pseudo-R2 | Default |
|---|---|---|---|---|
| P01 class 0 (food) | 13.36 (1.24) | 46,440 | 0.758 | 2.62 |
| P01 class 1 (goods 4, 6) | 13.20 (1.42) | 92,880 | 0.855 | 5.88 |
| P01 class 2 (goods 0, 1, 2, 5) | 11.82 (1.02) | 162,031 | 0.769 | 12.55 |
| P05 border friction nu | 0.204 (0.035) | 44,800 | 0.992 | 0.3 |
| P04 rail / road | 52.6 (24.1) | 301,351 | 0.866 | 0.4 |
| P04 sea / road | 115.3 (46.3) | 301,351 | 0.866 | 0.08 |
| P04 air / road | -2.39 (34.4) | 301,351 | 0.866 | 8 |

Since the OECD key (ADR 0023 amendment, 2026-10-25) the P01 defaults are 2.62 for food, 5.19 for good 6
and 10.57 for goods 0, 1, 2, 4 and 5. The estimates were made on the first partition and were not rerun.

theta is minus the coefficient on ln(1 + tariff), with ln tau (S05) as a free control. P05 pools the
classes' ln(1 + nu_c) = b_c / beta_c (border and tariff coefficients of one fit; class nu 0.388, 0.158,
0.272) with 2000 trade weights. P04 is nonlinear PPML (Gauss-Newton) of ln tau(r) with road at P04a as
numeraire.

**Sensitivity** (scratch runs on the same samples, not committed). Adding CEPII 2000 contiguity, common
official language and colony controls lowers theta to 9.36, 9.06 and 8.39 (8.4 to 9.4); CEPII
distw_harmonic in place of S05, with those controls, gives 6.57, 6.63 and 5.89 (5.9 to 6.6), distance
elasticities -0.82, -0.65, -0.94. Within importer x good the only tariff variation is agreement against
non-agreement partners at one WDI rate per importer and group, so theta is the agreement effect over
ln(1 + t) and absorbs proximity left out. P05 rests on the same-fit theta: at the Caliendo-Parro class
thetas the same border coefficients give nu of about 3.0 for food (exp(3.655 / 2.62) - 1).

**P04 is not identified.** The fit drifts to ratios (rail 53, sea 115 times road) at which the 1 in
tau = 1 + c w is negligible, so ln tau becomes ln of cost-weighted km and its coefficient falls to about
-1, the usual distance elasticity: the data prefer a log-distance form to the S05 iceberg form (pseudo-R2
0.866 against 0.845 at the default ratios). Air lies on about 70 of the 378,392 least-cost rows, so its
ratio has no support. A linear km form keeps the ordering (rail 0.12, sea 0.09, air 3.8) but is imprecise.

**Owner decision (2026-10-24).** P2 keeps the literature values: Caliendo-Parro class thetas 12.55, 5.88,
2.62 (ADR 0023; 10.57, 5.19, 2.62 since the OECD key), P05 0.3 and the geo SPEC P04 mode costs. The estimates are a recorded diagnostic; no
default, pack table or model input changes, and `--write` writes nothing while a stop rule fires.
Productivity calibration (PLAN Task 18 Steps 2 to 5) runs at the literature values. Joint estimation of
P01, P05 and P04 is revisited in P9 with richer tariff data (line-level applied and preferential rates).

**Risk for H2 (PLAN Task 20).** The 2000 data prefer log distance to the S05 iceberg form. Simulated
flows under S05 iceberg costs may therefore give a distance elasticity outside H2's -0.7 to -1.1, or a
tariff-based theta off its class value, for reasons of functional form rather than code.

**Amendment (2026-10-25, ADR 0050).** The sample's split of BACI lines over goods changed after these
estimates: the WITS and UNSD concordance gave way to the OECD BTiGE HS to CPA key, adapted locally
(gold to basic metals, books and recorded media to printing). Every line now lies in one good and all
2000 BACI lines are mapped. Traded value by good moves most for light manufacturing (1,563 to 952
billion USD), chemicals, refined fuels and metals (1,276 to 1,586) and machinery, electronics and
vehicles (2,421 to 2,891); the rest move by less than 10%. The ADR 0023 rule regroups the goods (light
manufacturing joins agriculture, mining and chemicals in the top class). The estimates above are
pending a re-run on the new sample and classes; the owner decision stands.

**Re-run after the C19 move and the four classes (2026-10-29, ADRs 0092 and 0101).** `ceteris
gravity` on the 2000 pack with its econ tables rebuilt in scratch (refined petroleum in good 1; the
installed pack is unchanged until it is rebuilt and recalibrated), BACI HS92 V202601 split by the
OECD key: 242 countries, 218 trading, 4,266 unreached pairs, international trade 6,495.1 bn USD
(6,495.0 bn in the sample), domestic flows for 80 ICIO economies with no cell dropped.

| Parameter | Estimate (s.e.) | N | pseudo-R2 | Default |
|---|---|---|---|---|
| P01 class 0 (goods 3, 5) | 12.42 (0.97) | 92,880 | 0.840 | 2.91 |
| P01 class 1 (good 6) | 14.01 (1.65) | 46,440 | 0.863 | 4.14 |
| P01 class 2 (goods 0, 4) | 10.94 (1.38) | 92,880 | 0.871 | 7.68 |
| P01 class 3 (goods 1, 2) | 14.05 (2.01) | 87,075 | 0.616 | 16.91 |
| P05 border friction nu | 0.206 (0.036) | 44,800 | 0.990 | 0.3 |

Class trade weights 0.268, 0.445, 0.174, 0.113. P05 by class: nu 0.263, 0.116, 0.342, 0.248 (s.e.
0.038, 0.024, 0.078, 0.058) at same-fit thetas 12.05, 15.80, 10.19, 12.75 (border coefficients
-2.81, -1.73, -3.00, -2.82). The ln tau coefficients of the theta fits are -8.49, -7.94, -8.80 and
-7.50.

The PPML thetas lie between 10.9 and 14.1 for every class, as on the earlier partitions: the
tariff variation identifies the agreement effect (see Sensitivity), not the spread of the
Caliendo-Parro classes. Only class 3 (16.91) has its default inside two standard errors of its
estimate.

P04 on the four classes: see the Amendment below. Without a guard the Gauss-Newton steps drove the
air ratio down (-5.2 to -12.2 by step 6) until tau was no longer positive on an air path, and
`ceteris gravity` stopped with "flows must be finite and nonnegative, regressors finite". On the
same flows with the three classes of before (food; good 6; goods 0, 1, 2, 4, 5) the unguarded fit
converged in 47 steps to rail 0.234 (s.e. 0.093), sea 0.102 (0.014) and air -1.59 (1.49); the P01
estimates on those classes are 12.98 (1.19), 14.01 (1.65) and 12.21 (1.06), and P05 is 0.207
(0.035).

**Amendment (2026-10-29): guarded P04 fit, support threshold, non-fatal failure.** Owner-delegated
ruling during pre-P3 B, for owner review. The estimator, objective and owner decision above are
unchanged; this records how "Gauss-Newton" is carried out and what the diagnostic reports.

- *Guard.* Each step is halved, at most `MODE_MAX_HALVINGS` = 30 times, until every tau is
  positive, the profile PPML (without the step regressors, at the trial ratios) converges, and its
  deviance D (`Fit.deviance`) does not rise beyond the resolution of the fits:
  D_trial <= D + `ppml::TOL` (|D| + 0.1). That allowance is the IRLS stopping rule of the PPML,
  so two deviances closer than it are not distinguishable at the tolerance the fits were solved
  to; it adds no free number. A strict comparison fails on that noise near convergence, where
  a step's true decrease falls below it and halving cuts the decrease but not the noise, and
  would report a converged fit as a failure. The allowance is the decrease of a move of about
  1e-5 of the ratios, far below the rises of order 1 to 1e3 the guard exists to stop. A trial
  whose PPML fails is a rejected trial. Convergence is tested on the undamped step, so a heavily
  halved step does not pass for convergence. The ln tau coefficients that scale the step
  regressors are those of the profile fit at the current ratios. At the cap the fit returns an
  error.
- *Support.* A mode is fitted only if at least `MODE_MIN_PAIRS` = 50 unordered country pairs have a
  positive flow on a least-cost path with km on that mode; otherwise it stays at its geo SPEC
  start, has no estimate (NaN) and the report names it as not identified with its count. Pairs
  are the clusters of the covariance. When the variation of a regressor comes from few clusters,
  cluster-robust inference fails however many clusters the sample has in all (few treated
  clusters: Conley and Taber 2011; MacKinnon and Webb 2017, 2018), so a ratio carried by a few
  pairs has no usable standard error. The threshold does not bind: on 2000 BACI air has 3 such
  pairs (about 70 least-cost rows, 4 to 6 positive per good, 0.1 bn USD; its step s.e. was 1.5
  to 20), while rail and sea have thousands of pairs and positive flows on tens of thousands of
  rows, so any threshold from about 5 to several thousand gives the same result. The synthetic
  samples of the tests (30 countries, 435 pairs) clear it on every mode, and the test that drops
  it keeps air on at most 10 pairs. This is an estimation rule, not a model parameter: an
  unidentified mode has no estimate, the ordering stop rule fails on it and nothing is written,
  so it cannot change a model input and has no register ID.
- *Failure.* A P04 error (no step at the cap, no convergence in 50 steps, a failed PPML) no
  longer stops `ceteris gravity`: P01 and P05 are reported, the report prints "P04 did not
  converge: ...", a stop rule fires and `--write` leaves `gravity_2000.toml` as it is
  (`gravity::write_toml`).
- *Positive ratios.* The ordering stop rule also requires every ratio to be positive
  (0 < sea < rail < 1 < air): a ratio at or below zero is not a cost and fires it.

Result on the 2000 pack with the B1 econ tables (scratch rebuild, as above): air not identified (3
pairs; rail 13,367, sea 14,654), held at 8; 33 steps, none halved (a strict deviance comparison
took 37 steps, halving 5 of them on deviance noise near convergence); profile deviance 191,825 to 179,517; rail / road
36.61 (s.e. 19.1), sea / road 84.59 (39.5); ln tau coefficients -0.99, -1.08, -1.05, -1.51; N
319,275, pseudo-R2 0.873. This is the log-distance corner described under "P04 is not
identified", now reached without a crash; the ordering stop rule fires and nothing is written.
P01 and P05 are as in the table above. The earlier convergence on the three classes (rail 0.234,
sea 0.102, air -1.59) was a local stationary point: the profile deviance there is 191,640, about
10,000 above the corner (181,360 on those classes, started from the corner); the four classes
remove the small barrier in front of the corner. Diagnosis:
`.superpowers/sdd/PLAN-pre-p3-rust/p04-diagnosis.md`.

## Consequences

The P2 gate is well posed, and fewer parameters enter joint inference in P9.
