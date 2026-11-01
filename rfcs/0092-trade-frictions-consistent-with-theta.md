# 0092. Trade frictions consistent with theta

Status: proposed (owner chose the options, 2026-10-25); partly built, see As built
Date: 2026-10-25
Design sections: 5.4, 8.3, 15.2, 18 (H2), 17 (P01, P05); trade SPEC Two-level kernel, Parameters; econ SPEC Goods; ADRs 0018, 0022, 0023
Amended by ADR 0264 (proposed, 2026-10-30): H2 distance is a reported diagnostic; H2 per class is gated with the model's country-pair kernel as the cost control.
Amended by ADR 0276 (proposed, 2026-10-31): border friction per good in theta class 3, the clamp applied per good.

## Context

- **theta and nu disagree.** P01 takes the Caliendo-Parro class values (2.62, 5.88, 12.55 when written;
  2.62, 5.19, 10.57 since the OECD key, ADR 0023),
  while P05 keeps nu = 0.3. The PPML border coefficient on 2000 BACI implies nu of about 0.2 at
  theta 12 to 13, and about 3.0 for food at 2.62. At theta 2.62, nu 0.3 cuts cross-border food
  access by (1.3)^-2.62 = 0.50, while the data say about (4.0)^-2.62 = 0.026.
- **Gateway count sets access.** K_od sums over all k_A x k_B gateway pairs. If n pairs are
  about equally good, access rises n-fold: at 10 x 10 pairs, a 31% trade-cost cut at theta 12.55
  and 83% at 2.62. Gateway counts scale with the square root of 2000 trade, so trade partly
  causes its own access.
- **Good 5 mixes elasticities.** Refined petroleum (ICIO C19, theta 64.85) is averaged with
  chemicals and metals (1.7 to 3.3) into 12.64 and placed in class 2.
- **H2 per class is loose.** It accepts any value in the P01 prior (2.62 to 12.55).

## Options

theta: (1) Caliendo-Parro classes; (2) own PPML; (3) a long-run value near 2.
Frictions: (a) fit nu per class to the cross-border share, H2 stays the gate; (b) also fit a
leg-cost scale to the PPML distance coefficient and replace H2 with a holdout; (c) keep nu 0.3.
Petroleum: (i) keep in good 5; (ii) move into good 1; (iii) a new good.

## Decision

(1), (a) and (ii) (owner, 2026-10-25), plus gateway mass weights.

- **theta.** P01 stays at the Caliendo-Parro class values, recomputed after the C19 move. The
  PPML estimates stay diagnostics.
- **nu per class.** nu_c (P05, now one per theta class) solves, given theta_c, the
  absorption-weighted cross-border share of the class's goods in ICIO 2000. One moment per class,
  exactly identified and monotone in nu. Each evaluation is ADR 0089's stage 1 and 2 solve
  (about 20 Sinkhorn solves per class). Leg costs stay from P04 and P43, so H2's distance range is
  not fitted and stays the gate. If H2 misses after this, work stops and the miss goes to the
  owner, as now.
- **H2 per class.** Each class must recover its own theta_c within plus or minus 2 standard
  errors of the PPML estimate, not the whole P01 prior.
- **Gateway mass weights.**

  ```
  K_od = sum_a sum_b K_oa m_a K_ab m_b K_bd,    sum_{a in G(A)} m_a = 1
  ```

  m_a is the gateway's share of its country's 2000 trade throughput, else 1/k_A. Splitting a
  gateway into copies changes nothing, and the low-rank evaluation is unchanged (m folds into
  the access and crossing legs).
- **Refined petroleum.** ICIO C19 moves from good 5 into good 1, renamed "Crude oil, gas and
  refined fuels". Ten goods remain. Good 5's class value is recomputed (expected about 3,
  class 0). Crude, gas and refined fuels split in P8 with ADR 0028.

## Tests

