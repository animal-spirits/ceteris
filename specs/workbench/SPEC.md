# Workbench SPEC

Status: draft
Phase: P3
Design sections: 14.1 to 14.5, 14.7, 14.8, 19.2, 19.4, 21.1 (P3), 21.2
ADRs: 0002, 0024, 0027, 0049, 0087, 0113, 0116, 0128, 0188, 0207, 0211, 0212, 0234, 0235, 0236, 0241, 0322, 0323
Plan: `specs/p3/PLAN.md`

## Purpose

The workbench owns everything that makes a run an experiment: scenarios, branches, the edit log,
forks and replay, the content-addressed run store and its retention, ensembles and common random
numbers, paired effects, attribution, the replay loader and the calendar-blind rule, the
`ceteris` verbs and their exports. It owns no economics: edits name targets that `ceteris-econ`
and `ceteris-trade` expose, and ensembles run the same tick. Event injection and decision
override are in the vocabulary with no targets until P5 and P6 (the parser rejects them with a
named error). The app's P3 views are in `specs/app/SPEC.md`. Crate: `ceteris-run` (new), with
`hazard.rs` and the snapshot encoding in `ceteris-core`.

## State components

| Component | Entity | Type | Units | Initialized from |
|---|---|---|---|---|
| Scenario | file | pack hash, engine id, parameter vector, replay selector, seeds, branches | n/a | scenario file (TOML) |
| Branch | scenario | id (user-chosen name, unique per file), parent, fork tick, edit list | n/a | scenario file |
| Edit | branch | tick, source, god flag, kind, target, value, sequence | tick, per target | scenario file, CLI, replay loader, commitment |
| Commitment | scenario | scheduled edit, offset, completion hazard (zero by default) | ticks, rate | scenario file; empty at P3 |
| Run | branch x seed | run hash | n/a | derived |
| Ensemble member | branch x k | seed k, tick-0 parameter draws as logged edits | n/a | register priors (0322) |
| Chunk | store | zstd piece of the canonical bytes, 64 KiB cuts, keyed by BLAKE3 of the raw bytes | n/a | snapshots |
| Manifest | snapshot | chunk hashes list, tick, state hash | n/a | snapshots |
| History tier | branch | monthly or annual series, declared outcomes | per series | run |

Files (ADR 0211): pack (immutable), scenario file (the save, plain text, the stable public
format), run hash = hash of (scenario file, pack hash, engine version), snapshots and run store
(caches keyed by run hash, deletable, recomputable). Engine version is the crate version plus the
build's git commit; a dirty tree appends `+dirty` and its runs are not shareable. Within a major
the scenario format and edit vocabulary change only additively.

## Edit vocabulary and ordering

`Edit { tick, source, god, kind }`, `source` in `user | cli | api | replay | commitment`,
`kind` in `Param | State | Policy | Event | Override | Replay | Switch`. Targets at P3: parameters,
tariffs and trade wedges, network edges, enterprise productivity, stocks and pop counts per cell.
`Switch` edits are ex ante only. Edits apply at the tick boundary, before any system runs, in
(tick, source, sequence) order; sequence is the order of insertion into the branch. A structural
parameter edit rebuilds the kernel (ADR 0293). An edit by query (0155, 0188) stores its predicate
and re-evaluates it at the edit's tick on replay, so the edit list stays the only causal record.
Nothing else writes state between ticks. In-world (`Policy`) edits are distinguished from god-mode
edits in the log (14.2).

## Store and retention

