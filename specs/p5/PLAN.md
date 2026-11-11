# P5 Implementation Plan: money, finance and the state

**Goal:** Add the monetary, financial and fiscal layer to the population economy of P4, in the two gates of ADR 0207: P5a money and fiscal, P5b banks, debt and flows (spec/21 section 21.1 item 6; spec/09). This plan is a skeleton at the level of tasks and named tests; each branch's detailed task plan is written when it starts, against the kernel signatures the previous phase leaves.

**Gate P5a (spec/21 item 6):**

1. Stock-flow-consistent matrices close every tick: balance-sheet and transactions-flow matrices of spec/09 section 9.1 sum to zero across rows and columns, and agent claims aggregate exactly to the sector matrices.
2. A policy-rate shock moves credit, output, inflation and the exchange rate with plausible signs and lags.

**Gate P5b (spec/21 item 6):**

3. Responses within tolerance of the sequence-space HANK reference given matched intertemporal MPCs (ADR 0025).
4. Sovereign spreads respond to debt and stress.

Both gates also need spec/21 section 21.2 done (invariants over 100 years, budgets in 19.2, the 20% regression gate), the coupling register check of ADR 0080 for the modules implemented, and the P5 criterion carried over from pre-P3: the x10 border shock runs 1,200 ticks frictionless with no invariant violation and the largest |d ln p| over the last 50 ticks at most 1e-6 (ADR 0090).

**Starts from:** main after the P4 gate. Kernel signatures are those P4 leaves.

**Spec:** new `specs/money/SPEC.md` (balance sheets, regimes, rules, fiscal state, banks, debt, flows, bounds, test plan); `specs/econ/SPEC.md` (investment, price rule), `specs/trade/SPEC.md` (price rule, services); spec/09, 07 (7.4), 15, 17, 19, 21; ADRs 0025, 0040, 0042, 0043, 0080, 0090, 0168, 0172, 0176, 0202, 0204, 0242, and RFC 0286.

**Out of P5:** ADR 0207's 1.x and 2.0 items; monetary union entry and exit (fixed at t0 until 2.0, ADR 0242); endogenous immigration or fiscal politics (P6); hazards of pandemics and technology jumps (P8); non-1.0 asset classes beyond those of 9.10 that ADR 0207 defers.

## Open points for the owner

| # | Question | Default if not answered |
|---|---|---|
| O1 | Closed: RFC 0337 accepted ADRs 0025, 0040, 0042, 0043, 0080, 0090, 0168, 0176, 0202 and 0204 as written (owner, 2026-11-10) | Done |
| O2 | Build order inside P5a: balance sheets first (spec/09 section 9.14 stage 1), or the price rule and storage fix first because they change the real economy's goldens? | Balance sheets first; Branch E (price rule) before any branch that re-blesses, so one re-bless covers it |
| O3 | JST Macrohistory is likely non-commercial and priors-only (9.14); BIS, IMF and ECB commercial terms are unconfirmed. Which holdings series ship in the paid build? | Research pack only until #231 (NC audit) answers |

## Global constraints

P3's and P4's constraints carry over (determinism at 1, 3 and all threads with `fixed_sum`; no `HashMap` reaching state; exact money, with full-width accounts where flows can exceed i64; goldens change only in a commit whose subject says re-bless and why; register codes and decision ids from `cargo run -p ceteris-rows -- next-code P` and `-- next N`, only in the PR that writes them; calendar-blind). Also:

- **Switches.** Every mechanism has a switch whose off position reproduces the status quo bit for bit; the test is named `<mechanism>_off_equals_status_quo`.
- **Closure.** Goods and money invariants close every tick with the new instruments (work in progress, claims, reserves).
- **Bounds.** Every new table states its bound and size at 2150 in its SPEC (ADR 0236).
- **Finance parameter budget.** At most 12 of the 60 free behavioural parameters are finance (9.14); each new free parameter says which of the 12 it spends.

---

## Branch 0: `p5/records` (before code)

