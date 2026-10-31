# 0272. Log-distance trade cost on the border crossing

Status: proposed
Date: 2026-10-31
Design sections: 5.4 (nu_od, non-policy frictions), 8.3 (two-level kernel), 17.1 (P05; new P158),
18 (H2), 21 (P4, P9); amends ADR 0092 (nu per class) and ADR 0264 (Decision 3, option F);
ADRs 0022, 0200, 0241, 0247, 0257, 0265, 0268, 0269, 0271

## Context

ADR 0264 made H2's distance coefficient a reported diagnostic and scheduled option F, "a
log-distance component in leg costs", because the model's pooled PPML distance coefficient is
-0.463 (s.e. 0.011) on the pre-P3 B pack against about -0.8 in the 2000 data (BACI on CEPII
distw, the same specification). Per class the model gives about -0.28, -0.32, -0.66 and -0.38
(theta 2.91, 4.14, 7.68, 16.91); the data give about -0.8 in each class, whatever theta
(ADR 0264, Mechanism; ADR 0022 sensitivity: -0.82, -0.65, -0.94 on the earlier keys). ADR 0022
found the same from the other side: the P04 fit drifts to a log-distance form (pseudo-R2 0.866
against 0.845). ADR 0269 then borrowed the mode ratios as freight rates and moved their fit to
P9 "with ADR 0264's option F, one refit of the transport form instead of two".

This ADR fixes what option F is before anyone builds it. The literature says the missing
distance cost is mostly not freight, so it should not go into the freight legs.

### Freight is concave in distance, and the model already is

- **Freight rates rise less than proportionally with distance.** Hummels ("Toward a Geography of
  Trade Costs", GTAP working paper 1162, Table 3), freight rate on distance with weight per value
  controls: ocean 0.220 (s.e. 0.002, US imports), air 0.459 (0.003), truck 0.275 (0.006) and rail
  0.388 (0.010) (US transborder), 0.267 (0.002) pooled across importers. At the mean weight per
  value, US freight is 7.1% of value at 2,500 km and 11.4% at 15,000 km. Limao and Venables
  (2001, WBER 15(3)) find the same concavity in shipping quotes, with land km dearer than sea
  km by a multiple (their quoted per-1,000-km figures are to be checked against the text before
  this ADR is accepted).
- **The S05 form reproduces it.** A leg's cost per tonne is a fixed part (port and access fees,
  P04m-n) plus a per-km part, so its elasticity to distance, s l / (F + s l), rises from 0
  towards 1 with length. ADR 0247's sourced bulk line (F 6.47 USD/t, s 2.05e-4 USD/tkm) gives
  0.11 at the Curacao-Houston 3,840 km and 0.24 at 10,000 km, inside Hummels's ocean 0.22. A
  concave per-km term (c l^alpha, alpha < 1) would lower the leg slope, not raise it, and
  contradicts the sourced linear-plus-fixed fit of 0247 and the freight ratios of 0269.
  **Concave freight is not the fix.**

### Freight cannot carry a distance elasticity of -0.8

- **The trade distance coefficient is about -0.9 and has not fallen.** Disdier and Head (2008,
  REStat 90(1)): 1,467 distance effects in 103 papers, mean about 0.9, persistently high since
  mid-century while freight costs fell. Head and Mayer (2014, Handbook of International
  Economics vol. 4, ch. 3): median -0.89 over all gravity estimates, -1.14 over structural ones.
- **Freight alone gives a fraction of it.** With ad valorem freight f of 0.07 to 0.11 and a rate
  elasticity of 0.27, d ln(1 + f) / d ln d is about 0.02 to 0.03; times a theta of 4 to 8 that
  is -0.1 to -0.2, a fifth or less of -0.9. Grossman (1998, comment in Frankel ed., The
  Regionalization of the World Economy) made this point; Hummels's own conclusion is that
  distance may carry information costs as well as freight (citing Rauch 1999, JIE 48). The
  model's slopes (0.06 to 0.12 by class) already exceed freight's alone because they carry time
  (lambda T) and port costs; they are still short.
- **The missing cost has a trade elasticity that does not scale with theta.** If distance cost
  were an iceberg cost like freight, each class's distance coefficient would be theta_c times
  a common slope and would rise sixfold from class 0 to class 3. The data give about -0.8 in
  every class. Chaney (2018, JPE 126(1)) derives a distance elasticity close to 1 from the
  formation of firms' contact networks, independent of freight and of the trade elasticity, and
  constant over time. Head and Mayer (2013, CJE 46(4), "What separates us?") call such costs
  dark trade costs; Allen (2014, Econometrica 82(6)) measures information frictions directly.
