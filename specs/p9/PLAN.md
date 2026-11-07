# P9 Implementation Plan: calibration and hindcast

**Goal:** Make the model's claims checkable: pre-register the tests, calibrate with simulation-based inference and emulators in Rust, score out-of-sample against real-time vintages and hard benchmarks, and replicate natural experiments (spec/21 section 21.1 item 10; spec/15; ADRs 0052, 0053, 0065, 0096, 0107, 0111, 0113, 0241, 0242). This plan is a skeleton at the level of tasks and named tests; each branch's detailed plan is written when it starts, against the kernel signatures P8 leaves.

**Gate (spec/21 item 10):**

1. The out-of-sample scorecard (15.4) and the natural-experiment table (15.5) are published with CRPS and interval coverage.
2. Robustness across structural switches is documented.
3. The pre-registration was committed before the first hindcast run, and the sealed 2019-2024 score was computed once at this gate and recorded in the changelog (ADR 0107).
4. spec/21 section 21.2 done: invariants over 100 years, budgets (19.2), the 20% regression gate, the coupling register of ADR 0080.

**Starts from:** main after the P8 gate. Branches A, B and C need no P8 output and can start with the P8 tail; the hindcast (Branch G) needs the origin packs and the full 1.0 model.

**Spec:** new `specs/inference/SPEC.md` (derivatives, SBI, emulators, Sobol, scorer, hindcast protocol, bounds, test plan); spec/14, 15, 16, 17, 19, 21; ADRs 0052, 0053, 0065, 0096, 0107, 0108, 0111, 0113, 0241, 0242; RFC 0314 (held-out register and guard, merged).

**Out of P9:** the 2.0 politics-dependent scores; Steward mode; ADR 0207's 1.x and 2.0 items.

## Open points for the owner

| # | Question | Default if not answered |
|---|---|---|
| O1 | ADRs 0052, 0053, 0065, 0096, 0107 and 0111 are still `proposed`. Accept them as written for the 1.0 path in one status RFC? | Branch 0 writes that RFC with one decision per ADR; nothing is built before it is accepted |
| O2 | ADR 0065 ties statistical manipulation to autocracy, media freedom and election proximity, which are P6 state that 1.0 does not have. What does 1.0 build behind switch S21? | Lags, revisions and noise scaled by statistical capacity; the manipulation term and its register entries (P111, P112) wait for P6; S21 off by default |
| O3 | ADR 0111 asks for a rational-expectations storage switch; #227 says "implemented or re-deferred". Which? | Decide at Branch A from the cost of the `Dual` retrofit: re-defer if A pushes the 1.0 range past RFC 0313's upper bound |

## Global constraints

P3 to P8's constraints carry over (determinism at 1, 3 and all threads with `fixed_sum`; no `HashMap` reaching state; exact money; goldens change only in a commit whose subject says re-bless and why; register codes and decision ids from `cargo run -p ceteris-rows -- next-code P` and `-- next N`, only in the PR that writes them; calendar-blind). Also:

- **Pre-register first.** No tolerance, metric or target file changes after its first score; a miss goes to the owner with the table (RFC 0314, ADR 0107).
- **Leakage.** A source moves from held-out to calibration only by an RFC naming its replacement; sources sharing upstream with a calibration source are never cited as independent passes.
- **Sealed period.** 2019-2024 targets are readable only by the scorer under the sealed-gate flag, which writes a changelog entry.
- **Reproducible.** Inference outputs carry hashes that reproduce on a clean machine.
- **Bounds.** Every new table states its bound and size at 2150 in its SPEC (ADR 0236).

---

## Branch 0: `p9/records` (before code; #223, #242, #247)

