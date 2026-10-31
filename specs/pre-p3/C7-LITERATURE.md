# C7 literature groundwork

Prepared 2026-10-31 for pre-P3 Task C7 (PLAN, "Rerun the 28 October benchmark") and the
literature-fidelity pass ADR 0257 puts inside it. Reading only: no run, build or calibration.
Numbers marked *k263* are from the scratch pack `k263/pack` on `pre-p3/econ-review` (B2b,
B2b fix, labour rounds; progress ledger 2026-10-30/31), not from the B8 pack C7 will grade.

Already ruled by the owner, so not reopened here: the ACR criterion uses input-output linkages
(ADR 0265); H2 distance is a reported diagnostic and H2 per class is gated on the model's own
kernel, with a log-distance leg cost scheduled for P9 (ADR 0264); the x10 border shock is
reported, not asserted, and becomes a P5 criterion (ADR 0090, ADR 0207 amendment of
2026-10-31).

## 1. What C7 grades

| Item | Tier (ADR 0257) | Latest number | Literature benchmark | State |
|---|---|---|---|---|
| ACR, input-output form, x1.1 | 1 | median 1.058, corr 0.856, 0 wrong signs (k263, unsettled base) | Costinot and Rodriguez-Clare (2014) eq. (28); Caliendo and Parro (2015) | Correlation misses 0.9: open |
| Openness per class | 2 | 1.109, 1.039, 1.125, 0.998 of ICIO (k263) | ICIO 2000; nu fitted to it (ADR 0092) | Passes, by construction |
| Japan under x1.1 | 1 (sign) | -0.53% (28 Oct: +0.6%) | ACR: every country loses | Passes |
| Brexit soft, hard | 2 (magnitude), 1 (sign) | GBR -0.238%, -0.631%; EU27 -0.043%, -0.107%; IRL -0.193% hard | Dhingra et al. (2017) Table 4 | About 0.45 of the like-for-like source values |
| H2 distance | 2, unbuilt mechanism (ADR 0264) | -0.463 | Head and Mayer (2014): -0.89 all, -1.14 structural | Graded, not fixed |
| H2 per class | 2, test changed (tier 4 record) | within 2.7e-5 of theta (b2c) | Caliendo and Parro (2015) via ADR 0023 | Passes on the new test |
| P04 mode costs | 2 | no converged fit; priors in use | Allen and Arkolakis (2014); Donaldson (2018); BTS | Open |
| x10 border | reported (P5) | stops t61, ledger overflow, index 1.2e3 | none in P2's range | Reported only |
| Tier 3 dynamics | 3 | not yet measured as moments | Section 4 | To grade |
| Tier 4 patches and test changes | 4 | 20 patches, 2 + 3 test changes | ADR 0257 rule | To list |

## 2. Tier 1: the ACR correlation

### What the formula assumes

Costinot and Rodriguez-Clare (2014, NBER WP 18896, section 3.4) derive eq. (28),
C_hat_j = prod_{s,k} lambda_hat_jj,k^(-beta_j,s a_j,sk / eps_k), for a model in which (i) unit
costs are **Cobb-Douglas** in value added and sector price indices (their eq. 25), so cost
shares are fixed; (ii) lambda_jj,k is the domestic share of **gross** absorption (final and
intermediate use under one CES aggregator, their eq. 24); (iii) each country is **one
integrated location**; (iv) trade is balanced. Under perfect competition the adjusted Leontief
inverse is the plain one, (I - A_j)^-1. Taking logs gives the ADR 0265 form. It is exact in
hats under (i) to (iv), first order otherwise. (The equation number is checked against the NBER
WP text; the Handbook print may number it differently, the B2b concern.)

The model departs from each assumption, and each departure makes the country-level formula
inexact by an amount that varies by country, which is what lowers a cross-country correlation
while leaving the median near 1:

