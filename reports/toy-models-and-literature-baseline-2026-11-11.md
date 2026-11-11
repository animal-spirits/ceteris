# Toy-model zoo and the unreconciled literature baseline, 11 November 2026

Follow-up to `reports/modelling-research-2026-11-11.md`, at Antonio's request: test every model
against known toy models now and at every later phase, add more toy models from the literature
(well known and less known) as sanity checks, and build a version that runs every module as its
source paper has it, before any reconciliation or fix, then calibrate it, run it and see what
happens. Plan only; no code, spec or rows change here. Literature from memory; values marked
**(unverified)** need checking before they become pass numbers.

## 1. How the zoo works

**Two tiers per toy.**

- **Analytic tier.** The toy's known result (closed form, or a published simulation result) is
  computed independently, in `ceteris-reference` or in the test itself, never through the
  simulator's structs (the same separation rule the reference follows today).
- **Simulator tier.** The simulator runs a degenerate world built to match the toy's
  assumptions (2 countries, 1 good, eta 1, no storage, and so on), and its result is compared to
  the analytic tier. A toy that cannot be expressed in the simulator yet says so and waits for its
  phase.

**Infrastructure needed first: a toy-world builder.** `crates/ceteris-econ/src/fixture.rs`
builds one fixed world (3 mainland countries of 8 provinces, an island, 10 goods). The zoo needs
a small builder for N countries, P provinces each, G goods with chosen traded/local split,
technology, demand weights, trade costs and switches, producing the same `EconInputs` the
fixture does, then calibrated by the existing steady-state stages. Effort M. Everything below
runs on it, in the cloud, with no pack.

**Pass numbers are fixed before the first run** (development laws), written in the test next to
the citation. A toy failure is a finding, never a reason to change the tolerance; a toy whose
assumptions turn out not to match the simulator is moved to "not expressible", with the reason
written down.

**Cadence.** The zoo becomes a standing test target (`cargo test -p ceteris-zoo` or an ignored
group in the econ crate), run on every econ PR as fixture-only tests, and rerun in full at every
phase gate next to the ADR 0257 benchmark rerun (P4, P5a, P5b, P8, P9). Each toy is tagged with
the phase from which it can run, so the list grows with the spec.

## 2. Catalogue

Columns: what the toy checks, the known result used as pass number, the module it tests, and when
it can run. "Now" means on main today with the builder.

### 2.1 Trade

| # | Toy (source) | Known result to check | Tests | When |
|---|---|---|---|---|
| Z1 | Ricardian 2x2 (Ricardo 1817; Dornbusch, Fischer and Samuelson 1977 continuum) | Specialisation follows comparative advantage; with theta large, gains approach the Ricardian bound | production, sourcing at high theta | Now |
| Z2 | Armington two countries (Armington 1969) | Welfare change = lambda_hat^(-1/theta) exactly (ACR 2012) for a cost shock; autarky gains lambda^(-1/theta) | ACR identity without IO, the x10 path | Now |
| Z3 | Eaton-Kortum symmetric N countries on a line (Eaton and Kortum 2002) | Gravity distance elasticity = -theta times the tau-distance elasticity exactly; PPML recovers theta | H2 machinery, kernel, ADR 0272's friction | Now |
| Z4 | Anderson-van Wincoop border (2003; McCallum 1995 Canada-US) | Border effect after multilateral resistance much smaller than McCallum's naive 22 (AvW report about 10.5 for Canada, **unverified**); small country's border effect larger | gateway kernel, MR terms | Now |
| Z5 | Caliendo-Parro IO amplification (2015; Costinot and Rodriguez-Clare 2014) | Welfare with roundabout production = ACR scaled by 1/(1 - a), a the intermediate share | Leontief block, ACR-IO | Now |
| Z6 | Ricardo-Viner specific factors (Jones 1971) | A tariff raises the protected sector's fixed-factor return more than proportionally, lowers the other's; signs closed form | CES value added with fixed factor (it is this model) | Now |
| Z7 | Transfer problem (Keynes 1929; Ohlin 1929; Dekle, Eaton and Kortum 2008) | Removing D_c moves relative wages in sign set by home bias; DEK hat algebra gives the number | transfers (0100/0308), reference | Now |
| Z8 | Balassa-Samuelson (Balassa 1964; Samuelson 1964) | Faster tradable productivity raises the non-traded relative price and the price level; Penn effect slope positive | local vs traded split, P4 drift | After P4 drift |
| Z9 | Metzler paradox check (Metzler 1949) | With large-country tariffs and low own demand elasticity the domestic relative price of the import can fall; the simulator must reproduce it only where the condition holds | tariff wedges, terms of trade | Now (less known) |
| Z10 | Lerner symmetry (Lerner 1936) | A uniform import tariff equals a uniform export tax in real effects, given balanced trade | wedge accounting; a sharp bug detector | Now (less known) |

