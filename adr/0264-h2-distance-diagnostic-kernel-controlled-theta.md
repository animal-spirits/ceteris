# 0264. H2 distance a reported diagnostic, theta per class gated with the model's kernel

Status: proposed
Date: 2026-10-30
Design sections: 18 (H2), 21.1 (P2 gate); amends ADR 0092 (Tests); ADRs 0022, 0089, 0247, 0257

## Context

H2 has two parts (DESIGN 18; ADR 0092, Tests): the pooled PPML distance coefficient of simulated
country-pair trade must lie in -1.1 to -0.7, and per theta class the tariff regression (ln(1 + t)
and ln distance as the cost control) must recover theta_c within 2 pair-clustered standard
errors. On the pre-P3 B pack both miss: distance -0.463, classes 0 and 1 outside their bands
(pre-P3 diagnosis H2, 2026-10-30).

**Path.** The P2 gate passed at -0.753 (gate thetas 2.62, 5.19 and 10.57, nu 0.3, no gateway
masses). Then: gateway masses 1/k -0.536; the B1 class thetas (2.91, 4.14, 7.68, 16.91) -0.433;
nu per class (ADR 0092) -0.509; the bulk freight rate (ADR 0247) -0.463; the selection constant
(ADR 0200) no change. A fair split on one pack from the gate corner (-0.693): B1 thetas +0.106,
masses +0.093, nu +0.031 (Shapley), bulk rate +0.046, selection 0.

**Mechanism.** Equal masses multiply every country pair's kernel by 1 / (k_A k_B), which PPML's
exporter and importer effects absorb. They mattered at the gate only through an over-open model
(cross-border shares 0.99, 0.99, 0.79 and 0.32 against ICIO 0.15, 0.31, 0.14 and 0.30) and the
class mix at the gate thetas; at today's thetas and nu, masses one against 1/k move distance by
0.013. The pooled coefficient is the international-value-weighted sum of the class
coefficients (within 0.01), and each class coefficient is about theta_c times the model's
cost-distance slope, d ln tau_eff / d ln distw = 0.117, 0.083, 0.108 and 0.062 by class: -0.28,
-0.32, -0.66 and -0.38. The 2000 data (ADR 0022, BACI on CEPII distw) give about -0.8 per class
whatever theta. With iceberg leg costs tau = 1 + c w + lambda T and the class thetas, the slope
is too small to reach -0.7; the distw basis is like for like, the miss is the level of the slope.
ADR 0022 foresaw this (Risk for H2): the data prefer a log-distance form to the S05 iceberg form.

**Class bias.** Classes 0 and 1 miss through omitted cost in the test regression, not in the
model: within importer x good the only tariff variation is agreement against non-agreement
partners, agreement partners are near, and ln distw does not span the model's costs. With the
model's own country-pair kernel as the control theta is recovered (2.912, 4.146 and 7.687 at the
reference flows; class 3 16.87 with distw, contiguity and the kernel). Contiguity alone fixes
class 1 (3.92), not class 0 (2.557 against 2.34 to 2.77). The bias was there at the P2 gate
(food 1.75 against 2.62) under the wider P01 band. Neither masses nor leg length cause it.

## Options

- **A. Gateway masses from data.** Distance -0.470 to -0.473; classes still miss; no throughput
  source in the pack.
- **B. Masses one and nu refit.** nu leaves its bracket (16.5, 5.07, 1.53, 0.32), distance -0.439,
  and ADR 0092's duplication rule is lost.
- **C1. Another distance basis.** No grounds: distw is the basis of the data estimate.
- **C2. Theta per class gated with the model's kernel as the cost control.** Passes classes 0
  and 1; a test change and an ADR 0092 note.
- **D. Fit a leg-cost scale with nu to a distance target.** About 2.2 to 2.4 times leg costs for
  -0.70; class 3 clamps, Sinkhorn turns fragile; contradicts ADRs 0102 and 0247 and turns the
  gate into a fit.
- **E. Distance as a reported diagnostic.** An ADR amending DESIGN 18 and ADR 0092's tests.
- **F. A log-distance component in leg costs.** ADR 0022 says the data prefer it; likely about -1
  per class and likely removes the class bias too; needs a transport-cost form, a P04 refit and a
  recalibration. Not probed.

## Decision

Owner ruling A (2026-10-30): the diagnostician's E and C2 now, F scheduled.

