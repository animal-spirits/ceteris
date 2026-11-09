# P8 Implementation Plan: growth, energy and hazards

**Goal:** Give the economy a long-run engine and an energy system: frontier productivity growth with technology jumps and catch-up, learning curves, structural change, emissions, oil and gas markets, a power sector, technology diffusion under energy policy, and the endogenous pandemic and chokepoint hazards (spec/21 section 21.1 item 9; spec/13; ADRs 0028, 0034, 0035, 0051, 0108, 0242, 0278). The climate emulator, damages and endogenous infrastructure are minors after 1.0 (climate is 1.1, ADR 0207) and are out. This plan is a skeleton at the level of tasks and named tests; each branch's detailed plan is written when it starts, against the kernel signatures P5 leaves.

**Gate (spec/21 item 9):**

1. 2000-2010 growth, sector shares and emissions within tolerance (tolerances set in Branch 0).
2. Open-ended runs to 2150 stay numerically stable.
3. Pandemic and chokepoint events emerge from hazards (#222), and with every hazard rate at zero the golden hashes equal the hazard-free run.
4. spec/21 section 21.2 done: invariants over 100 years, budgets (19.2), the 20% regression gate, the coupling register of ADR 0080 for the modules implemented.

**Starts from:** main after the P5b gate. The 50-goods pack (#206) and the physical oil and gas split (ADR 0278 item 5, a split ADR 0271 leaves to its own ADR) must be in the pack before Branch B.

**Spec:** new `specs/growth/SPEC.md` (frontier, catch-up, jumps, estimation windows) and `specs/energy/SPEC.md` (zones, plants, oil and gas, power sector, diffusion, policy, emissions); `specs/core/SPEC.md` (hazard library); spec/07, 13, 15, 17, 19, 21; ADRs 0028, 0031 (the sector-class factor on the catch-up rate), 0034, 0035, 0051, 0103, 0108, 0242, 0278.

**Out of P8:** the climate emulator and damages (1.1); endogenous infrastructure; earthquakes and volcanoes unless the owner names them (O2); monetary union entry and exit; ADR 0207's 1.x and 2.0 items.

## Open points for the owner

| # | Question | Default if not answered |
|---|---|---|
| O1 | ADRs 0028, 0034, 0035, 0051 and 0108 are still `proposed` (0242 and 0278 are accepted). Accept them as written for the 1.0 path in one status RFC? | Branch 0 writes that RFC with one decision per ADR; nothing is built before it is accepted |
| O2 | Issue #254 reads "0028 crude-only scope confirmed", but the accepted ADR 0278 (2026-10-31) brings 0028's gas part into 1.0 (regional gas markets, pipelines and LNG). Which holds? | ADR 0278: crude oil and gas, as accepted; #254's line is stale |
| O3 | ADR 0051 also lists earthquakes and volcanoes; spec/21 names only pandemic and chokepoint hazards for P8. Include the others in 1.0? | Only pandemics and chokepoints; the rest are 1.x minors behind the library |

## Global constraints

P3 to P5's constraints carry over (determinism at 1, 3 and all threads with `fixed_sum`; no `HashMap` reaching state; exact money; goldens change only in a commit whose subject says re-bless and why; register codes and decision ids from `cargo run -p ceteris-rows -- next-code P` and `-- next N`, only in the PR that writes them; calendar-blind). Also:

- **Switches.** Every mechanism has a switch whose off position reproduces the status quo bit for bit; the test is named `<mechanism>_off_equals_status_quo` (S13 competitive, S15 fixed, S16 cost-minimizing for the three ADRs that define them).
- **Offline estimates.** Every estimate fitted outside the simulation reproduces a published benchmark on the same data within its standard error before it enters the pack (ADR 0108).
- **Hazards register once.** Every hazard registers with the 0051 library and draws by thinning on shared uniforms (ADR 0241, 0242).
- **Bounds.** Every new table states its bound and size at 2150 in its SPEC (ADR 0236).

---

## Branch 0: `p8/records` (before code)

- [ ] Write `specs/growth/SPEC.md` and `specs/energy/SPEC.md` from `specs/TEMPLATE.md`, with the tolerances of gate item 1 and the test plan from this plan.
- [ ] RFC for O1 with the register entries (P54, P66, P68, P32, P69 and those of 0051, 0108 as they are today) checked against `spec/rows.toml`; record O2 and O3 in it.
- [ ] Commit `docs(energy): p8 spec and accepted p8 adrs`.

## Branch A: `p8/growth` (ADR 0108; #220, #173)

### Task A1: Offline estimates

- [ ] Catch-up speed from PWT 1960-2019 depending on education and the V-Dem institution vector (two interaction parameters) by panel regression, outside the simulation, in the pack builder. Failing test: `catch_up_estimate_reproduces_the_published_benchmark_within_se`.

### Task A2: Frontier growth, technology jumps and the catch-up term

- [ ] Frontier productivity growth with technology jumps registered as hazards (ADR 0242 D6); catch-up term multiplied by the sector-class factor of ADR 0031 (P4); ADR 0103 drifts become the 1995-2000 residuals left after it, so convergence is counted once.
- [ ] Failing tests: `catch_up_off_equals_status_quo`; `drift_plus_catch_up_do_not_double_count_convergence`; `growth_2000_2010_and_sector_shares_within_tolerance` (reported at the branch, asserted at the gate).

## Branch B: `p8/oil-and-gas` (ADRs 0028, 0278)

- [ ] **B1** Physical split of B06 into crude oil and natural gas in the pack (the ADR 0271 split), with 2000 gas capacities from the Global Energy Monitor gas trackers.
- [ ] **B2** OPEC reaction rule (membership a state attribute, quarterly production on the price relative to a target band and OECD commercial inventories, capacity-bound), non-OPEC supply with short-run elasticity P54 and a long-run response through investment and depletion. Tests: `competitive_s13_equals_status_quo`; `supply_disruption_price_impact_falls_with_spare_capacity`; `oil_price_path_2000_2010_within_calibration_bands` (Baumeister-Hamilton decomposition as a pattern target, not a fit).
- [ ] **B3** Gas in regional markets linked by pipelines and LNG with capacity limits (liquefaction, regasification and pipeline throughput are capital stocks of ADR 0204; the pipeline mode of 5.3 is built). Tests: `pipeline_and_lng_flows_respect_capacity`; `gas_shock_2021_22_moves_power_prices_where_gas_is_marginal` (shared with Branch C).

## Branch C: `p8/power-sector` (ADRs 0034, 0278)

- [ ] **C1** Grid zones (a pack table; default a zone is a state), interconnector flows set before the zone pools clear, congestion rent to the interconnector owner. Test: `zone_pool_clears_without_intra_zone_friction`; `interconnector_flow_goes_to_the_higher_price_up_to_capacity`.
- [ ] **C2** Plant capacity in MW per province and class from the Global Energy Monitor trackers (start year up to 2000, placed by coordinates, the gap to national capacity spread over tracked plants); D's province VA follows capacity weighted by class capacity factor. Test: `good_7_unchanged_on_the_ten_goods_pack`; `province_capacity_sums_to_national_2000_capacity`.
- [ ] **C3** Residual load duration curves per zone (4 to 8 segments, about ten generic classes served in order of marginal cost including fuel and carbon prices; storage shifts energy within round-trip efficiency). Tests: `fixed_s15_equals_the_stub`; `rising_solar_share_lowers_capture_prices_and_raises_curtailment` (Ueckerdt et al. shape); `gas_shock_2021_22_moves_power_prices_where_gas_is_marginal`.

## Branch D: `p8/diffusion-and-policy` (ADR 0035)

- [ ] **D1** New-capacity shares by pairwise logistic substitution on levelized cost differences (P68) for power classes, road vehicles and industrial heat; the quarterly investment step allocates by these shares. Tests: `cost_minimizing_s16_equals_status_quo`; `solar_and_wind_growth_2000_2024_s_curve_timing_within_tolerance`.
- [ ] **D2** Stochastic experience curves (learning rate P32, noise P69 drawn per run on the shock tape). Test: `noise_reproduces_way_et_al_forecast_error_distribution_on_held_out_years`.
- [ ] **D3** Policy instruments on levelized costs and allowed choices: carbon prices, trading caps (EU ETS from 2005 through the replay set), CBAM on embodied emissions; carbon prices, caps and CBAM start at zero, fuel excises and subsidies are the per-unit wedges held at 2000 rates from P5 (ADR 0278). Test: `zero_carbon_policy_equals_status_quo`; `cbam_charges_embodied_emissions_of_imports_only`.

## Branch E: `p8/emissions-and-structure` (#221)

- [ ] Emissions accounting from fuel use, process and land channels; learning curves in energy feed back into costs; structural change reads the P4 sector-class factors. Tests: `emissions_2000_2010_match_within_tolerance`; `run_to_2150_stays_numerically_stable_with_ensembles` (gate item 2; Mac).

## Branch F: `p8/hazards` (ADRs 0051, 0242; #222)

- [ ] **F1** Hazard library: each hazard registers a frequency model, a severity model and pooling; next-event times and severities from the run's RNG streams recorded on the shock tape. Tests: `draws_reproduce_fitted_distributions_by_ks_on_1e6_draws`; `draws_are_deterministic_per_seed`; `ensemble_spread_widens_with_tail_index_uncertainty_by_the_analytic_amount`.
- [ ] **F2** Epidemics: arrival and intensity from the Marani et al. distribution, SEIR with endogenous behavioural distancing, vaccine arrival as a stochastic research lag, non-pharmaceutical interventions as policy choices, post-infection labour-supply hazard. Tests: `pandemic_frequency_intensity_matches_the_marani_curve_by_ks`; `pandemics_start_more_often_where_spillover_covariates_are_higher` (rank correlation with lambda_p).
- [ ] **F3** Chokepoint closures as hazards on the transport network (edge capacity to zero for a duration). Tests: `chokepoint_events_emerge_from_hazards`; `zero_rate_hazards_leave_hashes_unchanged`.

## Branch G: gate

- [ ] Growth, sector-share and emissions table against the tolerances, the 2150 stability run, budgets against 19.2, coupling register complete, result recorded in `tests/perf/p8-gate.md` and the SPEC. Mac and the full pack.

## Dependencies

B1 needs the 50-goods pack (#206) and the oil and gas split; A needs the P4 sector-class factor (ADR 0031) or builds the catch-up term with that factor at 1; B before C3 and D3 (fuel prices); C before D1 (power classes); D1 and D2 before E; F is independent after Branch 0 and shares the thinning primitive of P3; G last.