### 2.2 Prices, markets and expectations

| # | Toy (source) | Known result | Tests | When |
|---|---|---|---|---|
| Z11 | Cobweb (Ezekiel 1938) | Lagged-supply market converges iff supply slope / demand slope below 1 | the one-market loop and J2 (0263): stable iff kappa J below 2 | Now |
| Z12 | Scarf's three goods (Scarf 1960) | Tatonnement cycles forever around the unique equilibrium | coupled cross-good modes (0267's open item); the rule should cycle exactly where Scarf predicts, which tells whether coupled-mode bounds are a model or a code question | Now (less known) |
| Z13 | Hume price-specie flow (Hume 1752) | Fixed money, two countries: a deficit closes by relative price levels; T1 of the research note | E3 package, B1 | Now |
| Z14 | Beer game (Sterman 1989) | Bullwhip ratio above 1 rising up the chain under stock-target ordering | order rule, self-input chain (T3) | Now |
| Z15 | Deaton-Laroque storage (1992) | Price distribution skewed right, stockouts produce spikes, autocorrelation high but below the data's (the famous miss) | competitive storage (0091, 0280) | Now (storage on) |
| Z16 | Brock-Hommes heterogeneous expectations (1997) | A cobweb with costly rational vs free naive predictors routes to chaos as the intensity of choice rises | pi^e and storage expectations (0019, 0091 g_s); finds whether adaptive gains create cycles | Now (less known) |
| Z17 | Quantity-theory neutrality | Doubling M doubles every price and wage, leaves real allocations unchanged within tolerance | money anchor (0338 nominal-shock test generalised) | Now |
| Z18 | Okun customer markets (Okun 1981) | Sticky-price goods adjust quantities first, auction goods prices first | A2 of the research note | With A2 |

### 2.3 Production networks

| # | Toy (source) | Known result | Tests | When |
|---|---|---|---|---|
| Z19 | Leontief multiplier (Leontief 1936) | Fixed prices: a final demand shock dx = (I - A)^-1 dF exactly | local chain, production, rationing off | Now |
| Z20 | Hulten's theorem (Hulten 1978) | First-order effect of a sector TFP shock on aggregate output = its Domar weight (sales / GDP) | production network at the steady state | Now |
| Z21 | Baqaee-Farhi second order (2019) | With complementarities (Leontief) the second-order loss exceeds Hulten; sign and size of the convexity | Leontief intermediates at large shocks (x10, oil) | Now |
| Z22 | Acemoglu, Carvalho, Ozdaglar and Tahbaz-Salehi (2012) | Aggregate volatility from idiosyncratic shocks decays slower than 1/sqrt(n) when the network is star-like | network propagation; checks the 45-goods network | Now (less known) |
| Z23 | Pichler et al. partial Leontief (2022) | Output path of their published UK 2020 forecast toy (reduced) | rationing (S11), C1 | With C1 |

### 2.4 Macro, money and finance

| # | Toy (source) | Known result | Tests | When |
|---|---|---|---|---|
| Z24 | Godley-Lavoie SIM (2007, ch. 3) | Steady state Y* = G / theta (tax rate); money stock and income converge geometrically with known rate | sector accounts, SFC matrices (money SPEC A1) | After P5 Branch A |
| Z25 | Godley-Lavoie OPEN (2007, ch. 6) | Two countries with gold reserves: the deficit country loses reserves until incomes adjust; the textbook BoP closure | B1, E3 package; the closest published twin of the island case | After P5 Branch A (Hume version now) |
| Z26 | Solow (1956) | Convergence rate (1 - alpha)(n + g + delta) near steady state | drift half-life P153, capital (0204) | After P5 Branch D |
| Z27 | Diamond OLG (1965) | Dynamic inefficiency when r below g; capital per worker steady state closed form | demography x saving, F1 | After P4 + P5 |
| Z28 | Three-equation New Keynesian (Clarida, Gali and Gertler 1999) | Taylor principle: inflation stable iff the rule's response to inflation exceeds 1 | policy rule (money SPEC 9.2) | After P5 Branch B |
| Z29 | Mundell-Fleming and Dornbusch overshooting (Dornbusch 1976) | Under floating rates a monetary expansion overshoots the long-run depreciation | FX clearing (9.9) | After currencies |
| Z30 | Diamond-Dybvig bank run (1983) | Two equilibria; a run at the threshold; deposit insurance removes it | banks and runs (9.11) | After P5 Branch F |
| Z31 | Kiyotaki-Moore (1997) | Collateral constraints amplify and persist a productivity shock; amplification grows with leverage | B3 credit lines, housing | After P5 Branch F |
| Z32 | Fisher debt deflation (1933), reduced | Nominal debt with falling prices raises real burden and default; a guard for the anchor | B2, the E3 mainland deflation episode | After P5 Branch F |
| Z33 | Goodwin growth cycle (1967) | Wage share and employment orbit (Lotka-Volterra) with closed-form period | wage rule (Phillips) x accumulation | After P5 Branch D (less known) |
| Z34 | Gabaix-Maggiori (2015) | Exchange rate moves with net currency imbalance over financiers' risk-bearing capacity | FX clearing | After currencies |