1. **Provinces, not countries** (iii). Countries are 4,796 provinces with internal trade costs
   and immobile pops. Ramondo, Rodriguez-Clare and Saborio-Rodriguez (2016) show that with
   domestic frictions the ACR formula holds at the country level only if regions are
   symmetric, and give a generalisation for asymmetric regions; Redding (2016) shows the own
   trade share stops being sufficient once factors move. With immobile labour each province's
   real income follows its own share lambda_pp (the one-location result applied region by
   region), and country welfare is the income-weighted mean of province changes, which differs
   from the formula applied to the country's aggregate lambda. Large, unequal countries (BRA,
   CHN, IND, RUS, USA) should carry the largest residuals.
2. **Leontief intermediates** (i). Cost shares move with prices, so the formula is first order.
   Baqaee and Farhi (2024) show that with complementarities in the network the second-order
   terms of trade-cost shocks are material. A x1.1 shock to every crossing is not small for
   class 0 (theta 2.91).
3. **PIGL demand**. Omega is the data's final-demand share, not the run's expenditure share
   (benchmark.rs module doc), and non-homothetic demand shifts it with income (Fajgelbaum and
   Khandelwal 2016 on unequal gains under non-homothetic demand). First order only.
4. **An unsettled baseline**. k263's base has largest |d ln p| 0.17 to 0.37 over the last 50
   ticks, all Sudan. A few drifting countries can move a Pearson correlation over 80 points
   without moving the median.

### Options

