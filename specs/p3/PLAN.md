# P3 Implementation Plan: the counterfactual workbench

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the P2 economy as left by pre-P3 into the workbench of DESIGN 14 and 21.1 item 4: branches, the interventions of 14.2, common random numbers, ensembles on lanes, effect estimation, difference maps and fan charts, the `ceteris` CLI with scenario files and Arrow exports, run hashes, the replay loader and calendar-blind runs (ADR 0241), and the performance gates P3 owns (ADRs 0234, 0235, 0236). 1.0 is a pure simulator (ADRs 0239, 0240): nothing here is a seat, score, objective, narration or flavour.

**Gate (DESIGN 21.1 item 4, ADRs 0234, 0241):**

1. A null intervention reproduces the parent bit for bit, at 1, 3 and all threads.
2. Relabelling the start year (2000 to 2100) of the same pack leaves the golden hash unchanged apart from labels.
3. A tariff intervention (US +25% on manufactured imports from China at month 24, full pack) yields paired effects whose intervals under CRN are tighter than under independent seeds, on the declared outcomes (US and China manufacturing output, bilateral flow, US manufactured price index).
4. The 0234 flatness gate: median tick time over years 96 to 100 within 25% of years 1 to 5, decade row counts within each table's 0236 bound.
5. DESIGN 21.2 done: invariants over 100 years, budgets (19.2 as restated by 0235), the 20% regression gate.

**Starts from:** main after pre-P3 ends (B8 and C6 recalibrated, D1 and D2 snapshots across interventions and parameters as state, E lanes with `const L`, the benchmark rerun passed). The native app shell (ADR 0088 stages 1 and 2, `specs/app`) is a separate track; the P3 app tasks (Branch W) start once its exit criteria pass.

**Spec:** new `specs/workbench/SPEC.md` (state, edit vocabulary, store, invariants, bounds), `specs/core/SPEC.md` (hazard draws, chunk store), `specs/econ/SPEC.md` and `specs/trade/SPEC.md` (edit targets, ADR 0027), `specs/app/SPEC.md` (P3 views). DESIGN 14, 19.2, 19.4, 21; ADRs 0002, 0024, 0027, 0049, 0087, 0116, 0128, 0137, 0143, 0155, 0188, 0197, 0207, 0211, 0212, 0234, 0235, 0236, 0241.

**Out of P3:** everything ADR 0207 moved to 1.x or 2.0 (0078, 0079, 0118 to 0121, 0124, 0125, 0140, 0141, 0144, 0157, 0159, 0160, 0196); 0242's hazards (P5b, P8; P3 builds only the draw primitive they register with); predictability horizons (14.6) and Sobol (14.7), which need an emulator (P9); writes and lockstep in `ceteris serve` (1.x, ADR 0197); 0128's annual report, engine crate and wasm (after 1.0).

## Decisions (question cards, answered 2026-10-30)

The owner chose the recommended option of each.

| # | Question | Recommended | Tasks affected |
|---|---|---|---|
| Q1 | Where does ensemble spread come from at P3? The P2 tick draws no random numbers, so K seeds of the same branch give K identical runs and gate 3 cannot pass. | Decided (owner, 2026-10-30): each ensemble member draws the uncertain parameters from their register priors (theta by class, rho, nu, the price-rule gains); CRN means a branch and its baseline share the draw. New ADR. | C1, C2, J2 |
| Q2 | How much UI is in P3? 0207 keeps 0049, 0137, 0143, 0155, 0188 and 0197 lean in P3 at 4 to 6 owner-weeks; their own ADRs cost 15 to 25 part-time weeks. | Decided (owner, 2026-10-30): P3 ships the workbench views the gate needs (W1 to W4); the rest of 0155, 0188 and 0197 becomes a parallel lab-UI track after the gate. ADR amending 0207's table. | W5, W6, F4 |
| Q3 | ADR 0027 short-run trade adjustment: default on or off? | Decided (owner, 2026-10-30): fit rho from BACI local projections in P3; on by default if its falsification test passes, otherwise off (S12 instant), as with the fuels friction ruling of 0247. | E1, E2 |

## Controller rulings (cost if wrong in brackets)