1. **H2 distance is a reported diagnostic, not a gate.** `p2_gate.rs::h2_distance_elasticity`
   prints the coefficient, the data's band of -1.1 to -0.7 and the mechanism, and fails only on a
   non-finite or non-negative coefficient.
2. **H2 per class is gated with the model's kernel as the cost control.** Per class, PPML of what
   importers pay (FOB value times 1 + t) on ln(1 + t) and ln K_em,g, with exporter x good and
   importer x good effects, over the foreign pairs a crossing links (`ppml::class_tariff_fit`).
   K_em,g is the model's country-pair trade-cost kernel net of tariffs at the calibrated point
   (`model::ln_country_kernel`):

   ```
   K_em,g = sum over crossings a -> b of  u_a K_ab (1 + t_ab)^theta_g  v_b
   u_a = sum_{o in e} w_o K_oa / sum_{o in e} w_o
   v_b = sum_{d in m} (E_d / Phi_d) K_db / sum_{d in m} (E_d / Phi_d)
   ```

   with the sourcing weights w_o = (p_o / p_min)^-theta and E_d / Phi_d. With these weights a
   pair's flow at the reference equilibrium is exactly W_e B_m (1 + t)^-theta K_em,g, so the
   regression recovers theta_c with coefficient 1 on ln K; the run's flows test the simulation
   against it. theta_c must lie within **max(2 pair-clustered s.e., 1% of theta_c)** of the
   estimate (controller ruling, 2026-10-30, replacing ADR 0092's 2 s.e. alone). With the model's
   own kernel as the control the regression is nearly an identity (pseudo-R2 1), so the
   clustered s.e. no longer measures sampling error, only the run's distance from the reference
   (s.e. 3.5e-7 to 1.2e-4 on the B pack): the test checks that the code's trade elasticity is
   theta. The 1% is V1's nesting tolerance (DESIGN 15.3; ADR 0089), the precedent for how far a
   run may sit from the reference. The test prints the estimate, its s.e., the deviation from
   theta_c and the ln distw version as a diagnostic.

   The earlier figures 2.912, 4.146 and 7.687 (Context) are point estimates with the kernel
   weighted by province revenue and spending shares instead of the sourcing weights. Under those
   weights the 2 s.e. band misses classes 0 and 3 on the B pack (2.9118 against 2.9106 to 2.9130;
   17.233 against 16.97 to 17.50) and the fixture's class 3 by far (194.6 against 16.91 with a
   tariff per pair): at high theta the aggregation weights matter.

3. **The structural fix is scheduled.** A log-distance component in leg costs (option F) is
   proposed for P9, where ADR 0022 already revisits P01, P05 and P04 jointly with richer tariff
   data: it changes the transport cost form (geo SPEC, S05), refits P04 and recalibrates nu and
   the steady state, a P9-sized change. The owner may move it.

## Consequences

- Under ADR 0257 the distance miss is tier 2 (a measured moment) traced to a mechanism not yet
  built, the log-distance leg cost, recorded here with its phase; C7 grades it as such and it
  does not block the pre-P3 gate. Per ADR 0257 tier 4, what the changed tests no longer check:
  H2 distance no longer bounds the level of the model's cost-distance slope; H2 per class no
  longer checks that ln distance spans the model's costs, and its band is no longer a sampling
  band (it checks that the simulated flows carry each class's theta on the model's own costs, to
  V1's 1%).
- B pack (b2c, the last 12 ticks of the 1,200-tick default run): per class 2.9099999, 4.1399984, 7.6799690
  and 16.9104584 against 2.91, 4.14, 7.68 and 16.91 (largest deviation 2.7e-5 relative, against
  the 1% tolerance); ln distw diagnostic 2.393, 3.507, 7.613 and 16.438; distance -0.4631 (s.e.
  0.0108), outside -1.1 to -0.7.
- The per-class gate needs the calibration tables and the reference at the calibrated point, as
  V1 does; the CEPII gravity file is still read for the diagnostics.
- Until F lands, counterfactuals whose answer depends on the distance gradient (distant against
  near partners, trade diversion by distance) understate it: pooled -0.46 against the data's
  about -0.8.
- DESIGN 18 (H2) and the P2 acceptance line, ADR 0092 (Tests), the econ SPEC (P2 gate) and the
  trade SPEC (Equilibrium nesting) state the new tests.
