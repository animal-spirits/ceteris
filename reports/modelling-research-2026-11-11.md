# Modelling research: lessons, new and combined models, 11 November 2026

Research only. No code, spec, register or queue rows change here; every idea that departs from
the design or an accepted ADR needs its own RFC (development laws). Decision ids and P codes are
deliberately not assigned.

Sources read for this note: `reports/` in the project files (econ-numbers-vs-literature,
approximate-solves, second-pass-brainstorm, roadblock-recon, architecture-overview), the
C7 literature groundwork, `research/` (held-out validation sources, PWT/WDI price levels, open
wealth shares, Eurostat gaps, ocean freight), the P5 E-branch studies and spikes (`p5-e3/`,
`docs/research/p5-*-2026-11-10.md`), the storage-on misses and Sudan diagnoses, the 0263 draft,
and RFCs 0028, 0032, 0090, 0091, 0100, 0109, 0112, 0174, 0204, 0231, 0243, 0263, 0278, 0280,
0308, 0335, 0337, 0338, 0341, 0342 with `specs/money/SPEC.md` and `specs/pop/SPEC.md`.

Literature is cited by author and year from memory; nothing was re-read in this thread.
Numbers attributed to papers carry **(unverified)** where I am not confident of the exact value.
Effort: S under a day of spike, M a few days, L a week or more. Fit: **1.0** (inside the P4/P5
scope, behind a switch), **1.x**, **2.0+**.

---

## 1. What we learned

### 1.1 Where the modules disagreed, and how it was reconciled

| Collision | What happened | Reconciliation (status) |
|---|---|---|
| Trade calibration vs free trade moments | Fitting border nu to ICIO openness (ADR 0092) moved the free moments away: H2 distance elasticity -0.753, -0.536, -0.509, -0.463, -0.454 against about -1.1 in the data; Brexit welfare 4-5x too small after ADR 0200; ACR-IO correlation 0.862 vs the 0.9 bar | ADR 0272 log-distance crossing friction in progress (P4 Branch G); H2 demoted to a report (0264); C7 diagnosis options (geography vs second order vs baseline drift) |
| Buyer price vs welfare formula | Harmonic-mean buyer price q = 1/alpha fell when borders got costlier: Japan gained, the EU gained from Brexit | Buyers pay the EK index P_d times a calibrated selection constant (ADR 0200) |
| Price rule vs supply elasticity | Loop gain kappa1 J with J = E + eta eps / (2 - eta); frictionless eta 1 puts the full eps (up to 25) in the loop; Albania, São Tomé cycles | Per-market gain cap at m of the stability limit (0263), per coupled block (0267); coupled cross-good modes still unbounded |
| Price rule vs storage | Gap divided by Q_floor at Q = 0, every step at the 0.3 clip, theta 16.9 moves orders by e^5; Switzerland and France cycles; each patch round moved the failure (stored goods, then local goods, then storage-off starts) | Deferred (RFC 0286); bounded midpoint gap at zero output (ADR 0340); the storage study's conclusion stands: the Q = 0 step needs a redesign for every good |
| Price level vs real shortage | ADR 0090's world aggregate term (kappa_n / h) lbar turned a local real shortage into world inflation | Money anchor plus good-level term (ADR 0338) |
| Money anchor vs non-trading cells | A local cell with D > 0, Q = 0 at 316x base lifted the fixed-weight index from 1.04 to 1.37, and the anchor deflated every other price to 3% of base | Chained index over cells that trade (ADR 0343) |
| Accounts vs losses | Every account allowed to go negative absorbed the shock without bound: pops under the signed dividend (thousands of months of income, ADR 0112), enterprises (5e12), province markets (index 15 at 1,200 ticks), the island at x10 (-1.2e10 by t300, tripling positive balances elsewhere) | Write-off cap M = 3 (0112); pay on sales plus credit line of one loss cap (ADR 0344); E3 package off by default (0345) |
| Pack data vs technology | Sudan's oil cells took value added from rents but the input column from a donor without crude: gross output 14x VA, huge J, drift through local good 9 | World oil and gas technology from rents (0256) and the extraction column (0270) |
| Final demand vs trade | With r_g removed, world spending on a good no longer equals sales; balancing column factors 0.48 to 3.24 | Exogenous current-account transfers D_c (0100) and RAS on traded final demand (0308) |
| Price-level sources | PWT and WDI disagree off-benchmark (12.7% interpolated, 50.7% extrapolated median gaps) | Check restricted to the 31 shared-benchmark economies |
| Wealth distribution | No open 2000 source outside the US | Lognormal proxy from SWIID Gini, one constant fitted to US DFA (0308/0311) |
| Long-run vs short-run elasticity | Long-run theta applied monthly; rho fit failed placebo tests on the trade war | HS6 three-way FE LP plus LP-DiD (research); indirect inference on BLP-N IRF shape (paid build) |
| Residual still open | Under the E3 package world D/Q sits at 1.04-1.12: uniform excess demand the anchored rule does not price (RFC 0342, "record, not fix") | None yet |