### 2.5 Population, labour, distribution

| # | Toy (source) | Known result | Tests | When |
|---|---|---|---|---|
| Z35 | Lotka stable population (Lotka 1907) | Constant vital rates converge to the stable age distribution and intrinsic growth r solving sum e^-ra l(a) m(a) = 1 | ageing, Brass mortality, fertility (P4 A1/A2) | Now (P4 A) |
| Z36 | Population momentum (Keyfitz 1971) | Fertility dropped to replacement: population grows by the momentum factor before stopping | same, with integer apportionment | Now |
| Z37 | Bongaarts-Feeney tempo (1998) | Postponement lowers period TFR, completed fertility unchanged | already in the pop SPEC test plan | P4 A2 |
| Z38 | Harris-Todaro (1970) | Equilibrium urban unemployment where (1 - u) w_u = w_r | migration logit V_d, F2 | P4 D |
| Z39 | Migration gravity (Anderson 2011) | Bilateral migration follows a structural gravity equation with multilateral resistance | migration logit | P4 D |
| Z40 | Katz-Murphy (1992) | Relative wage elasticity to relative supply = -1/s (s 1.6) | skills CES (0105) | P4 C |
| Z41 | Kesten random growth (Kesten 1973; Gabaix 2009; Benhabib, Bisin and Zhu 2011) | Multiplicative wealth shocks with a reflecting barrier give a Pareto tail with an exponent set by the shock moments | wealth brackets (0311), the proxy's lognormal assumption | After P5 (less known) |
| Z42 | Schelling (1971) | Mild preferences produce strong sorting | within-country migration at province level | 2.0+ |

### 2.6 Energy and resources

| # | Toy (source) | Known result | Tests | When |
|---|---|---|---|---|
| Z43 | Hotelling (1931) | Net price of an exhaustible resource rises at the interest rate | extraction, depletion (0028) | After P5 rates |
| Z44 | Corden-Neary Dutch disease (1982) | Resource boom raises non-traded prices and shrinks the other traded sector | E1 (T4) | With E1 |
| Z45 | Kilian oil shocks (2009) | Sign-restricted VAR on simulated data separates supply, aggregate demand and precautionary shocks; the simulator's known shocks must be recovered | oil block, storage | After 0028 1.0 part (less known) |

### 2.7 Agent-based classics (sanity of the agent layer)

| # | Toy (source) | Known result | Tests | When |
|---|---|---|---|---|
| Z46 | Gode-Sunder zero intelligence (1993) | Budget-constrained random traders reach near-full allocative efficiency | that budget constraints, not rationality, carry the market results; relevant to the "every negative account" pattern | Now (less known) |
| Z47 | Kirman ants (1993) | Herding with a recruitment rate gives a bimodal opinion share | expectations and risk appetite (9.12) | After P5 (less known) |
| Z48 | Santa Fe artificial market (Arthur et al. 1997) | Fat tails and volatility clustering from learning traders | asset demand system (9.9) | 2.0+ |

## 3. The unreconciled literature baseline

**Goal.** Run the world with every module exactly as its source paper or the original DESIGN has
it, before any of the reconciliation fixes, calibrate it, run it, and record what happens. The
useful product is not the run itself but a **failure atlas** and a **leave-one-out ablation**:
which reconciliations are still load-bearing, which are now redundant and can be deleted (KISS),
and which literature forms survive once the others are fixed.

### 3.1 What "as-is" means, module by module

| Module | Literature form | Reconciliations reverted |
|---|---|---|
| Trade | EK/AvW gravity with Simonovska-Waugh theta 4 for every good, iceberg costs only, buyers pay q = 1/alpha | per-class thetas (0274), fitted border nu (0092), log-distance friction (0272), EK-index buyer price (0200), bulk rate (0247) |
| Prices | DESIGN stock-flow rule, kappa1 0.2, kappa2 0.1, clip as designed | 0263 per-market cap, 0267 block cap, 0338 anchor, 0340 bounded gap, 0343 chained index, 0106 widths |
| Production | Leontief intermediates, normalized CES value added, ICIO donor columns as read | 0256 world oil technology, 0270 extraction column, 0281 rents to VA, 0266 input-capped employment |
| Demand | PIGL with Alder-Boppart-Mueller parameters, CES within | 0094 country weight inversion stays (needed to calibrate at all; marked) |
| Money and accounts | signed rule, interest-free clearing | 0112 write-off, 0344 credit line and pay on sales |
| Imbalances | per-good rescale r_g of ADR 0089 | 0100/0308 transfers and RAS |
| Storage | ADR 0091 competitive storage as first written | 0280 partial adjustment |
| Population | WCDE schedules, Brass, logit migration without caps | 0109 admission caps |

