# P4 Implementation Plan: population

**Goal:** Give the economy people that age, die, are born, learn, switch sectors and move, and replace the zero-growth steady state with a calibration to a moving equilibrium (spec/21 section 21.1 item 5, ADR 0207 as re-rated by ADR 0324). Culture and religion stay at their 2000 shares until P6.

**Gate (spec/21 section 21.1 item 5):**

1. Fertility, mortality and urbanization for 2000-2010 within tolerance of UN data (WPP for fertility and mortality, UN DESA for urbanization and migrant stocks); the tolerances are set in Branch 0.
2. Population conservation holds every year, by cohort cell, in exact integers.
3. Each 1.0-path ADR below meets its own switch-off test: with its switches off, golden hashes equal the status quo.
4. spec/21 section 21.2 done: invariants over 100 years, budgets (19.2), the 20% regression gate.

**Starts from:** main after the P3 gate (#171). Branch F depends on nothing in P4 and can start with P3's last branches; everything else needs the age-band state of Branch A. Kernel signatures are those P3 leaves, so each branch re-reads them when it starts.

**Spec:** new `specs/pop/SPEC.md` (state, cohort cells, demography equations, migration, bounds, test plan); `specs/econ/SPEC.md` (Labour: skill groups, sector switching; Calibration: moving reference), `specs/trade/SPEC.md` (crossing friction). spec/06, 07, 08, 15, 17, 19, 21; ADRs 0031, 0032, 0033, 0074, 0105, 0109, 0167, 0243, 0272, plus 0095, 0098, 0106, 0207, 0241.

**Out of P4:** ADR 0207's 1.x and 2.0 items; hazards that belong to P8 (pollution and heat in 0033, malaria's link to the climate emulator in 0167); endogenous immigration policy (0109 option 3, P6); culture and religion (P6); the 50-goods pack build is #206 and lands before Branch G's recalibration.

## Open points for the owner

| # | Question | Default if not answered |
|---|---|---|
| O1 (closed, RFC 0335 D1: accepted) | ADRs 0031, 0032, 0033, 0074, 0105, 0109, 0167 are still `proposed`. Accept them as written for the 1.0 path in one status RFC (as RFC 0312 did for the built ones)? | Branch 0 writes that RFC with one decision per ADR; nothing is built before it is accepted |
| O2 | Order: Branch F (moving reference) first, or after demography? | F first: population growth breaks the steady state, and demography without it cannot be tested against the reference |
| O3 (closed, RFC 0335 D3: World Bank CC BY cause groups ship in the paid build, else all-cause) | WHO GHE and IHME are non-commercial (ADR 0050, 0033): the release pack takes World Bank re-publications or published cause-group parameters. Which cause shares ship in the paid build? | Research pack only until #231 (NC audit) says otherwise |

## Global constraints

P3's constraints carry over (determinism at 1, 3 and all threads with `fixed_sum`; no `HashMap` reaching state; exact money; goldens change only in a commit whose subject says re-bless and why; register codes and decision ids from `cargo run -p ceteris-rows -- next-code P` and `-- next N`, only in the PR that writes them). Also:

- **Calendar-blind** (ADR 0241): ages and durations are state; no system reads a date.
- **Switches.** Every mechanism has a switch whose off position reproduces the status quo bit for bit; the test is named `<mechanism>_off_equals_status_quo`.
- **Bounds.** Every new table states its bound and size at 2150 in its SPEC (ADR 0236).
- **Tick budget.** Pop state widens by age band, sex and education; each branch reports its tick cost against 19.2 before merging.

---

## Branch 0: `p4/records` (before code)

- [ ] Write `specs/pop/SPEC.md` from `specs/TEMPLATE.md`: cohort cells (province, age band, sex, education), equations of ADRs 0032 and 0033, migration of 0109 and 0074, bounds, test plan from this plan.
- [ ] RFC for O1 with the tolerances of gate item 1, UN WPP and DESA series named, and the register entries (P62, P63 and those of 0105, 0109, 0031, 0074, 0167 as they are today in the register) checked against `spec/rows.toml`.
- [ ] Commit `docs(pop): p4 spec and accepted p4 adrs`.

## Branch F: `p4/moving-reference` (ADR 0243, #211)

### Task F1: Trend inputs and the moving GE reference

**Interfaces:** per-country monthly trends (n_i from ILOSTAT labour-force growth 1995-2005, g_i = 0 until P8, pi*_i = 0 until P5a) in the pack; `ceteris-reference` solves the frictionless equilibrium along the trend path.

- [ ] Failing tests: `zero_trends_equal_the_steady_state_calibration` (every golden holds); `common_trend_moves_no_detrended_quantity` (120 ticks, 1e-9 on the fixture); `country_trends_stay_within_one_percent_of_the_moving_reference` (to year 25 where the uniqueness condition of ADR 0022 holds). Implement, commit.

### Task F2: Stage 5 state and the wage rule

- [ ] Pipelines and stocks carry each country's trend; the wage step becomes pi^e + g_i. Failing test: `trend_wage_rule_holds_wages_on_trend`. Commit.

### Task F3: Full-width money (ADR 0106, #211)

- [ ] Retrofit the ledger to full-width accounts where the trend path overflows i64; failing test `ledger_overflow_is_an_error_not_a_wrap` first. Commit.

## Branch A: `p4/demography` (ADR 0032, #209)

### Task A1: Cohort cells and ageing

**Interfaces:** pop state by (province, age band, sex, education), integers; ageing shifts bands each year; the 2000 initial state from the Wittgenstein reconstruction (`wic-reconstruction`) and UN WPP.