- [x] Write `specs/money/SPEC.md` from `specs/TEMPLATE.md`: sector balance sheets and the matrices of 9.1, regimes and rules of 9.2, fiscal state of 9.6, banks and asset markets of 9.3, 9.10 and 9.11, sovereign debt of 9.4, flows of 9.5 and 9.12, bounds, test plan from this plan.
- [ ] RFC for O1 with the register entries of each ADR (P35, P171, P172, P173, P174, P175, P176 and those of 0025, 0090, 0168, 0202, 0204 as they are today) checked against `spec/rows.toml`. Create `specs/COUPLINGS.md` (ADR 0080) with the module pairs implemented through P4 and their tests.
- [ ] Commit `docs(money): p5 spec and accepted p5 adrs`.

## Branch E: `p5/price-rule` (ADR 0090, RFC 0286; #215, #216)

### Task E1: Split price rule with the money anchor (ADR 0338)

- [x] Origin-market rule of ADR 0338 (trade SPEC, Money-anchored rule): `dlnp = clip(a_o (l_o - lbar_g) + kappa_b (lbar_g - lbar) - kappa_m (ln(I/I0) - ln(M/M0)) - stock term)`, `a_o = kappa_r / E_g`; one reduction per good per tick with `fixed_sum`. Before the build: the Mac pack data-state run and the derived kappa_b (RFC 0338, Before the build; the fixture part is done, the Mac pack run is pending, and the rule stays off by default until it passes).
- [ ] Tests: `price_rule_off_equals_status_quo` (goldens, unchanged); `fixture_steady_state_unchanged` and `nominal_shock_returns_within_30_ticks` (done, `p5_price_anchor.rs`); `x10_border_shock_converges_frictionless` is carried by E3 (ADRs 0339, 0341). Re-bless, if the rule is switched on by default, in its own commit.

### Task E2: Storage-on frictionless fix (RFC 0286)

- [ ] Fix the storage-on misses on all 16 V1 starts and the fixture tests, as RFC 0286 records the screen and the tests any fix must pass. Failing test first: the recorded screen. Commit.
- [x] Bounded origin gap of the anchored rule (ADR 0340, RFC 0339): l_o = 2 (D + b - Q) / (D + b + Q). Test `x10_border_shock_keeps_the_index_off_the_bound` (fails with the Q-floor gap). The RFC 0286 screen with the anchor and this gap runs on the Mac (pending).

### Task E3: Zero-output production trap (ADR 0341)

- [x] Built behind `PriceAnchor::e3` (ADRs 0343 to 0345, RFC 0342): chained anchor index over cells that trade, pay on sales, traded-input credit line of one loss cap. Tests `e3_x10_border_shock_survives_700_ticks`, `e3_steady_state_unchanged`, `e3_nominal_shock_returns_within_30_ticks`, `e3_snapshot_replays` (`p5_price_anchor.rs`). Pack runs (data state, RFC 0286 screen) pending on the Mac.

- [ ] Diagnose on the fixture why output dies province by province at x10 with the index held (spike: Q halves each tick while D > 0; hypothesis the Leontief zero state is absorbing, inputs ordered on constrained output, and the eta 1 labour flip). Spec and RFC before code. Carries `x10_border_shock_converges_frictionless` (ADR 0339).

## Branch A: `p5/balance-sheets` (spec/09 section 9.1, #213)

### Task A1: Sector balance sheets and the matrices

**Interfaces:** sector accounts for households, enterprises, banks, government, central bank and rest of world; balance-sheet matrix and transactions-flow matrix computed every tick; initial sheets from national financial accounts where available and imputed elsewhere.