- **State hashes stay over the canonical bytes of ADR 0002.** Arrow chunks become the storage format only, so moving to them re-blesses nothing; 0002's "re-bless when snapshots move to Arrow" does not happen. [one re-bless if the owner wants hashes over Arrow]
- **Scenario files are TOML.** Plain text and diffable (0211), and the CLI already reads TOML configs. [a converter, if JSON is preferred]
- **Engine version in the run hash** is the crate version plus the git commit of the build; a dirty tree gets a `+dirty` suffix and its runs are not shareable. [a build-id scheme change]
- **Edit targets at P3 are what P2 has:** parameters, tariffs and trade wedges, network edges, enterprise productivity, stocks and pop counts per cell. Event injection and decision override exist in the vocabulary with no targets until P5 and P6; the parser rejects them with a named error. [none]
- **Commitments in force at t0 (0241 D2):** the mechanism (a state table of scheduled edits with a completion hazard) lands in P3 with an empty table; tariff phase-ins and other content are pack work for P9's hindcast. [D2 content moves earlier if P5a needs it]
- **0143 at P3 is English short names for the 2000 states** from the existing countries table; borders are fixed in 1.x, so names by date have nothing to do until P6. [none]

## Global constraints

Pre-P3's constraints carry over: determinism at 1, 3 and all threads with `fixed_sum`; no `HashMap` reaching state; exact money; goldens change only in a commit whose subject says re-bless and why; `CETERIS_PACK` absolute; Conventional Commits, no em dashes, comments only for counterintuitive logic, no task numbers in code; code PRs need the owner's review. Also:

- **Edits are the only causal record.** Every change to a run after t0 is an edit with a tick, a source (`user`, `cli`, `api`, `replay`, `commitment`) and a god flag (14.2). Nothing else writes state between ticks.
- **Calendar-blind.** No system reads a calendar date; month of year, ages and durations are state (0241). The pack loader asserts it.
- **Bounds.** Every new table states its bound and expected size at 2150 in its SPEC (0236), including the edit log, branch tree and chunk store.
- **Register.** New parameters take IDs from the next free P number on main (P150 at the time of writing; pre-P3 may take more). P53 and S12 are 0027's.
- **ADR numbers** are taken from main right before pushing; 0247 is the P3-prerequisite thread's.

---

## Branch 0: `p3/records` (before code)

### Task 0.1: Specs and status