A snapshot is a manifest of chunk hashes. Chunks are shared across branches and lanes;
an edit rewrites only the chunks its changed bytes fall in (ADR 0326). State hashes stay over
the canonical bytes of ADR 0002: chunks are a storage format only, so no golden re-blesses when
snapshots move to chunks. `restore` decodes chunks to the canonical state and checks the
state hash. A chunk written by another engine version is a `DecodeError` ("cache from another
version"); stored results open read-only and "re-simulate" runs the scenario as a labelled sibling
branch (0211).

Retention (ADR 0212, first estimates until the A3 benchmark):

| Tier | Holds | Series beyond it |
|---|---|---|
| Live branch | monthly history for the latest 25 simulated years, annual before | n/a |
| Other branches and members | declared outcomes monthly, snapshots every 12 ticks for 25 years, then every 60 | recomputed from the nearest snapshot, bit for bit |

Budgets: one run to 2150 within 20 GB on disk, an ensemble of 32 to 2050 within 50 GB. If the
measurement misses, snapshot intervals widen before the tiers change.

## Ensembles and common random numbers

Ensemble member k draws the uncertain parameters (theta by class, rho, nu, the price-rule gains)
from their register priors with streams keyed by (seed k, `SystemId::PARAM_DRAW`, parameter id),
applied as tick-0 parameter edits from the calibrated steady state, so the draw is in the log
(ADR 0322). A branch and its baseline with the same k share the draw (CRN, 14.4). Without draws,
K seeds give K identical members because the P2 tick draws no random numbers. Members run in lane
blocks of `L` (ADR 0024, plan Branch L); lane l of an `L = 8` run equals the `L = 1` run with
lane l's parameters and seed. The 100-year invariant run also uses the undrawn baseline.

Hazards (ADR 0241): `draw(seed, tick, hazard, cell)` returns one uniform per (hazard, cell, tick)
whether or not the rate is positive; an event fires when the uniform is below the rate, so
branches share the events of untouched hazards and a zero-rate hazard leaves hashes unchanged.
The commitments table (0241 D2) is the first user; P5b and P8 hazards register with the same
primitive.

## Effects and attribution

Paired effect per seed on declared outcomes; mean with bootstrap intervals (resampling keyed by
the counter RNG, so intervals are deterministic), quantiles and event probabilities. Per-province
mean difference with a mask where the share of seeds agreeing with the mean's sign is below a
display threshold (a view setting, not a register parameter). Attribution (14.7): channel
shutdown for prices, trade shares and wages; Shapley over interventions, exact up to five,
sampled beyond; `blame` ranks edits and replay entries by effect; for a tariff branch the
`ceteris-reference` counterfactual equilibrium is reported beside the simulated long-run effect.
Predictability horizons (14.6) and Sobol (14.7) need an emulator and wait for P9.

## Calendar-blind and replay

The start year is a label in the scenario and in output only. No system reads a calendar date;
month of year, ages and durations are state. The pack loader fails on any table keyed by calendar
date read after t0. The replay loader reads tape tables and emits `Edit { source: replay }` by
family (`shocks`, `policy`) for switch S07 `off | shocks | policy | all`; at P3 only policy
(tariffs and wedges) has a target. The fixture tape is `date,family,target,value` lines (date `YYYY-MM`), turned into ticks against the scenario's `start_year` label at t0; rows before it are dropped, and the label is not in the run hash.

## Outputs

Declared outcomes per (branch, member, tick), effects, quantiles, masks and channel shares as
Arrow IPC (no Parquet writer in 1.0; Polars and DuckDB read IPC; Parquet may follow 1.0), schema `workbench/v1` listed in the repository next to the writer and
checked by a schema test. 0087's observable tables (night lights, NO2, built-up, shipping
density, flaring, greenness) are in the schema; only shipping density from sea-lane flows has an
operator at P3. Verbs: `ceteris new | fork | edit | run | ensemble | diff | effects | explain |
blame | bisect | rebase | cherry-pick | share | fetch`, names and arguments from one command
registry (0188) shared with the app. Read-only `ceteris serve` (0197) belongs to the lab-UI track
after the gate (0323).

## Invariants

| Invariant | Statement | Tolerance | Checked where |
|---|---|---|---|
| Null intervention | A branch with no edits reproduces its parent's state hashes at every tick, at 1, 3 and all threads | bit for bit | `null_intervention_reproduces_parent` (CI: toy, P2 fixture; nightly: full pack) |
| Fork equality | Forking at any tick equals the continuous run | bit for bit | `fork_at_any_tick_equals_continuous_run` |
| Replay equality | Replaying the scenario from an empty cache reproduces every stored state hash | bit for bit | `deleting_the_cache_and_replaying_reproduces_hashes` |
| Edit ordering | Edits at one tick apply in (source, sequence) order | exact | `edits_at_one_tick_apply_in_source_then_sequence_order` |
| Store round trip | `restore` of a manifest equals the canonical state | bit for bit | `chunk_store_restore_equals_canonical` |
| Recompute equality | A series recomputed from a snapshot equals the dense series | bit for bit | `recomputed_series_equals_dense_series` |
| Lane equality | Lane k of an `L`-lane run equals the scalar run with seed k | bit for bit | `lane_k_equals_scalar_run_with_seed_k` |
| Calendar blindness | Relabelling the start year (2000 to 2100) changes labels only | hash unchanged | `year_relabel_2000_to_2100_keeps_golden_hash` |
| Null paired effect | The paired effect of a null edit is exactly zero | 0 | `paired_effect_of_null_edit_is_exactly_zero` |
| Channel shares | Shapley channel shares sum to the total effect | Shapley tolerance (0049) | `channel_shares_sum_to_total_within_shapley_tolerance` |
| Flatness | Median tick over years 96 to 100 within 25% of years 1 to 5; every table within its bound | 25% | flatness benchmark (G1), nightly |