- **The log form.** A constant elasticity of trade to distance is the gravity convention and is
  what ADR 0022's fit converged to. Eaton and Kortum (2002, Econometrica 70(5)) use distance
  intervals instead; that is the robustness check below, not the default.

### Where the model can carry it

The kernel is two-level (trade SPEC): domestic pairs on direct legs, cross-border pairs through
gateway access legs and crossings, K_ab = (tau_ab (1 + nu_AB) (1 + t_AB,g))^-theta. nu is DESIGN
5.4's nu_od, non-policy frictions (language, legal system, currency), fitted per class to ICIO
2000 cross-border shares (ADR 0092). The data moment is country-pair (distw), and land crossings
join two gateways a few km apart, so a factor on leg km would price Germany-France at about
nothing and a sea crossing at a lot, for reasons of gateway geometry.

## Options

**Placement** (card 2; owner chose A, 2026-10-31).

- **A. A country-pair factor on the crossing, beside nu** (recommended). ln D_AB,c =
  (epsilon / theta_c) (ln d_AB - ln d_0). Keeps the gateway factorization, Dijkstra, the
  legs, mode choice, 0247 and 0269 unchanged; matches the data moment's basis; one parameter.
- **B. A factor on every leg, d^delta_c on great-circle leg km, domestic included.** Domestic
  trade gets a distance gradient too, but a cross-border pair pays the sum of its legs' logs,
  not the log of its distance, and land crossings pay almost nothing; changes the domestic
  steady state everywhere.
- **C. A factor on the origin-destination pair.** Exact, but breaks the gateway factorization:
  K_od dense over 4,594 squared pairs per good, O(P^2) memory and time per tick. Rejected on the
  160 ms budget (ADR 0235).
- **D. Concave freight (c l^alpha).** Rejected above: lowers the slope and contradicts
  0247 and 0269's sourced rates.

**Elasticity across classes.**

- **One epsilon, delta_c = epsilon / theta_c** (decided). Each class's trade elasticity to the
  term is epsilon, as the data's flat per-class pattern asks. One fitted parameter.
- Common delta (the iceberg reading) gives class coefficients theta_c delta, a sixfold spread
  the data do not show. Per-class epsilon_c fits four moments with four parameters and leaves
  nothing held out.

**Phase** (card 1; owner chose P4, 2026-10-31).

- **P4** (recommended): built, fitted and calibrated with P4's recalibration on the 50 ICIO
  goods (ADR 0271), which recalibrates nu and the steady state anyway; P5 to P8 counterfactuals
  get the data's distance gradient; P9 re-estimates epsilon jointly with P04 and P01.
- **P9**, as ADR 0264 proposed: one transport refit with 0269's mode-share fit. Every
  counterfactual through R understates the distance gradient (ADR 0264, Consequences).
- **P3 now**: the workbench gets it first, but costs a recalibration on 4 classes that P4's 50
  goods then redo.

## Decision

Owner rulings (2026-10-31): phase P4 and placement A, the country-pair factor beside nu (both
recommended options). Status stays proposed until the Limao and Venables figures are checked.

1. **Form.** On every crossing a -> b between countries A and B, for good g of class c:

   ```
   K_ab = (tau_ab * (1 + nu_c) * D_AB,c * (1 + t_AB,g)) ^ -theta_g
   ln D_AB,c = (epsilon / theta_c) * (ln d_AB - ln d_0)
   ```

   epsilon is P158 (the elasticity of trade to the dark distance cost; ID provisional, renumbered
   if taken when the register is updated). d_AB is the population-weighted great-circle distance
   between A's and B's provinces (Head and Mayer 2002, the CEPII distw formula), computed from
   the pack at load and when borders or populations move, so it is an initial condition under
   ADR 0241 and follows emergent borders (ADR 0253), not a 2000 table. CEPII Gravity's distw
   (sources.toml `cepii-gravity`, Etalab 2.0) checks it: a test compares the 2000 values.
   D is an iceberg cost destroyed at the crossing like nu, tallied apart from the border loss
   (`distance_loss`) so the ledger shows both. Legs, freight, hours, lags and tariffs are
   unchanged; D carries no hours.
