# Money SPEC

Status: draft (P5 Branch 0; nothing here is built; awaiting owner approval; ADRs 0025, 0040, 0042, 0043, 0080, 0090, 0168, 0176, 0202 and 0204 accepted by RFC 0337, owner decision 2026-11-10)
Phase: P5 (P5a money and fiscal, P5b banks, debt and flows)
Design sections: 7.4, 9.1 to 9.14, 15.3 to 15.5, 17, 19.2, 19.4, 21.1; ADRs 0025, 0040, 0042, 0043, 0080, 0090, 0095, 0100, 0168, 0176, 0202, 0204, 0227 to 0231, 0233, 0236, 0242, 0278, RFC 0286; plan `specs/p5/PLAN.md`

## Purpose

`ceteris-finance` (balance sheets, rules, banks, asset markets, sovereign debt, flows) and the
money and fiscal parts of `ceteris-econ` own claims and the state's budget: sector balance
sheets and the matrices that close them, monetary and exchange-rate regimes and policy rules,
the fiscal state, banks and their contagion, asset-price clearing, sovereign debt and
restructuring, capital flows. It reads real output, wages, prices, trade balances and population
from `specs/econ/SPEC.md`, `specs/trade/SPEC.md` and `specs/pop/SPEC.md`; it writes interest, taxes,
transfers, credit conditions and the exchange rate back. It does not own the real price rule and
investment orders (`specs/econ/SPEC.md`; Branches E and D build them against this contract),
elections and fiscal politics (P6), war finance (P7), hazards of pandemics and technology jumps
(P8), monetary union entry and exit (fixed at t0 until 2.0, ADR 0242), derivatives and credit
derivatives (excluded, 9.8), or non-1.0 asset classes (ADR 0207). Until Branch A the observed
imbalances stay exogenous transfers (9.5, ADR 0100) and profits are owned nationally (ADR 0308).

## State components

| Component | Entity | Type | Units | Initialized from |
|---|---|---|---|---|
| Sector balance sheet | (country, sector in {households, enterprises, banks, government, central bank}) x instrument | `i128` | local currency minor units | National financial accounts (OECD, ECB, Fed Z.1), imputed elsewhere (9.1, 9.14) |
| Rest-of-world position | country x instrument | `i128` | minor units | IMF IIP, Lane and Milesi-Ferretti (2018) |
| Holdings matrix | (holder class, instrument) | `i128` | minor units | IMF CPIS, ECB SHS, US TIC (research pack, 9.14) |
| Transactions-flow matrix | (country, sector, flow) | `i128` | minor units per tick | Built by the tick from journal postings, not initialized |
| Regime | country | enum | n/a | Ilzetzki, Reinhart and Rogoff de facto classification (9.2) |
| Policy rate, neutral-rate filter state | central bank | `f64`, small struct | per year | Policy-rate series; initial r* from published HLW estimates (ADRs 0176, 0182) |
| Reserves | central bank | `i128` | minor units | IMF IFS, COFER |
| Fiscal capacity kappa | (country, revenue instrument) | `f64` | share | Government revenue statistics (9.6) |
| Public capital K_g | (country, core / other) | `f64` | constant 2000 US dollars | IMF Investment and Capital Stock Dataset (ADR 0042) |
| Bank archetype book | (country or region, archetype) | `i128` accounts, `f64` ratios | minor units | Financial accounts, BIS banking statistics (9.11) |
| Latent demand eta | (investor class, asset) | `f64` | logit units | Backed out so 2000 holdings reproduce exactly (9.9); data-determined |
| Asset price P, quantity Q | asset | `f64`, `i128` | price index, units | 2000 market values |
| Sovereign debt stock | (state, currency, maturity bucket) | `i128` | minor units | BIS debt securities, Arslanalp and Tsuda (9.8) |
| Default and restructuring state | state | enum, `f64` | n/a | Default history (Cruces and Trebesch; Horn, Reinhart and Trebesch) |
| Province housing stock, price | province | `f64` | dwellings, price index | Residential price series, land availability (ADR 0040) |
| Capital pipeline | (cell, asset class, stage) | `f64` | work in progress, money | 2000 steady state of ADR 0089 (ADR 0204) |
| Risk-appetite state | world | `f64` | log scale | Stationary draw from the run's streams (9.12) |

Money amounts are exact integers; `i128` where flows can exceed `i64` (aggregates, world and
rest-of-world accounts, as in ADR 0106). Prices, rates and ratios are `f64` (ADR 0108's f64 rule
stands). No `HashMap` reaches state.