- [~] (kernel in `sfc.rs`, not in the tick; agent claims, flow rows and columns close in unit tests; `sfc_off_equals_status_quo`, the national-accounts initial sheets and the tick wiring wait on Branch A2's data and the system ID check) Failing tests: `matrices_sum_to_zero_across_rows_and_columns_every_tick`; `agent_claims_aggregate_to_sector_matrices_exactly`; `sfc_off_equals_status_quo`. Implement, commit.

### Task A2: Who-to-whom holdings

- [ ] Instrument by holder matrix of 9.8 from IMF CPIS, ECB securities holdings and US TIC (research pack); net international investment positions. Test: `who_to_whom_row_and_column_sums_equal_positions`.

## Branch B: `p5/regimes-and-rules` (spec/09 section 9.2, ADR 0176)

- [ ] **B1** Regime classification at t0 from Ilzetzki, Reinhart and Rogoff; Taylor-type rule with effective lower bound and asset purchases at the bound; band defence from reserves; hard peg; dollarization; union with one central bank. Tests: `regime_off_equals_status_quo`; `peg_defends_with_reserves_until_cover_runs_out`; `union_members_share_one_rate`.
- [ ] **B2** Regime transition hazards on inflation, reserve cover and recent peg breaks (ADR 0242). Test: `transition_hazards_at_zero_keep_regimes_fixed`.
- [ ] **B3** Endogenous neutral rate (ADR 0176): annual one-sided HLW Kalman filter with published variances, P35 as the initial value. Tests: `frozen_filter_leaves_hashes_unchanged`; `filter_reproduces_published_hlw_estimates_within_bands` (US, euro area, UK); `lower_bound_share_2009_2015_rises_toward_history` (holdout).
- [ ] **B4** The policy-rate gate: a rate shock in a toy economy and on the full pack moves credit, output, inflation and the exchange rate with the expected signs and lags. Test: `policy_rate_shock_has_plausible_signs_and_lags`.

## Branch C: `p5/fiscal-state` (spec/09 section 9.6, ADR 0042)

- [~] (kernel in `fiscal.rs`, not in the tick; `informality_shrinks_the_base` and `revenue_and_spending_close_in_the_government_account` pass, the latter through the SFC matrices; `fiscal_off_equals_status_quo`, spending categories as state-AI choices and the pack initial rates wait on the tick wiring) **C1** Revenue instruments `R = tau x Base x kappa x compliance` with fiscal capacity kappa per instrument that investment builds and depreciation erodes; spending categories; deficits financed by borrowing. Tests: `fiscal_off_equals_status_quo`; `informality_shrinks_the_base`; `revenue_and_spending_close_in_the_government_account`.
- [ ] **C2** Public capital `A = A_cal (K_g / K_g0)^P173`, public investment times an efficiency factor from PIMA scores and governance (P174); r minus g reported per state. Tests: `public_capital_elasticity_zero_equals_status_quo`; `spending_multipliers_in_slack_and_non_slack_states_order_as_ramey_zubairy`.

- [ ] **C3** Energy policy held at t0 (ADR 0278): fuel excises and subsidies as per-unit wedges on goods B06, C19 and D at their 2000 rates per state, held for the run like trade policy; a subsidy is a fiscal cost at the wedge times quantity. Tests: `energy_wedges_off_equals_status_quo`; `subsidy_cost_equals_wedge_times_quantity`.
- [ ] **C4** State energy cells (ADR 0278, applying ADR 0201's ownership split to crude oil and gas extraction and power only): a cell splits where the state share is at least 5% of (province, sector) VA in 2000; state cells' profits go to the state's account and their investment follows the 7.4 logit like any owner. Tests: `state_cell_split_off_equals_status_quo`; `state_cell_profits_reach_the_government_account`.

## Branch D: `p5/investment` (ADRs 0202, 0204; #214)

- [ ] **D1** Capital formation by asset class (machinery, structures, ICT, dwellings, public core and other) priced on province markets from the ICIO investment column, orders with a schedule, coefficients locked at order. Tests: `instant_orders_equal_the_instant_rule_bit_for_bit`; `goods_and_money_close_with_work_in_progress`; `steady_state_2000_pipeline_holds_for_100_years`; `price_shock_after_order_keeps_pre_shock_coefficients`.
- [ ] **D2** Firm entry, exit and the zombie band, quarterly per cell. Tests: `free_entry_mass_scales_with_labour_and_not_trade_costs`; `cutoff_rise_removes_exactly_the_share`; `entry_exit_off_equals_fixed_mass_path`. The zombie share needs the bank capital gap (Branch F); with `z_b = 0` there are no zombies.
- [ ] **D3** Investment-to-GDP and capital-output paths 2000-2010 within tolerance of PWT (reported at the P5a gate, asserted at P9).

## Branch F: `p5/banks-and-housing` (spec/09 sections 9.3, 9.10, 9.11; ADR 0040, #217)

- [ ] **F1** Bank archetypes, the monthly fixed-point loop, the four contagion channels, deposit insurance and the safety net. Tests: `banks_off_equals_status_quo`; `bank_balance_sheets_close_every_tick`; `run_threshold_triggers_contagion_through_interbank_exposures`.
- [ ] **F2** Housing supply elasticity per province (land unavailability and regulation, P171), mortgage caps by LTV, LTI and DSTI (macroprudential levers), investor demand under extrapolative expectations (P172). Tests: `non_binding_caps_and_zero_investors_equal_status_quo`; `tighter_ltv_lowers_the_peak_of_prices_and_credit_in_a_toy_boom`.
- [ ] **F3** Equity and housing price paths and credit-to-GDP against targets of 9.14, reported.

## Branch G: `p5/sovereign-and-flows` (spec/09 sections 9.4, 9.5, 9.12; ADR 0043)

- [ ] **G1** Sovereign debt, default hazard, spreads from debt and stress, IMF lending into arrears under conditionality. Test: `spreads_respond_to_debt_and_stress` (gate item 4).
- [ ] **G2** Creditor classes, restructuring by bargaining on capacity to pay with holdouts reduced by collective action clauses, market exclusion with a reputation period (P175, P176). Tests: `haircut_distribution_and_default_durations_within_tolerance_of_cruces_trebesch`; `larger_haircuts_give_deeper_post_default_losses`.
- [ ] **G3** Capital flows, reserves, the global financial cycle, the dollar and capital controls (9.5, 9.12). Test: `flows_off_equals_status_quo`; `current_account_and_financial_account_close_each_tick`.

## Branch H: `p5/hank-reference` (ADR 0025, #218)

- [~] (MPC harness and the closed-form test are in `crates/ceteris-econ/tests/p5_mpc.rs`; the reference and the matched-MPC test wait on the consumption rule with m and lambda of P5a) Sequence-space HANK reference in Rust in `ceteris-reference`. Failing tests first: `closed_form_case_mpc_matrix_is_scaled_identity` (m = 1, lambda = 0: intertemporal MPC matrix is `(1 - s_c - g_c)` times the identity, spending out of income is 1); then `responses_match_hank_given_matched_intertemporal_mpcs` (gate item 3).

## Branch I: `p5/hazards` (ADR 0242, #219; P5b share)

- [ ] Regime and prudential hazards register with the thinning primitive of P3 (`hazard.rs`); epidemic and technology-jump hazards belong to P8. Tests: `zero_rate_hazards_leave_hashes_unchanged`; `branches_share_events_of_untouched_hazards`.

## Branch J: `p5/services-trade` (ADR 0168, #158)

- [ ] Traded services by a country-pair gravity kernel without routes (PPML on BaTiS), tourism as a logit over destinations buying the destination's local services; border closures and advisories as wedges. Tests: `services_off_equals_status_quo`; `ppml_on_simulated_services_flows_recovers_elasticity_and_friction`; `tourism_economies_earn_their_surplus_from_trade_not_transfers` (Aruba, Bermuda, Andorra, Antigua, Palau, Macao household income share rises toward data); `replayed_2020_border_closures_reproduce_travel_receipt_falls`.

## Branch K: gates

- [ ] P5a gate: matrices close every tick, policy-rate shock table, x10 criterion, budgets against 19.2, coupling register complete. Recorded in `tests/perf/p5a-gate.md` and the SPEC.
- [ ] P5b gate: HANK comparison, spread response, crisis frequencies reported against 9.14 targets, budgets, coupling register. Recorded in `tests/perf/p5b-gate.md`. Mac and the full pack.

## Dependencies

E before any branch that re-blesses; A before B, C, D, F, G; B before G; C before D (public capital classes); F before the zombie share of D2; H independent after A; I after D and F; J independent after A. P5a is Branches E, A, B, C, D (D1, D3); P5b is F, G, H, I, D2, J.