- [ ] Write `specs/inference/SPEC.md` from `specs/TEMPLATE.md`.
- [ ] Pre-registration document committed before any hindcast run: the metric, tolerance and target file hash of every score; the moment and tolerance for each of kappa1, kappa2, kappa_w, rigidity and eta (Nakamura-Steinsson price-change frequencies, Phillips slope, Okun; #242); the leave-one-region-out protocol and tolerances (seven regions, spec 15.7; #247); the joint check of the block calibrations made at each gate.
- [ ] RFC for O1 with the register entries (P111 and P112 only if O2 changes) checked against `spec/rows.toml`.
- [ ] Commit `docs(inference): p9 spec and pre-registration`.

## Branch A: `p9/derivatives` (ADRs 0096, 0111; #227)

- [ ] **A1** The core generic over `Real` with `Dual<N>` forward mode for about 10 parameters. Failing tests: `f64_instantiation_reproduces_golden_hashes_bit_for_bit`; `dual_gradients_match_central_finite_differences_under_crn_to_1e_6` (P2 golden scenario). A criterion benchmark of `Dual<10>` against f64 sets the calibration-build budget in the spec.
- [ ] **A2** Rational-expectations storage switch (ADR 0111) per O3: implemented, or re-deferred with the reason in the SPEC. Test if built: `re_storage_off_equals_status_quo`.

## Branch B: `p9/inference` (ADRs 0053, 0113; #224)

- [ ] **B1** Simulation-based calibration (Talts et al.), learned and hand-picked summary statistics, misspecification-robust inference with an explicit discrepancy, ablation and switch choice by Bayesian stacking of holdout predictive densities. Tests: `sbc_on_a_toy_model_with_known_posterior_has_uniform_ranks`; `causal_validation_runner_recovers_a_planted_effect_in_a_toy_panel`.
- [ ] **B2** Emulators of the simulator and Sobol indices in Rust, in tree. Tests: `emulator_reproduces_held_out_simulator_outputs_within_tolerance`; `sobol_indices_match_the_analytic_values_of_a_toy_function`; outputs reproduce their hashes (acceptance of #224).
- [ ] **B3** Hazard fits outside the simulation (ADR 0242): the Marani frequency-intensity curve and the IRR regime-transition panel since 1970, fitted offline into the pack. Test: `fitted_hazards_reproduce_their_published_benchmarks_within_se`.

## Branch C: `p9/scorer` (ADRs 0052, 0107; #246)

- [ ] **C1** The scorer crate: the only reader of sources registered `use = "validation"` (the register and the check-sources guard exist, RFC 0314); real-time vintages (ALFRED, OECD Economic Outlook, IMF WEO) for each rolling origin; benchmarks (random walk, AR(1), small VAR, IMF WEO, a combination, Wiese et al. forecasts where they overlap, BeforeIT.jl for Austria and Italy run separately and never embedded because it is AGPL, ViEWS for conflict); metrics CRPS, log score, interval coverage, mean interval score, PIT.
- [ ] **C2** Failing tests: `scorer_reproduces_published_views_benchmark_scores_on_the_challenge_data`; `a_2010_origin_forecast_reads_only_data_released_by_then`; `scorer_refuses_2019_2024_targets_without_the_sealed_gate_flag_and_the_flag_writes_a_changelog_entry`; `bernoulli_toy_at_k_32_reports_a_standard_error_within_10_percent`.

## Branch D: `p9/origin-packs` (#226)

- [ ] Packs for 2005 and 2010 build with the same code and recorded hashes (ADR 0241: only initial conditions differ). Tests: `year_relabel_keeps_the_golden_hash`; `no_pack_table_read_after_t0_is_keyed_by_calendar_date` (asserted by the loader). Needs the Mac and the data fetches.

## Branch E: `p9/calibration` (#241, #242, the joint block check)

- [ ] **E1** PPML thetas 10.9 to 14.1 against the class spread (#241): cause found in data, estimator or class definition, or recorded as a tier-2 miss with a removal phase.
- [ ] **E2** Price and wage speeds calibrated to the pre-registered moments (#242). Test: `each_speed_parameter_hits_its_moment_within_the_pre_registered_tolerance`.
- [ ] **E3** The joint check of the block calibrations made at each gate: the blocks calibrated separately are run together and the pre-registered moments re-scored. Test: `joint_block_check_reports_every_moment_and_fails_on_a_planted_break`.

## Branch H: `p9/reported-statistics` (ADR 0065)

- [ ] Statistical offices publish the national accounts, inflation, unemployment and population from the true state with lags, revisions and noise scaled by statistical capacity (World Bank SPI), behind switch S21, per O2. Tests: `s21_off_equals_status_quo`; `published_series_lag_and_revise_with_statistical_capacity`; the Martinez night-lights regression test waits for the manipulation term (P6).

## Branch G: `p9/hindcast` (#225, #247)

- [ ] **G1** Hindcast 2011-2024 out of sample from the 2005 and 2010 origins, replay off and on (ADR 0241). Selection and ablation use 2011-2018 only. Tests: `replay_all_reproduces_observed_lever_paths_exactly`; `two_branches_differing_in_an_edit_that_misses_a_hazard_share_every_event`.
- [ ] **G2** Natural-experiment table (15.5): replication of the declared experiments with the paired-effect machinery of P3.
- [ ] **G3** Leave-one-region-out validation per the pre-registered protocol (#247).
- [ ] **G4** Scorecard with CRPS and coverage; robustness across structural switches documented. Sealed 2019-2024 score computed once, at the gate. Mac and the full pack.

## Branch I: gate

- [ ] Scorecard, natural-experiment table, switch-robustness table, budgets against 19.2, coupling register complete, result recorded in `tests/perf/p9-gate.md`, the SPEC and the changelog.

## Dependencies

Branch 0 before any run of E, G or the scorer on real data; A, B and C independent of each other; B2 needs A1; D needs the data fetches (Mac); E needs Branch 0 and the full pack; G needs C, D, E and the full 1.0 model; H independent after C; I last.