**Bounds (ADR 0236).** The demand system is about 2,000 assets by 2,000 investor classes
worldwide (9.9), held block-sparse by home bias; at one `i128` per nonzero holding and a few
percent density this is in the low megabytes at 2150. These are estimates from the design text,
not measurements; Branch A replaces them with measured sizes and a stated bound per table before
it merges. The transactions-flow matrix is rebuilt each tick, not stored. The capital pipeline
is bounded by the longest stage (ADR 0204 time to build) times cells times five classes. Event
logs of crises and restructurings are bounded by retention (ADR 0212).

## Equations

Notation follows spec/09. Parameter IDs are those of spec/17 where they exist; marks one
the accepting RFC must register (codes are taken only in that PR).

**Matrices (9.1; Godley and Lavoie 2007).** Balance-sheet matrix `B` (sectors by instruments,
columns sum to the instrument's issuer liability) and transactions-flow matrix `T`: every row
and column sums to zero each tick. Agent claims aggregate to the sector entries exactly, in
integers (Caverzasi and Godin 2015). Every asset is someone's liability: the who-to-whom matrix
sums to zero (V0, 9.8).

**Policy rule (9.2, 9.3; ADR 0176).** `i_t = max(lb, r*_t + pi_t + a_pi (pi_t - pi*) + a_y y_t)`
with coefficients P35 per central bank and `r*_t` the one-sided annual HLW Kalman filter state
(published variances); P35's intercept is the 2000 initial value of `r*`. Asset purchases at the
bound enter the demand system as one more investor (9.9). Hard pegs set `i` to defend the parity
and stop when reserve cover `R / short-term external debt` is exhausted; dollarized and union
members take the anchor's rate. Regime transition hazards on inflation, reserve cover and recent
peg breaks come from the hazards register (ADR 0242, Branch I); at zero rates regimes stay fixed.

**Fiscal state (9.6; ADR 0042).** `R_{j,i} = tau_{j,i} Base_{j,i} kappa_{j,i} compliance_j`;
kappa rises with spending on capacity and depreciates slowly (Besley and Persson 2009);
informality shrinks Base. Spending categories as 9.6; deficit `= spending + interest - revenue`,
financed by borrowing or monetary financing, budgets set by the state AI under institutional
rules (Section 10). Energy wedges and state energy cells (ADR 0278) enter as per-unit wedges and
profits to the government account. Public capital: `A = A_cal (K_g / K_g0)^P173`, public investment
times an efficiency factor from PIMA scores and governance (P174); the rest is lost. r minus g is
reported per state.

**Asset-price clearing (9.9).** `w_{i,a} = exp(delta_{i,a}) / (1 + sum_b exp(delta_{i,b}))`,
`delta_{i,a} = gamma_i E_i[r_a] + beta_i' x_a + eta_{i,a}`; prices solve
`P_a Q_a = sum_i A_i w_{i,a}(P)` monthly. Damped Newton on log prices, block-sparse Jacobian,
fixed iteration count, fixed reduction order; each class's demand slopes down in its own price;
a residual counter per market records every tick the fixed count leaves error above tolerance.
Exchange rates clear here jointly (Gabaix and Maggiori 2015 as the special case, ADR 0228).
Housing is cleared per province outside the system against a slowly adjusting stock (9.10).

**Banks and contagion (9.11).** Up to five archetypes per state with risk-weighted capital,
leverage and liquidity ratios; lending tightens as buffers shrink and eligibility terms push
risky sales. Four channels resolved inside the monthly fixed-point loop: direct exposures, fire
sales on common holdings, funding runs past a threshold with seeded noise keyed by the run's
streams (14.4), the sovereign-bank loop. Margin-constrained leverage per ADR 0233. Safety-net
levers per 9.11 and ADR 0229 state-triggered rules.

**Housing (ADR 0040).** Supply elasticity per province from land unavailability and regulation
(P171); mortgage demand per wealth bracket capped by the binding of LTV, LTI and DSTI limits
(macroprudential levers, binding share reported); investor demand responds to expected gains
under extrapolative expectations (9.13), share of purchases P172.

**Sovereign debt (9.4; ADR 0043).** Spread `s = f(debt / revenue, deficit, foreign-currency
share, reserves, institutions, default history, lender of last resort)` tied to the default
hazard of the state AI's own rule on the same variables. Restructuring: creditor classes,
bargaining on capacity to pay with holdouts reduced by collective action clauses (P175),
multilaterals senior with no haircut, market exclusion until agreement plus a reputation period
rising with the haircut (P176), IMF lending into arrears under conditionality.

**Flows (9.5, 9.12; ADR 0168).** Gross flows emerge from portfolio rebalancing in the demand
system; global risk appetite follows a stochastic-volatility process drawn from the run's
streams and written to the event log (ADR 0241); capital controls and sanctions are weights and
eligibility in the demand system. Services trade is a country-pair gravity kernel without routes
(PPML on BaTiS), tourism a logit over destinations buying the destination's local services.

**Investment and entry (7.4; ADRs 0202, 0204).** Specified in `specs/econ/SPEC.md`; this SPEC
owns the cash-call and credit side: orders with a schedule draw finance over the stages, and the
zombie share `z_b` comes from the bank capital gap (zero when banks are off).

**Price rule (ADR 0090, RFC 0286).** Specified in `specs/econ/SPEC.md` and `specs/trade/SPEC.md`;
this SPEC's only dependency is that Branch E re-blesses before any finance branch does.

**HANK reference (ADR 0025).** Sequence-space Jacobian reference in `ceteris-reference`;
gate item 3 compares impulse responses given matched intertemporal MPCs (P37).

## System order

System IDs 230 to 249 are proposed for money (econ 201 to 210, pop 220 to 229); Branch A confirms
them against the registry. Cadence is monthly unless noted; finance runs after the real economy of
the tick (E4) and population, and before the next tick's E1.

| System ID | Name | Cadence | Reads | Writes | Position |
|---|---|---|---|---|---|
| 230 | policy and regimes | monthly | pi, y, reserves, r* | policy rate, regime | after 228 |
| 231 | r* filter (B3) | yearly | pi, y, i | filter state | after 230 |
| 232 | fiscal collection and spending | monthly | Base, kappa, rules | government account, kappa, K_g | after 230 |
| 233 | investment financing | monthly | orders, credit | pipeline, loans | after 232 |
| 234 | demand-system clearing | monthly | holdings, eta, expectations | P, Q, FX | after 233 |
| 235 | banks and contagion | monthly, inside 234's loop | B, P | bank books, runs | with 234 |
| 236 | housing | monthly | income, credit | prices, mortgages | after 234 |
| 237 | sovereign debt | monthly | fiscal, spreads | debt, default, restructuring | after 234 |
| 238 | flows and risk appetite | monthly | all | positions, risk state | after 237 |
| 239 | services trade | monthly | incomes | traded services | after 238 |
| 240 | SFC assert | monthly | all | | last |

RNG: only risk-appetite innovations, run thresholds and hazards; each draws from the stream of
its system ID and logs to the event log, with the thinning primitive of P3 so branches share
events of untouched hazards. All reductions use `fixed_sum`.

## Invariants

| Invariant | Statement | Tolerance | Checked where |
|---|---|---|---|
| SFC matrices close | `B` and `T` sum to zero across rows and columns | exact | 240; `matrices_sum_to_zero_across_rows_and_columns_every_tick` |
| Claims aggregate | agent claims equal sector matrices | exact | `agent_claims_aggregate_to_sector_matrices_exactly` |
| Who-to-whom | row and column sums equal positions | exact | `who_to_whom_row_and_column_sums_equal_positions` |
| Goods and money close | with work in progress, claims and reserves | exact | `goods_and_money_close_with_work_in_progress` |
| Government account | revenue and spending close | exact | `revenue_and_spending_close_in_the_government_account` |
| Current and financial account | close each tick | exact | `current_account_and_financial_account_close_each_tick` |
| Bank books | balance sheets close every tick | exact | `bank_balance_sheets_close_every_tick` |
| Switch-off equals status quo | each mechanism's switch off reproduces the golden hash | bit | `<mechanism>_off_equals_status_quo`, one per mechanism |
| Thread-count independence | state and journal hashes equal at 1, 3 and all threads | bit | `tests/golden.rs` |
| Clearing residual | residual counter zero above tolerance in the gate runs | per Branch K | 234 counters |
| x10 border shock | 1,200 ticks frictionless, no violation, largest abs dln p over the last 50 ticks | 1e-6 | `x10_border_shock_converges_frictionless` (ADR 0090) |
| P5a policy shock | rate shock moves credit, output, inflation, exchange rate with plausible signs and lags | signs and timing | `policy_rate_shock_has_plausible_signs_and_lags` |
| P5b HANK | responses within band of the reference given matched MPCs | per Branch H, fixed before running | `responses_match_hank_given_matched_intertemporal_mpcs` |
| Spreads | respond to debt and stress | sign | `spreads_respond_to_debt_and_stress` |

Tolerances marked "per Branch H" and "per Branch K" are fixed in writing before the first run;
none changes after seeing results without an RFC (development laws).

## Parameters

| ID | Name | Prior / value | Class |
|---|---|---|---|
| P35 | Taylor-rule coefficients; intercept is the 2000 initial r* | Inflation above 1, output positive | literature, calibrated |
| P37 | Consumption propensity m, wealth correction lambda, target wealth ratio | spec/17 | calibrated to intertemporal MPCs |
| P10 | Depreciation by asset class (range widened to 1 to 35%, ADR 0204) | PWT rates | data |
| P50, P52 | sigma_g by theta class (range 2 to 11, ADR 0202); entry cost f_e | Zipf 1.05; OECD SDBS, Eurostat SBS | literature, data |
| P03a, P03b, P177 | kappa_r, kappa2, kappa_n (ADR 0090) | 0.5, set in region, 0.2 | free, calibrated |
| P171, P172 | Housing supply elasticity map; investor purchase share | ADR 0040 | data, free |
| P173, P174 | Public capital elasticity (centred 0.08); efficiency | ADR 0042 | literature, data |
| P175, P176 | Restructuring options and CAC holdout reduction; reputation period | ADR 0043 | free, calibrated |
| delta, eta_e | Firm death rate; entry response (ADR 0202) | BDS, DynEmp; 2008-10 start-up response | data, calibrated |
| Services kernel, 2 | Services elasticity, border friction (ADR 0168) | PPML on BaTiS | estimated |
| Finance free set | Run thresholds, fire-sale impact, diagnostic weight, risk-appetite process (9.14) | spec/17 | free |

**Finance budget.** At most 12 of the 60 free behavioural parameters are finance (9.14; ADR
0233 spent the last of them). Every row above states in its accepting PR which of the
twelve it spends; any that does not fit is a post-1.0 item, not an exception. Demand elasticities
are literature-anchored and latent demand and initial balance sheets are data-determined.

## Equilibrium nesting

With every finance switch off, the economy equals the P4 status quo and every golden holds. With
the neutral-rate filter frozen, hashes are unchanged; with P173 zero, public capital drops out;
with non-binding caps and zero investors, housing equals the status quo; with `z_b = 0` there are
no zombies; with instant orders the investment rule equals the instant rule bit for bit; with
perfect foresight in place of heuristics and frictions off, small-shock responses lie within
tolerance of the sequence-space HANK reference (ADR 0025). With QE and reserve demand off, the
demand system collapses to Koijen and Yogo (2019, 2020).

## Test plan

- Unit: one test per equation above, named as in `specs/p5/PLAN.md` (Branches A to J).
- Property: every invariant above over 100 simulated years at 1, 3 and all threads, including the
  coupling-register check (ADR 0080) over declared read and write sets (19.4).
- Golden: finance-on 120-tick scenario with its hash file under `tests/golden/`; one golden move
  per branch, each a reviewed commit that says re-bless and why; Branch E first.
- Gates: P5a and P5b recorded in `tests/perf/p5a-gate.md` and `tests/perf/p5b-gate.md`.
- Validation (reported, asserted at P9): the 9.14 targets and 9.13 crisis frequencies against
  Laeven and Valencia (2018), Cruces and Trebesch haircuts, PWT investment paths, HLW estimates.
- Benchmarks: tick cost of each system against 19.2 before each branch merges; the 19.2 finance
  budget is unmeasured today and is measured at Branch A.
- Data: JST Macrohistory is priors-only and never bundled; BIS, IMF, ECB commercial terms and
  the holdings series that ship in the paid build are open (below, issue #231).

## Open questions

- Holdings series in the paid build (plan O3): research pack only until the NC audit (#231).
- Build order inside P5a (plan O2): default balance sheets first, Branch E before any re-bless.
- Whether the 19.2 finance budget fits next to the 45-goods tick (about 200 ms against 160 ms
  budget in the last measurement); unmeasured, so Branch A reports it first.
- The system ID block 230 to 249 is a proposal until Branch A checks the registry.