### 1.2 Recurring failure patterns

1. **An account that may go negative becomes the system's shock absorber.** Pops (0112),
   enterprises (0112, E3 D2 spike), province markets (0112, 0341), a whole country's external
   position (E3). Each time the fix bounded one account and the loss moved to the next one that
   could still go negative. The general lesson is a closure problem, not a gain problem: the model
   lacked a rule that says who lends, at what limit, and what price moves when the limit binds.
2. **Zero-output singularities.** Frictionless eta 1 takes Q to 0 in one tick (the "flip"); gaps
   divided by a floor; Leontief zeroes a cell when one input runs out; the clip then binds every
   tick. Stock-blind price rules ratchet against unsold stock on idle ticks. Almost every
   frictionless failure on record passes through a cell with Q = 0.
3. **Prices read the wrong signal.** Mean-zero relative and good terms cannot price a world-level
   excess demand; a fixed-weight index reads prices nobody pays; a harmonic buyer price moved the
   wrong way; kappa2 = 0 ignores stocks. Each was a mismatch between the quantity a term reads and
   the disequilibrium it is meant to close.
4. **Stability checks valid only at the linearisation point.** J2 and the storage loop map are
   linear at z = 1 and one-market; the failures came from the nonlinear region (stockout yield,
   0280) and from coupled modes across goods (0267's open item).
5. **Fitting one moment pushes the free ones away.** Openness fitted, distance and Brexit
   drifted. About half of the "agreements" in the econ-numbers report are by construction.
6. **Patches relocate failures.** The storage fix rounds, the E3 spikes (level term, budget
   orders, stock term, pay on sales alone, credit limit alone) each moved the failure. The E3
   package passed only when three parts went in together.
7. **No external closure.** With one numeraire and interest-free clearing there is no exchange
   rate, no balance-of-payments price, and the current account is either exogenous (D_c) or
   financed by whoever can overdraft. Antonio's record (RFC 0341 follow-up) already says the model
   needs both a credit limit and a balance-of-payments price.
8. **Frequency mismatch.** Long-run elasticities (theta, migration logit, supply eps) applied at a
   monthly tick make the short run too elastic, which feeds pattern 2.

---

## 2. Catalogue of new model ideas

Each entry: sketch, state, parameters, data, failure addressed, interactions, test, determinism
and cost, effort, fit. Notation follows the trade and econ SPECs (l gap, I index, M money, D
demand, Q output, S stock, eta employment speed).

### A. Price formation and the money anchor

#### A1. Output-adjusted money anchor (P-star)

Sketch. The ADR 0338 anchor targets ln(I/I0) = ln(M/M0). The quantity equation says the price
level that clears at given money also depends on real output: P* = M V* / Y* (Hallman, Porter and
Small 1991). Replace the nominal term with

```
anchor = -kappa_m ( ln(I/I0) - ln(M/M0) + ln(Y_r/Y_r0) )
Y_r : chained real output over the same traded cells as ADR 0343,
      ln Y_r,t = ln Y_r,t-1 + sum_c s_c ln(Q_c,t / Q_c,t-1)
```

When real output falls with money fixed, the target level rises, so a uniform excess demand is
priced as a level move, bounded by the output ratio (unlike the rejected kappa_l lbar term, whose
target has no level).

- State: one chain scalar (snapshot), like 0343's index chain.
- Parameters: none new (kappa_m reused); optional velocity drift v* = 0.
- Data: none; nests ADR 0338 when Y_r = Y_r0.
- Addresses: the open residual world D/Q 1.04-1.12 under E3 (pattern 3).
- Interactions: the money SPEC's policy rule later sets M; P4 balanced growth (0243) needs
  Y_r0 on the trend path (detrended Y_r), or growth reads as deflationary pressure.
- Test: fixture x10 and ntb 3 under the E3 package; pass if D/Q falls below 1.02 with the index
  inside 1.10 of the 0338 path; ntb 1 exact. Compare head to head with the rejected level term to
  show it is not the same thing.
- Determinism/cost: one fixed-order sum over traded cells per tick, already done for the index.
- Effort S. Fit 1.0 (P5a, amends 0338/0343, so RFC).

#### A2. Auction goods and customer goods (two clearing regimes)

Sketch. Okun (1981) splits markets into auction markets (commodities, flexible prices that clear
fast) and customer markets (differentiated goods, sticky prices, quantities adjust). The storage
misses all sit on storable commodity cells, which are auction goods in the data. Clear the
storable goods (agriculture, crude, mining, metals) by a small within-tick solve per world good,
and keep the stock-flow rule for the rest:

```
for each storable good g (fixed K Newton steps, fixed order):
  solve sum_o [ D_o(p) + b_o(p) - Q_o - release_o(p) ] = 0  for one world level x_g,
  p_og = x_g * base_og * exp(rel_og)    (rel_og keeps the gravity-relative structure)
customer goods: anchored stock-flow rule as built
```

- State: none new (x_g derived); merchant stocks as now.
- Parameters: K (iterations, fixed), tolerance reported not enforced.
- Data: storable list exists (ADR 0091); the split is the design's own.
- Addresses: the storage-on frictionless cycles (RFC 0286, deferred) by removing the clip-bound
  step from the goods where it bites; pattern 2.
- Interactions: ADR 0016 clears across ticks; this is a deliberate exception for a few goods, so
  the ADR 0016 claim narrows. The oil block (0028) becomes a natural client: OPEC's reaction rule
  sets supply, the auction clears price.
- Test: the sixteen storage-on V1 starts plus the fixture spiral and sweep tests (the storage
  study's own bar).
- Determinism/cost: fixed-iteration per good, no cross-thread reduction beyond a per-good sum in
  fixed order; about K extra demand evaluations for 4 goods.
- Effort M-L. Fit 1.0 if RFC 0286 is still open at P5a end; otherwise 1.x.

#### A3. Price-change frequency as the discipline for kappa1

Sketch. Interpret kappa1 as a Calvo hazard: a share lambda of a cell's price contracts reset each
month, so the aggregate partial adjustment is dlnp = lambda (lnp* - lnp). Nakamura and Steinsson
(2008) give monthly frequencies by category (about 20% for goods excluding sales, much lower for
services; **unverified exact values**); Pink Sheet commodity prices move every month. Map the
category frequency to kappa1 per good instead of one gain set by stability limits.

- State/params: kappa1 per good from published frequencies (licence-clean: numbers are facts,
  per Antonio's 2026-11-08 ruling).
- Addresses: kappa1 is untested against any micro-price moment (econ-numbers report section 3).
- Interactions: J2 caps (0263) still bind where frequency data imply a gain above the limit;
  services would get much lower gains, which slows the local chain.
- Test: report the implied frequency per good; check the stability map at the new gains.
- Effort S. Fit 1.x (moves goldens everywhere).

### B. Money, credit and the external closure

#### B1. Balance-of-payments price closure (country price level)

Sketch. With one numeraire the exchange rate is implicit: a country's numeraire price level. Two
closures, which Antonio's record asks for together:

- Quantity (built as E3): accounts cannot overdraft beyond the credit line, so a deficit country
  runs out of money and its imports are rationed.
- Price (new): a country-level term in every price and wage rule of country c moves its price
  level against its external balance, which is Hume's (1752) price-specie flow made explicit and
  the transfer-problem adjustment (Keynes and Ohlin, 1929):

```
B_c    = net money inflow to country c this tick (exports + transfers in - imports - transfers out)
B*_c   = target external balance (D_c path, or the endogenous rule of F1)
x_c    = (B_c - B*_c) / Y_c
dln p_oc += kappa_e (x_c - xbar),   dln w_c += kappa_e (x_c - xbar)
xbar   = world value-weighted mean of x_c   (so the term is mean-zero and cannot fight the anchor)
```

- State: none (x_c from the journal); optionally a smoothed x_c (one scalar per country).
- Parameters: kappa_e (one new gain).
- Data: CA reversal speeds discipline kappa_e (Freund 2005 on current-account reversals; Milesi-
  Ferretti and Razin 1998; **adjustment horizons unverified**). Marshall-Lerner must hold: with
  theta near 4 the long-run condition holds easily, but with Leontief inputs the short-run import
  elasticity is near zero, so a J-curve is expected and kappa_e must sit inside a stability region
  derived like J2.
- Addresses: pattern 1 and 7, the island at x10 and the ntb 3 residual (an unbounded external
  deficit); gives the expenditure switching that credit limits alone cannot.
- Interactions: the money SPEC's FX clearing (9.9, Gabaix and Maggiori 2015) replaces this term
  for floating-currency countries once currencies exist; hard pegs (9.2) keep the quantity closure
  only, which is the right historical behaviour (deflationary adjustment under a peg). The anchor
  index (0343) sees only the mean-zero part.
- Test: two-island toy (T1 below), then fixture x10 and ntb 3 with the E3 package; pass if the
  island's external balance returns to B* with a monotone or once-overshooting path and output
  stays above 0.95.
- Determinism/cost: per-country sums in fixed order, negligible.
- Effort M. Fit 1.0 (P5a/P5b, RFC; the record already names it).

#### B2. Every overdraft is a loan (endogenous money accounting)

Sketch. Make the interest-free clearing facility explicit: any negative balance is a loan from the
country's bank sector, at a rate and against a limit. Credit creates deposits (McLeay, Radia and
Thomas 2014), and stock-flow consistency (Godley and Lavoie 2007) is the invariant.

```
loan_a = max(0, -balance_a);  interest_a = (i_c + spread_a) loan_a / 12
limit_a = f(collateral or cash flow)  -> B3
negative balance beyond limit_a -> rationing of the account's orders (E3 rule)
```

- State: loan stock per account (already the negative balance), the rate per country (money SPEC
  policy rule).
- Addresses: pattern 1 structurally: losses cost something and have a named creditor.
- Interactions: Branch F banks absorb this; the write-off becomes a bank loss, which links real
  losses to bank capital (9.11). Debt deflation: the E3 mainland deflation to 3% of base would,
  with nominal debts, multiply real debt 30-fold (Fisher 1933). Price-level stability (A1, 0343)
  is therefore a precondition for turning credit on, worth a gate assertion.
- Test: SFC matrices (Branch A1) close to zero every tick; fixture data state stays stationary
  with i = 0 (nesting).
- Effort M. Fit 1.0 (it is Branch A/F's contract stated as one rule).

#### B3. Earnings-based credit lines

Sketch. ADR 0344's credit line is the loss cap T_e = M S_e on base sales. Lian and Ma (2021) find
most US non-financial corporate borrowing is constrained by earnings, not asset values; Kiyotaki
and Moore (1997) and Bernanke, Gertler and Gilchrist (1999) give the collateral and net-worth
versions.

```
CL_e = max( T_e_floor,  lambda_E * EBITDA_e^12m  )      (earnings-based)
or     lambda_K * q_K * K_e                              (collateral, after ADR 0204 capital)
```

- State: a 12-month EBITDA moving sum per enterprise (one f64 or i128).
- Parameters: lambda_E (debt to earnings multiple; country leverage from financial accounts as
  ADR 0231 already plans).
- Addresses: the CL 0 deadlock and the arbitrariness of "one loss cap"; adds a financial
  accelerator (procyclical credit).
- Interactions: ADR 0231 funding order; 9.10 credit spreads; B4.
- Test: financial accelerator IRF: a demand shock's output response grows with lambda_E;
  data-state stationarity unchanged.
- Effort M. Fit 1.0 inside Branch D/F, else 1.x.

#### B4. Trade credit and payment terms

Sketch. The credit-limit spike deadlocked at CL 0 "since inputs must be paid before the sales that
fund them". Real firms do not do that: suppliers extend trade credit, typically 30-60 days
(Klapper, Laeven and Rajan 2012, **range unverified**), and trade finance matters for exporters
(Antràs and Foley 2015; Amiti and Weinstein 2011). Mirror the goods pipeline with a payments
pipeline:

```
order at tick t, value V:   payable_e[t + k_g] += V,  receivable_m[t + k_g] += V
at tick t settle slot t:    balance_e -= payable_e[t];  balance_m += receivable_m[t]
overdue (balance below credit line at settlement) -> E3 rationing applies to new orders
k_g : payment term in ticks (0, 1 or 2), per good class
```

- State: a 3-slot payable ring per enterprise and receivable ring per market (bounded, like the
  4-slot goods pipeline).
- Parameters: k_g (data: published payment-term surveys; two values suffice).
- Addresses: the CL 0 deadlock, and makes the credit line a statement about overdue debt rather
  than about input purchases; chains of defaults become possible (Kiyotaki and Moore 1997, credit
  chains), which is the realistic contagion channel.
- Interactions: sector balance sheet gets a trade-credit instrument (money SPEC already has
  instruments by sector); write-off rules apply to overdue payables.
- Test: CL 0 with k = 1 must not deadlock from the data state; with k = 0 the run equals the
  E3 package exactly (nesting).
- Determinism/cost: integer postings, fixed order; one more ring per account.
- Effort M. Fit 1.0 (P5a/P5b, RFC).

#### B5. Sudden stops: financing-bounded current accounts

Sketch. A country's deficit is financed only while foreign investors lend; when they stop, the
current account reverses (Calvo 1998; Mendoza 2010). Combine B1 with the money SPEC's sovereign
spread and risk-appetite state:

```
F_c = foreign financing available = phi(spread_c, reserves_c, risk appetite) * Y_c
if  -B*_c > F_c:  B*_c <- -F_c      (target deficit capped by financing)
```

- Addresses: makes the target in B1 state-dependent; reproduces reversals and the output cost of
  sudden stops.
- Interactions: Branch G sovereign spread, 9.12 risk appetite, reserves under pegs.
- Test: Calvo-style episode: a risk-appetite draw that halves F_c for emerging economies should
  produce a CA reversal and a real depreciation (B1) within a year.
- Effort M. Fit 1.0 if Branch G lands with B1; else 1.x.

### C. Production and the zero-output trap

#### C1. Partial Leontief: critical and non-critical inputs

Sketch. Pichler, Pangallo, del Rio-Chanona, Lafond and Farmer (2022) propagate shocks through a
dynamic IO model where only critical inputs bind; non-critical inputs can be short without
cutting output. The project already uses their rationing. Extend to their production function:

```
Q_e <= min( capacity_e,  min_{j critical} S_ej / a_ej )
non-critical j: shortfall raises unit cost instead (or is ignored for output), purchases continue
critical set: inputs with cost share above s_crit, plus energy and the self-input
```

- State: a per-(sector, input) critical flag (pack table, small).
- Parameters: s_crit (or a published criticality classification; theirs is IHS Markit, which is
  proprietary, so a cost-share rule is the licence-clean choice).
- Data: ICIO cost shares; Pichler et al. report forecast skill for 2020 UK output.
- Addresses: the Leontief cascade (country 1 good 2, then p8 running out, then contagion to the
  mainland at t480-500), pattern 2.
- Interactions: the equilibrium is unchanged when nothing is short (nesting holds); the J
  calculation is unaffected at the steady state; stability near stockouts improves.
- Test: the x10 diagnostic harness: contagion should stop at the first non-critical link;
  the 2020-style supply shock test if one is added in P7.
- Determinism/cost: one min over fewer inputs; cheaper.
- Effort M. Fit 1.0 behind a switch (RFC, changes the econ SPEC production rule).

#### C2. Shutdown hysteresis band (smooth the eta 1 flip)

Sketch. Dixit (1989): with sunk restart costs a firm keeps running between an exit and an entry
price. The frictionless eta 1 mode jumps from Q 0 to 0.8 in one tick when value added changes
sign. Add a band only at the boundary:

```
mode_e in {running, idle}
running -> idle  if VA_e < -h S_e for T_h ticks
idle -> running  if VA_e > +h S_e ; restart ramps L at eta_r per tick
interior (running, VA > 0): as built
```

- State: mode and a counter per enterprise.
- Parameters: h (band width as a share of base sales), T_h, eta_r.
- Data: weak; capacity utilisation persistence and plant restart costs (Ramey 1991 on
  non-convexities in production, **unverified**). Keep h small and report.
- Addresses: the island flip at x10 (Y/Y0 0 to 0.8 per tick), market losses from one-tick
  production nobody buys, pattern 2.
- Interactions: the frictionless reference: equilibrium unchanged where VA > 0 everywhere, so V1
  nests; the island case sits outside V1's J2 region anyway.
- Test: island trace harness: no tick-to-tick flip; ntb 1 exact.
- Effort S. Fit 1.0 (P5a, RFC).

#### C3. Heterogeneous producers inside a cell (Pareto supply curve)

Sketch. One enterprise per (province, sector) makes the extensive margin binary. With a Pareto
productivity distribution inside the cell (shape k, Melitz 2003; Chaney 2008), the active share
and output become smooth in price:

```
cutoff  phi* = c / p
Q(p)  = Qbar * (phi_min p / c)^(k - beta)   for p < c / phi_min,  Qbar above
supply elasticity eps = k - beta (data: the firm size tail, Axtell 2001, exponent about 1.06,
      which pins k / beta rather than k - beta; the mapping is still to derive)
```

- State: none per firm; k per sector.
- Addresses: the binary flip and the implausible eps up to 25 in frictionless mode; J becomes a
  data-disciplined number instead of a by-product of labour shares.
- Interactions: changes the equilibrium model, so `ceteris-reference`, calibration stage 3, J2
  and the ACR checks all move; ADR 0026 (firm heterogeneity, proposed) is the natural home. Also
  gives the extensive margin that explains distance elasticities in Chaney (2008), so it touches
  the H2 miss.
- Test: V1 against a reference with the same supply curve; H2 distance moves toward -1.
- Effort L. Fit 2.0+ (or 1.x after ADR 0026).

#### C4. Order rule with partial catch-up and supply-line weight

Sketch. The intermediate order rule is a full-catch-up stock target. Sterman (1989) and Lee,
Padmanabhan and Whang (1997) show such rules amplify demand swings up a chain (bullwhip),
worst for self-using inputs. Anchoring and adjustment:

```
O_e,j = a_ej Qplan_e + (S*_ej - S_ej) / T_S + beta_SL (SL*_ej - SL_ej) / T_S
T_S >= 2 ticks, beta_SL in [0, 1]  (beta_SL = 1 and T_S = 1 is today's rule)
```

- Parameters: T_S, beta_SL; inventory-to-sales ratios by sector (Census M3) as targets.
- Addresses: the bullwhip channel first suspected at x10. Honest note: the corrected E3 cause was
  the money sink and intermediate orders were flat at 0.24 pQ, so this is a secondary fix; it
  matters most for the 45-goods pack, where more goods use themselves.
- Test: beer-game toy (T3); bullwhip ratio var(O)/var(D) per stage.
- Effort S. Fit 1.0 (small) or 1.x.

### D. Trade dynamics and frictions

#### D1. Two-speed sourcing (fast prices, slow relationships)

Sketch. BLP-N (Boehm, Levchenko and Pandalai-Nayar 2023) estimate about 0.76 at one year and
about 2.1 at ten. The planned rho partial adjustment (ADR 0027) uses one speed. A two-component
kernel separates a fast price response from slow relationship capital (customer capital, Gourio
and Rudanko 2014; Alessandria, Choi and Ruhl 2021 on sunk export costs):

```
X_od = E_d K_od R_od p_o^-theta / Phi_d
ln R_od,t = (1 - rho) ln R_od,t-1 + rho ( -(theta_LR - theta_SR) ln (p_o / P_d) + const )
short-run elasticity theta_SR (on p), long-run theta_LR via R
```

- State: R per active pair per good: 623k nonzeros per good, too large for 45 goods per pair;
  store R at the gateway level (country pair per good) instead.
- Addresses: pattern 8; too elastic monthly trade.
- Test: indirect inference on the BLP-N IRF ratio (roadblock recon 2f) fits rho; UK Global Tariff
  2021 as the out-of-sample check.
- Effort M-L. Fit 1.0 only as the planned single-speed rho; two-speed 1.x.

#### D2. Endogenous freight rates (shipping capacity)

Sketch. Brancaccio, Kalouptsidi and Papageorgiou (2020) make trade costs endogenous to ship
supply and trade imbalances (empty backhauls). A capacity-utilisation freight rate per sea lane
group:

```
u_r = tonne-km demanded on route group r / fleet capacity K_r
c_sea,r = c0_r * (u_r / u0_r)^xi ;  K_r grows by investment with a 2-3 year build lag (0204)
backhaul: c on the lighter direction scaled by imbalance share
```

- Data: 2000 rates from the ocean-freight study; Drewry/Clarksons series are proprietary
  (validation only); xi from the 2021 container spike episode (**not quantified here**).
- Addresses: realism of freight shocks (Suez, 2021), part of the openness and distance puzzle via
  imbalance-dependent costs.
- Interactions: dispatch cost per unit changes every tick, which breaks the "kernel fixed between
  network edits" assumption and the skip-unchanged-goods idea; cost is a kernel rebuild per
  change.
- Effort L. Fit 2.0+.

#### D3. Time and information frictions on distance (a check on ADR 0272)

Sketch. The SPEC finds each class's iceberg distance coefficient near -0.09 theta_c, so with theta
4 the implied tau-distance elasticity is about 0.09 against about 0.2 needed. Time costs are a
literature-grounded way to steepen it: Hummels and Schaur (2013) value a day in transit at
0.6-2.1% ad valorem (**range unverified**); information frictions (Allen 2014) scale with
distance too. ADR 0272's log-distance crossing friction is the reduced form in progress; this
entry is the structural reading to check it against: if the fitted log-distance coefficient
implies a time value outside the Hummels-Schaur range, the friction is doing work distance should
not do.

- Effort S (a report on 0272's fitted value). Fit 1.0 as a validation note.

### E. Energy, rents and trade

#### E1. Oil rents through a fund and Dutch disease

Sketch. In 1.0 oil profits go to pops by population (0278). Rent economies in the data save a
large share abroad (sovereign funds) and spend the rest at home, raising non-traded prices and
squeezing other traded sectors (Corden and Neary 1982). A fiscal rule plus B1:

```
R_c   = rent income of extraction cells (ADR 0281)
F_c,t+1 = (1 + r_f) F_c,t + alpha_c R_c - r* F_c       (foreign assets, a world account claim)
G_c  += r* F_c + (1 - alpha_c) R_c                      (spent on the country's local goods)
B*_c  += alpha_c R_c                                    (B1 target: rents saved abroad are a surplus)
```

- State: F per rent country (one i128).
- Parameters: alpha_c and r* (Norway's 3-4% rule is a published rule, **check current value**).
- Data: WDI rents (already read, 0249); SWF assets 2000 (licence check needed; SWF Institute is
  not open).
- Addresses: Sudan-like rent cells feeding local loops through pops' spending; makes Gulf current
  account surpluses endogenous when the oil price moves.
- Interactions: B1 delivers the real appreciation; 0028's OPEC rule sets the rent; money SPEC
  government account and public capital (0042).
- Test: toy T4; on the pack, raise the crude price 50% and check non-oil traded output share
  falls in rent economies and their external surplus rises.
- Effort M. Fit 1.x (needs the government owning rents, which 0278 moved past 1.0).

#### E2. Bounded convenience yield in storage

Sketch. The storage diagnosis found the stockout yield unbounded (0280 context). The theory of
storage (Working 1949; Brennan 1958; Pindyck 2001; Deaton and Laroque 1992) gives a convenience
yield that falls with stocks and is finite at stockout. A saturating form:

```
y(z) = y_max / (1 + (z / z0)^zeta),  z = S / (s D)
merchant order uses y(z) instead of c z^-zeta; y(0) = y_max finite
```

- Parameters: y_max (data: backwardation peaks in futures curves; Pink Sheet spreads are not
  enough, **needs a source**).
- Addresses: the nonlinear storage region (pattern 4).
- Note: check against 0280 as built before an RFC; 0280 may already bound this.
- Effort S. Fit 1.0 with RFC 0286, if still open.

### F. Demography, labour and migration

#### F1. Demographic current account (endogenous D_c)

Sketch. D_c is exogenous (0100, 0308). Runs after t0 are calendar-blind (ADR 0241), so an
observed path cannot extend into an open-ended run; a rule is needed. Saving and current accounts
depend on age structure (Modigliani life cycle; Higgins 1998; the IMF External Balance Assessment,
Phillips et al. 2013):

```
B*_c / Y_c = b_old (old_c - old_w) + b_young (young_c - young_w) + b_aging (Δold_c^exp - Δold_w)
           + b_nfa NFA_c / Y_c + b_y ln(y_c / y_w) + b_oil R_c / Y_c      (country minus world means)
sum_c B*_c = 0 by construction (deviations from world means, then exact integer rebalance)
```

- State: none new; pop SPEC cohorts give dependency ratios; NFA from the world account.
- Parameters: published EBA-style coefficients (licence-clean as published numbers; **exact
  values unverified**).
- Data: 2000 CA from WDI (already read); EBA coefficients.
- Addresses: pattern 7: the target external balance for B1; replaces the exogenous path for open
  runs.
- Interactions: P4 demography (Branch A) feeds it; E1 adds the rent term; B5 caps it.
- Test: sign of CA vs old-age dependency across the pack after 20 simulated years; Japan and
  Germany surpluses, US deficit at t0 within the WDI values (fitted, so a check of the
  implementation, not validation).
- Effort S-M. Fit 1.0 (P5, RFC amending 0100/0308).

#### F2. Harris-Todaro expected wages and remittances in the migration logit

Sketch. The pop SPEC's logit uses beta V_d. Harris and Todaro (1970) make the relevant value the
expected wage, (1 - u_d) w_d, which explains urban migration despite urban unemployment.
Remittances close the loop to the external balance:

```
V_d = ln( (1 - u_d) w_d / P_d ) + amenity_d
Rem_o,t = r_share * sum_d M_od,t * (1 - u_d) w_d      (contract pops, ADR 0174: higher r_share)
Rem enters B_c (B1) as a transfer
```

- Data: World Bank bilateral remittance estimates (Ratha; **licence check needed**); UN DESA
  stocks (CC BY 3.0 IGO).
- Addresses: Gulf contract labour and remittance-dependent economies (0174's own motivation).
- Effort S-M. Fit 1.0 (P4 Branch D, small change to V).

#### F3. Skill supply race

Sketch. With two-skill CES (ADR 0105, s 1.6, Katz and Murphy 1992) and WCDE cohorts, the skill
premium follows the race between education supply and skill-biased drift (Goldin and Katz 2008):

```
d ln(w_h / w_l) = (1/s) ( g_A_h - d ln(L_h / L_l) )
```

Already implied by the planned build; the new part is a validation: the premium path should be
flat or rising where tertiary supply grows slower than about 3% a year (**unverified threshold**).
Effort S (a test, not a model). Fit 1.0.

### G. Calibration and reconciliation

#### G1. Joint reconciliation of 2000 targets with reliability weights

Sketch. The data state is assembled in stages (outputs, prices, technology, PIGL weights, stocks),
and each conflict shows up as a residual somewhere else: RAS factors 0.48-3.24, PWT vs WDI, nu
against distance. KRAS (Lenzen, Gallego and Wood 2009) reconciles a matrix against conflicting
constraints with reliability weights and reports which constraints gave way:

```
min_x  sum_i ((x_i - x0_i) / sigma_i)^2  +  sum_k ((g_k(x) - target_k) / sigma_k)^2
x: the calibrated values; targets: ICIO, WDI CA, PWT price levels, GDP; sigma from source quality
```

- Addresses: pattern 5 and "calibration as hidden global state": conflicts become a report of
  which source moved, instead of a factor in an intermediate stage.
- Interactions: replaces parts of stages 1-4; must keep the steady-state fixed point (1e-12)
  afterwards.
- Effort L. Fit 1.x (C1 fast calibration).

#### G2. Free-moment scoreboard as a model, not a list

Sketch. Tag every gate number fitted or free (#185) and keep a scoreboard that every model change
reruns. The model-level idea: decide in advance which free moments each new mechanism here should
move (A1: D/Q; B1: CA half-life; C1: contagion depth; F1: CA vs dependency slope) and write those
targets before the spike, per the development laws.

- Effort S. Fit 1.0.

### H. Toy models for the fixture

Small enough to write as ignored tests in the fixture (as the E3 harnesses were). Each one answers
one question before an RFC.

| Toy | World | Question | Pass number fixed in advance |
|---|---|---|---|
| T1 Hume two islands | 2 provinces in 2 countries, 1 traded good each, 1 local good, fixed money, ntb shock x3 | Does the E3 package (no overdraft beyond the line) already deliver price-specie flow, and does B1 speed it up without a cycle? | CA gap half-life measured; no sign change after the first overshoot; output above 0.95 |
| T2 Zero-output market | 1 traded good, 2 origins, eta 1 | Does C2 (band) remove the flip? Does A2 (auction clearing) remove the clip cycle? | No tick with Q = 0 after the shock; gap below 1e-6 by 300 ticks |
| T3 Beer game | 3-stage chain, stage 2 uses 10% of its own output (like fixture good 2) | Bullwhip ratio under today's rule vs C4 | var(O)/var(D) per stage below 1.5 with C4 |
| T4 Dutch disease | 1 rent country, 2 traded goods + local, 1 partner | Do E1 + B1 give real appreciation and a falling non-rent traded share? | Signs as Corden-Neary; surplus = alpha R in the long run |
| T5 Demographic CA | 2 countries, one ageing fast (pop SPEC ageing only) | Does F1 + B1 move the CA in the EBA sign? | Sign over 20 years; world sum exactly 0 |
| T6 P-star | fixture, uniform 10% productivity loss in one country | Does A1 price the uniform excess demand? | D/Q below 1.02 within 60 ticks; index within 1.10 |
| T7 Trade credit | fixture, CL 0 | Does B4 with k = 1 remove the deadlock? | Data state stationary at 1e-12 with k = 1; k = 0 equals E3 bit for bit |

---

## 3. Synthesis: how the pieces fit

The catalogue points at one missing layer: **an external closure** made of a target (F1, E1, B5),
a quantity limit (E3 package, B2, B4) and a price (B1). Today the model has only the quantity
limit, built last week. The rest of the ideas either smooth the singularity at Q = 0 that every
frictionless failure passes through (C1, C2, A2, E2) or price the remaining uniform disequilibrium
(A1). The sequencing logic is to close the external accounts first, because every earlier
failure that "moved elsewhere" moved along an account; then smooth the boundary; then add data
discipline (A3, D1, D3, G1).

---

## 4. Ranked top 8 and prototype order

Ranking by expected value for the 1.0 path (failures they address, scope fit), then cost.

| Rank | Idea | Why | Effort | Fit |
|---|---|---|---|---|
| 1 | B1 Balance-of-payments price closure | Completes Antonio's "both" record; addresses patterns 1 and 7; the open x10/ntb 3 residual | M | 1.0 |
| 2 | A1 Output-adjusted money anchor | Prices the recorded D/Q 1.04-1.12 residual with no new parameter | S | 1.0 |
| 3 | C2 Shutdown hysteresis band | Removes the eta 1 flip that starts the island chain; nests the equilibrium | S | 1.0 |
| 4 | C1 Partial Leontief (critical inputs) | Stops the Leontief contagion that turned one dry cell into a world collapse | M | 1.0 |
| 5 | F1 Demographic current account | Gives B1 its target; makes D_c an open-run rule (calendar-blind runs need one) | S-M | 1.0 |
| 6 | B4 Trade credit and payment terms | Removes the CL 0 deadlock; realistic default chains; prepares P5 balance sheets | M | 1.0 |
| 7 | A2 Auction vs customer goods | A redesign of the Q = 0 step for storables, where the storage study said a patch cannot hold | M-L | 1.0 or 1.x |
| 8 | E1 Oil-rent fund and Dutch disease | Combines rents, fiscal, B1; explains Gulf surpluses | M | 1.x |

Next tier: B3 earnings-based credit, B5 sudden stops, C4 order rule, A3 Calvo discipline, D1
two-speed sourcing, G1 KRAS, D3 0272 check, E2 bounded yield, F2 Harris-Todaro.

**Prototype order** (each a one-day spike with the go/no-go number of section 2H written first):

1. T1 Hume two islands: measures whether the E3 package already gives price-specie flow and what
   B1 adds. Decides whether B1 is needed in P5a or can wait for currencies.
2. T6 P-star on the fixture: quick, reuses the E3 harness.
3. T2 zero-output market with C2, then C1 on the x10 diagnostic harness: the contagion depth with
   and without critical inputs.
4. T5 then F1 on the fixture's countries (needs P4 ageing, already merged in part).
5. T7 trade credit.
6. A2 on the storage-on V1 starts (pack, owner-side Mac).
7. T4 then E1 (after 1.0 unless the fiscal branch pulls rents into government sooner).

---

## 5. Open questions

1. Is B1 wanted in P5a, or only after currencies exist (money SPEC FX clearing)? T1 answers how
   much the quantity closure already does.
2. Should the target external balance after t0 be a rule (F1) or held at the 2000 shares? ADR
   0241 makes runs calendar-blind, so RFC 0308's observed path can only serve validation runs;
   this should be confirmed against 0308 D6.
3. A1 reads chained real output: should it be detrended by the P4 balanced-growth trends (0243),
   so trend growth does not read as deflationary pressure?
4. C1's criticality rule: cost-share threshold (licence-clean) or a published classification
   (IHS Markit, proprietary)? And should energy and self-inputs always be critical?
5. Is a debt-deflation guard (index floor or a gate assertion that the anchored index stays
   within a band) required before Branch F turns on nominal debt? The E3 mainland fell to 3% of
   base; with debts that is a 30-fold real debt rise.
6. C3 (within-cell heterogeneity) changes the equilibrium model and the reference. Is it in scope
   for ADR 0026 at 1.x, or 2.0?
7. E1 needs government ownership of rents, which 0278 left at pops by population for 1.0. Accept
   a rent-fund rule in 1.0 without SOEs?
8. Which free moments should each mechanism be graded on, fixed before the spikes (section 2, G2)?