- Calibration: at the solved nu_c, the simulated cross-border share matches ICIO per class
  (1e-6), for unclamped classes; a clamped class (Amendment 2026-10-30) sits at nu = 0.
- Gateway invariance: duplicating a gateway with m split in half leaves every flow unchanged
  (1e-12).
- H2: distance coefficient in -1.1 to -0.7 (unchanged); per class within 2 s.e. of PPML.
  Amended by ADR 0264: the distance coefficient is reported against -1.1 to -0.7, not gated;
  per class the PPML controls for the model's country-pair kernel net of tariffs instead of ln
  distance, within max(2 s.e., 1% of theta_c).
- Pack: good 1 and good 5 totals equal the ICIO aggregation with C19 moved; econ golden hashes
  regenerated.

## Amendment (2026-10-30): class 3 clamped at nu = 0

Owner ruling 1B (2026-10-30) added a bulk freight rate for class 3 (ADR 0247) with an ablation
test whose fallback is this amendment: the rate stays off by default and a class whose ICIO
target is above its share at nu 0 is clamped at nu = 0, the corner of nu >= 0, with the miss
recorded. The fit no longer stops there; it reports the clamped classes (`Fit::clamped`) and
fits the others. A target below the share at the bracket's upper end still stops it.

Miss on the 2000 pack (B1 econ tables, masses, home shares; rate off): class 3's ICIO target
0.2964 against the model's 0.2007 at nu 0. The ablation result that triggered the fallback is in
ADR 0247, As built.

Fit on that pack with the rate off (scratch `b2/pack-off`): nu = 2.6767, 0.8032, 0.4003 for
classes 0 to 2 (shares equal to the targets 0.1544, 0.3085, 0.1391 within 1e-10 relative), class
3 clamped at 0 (share 0.2007), 17 evaluations; calibrated from cold in 70 passes (712 s with the
fit). At the calibrated steady state the shares are 0.1545, 0.3108, 0.1428 and 0.1963. H2 after
the fit: distance -0.509 (s.e. 0.014), outside -1.1 to -0.7; per class (theta_c against the PPML
estimate +- 2 pair-clustered s.e.) class 0 2.91 against 2.221 to 2.566, class 1 4.14 against
2.993 to 4.021, class 2 7.68 against 7.231 to 7.998 (pass), class 3 16.91 against 9.686 to 15.086.
Only class 2 passes. Before the fit (every nu 0.3) distance was -0.433.

Note (owner ruling 2026-10-30, ADR 0247 As built): the bulk freight rate is now on by default for
class 3 (P154 0.026, P155 0.40). With it, class 3's share at nu 0 is 0.3037, above its target, so
the fit has a root (nu 0.0078 on scratch pack `b2/pack-bulk`, task b2-1b) and the clamp no
longer applies to class 3. The clamp stays the general rule for any class whose target is above
its share at nu 0.

## Consequences

- The border effect is consistent with each class's theta. Food trade across borders falls to
  what the data show.
- P05 becomes three values (one per class), calibrated, not a prior.
- Trade access no longer depends on the gateway-count rule, which can be tuned for speed alone.
- ADR 0026 must map Pareto k = theta (sigma - 1)/sigma, since tariff-based estimates identify
  k sigma/(sigma - 1) under Melitz-Chaney; ADR 0027 must keep theta_c as its long run (ADR 0095).
- The econ SPEC goods table, BACI weights per value w_g of goods 1 and 5, P01 values and the pack
  pipeline change.

## As built

Only the gateway mass weights were built, in the pre-p3/speed branch, and they were deferred. They
entered the crossing factor as `k = m_a m_b (tau (1 + nu) (1 + t))^-theta` with m_a = 1 / k_A (the
pack has no throughput data), and the stored reciprocals, ADR 0093's log-domain weights and a
re-bless carried them. With them, H2 distance moved from -0.7531 (s.e. 0.0161) to -0.536 (s.e.
0.013), outside the -1.1 to -0.7 band. By owner decision (2026-10-26) the masses were removed and
are deferred to the per-class nu fit, with which they land; the band holds at -0.7531 without
them. The per-class nu, the border friction fit and H2 per class remain to be built.