- [ ] Write `specs/workbench/SPEC.md` from `specs/TEMPLATE.md`: objects of 14.1 with the file family of 0211; edit vocabulary and ordering (tick, source, sequence); store layout; invariants (null edit, replay equality, edit-log ordering); bounds per table; test plan (this plan's tests). Update `specs/core/SPEC.md` (hazard draws, chunk store) and `specs/app/SPEC.md` (P3 views per Q2).
- [ ] Mark 0234, 0235 and 0236 accepted (their options were chosen 2026-10-29) and apply their migrations: DESIGN 19.2 gains the disk row (0212) and per-good lines (0235, values after G2), 19.4 the bounds rule (0236), 21.2 item 8 the flatness gate (0234). Apply 0197's DESIGN migration (2.1, 14.8, 21.1 P3 line). Add the ADRs Q1 and Q2 call for.
- [ ] Commit `docs(workbench): p3 spec and accepted p3 gates`.

## Branch A: `p3/store` (scenario files, run hashes, chunk store, retention)

### Task A1: Scenario file and run hash (0211, 0128)

**Files:** new crate `crates/ceteris-run` (scenario model, parser, run hash, branch tree), `crates/ceteris-cli/src/main.rs`.

**Interfaces:** `Scenario { pack_hash, engine, params, replay: ReplaySel, seeds, branches: Vec<Branch> }`; `Branch { id, parent: Option<BranchId>, fork_tick, edits: Vec<Edit> }`; `run_hash(&Scenario, branch, seed) -> Hash`. Branch ids are names chosen by the user, unique per file.

- [ ] Failing tests: `scenario_roundtrip_is_byte_stable` (parse, write, parse); `run_hash_changes_with_any_field`; `run_hash_ignores_comments_and_key_order`; `unknown_edit_kind_is_named_error`. Implement, commit.

### Task A2: Content-addressed Arrow chunk store (14.3, 0002)

**Files:** `crates/ceteris-core/src/snapshot.rs` (keep canonical encoding and `state_hash`), new `crates/ceteris-run/src/store.rs`.

**Interfaces:** a snapshot is a manifest of (table, chunk hash) over zstd Arrow IPC chunks of at most 64k rows; chunks are shared across branches and lanes; the store is a directory keyed by run hash (0211: a cache, deletable). `restore` decodes chunks back to the canonical state and checks `state_hash`.

- [ ] Failing tests: `chunk_store_restore_equals_canonical`; `unchanged_tables_share_chunks_across_branches` (a tariff edit rewrites only the tariff and downstream chunks); `cache_from_another_engine_is_decode_error` (0211); `deleting_the_cache_and_replaying_reproduces_hashes`. Implement, commit.

### Task A3: Retention tiers and the disk benchmark (0212)

- [ ] Live branch: monthly history for the most recent 25 simulated years, annual before; other branches and members: declared outcomes monthly plus snapshots (every 12 ticks for 25 years, then every 60); other series recomputed from the nearest snapshot on demand.
- [ ] Failing tests: `recomputed_series_equals_dense_series` (bit for bit); `retention_drops_monthly_after_25_years`. Benchmark: bytes per tick per tier on the full pack; one run to 2150 and an ensemble of 32 to 2050 within 20 GB and 50 GB, or the measured numbers replace 0212's. Commit.

## Branch B: `p3/branches` (edits, forks, version-control verbs)

### Task B1: The edit log (14.2)

**Files:** `crates/ceteris-run/src/edit.rs`, `crates/ceteris-econ/src/world.rs` (`apply_edit`).

**Interfaces:** `Edit { tick, source, god: bool, kind: EditKind }`; `EditKind::{Param, State, Policy, Event, Override, Replay, Switch}` with typed targets; edits apply at the tick boundary in (tick, source, sequence) order; a structural parameter edit rebuilds the kernel (pre-P3 D2); a network edit uses `set_network` (pre-P3 D1). Policy edits (tariffs, wedges) are in-world; state edits are god mode; the log marks the difference.

- [ ] Failing tests: per edit kind with a P2 target, `edit_applies_at_its_tick`; `edits_at_one_tick_apply_in_source_then_sequence_order`; `switch_edit_after_t0_is_rejected` (switches are ex ante); `event_and_override_have_no_targets_yet`. Implement, commit.

### Task B2: Branches, forks and the null intervention (14.1, 14.3)

- [ ] `fork(parent, tick)` loads the nearest earlier snapshot and replays; a child shares the parent's chunks up to the fork.
- [ ] Failing tests: `null_intervention_reproduces_parent` (in CI, toy world and P2 fixture; full pack in the nightly gate); `fork_at_any_tick_equals_continuous_run`; `child_of_child_replays_from_root`. Implement, commit.

### Task B3: Edits by query (0155)

- [ ] A predicate over province columns (`port && food_deficit > 0`) stored with the edit and re-evaluated on replay at the edit's tick, so the edit list stays the only causal record. The predicate grammar is 0188's query language at its smallest (comparisons, and, or, not, column names from the run's tables).
- [ ] Failing tests: `edit_by_query_replays_to_same_cells_on_replay`; `query_names_unknown_column_error`. Commit.

### Task B4: Rebase, cherry-pick and bisect (0116)

- [ ] `ceteris rebase`, `cherry-pick`, `bisect` on branches; bisect probes are ensemble runs cached by (run hash, edit subset). Edits whose target is absent in the new parent are reported and skipped.
- [ ] Failing tests (0116): `bisect_finds_planted_edit_in_log2_probes` (20 edits, at most ceil(log2 20) + 1 probes); `rebase_onto_own_parent_reproduces`; `cherry_pick_all_onto_parent_reproduces`. Commit. `blame` lands in C4.

## Branch C: `p3/ensembles` (lanes, CRN, effects, attribution)

### Task C1: Ensembles on lanes with member draws (0024, Q1)

**Interfaces:** `Ensemble { branch, k }` runs in lane blocks of `L` (8 to 32, set by pre-P3 E's benchmark); member k draws its uncertain parameters from their register priors with streams keyed by (seed k, `SystemId::PARAM_DRAW`, parameter id), applied as tick-0 parameter edits from the calibrated steady state (pre-P3 D2's path), so the draw is in the log. Members of a branch and its baseline with the same k share draws. The set of drawn parameters and their priors is a table in the workbench SPEC.

- [ ] Failing tests: `lane_k_equals_scalar_run_with_seed_k` at every snapshot; `crn_pair_in_two_lanes_equals_two_scalar_runs`; `member_draws_are_shared_by_branch_and_baseline`; `k_identical_seeds_without_draws_give_identical_members` (documents why Q1 matters). Implement, commit.

### Task C2: Hazard draws by thinning (0241)

**Files:** `crates/ceteris-core/src/hazard.rs`.

**Interfaces:** `HazardId` registry; `draw(seed, tick, hazard, cell) -> f64` gives one uniform per (hazard, cell, tick) whether or not the rate is positive; an event fires when the uniform is below the rate. The commitments table of 0241 D2 is the first user (completion hazard, zero rate by default). P5b and P8 hazards register here.

- [ ] Failing tests: `branches_share_events_of_untouched_hazards` (0241); `zero_rate_hazards_leave_hashes_unchanged` (0242); `commitment_with_zero_hazard_executes_at_offset` (0241). Implement on the toy world and the commitments table, commit.

### Task C3: Paired effects and fan charts (14.5)

- [ ] Paired effect per seed on declared outcomes; mean with bootstrap intervals (resampling keyed by the counter RNG, so intervals are deterministic), quantiles, event probabilities; per-province mean difference with a mask where the sign is not robust across seeds (share of seeds agreeing with the mean's sign below a threshold, a display setting, not a register parameter).
- [ ] Failing tests: `paired_effect_of_null_edit_is_exactly_zero`; `bootstrap_is_deterministic`; `crn_intervals_tighter_than_independent_on_fixture` (fixture tariff with Q1 draws). Commit.

### Task C4: Channel shutdown, Shapley and blame (14.7, 0049, 0116)

- [ ] Channels that exist at P3: prices (frozen at baseline per market), trade shares (gravity kernel and sourcing frozen), wages. Shapley over interventions, exact up to five, sampled beyond. `ceteris explain <var> <province> <window>` runs the shutdown set under CRN; `ceteris blame` ranks the edits and replay entries that moved a series most.
- [ ] Equilibrium comparison (14.7): for a tariff branch, solve the counterfactual equilibrium in `ceteris-reference` from the same initial data and report the gap to the simulated long-run effect.
- [ ] Failing tests: `channel_shares_sum_to_total_within_shapley_tolerance` (0049); `shutting_every_channel_gives_zero_effect`; `blame_ranks_planted_edit_first`. Commit.

## Branch D: `p3/calendar-blind` (ADR 0241)

### Task D1: Year relabel and the loader assertion

- [ ] The start year is a label in the scenario and in output only. The pack loader fails on any table read after t0 that is keyed by calendar date.
- [ ] Failing tests: `year_relabel_2000_to_2100_keeps_golden_hash` (CI, fixture and toy; full pack nightly); `date_keyed_table_after_t0_is_load_error`. Commit.

### Task D2: The replay loader

- [ ] Tape tables keep their format; the reader emits `Edit { source: replay, .. }` by family (`shocks`, `policy`) for S07 `off | shocks | policy | all`. At P3 the only family with a target is policy (tariffs and wedges); the loader is tested on a small fixture tape. Building the 2000 to 2024 tariff tape from the WTO IDB (already in `sources.toml`) is a P9 data task.
- [ ] Failing tests: `replay_all_reproduces_tape_lever_paths` (0210's tape test on the fixture tape); `replay_off_reads_no_tape`; `replay_edits_are_logged_like_user_edits`. Commit.

## Branch E: `p3/short-run-trade` (ADR 0027, Q3)

### Task E1: Lagged gateway kernel

- [ ] Per (theta class, gateway pair) lagged kernel and per (origin, good) smoothed price term in tick state and snapshots; switch S12 (instant or partial), register P53 (rho by class, prior half-life 12 to 48 months). Short-run order elasticity rho theta + 1 in ADR 0090's E_o; the stability region re-derived with rho (amendment 0095).
- [ ] Failing tests (0027): `rho_one_equals_instant_hashes`; `tariff_elasticity_path_matches_closed_form` (toy world); steady state unchanged at any rho (V1). Commit.

### Task E2: Fit and falsification

- [ ] `ceteris calibrate rho`: local projections of bilateral flows around the 2018-19 tariff changes in BACI, by class, outside SBI. Falsification test: simulated elasticity at 1 year below 1 and at 7 to 10 years within the Boehm, Levchenko and Pandalai-Nayar (2023) band. Pass: S12 on by default, re-bless with the reason. Fail: S12 instant by default, recorded in the register. Commit.

## Branch F: `p3/cli-and-exports`

### Task F1: The `ceteris` verbs

- [ ] `ceteris new | fork | edit | run | ensemble | diff | effects | explain | blame | bisect | rebase | cherry-pick | share | fetch`, each reading and writing the scenario file; names and arguments come from one command registry (0188) that the app and `serve` reuse.
- [ ] Failing tests: an end-to-end script (new, fork, edit, ensemble, effects) on the fixture writes the expected Arrow tables and a stable run hash. Commit.

### Task F2: Arrow and Parquet outputs

- [ ] Declared outcomes per (branch, member, tick), effects, quantiles, masks and channel shares as Arrow IPC and Parquet; schema listed in the workbench SPEC; reads with Polars and DuckDB in a doc example (no Python in tests).
- [ ] 0087's observable tables (night lights, NO2, built-up, shipping density, flaring, greenness) in the schema with no operator yet; shipping density from sea-lane flows is the one operator P2 state supports and lands here. Commit.

### Task F3: Share and fetch (0128)

- [ ] `ceteris share` prints the run hash; `ceteris fetch <hash>` recomputes from the scenario file or pulls chunks from a directory or URL mirror. Failing test: `fetch_recomputes_state_hashes`. Commit.

### Task F4: Read-only `ceteris serve` (0197; lab-UI track after the gate, per Q2)

- [ ] Localhost socket, token file with a read scope, JSON-RPC from the registry, tables as Arrow IPC streams, per-tick subscriptions; `ceteris api schema`. Failing tests: `serve_reads_do_not_change_state_hash`; `non_loopback_bind_is_refused`. Commit.

## Branch G: `p3/perf-gates` (ADRs 0234, 0235, 0236)

Independent of B to F; can start the day P3 starts.

### Task G1: The flatness benchmark (0234)

- [ ] `benches/flatness`: release build, 100 years, full pack, default scenario; report median tick time over years 1 to 5 and 96 to 100, their ratio, row counts per table per decade, peak resident memory; checked-in report under `tests/perf/`. CI runs 10 years for row counts only.
- [ ] Failing test: a synthetic table appending one row per tick in a test build fails the gate. Commit.

### Task G2: Synthetic 45-goods pack and per-good budget lines (0235)

- [x] Goods count a pack value: `Goods` read from `econ_goods`, fixed-size goods arrays replaced by vectors and `Rows`; the 10-goods fixture and pack hash unchanged. The pack builder's 10-good aggregation and the P04 gravity estimation stay at P2's goods until the 50-goods pack (ADR 0271).

- [ ] Generator: the 10 real goods' parameters resampled across theta classes, IO coefficients scaled to keep value added, on the full province set; benchmark fixture only, never a calibration input.
- [ ] Benchmarks at 10, 20 and 45 synthetic goods fit fixed plus per-good cost per goods-indexed system; DESIGN 19.2 restated with formula and value at 45 goods. If 45 goods exceed 160 ms, levers in order: gateway count and crossings (0092), the dispatch plan (0097), only then the ceiling by new ADR.
- [ ] The synthetic pack runs 100 years without an invariant violation. Commit.

### Task G3: Bounds on P3's own tables (0236)

- [ ] Edit log, branch tree, chunk store, history tiers and member draws each state a bound and its 2150 size in the workbench SPEC; the G1 report checks them. Commit.

## Branch H: `p3/reproducible`

- [ ] Pinned toolchain and dependencies (rust-toolchain, `Cargo.lock`, a Nix flake or equivalent); a cross-platform golden test on macOS (self-hosted runner of pre-P3 D5), Linux and Windows. Failing test first: the golden test job. Commit `build: reproducible builds and cross-platform goldens (adr 0128)`.

## Branch W: `p3/workbench-views` (native app, after the shell's exit criteria)

W1 to W4 are the views the gate names. W5 and W6 belong to the lab-UI track after the gate (Q2), listed here so the track starts from them.

- [ ] **W1 Branch manager and edit log.** Tree of branches, fork at the cursor, step back (seek), run to tick, the edit log with source and god flag; every action through the registry (0188). Test: a headless run and an app run of the same scenario give the same run hash.
- [ ] **W2 Difference map.** Any map mode as branch minus baseline per province, diverging ramp passing the 0221 check, sign-robustness mask hatched; picking shows the paired interval.
- [ ] **W3 Fan charts.** Any series for a branch and its baseline as quantile bands across members, paired-effect panel beside it; 0137 chart tokens.
- [ ] **W4 Why panel (0049).** For a selected variable, province and window, the C4 channel shares with intervals, run as a background job.
- [ ] **W5 Lenses (0155, P3 part).** Route inspector with "close this edge" forking a branch, bottleneck lens, map numbers, peer radar.
- [ ] **W6 Keyboard (0188, P3 part).** Palette, hints, query bar for edits by query; exports (Ctrl+C TSV, xlsx, GeoPackage, SVG and PNG, `ceteris://` links, 0197).

## Branch J: `p3/gate`

### Task J1: Golden scenario and 100-year run

- [ ] `tests/golden/p3-workbench.txt`: the fixture scenario with a fork, a tariff edit, an edit by query and a 4-member ensemble; state hashes per branch and member. Full pack: 100 years, invariants hold, budgets met.

### Task J2: The tariff experiment

- [ ] Full pack, K = 32: baseline and a branch with US +25% on manufactured imports from China at month 24. Paired effects under CRN and under independent seeds (members of the branch take seeds K+1 to 2K); CRN intervals narrower on every declared outcome. Long-run effect compared with the `ceteris-reference` counterfactual (C4). Report in `tests/perf/p3-gate.md`.

### Task J3: Close

- [ ] Gate items 1 to 5 checked and recorded; 0234 report checked in; `CHANGELOG.md` entries; SPECs as built; ADR statuses (0027, 0116, 0128, 0211, 0212 accepted as built or amended); `spec-reviewer` against `specs/workbench/SPEC.md`.

---

## Order and parallelism

```
0 ── A ── B ── C ── J
     │    └─ D    │
     └─ F1-F3 ───┘
G, H, E: from day one, in parallel
W1-W4: after the app shell exits and C3 lands
```

- 0 first (records, no code). A then B then C in order, each PR reviewed before the next starts from main; a waiting PR may have the next stacked on it.
- D starts after B1 (replay emits edits). F1 to F3 follow A and grow with each branch's verbs.
- G, H and E touch nothing in A to D and run alongside. E's re-bless (if S12 goes on) lands before J's goldens.
- W needs the shell's exit and C3's effect tables; W1 can start after B2.

## Estimate

Owner-weeks of attention with agents coding, anchored like ADR 0207's: Branch 0 0.5; A 1 to 1.5; B 1 to 1.5; C 1.5 to 2.5; D 0.5; E 0.5 to 1; F1 to F3 0.5 to 1; G 1 to 1.5; H 0.5; W1 to W4 1.5 to 2.5; J 0.5 to 1. Total 9 to 14 owner-weeks on the recommended answers, against 0207's 4 to 6. The gap is mostly work 0207 did not count: ADRs 0234 to 0236 (G), 0241 (D, C2), the C1 member draws, and the views the gate needs. W5, W6 and F4 (4 to 7 more) go to the lab-UI track after the gate (Q2). The roadmap re-estimate thread should take these numbers.

## Risks

- **Pre-P3 E slips.** C1 needs lanes; without them C1 runs members as processes (slower, same hashes) and the lane benchmark moves to C.
- **CI minutes.** The cross-platform job (H) and nightly full-pack gates run on the self-hosted runner where possible; Linux-hosted jobs stay on path filters.
- **Member draws move the baseline off its steady state.** Each member starts its transition at tick 0; outcomes are reported as paired differences, so the level drift cancels, but the 100-year invariant run must use the undrawn baseline too.
- **Chunk store size.** If A3's measurement misses 0212's budgets, snapshot intervals widen before retention tiers change.