2. **Normalization.** d_0 is the exp of the 2000 trade-weighted mean of ln d_AB over linked
   pairs, so D redistributes crossing cost from near to far pairs and nu refits by little.
   Factors below 1 are rejected per crossing product, not per term: (1 + nu_c) D_AB,c >= 1 for
   every linked pair. Where that binds for a class, that class takes d_0 at its smallest linked
   d_AB (D >= 1) and nu_c clamps at 0 under ADR 0092's rule, and the miss is recorded.
3. **Fit.** epsilon and nu_c are fitted jointly: for each epsilon, ADR 0092's per-class nu fit to
   the ICIO 2000 shares; epsilon by Brent so the simulated pooled PPML distance coefficient on
   distw equals the data's coefficient in the same specification on the pack's 2000 BACI sample
   (`ceteris gravity`; the value is recorded with the fit, not the -0.7 to -1.1 band). First-order
   expectation: epsilon near 0.35, the data's about -0.8 less the model's -0.463.
4. **Tests.** H2 distance stays a reported diagnostic: the pooled coefficient is now the fit's
   target, so passing it tests nothing. Held out, and gated: the four class coefficients on distw
   within 2 pair-clustered s.e. of the data's class coefficients (the theta-invariance this form
   claims), and ADR 0264's ln distw version of the per-class theta recovers theta_c within its
   band for classes 0 and 1 (the class bias ADR 0264 traced to distance the regression could not
   span). The Eaton and Kortum distance-interval version (six intervals) is reported against the
   log form. The ablation test (DESIGN 15.7) compares epsilon 0 against the fit on these
   criteria; failing, epsilon stays 0 by default.
5. **Phase.** Built and fitted in P4 with the 50-goods recalibration (owner ruling, 2026-10-31).
   P9's joint estimation (ADR 0022; ADR 0269's mode shares, the Allen and Arkolakis 2014
   strategy) includes epsilon; the mode ratios and epsilon are identified by separate moments
   (mode shares by value against the distance gradient), and D does not depend on paths, so
   fitting epsilon in P4 does not force a second P04 refit. This replaces ADR 0264's Decision 3
   ("proposed for P9") and its "leg costs" wording.

## Consequences

- **Interaction with ADR 0269.** The mode ratios stay freight rates, the BTS concept. D is the
  part of distance cost that freight rates do not carry, so Donaldson's (2018) full-cost ratios,
  which load time, risk and information onto per-km cost, must stay out of P04: using them with
  D would count the dark cost twice. Least-cost paths and mode shares do not change with D.
- **Interaction with ADR 0247.** The bulk rate is sourced freight and is unchanged. Class 3's
  delta is epsilon / 16.91, about 0.02 at epsilon 0.35, so D_AB moves class 3 crossings by a few
  percent at most; its nu (0.0078 on the b2-1b scratch pack) has almost no room below it, so
  the class-3 root is the first thing the P4 fit checks. 0247's criterion 3 (distance inside
  -1.1 to -0.7), ill-posed without this term, has its well-posed form in item 4.
- **Calibration.** New: P158 epsilon (register class "calibrated, P4; joint in P9"). Refit: nu
  per class (ADR 0092), the selection constant per province and good (ADR 0200), the steady
  state; recompute the ACR criterion (ADRs 0265, 0268) and openness after. Not refit: P04
  (borrowed, ADR 0269), P154 and P155 (ADR 0247), theta (P01). Cost: the nu fit (about 600 s on
  the B pack) runs inside a Brent search on epsilon, about 8 to 12 evaluations, so 1.5 to 2 hours
  owner-side per pack.
- **Counterfactuals.** Distance-dependent ones (trade diversion by distance, near against far
  partners, ADR 0264) take the data's gradient from P4 on; tariff and border counterfactuals
  shift through the refit nu.
- **Kernel cost.** One more f64 factor per crossing and class, folded into K_ab at load and when
  d_AB moves; the tick is unchanged.
- **Golden hashes** move when it lands (every econ golden with crossings); the commit and the
  CHANGELOG carry `### BEHAVIOUR: log-distance crossing friction (ADR 0272)`.
- **Specs.** With the code: DESIGN 5.4 (nu_od gains the distance term), 8.3, 17.1 (P158), 18
  (H2's held-out class test), the trade SPEC (Two-level kernel, Payments) and ADR 0092's Tests.
- **Literature tier (ADR 0257).** Tier 2: a measured moment met by a fitted parameter whose
  form the literature chooses, with a held-out per-class check.