## Bounds at 2150 (ADR 0236)

First estimates at 1,800 ticks; Branch G replaces them with measurements and the flatness report
checks them. A bound is a cap that errors, never silent truncation.

| Table | Bound | Rule | Size at 2150 |
|---|---|---|---|
| Edit log | 100,000 edits per branch | `ceteris edit` refuses past the cap | at most 10 MB per branch (about 100 B per edit) |
| Branch tree | 1,024 branches per scenario | `ceteris fork` refuses past the cap | at most 1 MB |
| Member draws | K x uncertain parameters, fixed per ensemble | tick-0 edits, none later | 32 x about 40 = 1,280 edits |
| Commitments | 4,096 scheduled rows | rows leave the table when executed or cancelled | at most 1 MB; empty at P3 |
| History, live branch | 300 monthly + 125 annual rows per series | tier rule above | per series, constant after year 25 |
| Chunk store | 20 GB per run, 50 GB per ensemble of 32 | retention tiers; thin snapshots first | measured in A3 |
| Bootstrap and effect tables | one row per (outcome, window), not per tick | recomputed on demand | negligible |

## Equilibrium nesting

n/a: the workbench computes no economics. Effects of tariff branches are compared with the
`ceteris-reference` counterfactual equilibrium (J2).

## Parameters

| ID | Name | Prior / value | Class |
|---|---|---|---|
| P53 | rho by class, short-run trade adjustment (ADR 0027) | half-life 12 to 48 months; fitted from BACI local projections (E2) | data, then free |
| S12 | short-run trade adjustment switch: instant or partial | on if the falsification test passes, else instant | switch |
| S07 | replay: off, shocks, policy, all | all for hindcast | switch |

Member draws reuse the existing register priors; no new code is taken until the PR that writes it.
The display threshold for the sign mask is a view setting.

## Test plan

- Unit tests per invariant above, on the toy world and the P2 fixture; full-pack variants in the
  nightly `slow` job.
- Scenario: `scenario_roundtrip_is_byte_stable`, `run_hash_changes_with_any_field`,
  `run_hash_ignores_comments_and_key_order`, `unknown_edit_kind_is_named_error`,
  `cache_from_another_engine_is_decode_error`.
- Edits: `edit_applies_at_its_tick`, `switch_edit_after_t0_is_rejected`,
  `event_and_override_have_no_targets_yet`, `edit_by_query_replays_to_same_cells_on_replay`,
  `query_names_unknown_column_error`, `bisect_finds_planted_edit_in_log2_probes`.
- Ensembles and hazards: `crn_pair_in_two_lanes_equals_two_scalar_runs`,
  `member_draws_are_shared_by_branch_and_baseline`,
  `k_identical_seeds_without_draws_give_identical_members`,
  `branches_share_events_of_untouched_hazards`, `zero_rate_hazards_leave_hashes_unchanged`,
  `crn_intervals_tighter_than_independent_on_fixture`.
- Golden: `tests/golden/p3-workbench.txt` (fork, tariff edit, edit by query, 4-member ensemble).
- Gate: tariff experiment at K = 32 under CRN and independent seeds (J2), report in
  `tests/perf/p3-gate.md`; flatness report under `tests/perf/` (G1).
- Benchmarks (19.2): bytes per tick per retention tier; `L = 8` lanes; 10, 20 and 45 synthetic
  goods.

## Open questions

- Whether the 20 GB and 50 GB disk budgets hold; A3 measures.
- Lane width `L` (8 to 32): Branch L's benchmark.
- Hash over Arrow instead of canonical bytes, if the owner wants it (one re-bless).