The C19 move was built on 2026-10-29 with ADR 0101's harmonic means, on the Caliendo-Parro
replication vector (owner ruling; ADR 0023, Amendment 2026-10-29). The rule draws four classes,
not three: 2.91 (food; chemicals and metals), 4.14 (machinery, electronics and vehicles), 7.68
(agriculture; light manufacturing) and 16.91 (crude oil, gas and refined fuels; coal and other
mining). P05 therefore becomes four values, one per class. Good 5 is renamed "Chemicals and
metals". The PPML diagnostics on the new goods are in ADR 0022.

Side effects of the move, accepted in the B1 review (2026-10-29): refined fuels now take the
primary tariff group (group 0, with good 1; under good 5 they took the manufactured rate), which is
closer to WDI's primary-products rate (SITC 0 to 4 and 68, fuels included); good 5 stays a
storable commodity (P151) without refined fuels, since metals are; good 1's P02 coverage stays 1
month; PIGL category M is unchanged for goods 1 and 5.

Built on 2026-10-29 (pre-P3 B2), with the fixture's tests; the pack steps (the fit on the full
pack, H2 and the gateway count) wait until the full pack calibrates again:

- Gateway masses as above, m_a = 1/k_A (no throughput data in the pack), folded into the
  crossing factor; duplicating a gateway with half its mass on each copy changes no flow
  (1e-12).
- P05 is four values, one per theta class. `econ_home_shares.arrow` holds, per pack country and
  traded good, ICIO 2000's domestic-origin use over total use (intermediate plus final demand,
  DPABR left out) and the total use. The target of class c is the absorption-weighted
  cross-border share over the ICIO economies. The model's share is the value the destinations of
  those countries buy from other countries over all they buy, at origin prices. Each evaluation
  is ADR 0089's stage 2 at stage 1's spending of the data state, held for every trial (controller
  ruling, 2026-10-29), so the four classes are independent 1-D roots. Brent's method per class
  narrows its bracket to 1e-9 in nu over [0, 10] on the computed shares, the classes sharing each
  evaluation and every scaling starting from the one at nu 0. The root is known only to the
  scaling's noise in the share over the share's slope: on the fixture, targets made at nu 0.15,
  0.6, 0.35 and 1.2 come back within 2e-10 for classes 0 to 2 and 6.8e-9 for class 3, whose
  share moves by only 2.7e-7 per unit nu (noise about 1e-15 in the share, so about 4e-9 in nu),
  in 19 evaluations. The fixture test checks the shares relative to the target (1e-6), not the
  PLAN's absolute 1e-6, which class 3's flat share would pass for any nu from 0 to about 4. The shares the calibrated
  steady state reaches, with stage 1 no longer at the data state, are printed after the
  calibration (the coupled check).
- H2 per class: theta_c within 2 pair-clustered standard errors of the class's PPML estimate
  (controller ruling, 2026-10-29). The kernel-control regression is not built yet; it is on the
  B8 list.
- Interfaces as built (they differ from the PLAN): `fit_border_nu(inp, target, icio, limits,
  progress) -> Result<Fit, _>`, without `u_star` (stage 1 at the data state does not use it),
  with the ICIO flags and returning the shares and the evaluation count with the nu;
  `target_shares(pack, theta_class) -> Result<([f64; 4], Vec<bool>), String>`, which also returns
  the ICIO flag of every pack country.
- ICIO 2000 targets on the 2000 pack (80 ICIO economies): 0.154 (food; chemicals and metals),
  0.309 (machinery, electronics and vehicles), 0.139 (agriculture; light manufacturing), 0.296
  (crude oil, gas and refined fuels; coal and other mining).