- [ ] Failing tests: `population_is_conserved_by_cohort_cell` (births, deaths and moves net to zero); `ageing_shifts_bands_once_per_year`. Implement, commit.

### Task A2: Brass mortality and fertility with tempo

- [ ] Mortality: Brass level and slope by education from WCDE life tables. Fertility: quantum and tempo separate; age-specific schedule shifted by tempo (P63).
- [ ] Failing tests: `postponement_lowers_period_tfr_and_not_cohort_fertility` (Bongaarts and Feeney); `mortality_off_equals_status_quo`. Commit.

### Task A3: Education transitions and households

- [ ] Progression ratios for cohorts 15 to 34 respond to education spending and household income (P62); headship rates by age, sex and education.
- [ ] Failing test: `wcde_ssp2_nesting` (population by age, sex and education to 2100 within tolerance with WCDE inputs held fixed). Commit.

## Branch B: `p4/mortality-causes` (ADRs 0033, 0167)

- [ ] **B1** Cause-group Brass components (communicable, non-communicable, injuries) initialised from the cause shares of the research pack. Tests: `added_hazards_at_zero_equal_the_brass_level`; `cause_shares_by_age_2000_2019_within_tolerance`.
- [~] **B2** (kernel in `disease.rs`; prevalence step and excess deaths are in the demography system behind `EconWorld::with_disease`, off by default, snapshot section after demography; `disease_off_equals_status_quo_and_on_adds_deaths` passes in `world.rs`; the labour-factor feed, the WPP-band test and the initial prevalence wait on WPP data) Endemic disease state (HIV, malaria, TB prevalence by province and age band) with ART, bednet and TB coverage levers funded from the health budget. Tests: `zero_prevalence_equals_the_cause_group_path`; `hiv_life_expectancy_falls_and_recovers_in_the_wpp_band` (Botswana, South Africa, Zimbabwe, Malawi, historical coverage fed); `art_scale_up_raises_labour_supply_in_a_toy_province`.

## Branch C: `p4/skills` (ADR 0105)

- [~] **C1** (kernel in `skills.rs`, not in the tick; waits on the (province, sector, skill) wage state, the per-country premium in the pack and register codes for s and alpha) Two skill groups with a CES (s = 1.6, alpha calibrated per country to the 2000 skill premium); wage state widens to (province, sector, skill). Test: `skills_off_equals_status_quo`; `skill_premium_matches_2000_data`.
- [~] **C2** (kernel in `sectors.rs`, not in the tick; waits on the (province, sector, skill) wage state and register codes for m and omega_s) Sector switching by logit with monthly share m and dispersion omega_s from the literature. Tests: `switching_off_equals_status_quo`; `import_shock_leaves_persistent_local_losses` (China-shock sign).

## Branch D: `p4/migration` (ADRs 0109, 0074)

- [ ] **D1** Internal migration: dynamic discrete choice over provinces on real wages net of bilateral costs. Acceptance (#210): 2000-2010 bilateral stocks within tolerance.
- [ ] **D2** International admission caps Q_d, proportional rationing with diaspora weights, irregular share z. Tests: `infinite_caps_equal_the_pure_logit`; `admissions_never_exceed_the_cap_and_shares_sum_to_one`; PPML pseudo-R2 against Abel and Cohen reported.
- [~] **D3** (kernel in `settlement.rs`, in the tick as system 229 behind `EconWorld::with_settlement`, off by default, snapshot section after migration; `settlement_off_equals_status_quo` passes in `world.rs`; the out-of-sample urban-share and Zipf tests wait on GHSL class shares) Settlement classes per province (rural, town, city), urban growth with agglomeration and congestion, slum share. Tests: `settlements_off_equals_status_quo`; `urban_shares_2000_2020_out_of_sample_after_2010`; `city_sizes_stay_close_to_zipf_in_ensembles`.

## Branch E: `p4/structural-change` (ADR 0031)

- [~] (kernels in `structural.rs`, not in the tick; waits on ADR 0108's catch-up term, the pack's enforcement capacity and register codes P162, P163; the 2000-2024 cross-section test waits on ETD/GGDC data) Sector-class catch-up factor and informal-formal transitions scaled by enforcement capacity (register entries as in ADR 0031, provisional P60 and P61; codes from `next-code P`). Tests: `class_factors_one_equal_status_quo`; `manufacturing_converges_while_aggregate_does_not_2000_2024`; `enforcement_raises_formal_employment_in_a_toy_state`. Depends on ADR 0108's catch-up term (#173), so this branch waits for it or builds the factor with catch-up fixed at zero.

## Branch G: `p4/distance-friction` (ADR 0272, #212)

- [ ] Log-distance crossing factor D_AB,c beside nu, distance from the pack at load (population-weighted great circle), the elasticity epsilon (provisional P158 in ADR 0272; the code comes from `next-code P` in the PR that writes it). Needs the 50-goods pack (#206) for the recalibration.
- [ ] Failing tests: `distance_off_equals_status_quo`; `distw_matches_cepii_2000`; `distance_loss_is_tallied_apart_from_border_loss`. Acceptance: distance elasticity and Brexit magnitudes move toward the literature, the tier-2 rows of C7 removed.

## Branch H: gate

- [ ] UN 2000-2010 comparison table (fertility, mortality, urbanization), conservation over 100 years, budgets against 19.2, result recorded in `tests/perf/p4-gate.md` and the spec. Mac and the full pack.

## Dependencies

F before A (testing against the reference); A before B, C, D, E; D3 after D1; G after #206; H last. Branches B, C, D, E are independent of each other once A has merged.