Some reverts are not possible without the old code (the r_g rescale was removed, for example).
Phase one is an inventory: for each of the 41 recorded alterations (the 30 October audit) plus
later ADRs, does a switch exist, can it be added without touching the default path, or is it
"not revertible" (written down with the reason).

### 3.2 Calibration

Two variants, both worth running:

- **Joint calibration as built** (stages 1-5) applied to the literature model. Shows whether the
  calibration machinery itself is doing reconciliation work (for example RAS factors absorbing a
  conflict).
- **Independent calibration**: each module calibrated only to its own source's targets, then
  joined without any balancing. Expected to fail at stage 2 or 5 (no fixed point); where and by
  how much is itself a result (it measures the inconsistency between the literatures).

### 3.3 Runs and reading

1. Fixture, frictionless and default modes, 1,200 ticks, from the data state and from the 10%
   perturbation (V1's setup), plus x10 and the nominal shock. Cloud, no pack.
2. Leave-one-in: start from the literature baseline, add one reconciliation at a time.
3. Leave-one-out: start from main, remove one reconciliation at a time.
4. Pack runs of the same matrix on the Mac (owner-side), after the fixture matrix shows which
   cells are worth the time.

For every run: survives or not, tick and mechanism of failure (classified by the patterns of the
research note, section 1.2), the free moments (distance elasticity, Brexit, ACR-IO correlation,
openness) where the run survives. The matrix tells, for each ADR, whether it is load-bearing.

**Expected result (a prediction, written down so it can be checked).** The fixture literature
baseline dies within a few hundred ticks in frictionless mode through pattern 1 (negative
accounts) and pattern 2 (Q = 0), and inflates in default mode through the aggregate term; the
gravity free moments (distance, Brexit) come closer to the literature than main's, because the
fitted nu and per-class thetas are what moved them. If the second half is right, that is evidence
for the C7 reading that the crossing friction is doing distance's work.

### 3.4 Rules this needs to respect

- Default behaviour, goldens and the shipped build do not change: the baseline is a research
  profile of switches, off by default, used only by ignored tests and the benchmark harness.
- Whether adding revert switches for accepted ADRs needs an RFC is an owner call (open question
  1). My reading: switches that never change the default path are research tooling, not a design
  deviation, but each switch should cite the ADR it reverts.
- Determinism holds per configuration; every run records its switch set in its output.

## 4. Proposed work items (one issue each)

| Item | Content | Effort | Where |
|---|---|---|---|
| W1 | Toy-world builder (N countries, P provinces, G goods, switches) | M | cloud |
| W2 | Zoo batch 1, runnable now: Z2, Z3, Z5, Z6, Z7, Z10, Z11, Z13, Z14, Z17, Z19, Z20, Z35, Z36 | M | cloud |
| W3 | Zoo batch 2, less known and harder: Z4, Z9, Z12, Z15, Z16, Z21, Z22, Z46 | M | cloud |
| W4 | Literature baseline inventory: every reconciliation, switch exists / addable / not revertible | S-M | cloud |
| W5 | Literature baseline on the fixture: switches, both calibrations, leave-one-in and leave-one-out matrix, report | L | cloud |
| W6 | Pack runs of the W5 matrix | M | Mac (owner-side) |
| W7 | Later batches tied to phases: P4 (Z8, Z37-Z40), P5 (Z24-Z28, Z30-Z33, Z41, Z43), currencies (Z29, Z34), C1/A2/E1 (Z18, Z23, Z44) | per phase | cloud |
| W8 | Zoo and baseline rerun added to each phase gate next to the ADR 0257 benchmark | S | process |

Suggested order: W1, then W2 and W4 in parallel, then W5, W3, W6. The research note's T1-T7
toys belong to W2 (T1 = Z13, T3 = Z14) and to the C2/C1/A1/B4 spikes.

## 5. Open questions

1. Do revert switches for accepted ADRs, used only in a research profile, need an RFC, or is an
   issue enough?
2. Should the zoo be its own crate (`ceteris-zoo`), or ignored tests in the existing crates? A
   crate keeps the analytic tier independent, like `ceteris-reference`.
3. Should a zoo failure block a PR (required check) or only report, at least until batch 1 is
   green on main?
4. For the independent calibration variant, is a run that cannot be calibrated at all an
   acceptable result to record, or should it fall back to the joint calibration?