- **A. Grade the simulation against the model's exact counterfactual first** (recommended
  first step). Solve the shocked frictionless equilibrium with `ceteris-reference` (exact hat
  algebra on the model's own structure, Dekle, Eaton and Kortum 2008) and require the run to
  reach it to V1's 1% (DESIGN 15.3). This is the strict reading of tier 1 ("comparative statics
  must match the source model"): it separates "the dynamics do not reach the counterfactual"
  (a model bug) from "the analytic formula does not apply to this geography" (a benchmark
  issue). To check: whether `reference solve` accepts shocked trade costs; if not, that is the
  missing metric C7's Files line allows.
- **B. Province-level ACR aggregated to countries** (Ramondo et al. 2016). Compute the IO
  formula per province from province home shares and province cost shares, then weight by
  income. Needs province lambdas from flows in `benchmark::outcome`; changing what `acr_io`
  measures is a test change, so an ADR amending 0265 (tier 4 rule).
- **C. Diagnose before choosing.** On the B8 pack, regress the country residual (model less
  `acr_io`) on (a) the dispersion of province home shares within the country, (b) the column
  sums of A_c, (c) the base's late |d ln p|. If (a) explains it, B is the literature answer; if
  (c), the miss is the baseline's, not the formula's; if (b), it is second order (point 2) and
  a x1.01 shock should restore the correlation, which is a cheap test of the reading.

Recommended order: C on the B8 pack (it reuses runs C7 makes anyway, plus one x1.01 run), then
A as the tier 1 gate if the reference can take the shock, then B by ADR only if C points to
geography. No band or threshold changes.

## 3. Tier 2: moments

### Openness

Passes on k263 (foreign share over ICIO 1.11, 1.04, 1.13, 1.00 by class; traded 0.224). It
passes by construction: ADR 0092 fits nu per class to the same ICIO cross-border shares, so it
validates the fit, not the model. Two out-of-sample checks belong beside it in the SPEC record:
world goods exports over income against ICIO's 0.14 (already in the PLAN), and the
**cross-country distribution** of the per-country ratio (a world mean can pass while small
open economies and large ones miss in opposite directions). For reference, Head and Mayer
(2014, Table 4) report a median structural "home" (border) coefficient of 1.55 (all gravity
1.93); with their trade elasticity 5.03 that is a tariff equivalent of about 36% for
crossing a border, a sanity range for the fitted nu.

### Japan and Brexit

Japan: under ACR every country loses from costlier borders; -0.53% has the right sign (tier 1
sign check passes).

Brexit, like for like. The shock files take Dhingra et al.'s (2017) NTB increases (2.77% soft,
8.31% hard, plus MFN tariffs hard) but not their exclusion from future EU integration. Their
Table 4 decomposes the UK loss: NTB rise -0.53% (soft) and -1.31% (hard), tariffs -0.13%
(hard), future integration -0.90% and -1.61%. The comparable source values are therefore about
**-0.53% soft and -1.44% hard** for the UK, not the headline -1.3% and -2.7%. The model gives
-0.238% and -0.631%, a ratio of 0.45 in both. Ireland is the EU's largest loser in both
(source: highest UK input share; model -0.193% against the EU27's -0.107%), so the ranking
holds.

Two built-in reasons the model sits lower, both tier 2 "unbuilt mechanism" candidates rather
than bugs: (1) services are local in P2 (goods 7 to 9), and Dhingra et al. apply the NTBs to
goods and services (WIOD, 31 sectors); (2) the model's base is 2000, theirs WIOD 2014, when UK
trade with the EU was larger. A tier 1 cross-check that needs no source model: compute
`acr_io` for the Brexit runs too; the model must match its own formula there as under x1.1.
Recording proposal: report the ratio to Dhingra's NTB-plus-tariff parts, name services trade
as the unbuilt mechanism and its phase (owner to name; it is not on the 1.0 plan as far as I
can find), and do not tune.

### H2 distance (graded, not fixed)

Data: Head and Mayer (2014, Table 4) median distance elasticity -0.89 over 1,835 estimates,
-1.14 over 328 structural ones; PPML runs smaller in magnitude than OLS on the same data
(Santos Silva and Tenreyro 2006, Table 3: -0.784 PPML against -1.166 OLS in logs). The gate
band -1.1 to -0.7 is consistent with both. Model: -0.463.

The mechanism in ADR 0264 has direct literature support. The pooled coefficient is about
theta times the model's cost-distance slope (0.062 to 0.117 by class). Hummels (2007, Table 1)
estimates the elasticity of ad valorem freight costs to distance at 0.151 for ocean and 0.269
to 0.436 for air shipments to the US (air falling from 0.43 in 1974 to 0.16 in 2004). The
model's slope is below the ocean freight figure alone, before any non-freight distance cost
(information, networks: Chaney 2018). Option F (a log-distance leg component, tau = d^rho
style, as in Eaton and Kortum 2002) is the form the literature estimates, and ADR 0022 records
that the unconstrained P04 fit drifts towards it. C7 records: tier 2, mechanism named, phase
P9, pooled -0.46 against about -0.8; counterfactuals that depend on the distance gradient
understate it.

### H2 per class

Passes the ADR 0264 test. The literature values the classes carry: Caliendo and Parro (2015)
sector estimates, harmonically aggregated (ADR 0101), replication vector with Other Transport
at 8.22 (ADR 0023 amendment, a recorded patch). Cross-checks for the record: Head and Mayer's
preferred tariff-based elasticity 5.03 (Table 5 discussion); Simonovska and Waugh (2014) about
4; Boehm, Levchenko and Pandalai-Nayar (2023) about 0.76 one year after a tariff change and
about 2 in the long run, stabilising after 7 to 10 years. The last matters for C7's tier 3
grading and for ADR 0027: the model applies its long-run theta within ticks, so transition
paths respond faster than the data.

### P04 mode costs

Why the fit fails. Country-pair flows (BACI) identify only the least-cost envelope of legs, so
mode cost ratios are weakly identified: geo SPEC records rail/road 52.6 (s.e. 24.1) and sea/road
115.3 on the split pack, 36.6 and 84.6 on four classes, air on 3 pairs. Rail and sea come out
dearer than road, against every price source. The literature identifies mode costs from
**mode-specific** moments: Allen and Arkolakis (2014) estimate relative mode costs from the
2007 Commodity Flow Survey's mode-specific bilateral shares in a discrete-choice step;
Donaldson (2018) from salt price gaps along the network, reporting road, river and coastal
costs relative to rail of roughly 4.5 to 7.9, 3 to 3.8 and 2.25 to 3.9 by specification (check
the AER table before citing a single value).

Price evidence on the priors (P04a to d: road 0.10, rail 0.04, sea 0.008, air 0.80 USD per
tonne-km, ratios road/rail 2.5, road/sea 12.5): US freight revenue per ton-mile around 2000 was
26.2 cents truck (1998), 2.26 rail and 0.73 barge (BTS National Transportation Statistics,
Table 3-17), about 0.163, 0.014 and 0.0045 USD per tonne-km, ratios road/rail about 12 and
road/barge about 36. Revenue per ton-mile mixes haul length and cargo mix, so it overstates the
pure cost gap, but it places the priors' road/rail ratio low, not high.

Options:

- **A. Borrow the ratios with citations, estimate one scale** (recommended for C7). Fix road,
  rail, sea and air ratios from BTS 2000 and Donaldson (2018), registered with ranges for P9,
  and fit only a common scale (or nothing). Converges; honest about identification; tier 2's
  "no moment identifies it" case, recorded.
- **B. Add mode-share moments** (Allen and Arkolakis' strategy): value shares by mode from the
  US Census (air and vessel for US imports) and Eurostat Comext (extra-EU trade by mode),
  jointly with gravity. The literature answer, but a new data source (licence check,
  `sources.toml`) and a new estimator.
- **C. Penalised PPML towards A's values.** With a flat likelihood it returns the prior; adds
  machinery, not information.
- **D. Defer the whole fit to P9** with ADR 0022's joint refit and ADR 0264's option F, since
  F changes the cost form P04 is fitted on.

Recommended: A now, B together with F at P9 (one refit of the transport form, not two).

## 4. Tier 3: dynamics graded by moments

ADR 0257 grades the price rule, the local fill and the money stand-ins by stability, nesting and
data moments. Moments with a literature source that the benchmark runs can report without new
mechanisms:

| Moment | Model measure | Literature |
|---|---|---|
| Trade-flow adjustment after a cost shock | share of the long-run change in flows reached after 12 and 120 ticks of x1.1 | Boehm et al. (2023): about 0.38 of long run after one year (0.76 of 2), stable after 7 to 10 years |
| Tariff pass-through to buyer prices | d ln q / d ln (1 + t) on the hard-Brexit tariff lines | Amiti, Redding and Weinstein (2019); Fajgelbaum et al. (2020): near-complete pass-through of the 2018 US tariffs |
| Real price gaps between countries | half-life of a relative price shock between two countries | Rogoff (1996): consensus PPP half-life 3 to 5 years (nominal exchange rates, so only indicative before P5a) |
| Slowest mode | 0.9923 per tick (audit): half-life about 90 ticks | compare with the two rows above |

Stability and nesting are already measured (V1, the 100-year runs, ADRs 0263 and 0267 caps).
The price rule's design gains (kappa1 0.2, kappa2 0.1) against the built 0.05 and 0 are a tier 4
patch record, not a tier 3 fidelity question.

## 5. Tier 4: patches and test changes

ADR 0257 accepts a patch only if it (a) leaves the equilibrium unchanged, (b) is registered,
(c) names its removal phase. The audit counts 20 patches and lists ten that change economic
content (alterations 4, 6, 13, 15, 21, 31, 34, 38, 39, 41). Proposed grading method: one row per
patch with a, b, c as yes or no and the evidence (a V1 or reference comparison for a); a "no"
on (a) is a finding for the owner. Since the audit, B added: labour cap on traded inputs and the
Benassy split (ADR 0266), the per-coupled-block price cap (ADR 0267), the write-off and fill
fixes, and the scale-aware dispatch tolerance; each needs a row.

Test changes C7 must record (what each no longer checks):

1. V1 from the perturbed steady state, data-state run as a stress test (2026-10-26): V1 no
   longer checks convergence from the data state.
2. ADR 0247 criterion 3 ruled ill-posed: the bulk rate is not required to leave H2 distance in
   band.
3. ADR 0264, H2 distance: no longer bounds the level of the cost-distance slope.
4. ADR 0264, H2 per class: no longer checks that ln distance spans the model's costs; its band
   is V1's 1%, not a sampling band.
5. ADR 0090 amendment, x10: convergence under large border shocks is no longer a pre-P3
   criterion.

## 6. Decisions for the owner

1. ACR correlation: diagnose first (option C) and grade the run against the reference's exact
   counterfactual (A); province-level ACR (B) only by ADR if C points to geography.
   Recommended.
2. Brexit: compare with Dhingra et al.'s NTB and tariff parts (-0.53%, -1.44%), record services
   trade as the unbuilt mechanism, and name its phase.
3. P04: literature ratios now (A), mode-share estimation with option F at P9 (B, D).
4. Tier 3: add the four moments of section 4 to the benchmark report (report only).

## References

Verified against the source text in this session unless marked.

- Allen, T. and C. Arkolakis (2014). Trade and the topography of the spatial economy. QJE
  129(3), 1085-1140. (Method verified from the abstract and lecture notes; mode cost values not
  read.)
- Amiti, M., S. Redding and D. Weinstein (2019). The impact of the 2018 tariffs on prices and
  welfare. JEP 33(4), 187-210. (Not re-read.)
- Arkolakis, C., A. Costinot and A. Rodriguez-Clare (2012). New trade models, same old gains?
  AER 102(1), 94-130.
- Baqaee, D. and E. Farhi (2024). Networks, barriers, and trade. Econometrica 92(2), 505-541.
  (Not re-read.)
- Boehm, C., A. Levchenko and N. Pandalai-Nayar (2023). The long and short (run) of trade
  elasticities. AER 113(4), 861-905. (Values from the abstract.)
- Bureau of Transportation Statistics. National Transportation Statistics, Table 3-17, Average
  freight revenue per ton-mile (2000 and 2005 editions).
- Caliendo, L. and F. Parro (2015). Estimates of the trade and welfare effects of NAFTA. ReStud
  82(1), 1-44.
- Chaney, T. (2018). The gravity equation in international trade: an explanation. JPE 126(1),
  150-177. (Not re-read.)
- Costinot, A. and A. Rodriguez-Clare (2014). Trade theory with numbers: quantifying the
  consequences of globalization. Handbook of International Economics vol. 4, ch. 4; NBER WP
  18896, section 3.4, eqs. (24) to (28) read.
- Dekle, R., J. Eaton and S. Kortum (2008). Global rebalancing with gravity. IMF Staff Papers
  55(3), 511-540.
- Dhingra, S., H. Huang, G. Ottaviano, J. P. Pessoa, T. Sampson and J. Van Reenen (2017). The
  costs and benefits of leaving the EU: trade effects. Economic Policy 32(92), 651-705; CEP DP
  1478, Tables 3 and 4 read.
- Donaldson, D. (2018). Railroads of the Raj. AER 108(4-5), 899-934. (Mode ratios from two
  presentations; published table not read.)
- Eaton, J. and S. Kortum (2002). Technology, geography, and trade. Econometrica 70(5),
  1741-1779.
- Fajgelbaum, P., P. Goldberg, P. Kennedy and A. Khandelwal (2020). The return to
  protectionism. QJE 135(1), 1-55. (Not re-read.)
- Fajgelbaum, P. and A. Khandelwal (2016). Measuring the unequal gains from trade. QJE 131(3),
  1113-1180. (Not re-read.)
- Head, K. and T. Mayer (2014). Gravity equations: workhorse, toolkit, and cookbook. Handbook of
  International Economics vol. 4, ch. 3; CEPII WP 2013-27, Table 4 and the trade elasticity
  discussion read.
- Hummels, D. (2007). Transportation costs and international trade in the second era of
  globalization. JEP 21(3), 131-154; working version, Table 1 read.
- Ramondo, N., A. Rodriguez-Clare and M. Saborio-Rodriguez (2016). Trade, domestic frictions,
  and scale effects. AER 106(10), 3159-3184. (Result from the abstract and NBER WP 18532
  summary; generalised formula not read.)
- Redding, S. (2016). Goods trade, factor mobility and welfare. JIE 101, 148-167. (Abstract.)
- Rogoff, K. (1996). The purchasing power parity puzzle. JEL 34(2), 647-668. (Not re-read.)
- Santos Silva, J. M. C. and S. Tenreyro (2006). The log of gravity. REStat 88(4), 641-658;
  Table 3 read.
- Simonovska, I. and M. Waugh (2014). The elasticity of trade: estimates and evidence. JIE
  92(1), 34-50. (Not re-read.)
