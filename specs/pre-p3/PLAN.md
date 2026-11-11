# Pre-P3 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring P2 as built (main fc7ce3d; B onward from main 7219de2, Python removed) to the state P3 (counterfactual workbench, DESIGN 21.1 item 4) needs: enforced determinism and the dispatch speed plan, the P2 economics review (ADRs 0090 to 0095, 0101, 0200), main's assumption-audit ADRs (0098, 0099, 0100, 0103, 0104, 0106), P3 prerequisites (snapshots across interventions, structural edits, ensemble lanes), the open P2 issues, the records, and the rerun of the 28 October benchmark that closes pre-P3 (ADR 0207).

**Architecture:** Four code branches here (A, then B, C, D; E moved to P3 by ADR 0288), plus one records branch (F) that is independent. A changes speed and the kernel; B changes the price rule, storage, the trade frictions and the buyer price (selection, ADR 0200), lands the benchmark harness, then recalibrates and re-runs the P2 gate; C changes the calibration data and the demand and income blocks, recalibrates, re-runs the gate and reruns the 28 October benchmark as pre-P3's economic gate; D adds what P3's seek, branching and sweeps need. E, the retrofit of the kernels over a const lane count, is P3 Branch L since ADR 0288 (the retrofit generic over `Real` moved to P9, ADR 0207). `ceteris-reference` mirrors every model change, because V1 compares against it.

**Tech Stack:** Rust 2024 workspace only (ADRs 0113, 0207: no Python, including tools): `ceteris-core`, `-geo`, `-trade`, `-econ` (rayon, libm, arrow); `ceteris-pack` (pack build, `src/econ/`, configs in `crates/ceteris-pack/config/`), `ceteris-reference` (GE reference, PPML, uniqueness), `ceteris-calibrate` (steady-state calibration, gravity), `ceteris-cli` (the `ceteris` binary and the P2 gate tests); GitHub Actions.

**Python removed (ADR 0113, complete at main 7219de2):** Branch A (merged, PR #28) still names the Python files it changed then; B and C name the Rust crates. Pack econ tables: `ceteris pack build --econ-only`; calibration: `ceteris calibrate`; gravity: `ceteris gravity`; GE reference: `ceteris reference solve`; the P2 gate: `crates/ceteris-cli/tests/p2_gate.rs` with `crates/ceteris-econ/tests/p2_acceptance.rs`.

**Spec:** `specs/econ/SPEC.md`, `specs/trade/SPEC.md` (as built), ADRs 0089 to 0112, 0113 (Rust migration), 0200, 0207, the P2 ledger `.superpowers/sdd/PLAN/progress.md` and `.superpowers/sdd/PLAN/final-review.md` (main checkout), the owner request of 2026-10-26 (thread "Prepare the simulator for P3"), and the 28 October benchmark; harness `crates/ceteris-econ/examples/shock.rs` on `origin/claude/project-thread-cy0p06`, 90a6940).

## Owner decisions (2026-10-26)

1. Re-bless order: 0097 lints (no hash change), then gather dispatch proven bit-identical to the scatter (no re-bless), then one `Crossing` rewrite carrying the stored reciprocals, ADR 0093's kernel range and log-domain weights and ADR 0092's gateway mass weights, with one re-bless (as built: the masses moved to Task B2 by owner decision, 2026-10-26, and A re-blessed three times, ADR 0097). The economics re-bless again at each recalibration.
2. ADR 0090's anchor is redesigned as lbar_T = ln(sum_T p (D + delta S*) / sum_T p Q) over traded markets only; kappa_r starts traded 0.25, local 0.05.
3. ADR 0091 storage lives at origin markets; producer stock (`origin/p2-r3-producer-stock`) stays parked, revisited only if the V1 stress test still fails after B.
4. All of 0106, 0100, 0098, 0103 (drift switch off in the P2 steady-state test and V1), 0099 and 0104 land before P3.
5. One retrofit, generic over `Real` (ADR 0096) and a const lane count `L` (ADR 0024); f64 with `L = 1` reproduces the goldens bit for bit. (Amended 2026-10-28 by ADR 0207: E keeps the lanes; the `Real` part moves to P9.)
6. A structural parameter change on a calibrated pack is an ex-post intervention from the baseline steady state; the load guard stays.
7. A self-hosted GitHub runner on the owner's Mac runs a nightly full-pack gate; the owner registers it.
8. V1 excludes cells whose one-market J2 fails at the run's gains, and reports them separately.

## Owner decisions (2026-10-28, ADR 0207)

9. B1 to B7 start now (migration steps 2 and 4 have landed); B8 and C recalibrate on the Rust-built pack.
10. ADR 0200 (selection on trade links) is accepted and built in B together with ADR 0092's per-class nu.
11. Pre-P3 ends with a rerun of the 28 October benchmark: openness within 1.5 times the ICIO shares, the ACR formula holding, and a x10 border shock converging. Amended 2026-10-31 (owner, "A"): x10 is beyond P2's valid range and is reported, not asserted; it moves to P5 (ADR 0090, Deferred to P5).

## Owner decisions (2026-11-01, P3 readiness review)

12. D starts now in parallel with C, from main, and rebases (re-blessing any moved hashes) after C6 merges.
13. Ensemble lanes leave pre-P3: Branch E becomes P3 Branch L, before P3 C1 (ADR 0288, amending ADR 0207).
14. D5's workflow and script are drafted by an agent and committed to main by the owner, since the `rows` guard fails `claude/*` branches that touch `.github/`.

## Controller rulings in this plan (cost if wrong in brackets)

- ADR 0111's rational-expectations storage switch for 0091 is deferred to P9, where it is fitted and could become the default; P2 keeps adaptive expectations (owner choice D5b). [one task in P9]
- ADR 0103's drifts are not netted against ADR 0108's catch-up term: the catch-up equation is not in the model before P4, so nothing is counted twice yet; the netting lands with 0108. [drift values move when 0108 lands]
- ADR 0102 is not in scope (not requested; moved to 1.x by ADR 0207); ADR 0092's nu fit runs conditional on the current P04. [nu refit if 0102 returns]
- Parameters of a run become snapshot state (Task D2), because an ex-post edit of them must survive a restore. [codec bump, done once with 0106's]
- New ADR (D1, D2) records snapshots across interventions and parameters as state. It was planned as 0113, which is now the Rust migration ADR; it takes the next free number when D starts (0243 at main 7219de2). The anchor redesign is a section of ADR 0090, not a new ADR. [a renumber]
- ADR 0200 is its own task (B2a) after B2, not folded into it: ADR 0207 asks for both in branch B, and 0200's tests do not depend on the nu values. Each carries its own re-bless. [one extra re-bless commit]
- The ICIO 2000 home shares (domestic over absorption, per country and traded good) become a pack table, `econ_home_shares.arrow`, read by B2's nu fit and by C7's openness check, rather than read from the fetch cache at calibration time as `gravity::icio_domestic` does. [one pack table; if the owner prefers the cache, the fit and C7 read it through `ceteris_calibrate::gravity` instead]
- The harness lands on main in B (Task B2b), not only at the end: ADR 0200's own tests are benchmark cases (Japan, Brexit). Its shock files name countries by ISO3 code instead of table rows, so they survive pack rebuilds. Openness, welfare and the ACR value are computed in Rust (`ceteris_calibrate::benchmark`), since the 28 October numbers were computed outside the repository. [one small module]
- The three ADR 0207 criteria are made checkable as: (openness) per theta class, and for all traded goods together, the world absorption-weighted foreign share 1 - lambda in the model within [1/1.5, 1.5] times ICIO 2000's; (ACR) under every crossing tau x 1.1, every ICIO country with |ACR| above 1e-4 has the ACR sign, the median model/ACR ratio is in [0.8, 1.25] and the correlation across countries is at least 0.9, with ACR the multi-sector formula with input-output linkages d ln W_c = -omega_c' (I - A_c')^-1 diag(1/theta) d ln lambda_c, A_c the country's cost shares and d ln lambda 0 for local goods (Costinot and Rodriguez-Clare 2014; Caliendo and Parro 2015; ADR 0265, owner 2026-10-30; the plain -sum_g omega_cg / theta_g d ln lambda_cg of the original plan stays reported), graded per province since ADR 0268 (owner 2026-10-31): the formula on each province's home share lambda_p, and the model's real income change per province, both income-weighted up to countries (Ramondo, Rodriguez-Clare and Saborio-Rodriguez 2016); the country forms stay reported; (x10) every crossing tau x 10 runs 1,200 ticks frictionless with no invariant violation and the largest |d ln p| over the last 50 ticks at most 1e-6 (the baseline's is 2e-7); since 2026-10-31 reported only, a P5 criterion (ADR 0090). [the constants in C7's assertions change if the owner reads "holding" or "converging" otherwise]
- The P2 gate tests live in `crates/ceteris-cli/tests/p2_gate.rs` (ported there in migration step 4f), not in `ceteris-calibrate/tests`; the plan names them where they are. [none]

## Global Constraints

- Determinism: identical state hashes at 1 thread, 3 threads and all threads; parallel float sums only through `ceteris_core::reduce::fixed_sum` or fixed slots combined in index order; no `HashMap`/`HashSet` reaching state.
- Money is exact: i64 transfers rounded half to even (pops and enterprises), i128 for aggregate accounts after B5; every invariant in `EconWorld::end_tick` keeps holding.
- `CETERIS_PACK` is an absolute path; the pack is `data/packs/2000`, built and calibrated in Rust. `CETERIS_PACK_EXPORT` is a scratch directory written by `cargo run --release --bin ceteris -- export --pack $CETERIS_PACK --out $CETERIS_PACK_EXPORT`, read by `p2_gate.rs`.
- Pack econ tables: `cargo run --release --bin ceteris -- pack build --econ-only --pack $CETERIS_PACK` (sources from the fetch cache); then `cargo test -p ceteris-pack --release --test econ_build`.
- Calibrate: `cargo run --release --bin ceteris -- calibrate --pack $CETERIS_PACK` writes `target/calibration/<pack>`; install the tables into the pack yourself. Gravity: `cargo run --release --bin ceteris -- gravity --pack $CETERIS_PACK`.
- `ceteris-reference` mirrors the simulator: routing and payments to 1e-12 relative (today 3e-16; `ceteris-cli/tests/reference_cli.rs::routing_matches_rust`, `routing_matches_rust_on_the_pack`), the fixture equilibrium within the existing test tolerances (`ceteris-reference/tests/equilibrium.rs`). It stays independent: no dependency on `ceteris-core`, `-geo`, `-trade` or `-econ` (`ceteris-reference/tests/independence.rs`); it reads the export, never the simulator's types.
- Goldens (`tests/golden/p2-econ.txt`, `p2-pack.txt`, `p2-fixture/`) change only in a commit whose subject says re-bless and why, with a `### BEHAVIOUR:` CHANGELOG line.
- Every new parameter takes a register ID: next free P150 (none above P149 in `specs/` or `adr/` at main 7219de2; P150 to P153 are assigned by this plan). Reserved: P143 kappa_n, P144 psi, P145 zeta, P146 g_s (0091), P147 M_F (held), P56 storage cost.
- ADR numbers: next free 0243 (highest on main and on the open migration branches is 0242); check origin/main and open design PRs again before adding one.
- Budgets (M4 Pro, full pack, criterion medians): `source_dispatch_parallel` 8 ms, whole tick 35 ms. Re-measure at the end of A, B and C (`cargo bench -p ceteris-econ`).
- Never bypass bot checks; browser-only sources are downloaded by the owner. Public downloads are approved; pin each in `crates/ceteris-pack/config/fetch.toml` with sha256 and a `crates/ceteris-pack/config/sources.toml` licence row; `cargo run --release --bin ceteris -- pack check-sources --profile research` passes.
- Delete files by moving them to the Trash.
- Conventional Commits, no attribution, no em dashes. Comments only for counterintuitive logic; no process references (task numbers, briefs) in code.
- Code PRs need the owner's review; ADR-only PRs may be merged once green.

## Review Focus

1. **Seek across an intervention.** A snapshot taken before `set_network` or a parameter edit, restored after it, must replay to the same hashes as the continuous run (D1, D2: `seek_across_network_change_replays`, `seek_across_param_edit_replays`).
2. **A good with no traded activity in the anchor.** If sum_T p Q is 0 (no active traded market, or all output zero after a shock), lbar_T must not be NaN or infinite; the aggregate term is 0 that tick (B6: `anchor_is_zero_without_traded_supply`).
3. **Old snapshots after the codec bump.** Decoding a version-N snapshot after B5 (i128) or D2 (params) returns `DecodeError` naming the version, never a panic or a silent misread (B5: `old_codec_version_is_a_decode_error`).
4. **Storage at a stockout.** With S = 0 and a falling price expectation, storage sales cannot take S' below 0, and the price may move up to c_max 0.3 in one tick (B7: `storage_never_sells_below_zero`, `stockout_raises_price_more_than_surplus_lowers_it`).
5. **Countries missing a new source.** A country without an ICP price level, a current account, a WID wealth series or a SEA labour share takes the documented fallback with its flag set, never NaN (C1, C2, C4: `missing_source_falls_back_with_flag` in each pack test).
6. **A leg with no flow under selection.** pi_od = 0 (inactive origin, closed chokepoint, unreachable destination) leaves s_od undefined; the leg ships nothing and adds nothing to the selection tally, never NaN (B2a: `inactive_leg_has_no_selection_term`).

---

## Branch A: `pre-p3/speed` (Section 1, ADR 0097)

Done: merged, PR #28 (main 7695a2a). Tasks kept as built; the masses moved to B2.

### Task A1: Determinism lints

Done: merged, PR #28.

**Files:**
- Create: `clippy.toml`, `crates/ceteris-core/tests/clippy_bans.rs` (fixture, see Step 1)
- Modify: `crates/ceteris-econ/src/calibration.rs:290-300`, `crates/ceteris-econ/src/export.rs:133`, `crates/ceteris-core/src/snapshot.rs:4,193`, `crates/ceteris-econ/src/world.rs:1330-1360`, every other site the lint flags
- Test: `scripts/check_clippy_bans.sh`, `.github/workflows/ci.yml`

**Interfaces:**
- Produces: workspace `clippy.toml` with `disallowed-methods` and `disallowed-types` exactly as ADR 0097 lists them, each `reason` naming ADR 0097.

- [x] **Step 1: Write the failing fixture.** `scripts/check_clippy_bans.sh` copies a throwaway crate (`scripts/clippy-bans/`, outside the workspace members, with its own `Cargo.toml` pointing at the workspace `clippy.toml` via `CLIPPY_CONF_DIR`) that calls each banned rayon method, names `HashMap` and `HashSet`, and calls `f64::algebraic_add`; the script runs `cargo clippy -- -D warnings` on it and exits 0 only if clippy fails with one `disallowed` diagnostic per item (count them). The algebraic methods are unstable: if clippy cannot resolve their path, keep them in `clippy.toml` (ADR 0097 says an unresolvable path fails the build; check whether current clippy errors or warns, and record which in the ADR as built).
- [x] **Step 2: Run it, expect failure** (no `clippy.toml` yet, so clippy passes the fixture and the script exits 1).
- [x] **Step 3: Add `clippy.toml`; swap the metadata maps in `calibration.rs` and `export.rs` to `BTreeMap`; add `#[expect(clippy::disallowed_types, reason = "keyed lookup only, never iterated (ADR 0097)")]` on the `SnapshotStore` blob map; replace the journal scan's `par_chunks(1 << 16).fold(..).reduce(..)` in `world.rs` with fixed slots: one `Vec<i64>` per chunk index written in parallel, combined in chunk order.**
- [x] **Step 4: Run** `scripts/check_clippy_bans.sh` (exit 0), `cargo clippy --workspace --all-targets -- -D warnings` (clean), `cargo test --release --workspace` (goldens unchanged), and add the script to CI.
- [x] **Step 5: Commit** `build(lint): ban order-dependent reductions and hash maps (adr 0097)`.

### Task A2: Gather-parallel dispatch, bit-identical

Done: merged, PR #28.

**Files:**
- Modify: `crates/ceteris-trade/src/source.rs` (`dispatch`, the origin and domestic passes), `crates/ceteris-trade/src/kernel.rs` (`ByColumn` if a chunk API is needed)
- Test: `crates/ceteris-trade/src/source.rs` tests, `crates/ceteris-econ/benches/trade.rs`

**Interfaces:**
- Consumes: `Kernel::crossings_by_target`, `access_by_gateway`, `domestic_by_destination` (`ByColumn::column(c) -> &[(u32, u32)]`, row order).
- Produces: `dispatch(k: &Kernel, g: usize, price: &[f64], fill: &[[f64; 2]], out: &mut Sourced)` unchanged in signature; the old body kept as `#[cfg(test)] fn dispatch_scatter(..)` for the property test.

- [x] **Step 1: Write the failing property test** `gather_dispatch_equals_scatter_bit_for_bit`: proptest over random small kernels (2 to 4 countries, 1 to 3 gateways each, random prices and fills including zeros); every field of `Sourced` written by `dispatch` equals `dispatch_scatter`'s by `to_bits()`, including `lost`.
- [x] **Step 2: Run it**; it passes trivially while `dispatch` is still the scatter. Make it meaningful by first switching `dispatch` to call the new gather body, then run: expect failure until the gather is written.
- [x] **Step 3: Implement the gather** as ADR 0097 step 2 describes, without the reciprocals: per target, sum incoming legs over the `ByColumn` lists in row order, targets in fixed chunks in parallel; `lost` as per-chunk partials combined with `fixed_sum`. Where the bit order of `lost` or of `paid[d]`, `received[o]` cannot be kept, stop and report: the owner chose a bit-identical gather.
- [x] **Step 4: Run** the property test, `cargo test --release --workspace` (goldens unchanged at 1, 3 and all threads: `RAYON_NUM_THREADS`), and `cargo bench -p ceteris-econ --bench trade` (record `source_dispatch_parallel` and one-good dispatch).
- [x] **Step 5: Commit** `perf(trade): gather dispatch by target (adr 0097)`.

### Task A3: One `Crossing` rewrite and its re-bless

Done: merged, PR #28 (masses deferred to B2).

**Files:**
- Modify: `crates/ceteris-trade/src/kernel.rs` (`Entry`, `Crossing`, `Kernel::build`, `origin_access_into`), `crates/ceteris-trade/src/source.rs` (`origin_weights`, `price_index`, `source`, `dispatch`), `crates/ceteris-trade/src/network.rs` (gateway mass), `python/ceteris/packs/econ/gateways.py` and `build.py` (mass column), `crates/ceteris-econ/src/pack.rs` (read it), `python/ceteris/reference/model.py` (mirror), `specs/trade/SPEC.md` (Kernel, Dispatch, rounding), `tests/golden/*`
- Test: `crates/ceteris-trade/src/kernel.rs`, `source.rs` tests, `python/tests/test_reference.py`, `python/tests/test_econ_pack.py`

**Interfaces:**
- Produces:
  - `Crossing { col: u32, ln_k: f32, inv_duty: f64, inv_loss: f64, lag: u8 }` with `inv_duty = 1 / (1 + t)`, `inv_loss = 1 / (tau * border_tau)`; `Entry { col: u32, ln_k: f32, inv_tau: f64 }`. ln K in f32 (ADR 0093 option one; f64 K only if a test shows f32 ln K loses more than 1e-7 relative in K).
  - `Network.gateway_mass: Vec<f64>`, summing to 1 over each country's gateways; pack column `gateways.arrow: mass` = the gateway's share of its country's 2000 trade throughput (port and airport tonnage or value where the pack has it, else 1/k_A).
  - Origin weights stored as `ln_w: Vec<f64>`, shifted per country by its maximum; gateway sums by log-sum-exp; reachability tested on ln Phi_d; the 1e-100 floor removed except for K = 0 (no path).
- Consumes: A2's gather.

- [x] **Step 1: Failing tests:** `wide_price_spread_keeps_both_countries_reachable` (two countries, price ratio 1e12, theta 10.57; flows match an f64 dense reference to 1e-12 relative); `kernel_round_trip_at_extreme_tau` (tau 1e4, theta 64.85; ln K finite, K exp'd matches libm to 1e-7); `duplicating_a_gateway_with_half_mass_changes_no_flow` (1e-12); `test_gateway_mass_sums_to_one` (pack); Python `test_routing_matches_rust` updated to the mass-weighted kernel.
- [x] **Step 2: Run, expect failures.**
- [x] **Step 3: Implement** the struct changes, log-domain weights, mass weights (fold m into access and crossing legs so the low-rank evaluation is unchanged), reciprocals in `dispatch` and the access loops; mirror in `model.py`.
- [x] **Step 4: Rebuild the pack econ tables** (gateway mass), re-export the fixture goldens, recalibrate (structural digest moves) and install the tables; run `cargo test --release --workspace` with `CETERIS_PACK`, `pytest python/tests` with `CETERIS_PACK_EXPORT`, `cargo bench -p ceteris-econ`. Record the tick and `source_dispatch_parallel` against 35 and 8 ms in the trade SPEC.
- [x] **Step 5: Commit** code and records, then `test(golden): re-bless for reciprocals, log-domain weights and gateway mass (adrs 0092, 0093, 0097)` alone.

### Task A4: Typed, compressed history

Done: merged, PR #28.

**Files:**
- Modify: `crates/ceteris-econ/src/history.rs`, `crates/ceteris-econ/src/bin/ceteris.rs`, `Cargo.toml` (one compression dependency), `sources.toml` or `LICENSES` record if the repo keeps crate licences
- Test: `crates/ceteris-econ/src/history.rs` tests, `crates/ceteris-econ/tests/history_cli.rs`

**Interfaces:**
- Produces: `History` buffers with implicit tick, province and good keys and native widths (u32 keys, f64 values); on-disk format `run_history_*.arrow` unchanged in columns; `ceteris run --history-format csv` writes the same tables as CSV.

- [x] **Step 1:** Licence check: `cargo info alp` (or crates.io) for the `alp` crate's licence and maintenance; acceptable licences are those `deny.toml` (or the workspace's licence policy) allows. If none fits, use Arrow IPC's own zstd buffer compression (`arrow-ipc` `zstd` feature) with byte-stream-split, which needs no new crate. Record the choice in ADR 0097 as built.
- [x] **Step 2: Failing tests:** `history_round_trips_losslessly` (decode equals every recorded f64 by bits), `history_memory_per_market_row_at_most_49_bytes` (buffered bytes over rows), `history_csv_matches_arrow`.
- [x] **Step 3: Implement.** Recording only reads state: the existing `recording_leaves_the_run_unchanged` and `history_is_deterministic` must still pass.
- [x] **Step 4: Run** history tests and a 120-tick full-pack run with history; record peak memory (was 1.88 GB).
- [x] **Step 5: Commit** `perf(econ): typed and compressed run history (adr 0097)`.

### Task A5: Fused multi-good pass (conditional)

Done: merged, PR #28 (outcome in ADR 0097, As built).

- [x] **Step 1:** If after A3 `source_dispatch_parallel` is within 8 ms and one good's dispatch is no longer the critical path (profile: it is if the slowest good exceeds 80% of the stage), record that and skip this task.
- [x] **Gateway count (deferred here from Task 19b):** with A3's mass weights, access no longer scales with the number of gateways, so the count can be tuned for speed. Measure `source_dispatch_parallel` and the H2 distance coefficient at the current 600 gateways and at a smaller count (land crossings joined only to their neighbour, ledger Open questions); propose a count to the owner only if it buys at least 1 ms with Phi_d moving under 1e-3.
- [x] **Step 2 (if needed):** per-crossing per-good `ln_k` stored goods-innermost; inner arithmetic as a fixed-width loop over goods, each good keeping its own operation order. Test: state hashes and goldens unchanged at 1, 3 and all threads. Commit `perf(trade): fused multi-good dispatch (adr 0097)`.

### Task A6: Records and PR

Done: merged, PR #28.

- [x] ADR 0097 gets an "As built" section (lints result, gather numbers, reciprocal rounding, history format and crate, A5 outcome) and status accepted; ADR 0093 and 0092 note which parts landed in A3. Trade SPEC budgets updated. Open the PR to main.

---

## Branch B: `pre-p3/econ-review` (Section 2, plus 0106 and 0200)

Starts from main 7219de2 (A merged; Rust pack, reference and calibration on main). B1 to B7 may start now (owner decision 9). The reference mirror of every task lives in `crates/ceteris-reference/src/` and reads only the export; where an export column is added, `crates/ceteris-econ/src/export.rs` writes it, `crates/ceteris-reference/src/io.rs` reads it, and the export format number goes up by one (fixture goldens re-exported in the task's re-bless commit).

### Task B1: Refined petroleum into good 1 and harmonic class theta (ADRs 0092, 0101)

**Files:**
- Modify: `crates/ceteris-pack/src/econ/goods.rs` (`icio_industry`: "C19" to good 1; `NAMES[1]` "Crude oil, gas and refined fuels"; `hs_goods` through the CPA map), `crates/ceteris-pack/src/econ/theta.rs` (`by_good`, `classes`: harmonic class means), `crates/ceteris-pack/src/econ/build.rs` (goods metadata), `crates/ceteris-trade/src/params.rs` (`TradeParams` theta defaults, `theta_scale`), `crates/ceteris-trade/src/kernel.rs` (`Kernel::build` applies the scale), `crates/ceteris-econ/src/export.rs` and `crates/ceteris-reference/src/io.rs` (export and read `theta_scale`), `crates/ceteris-calibrate/src/gravity.rs` (`CLASSES`, `THETA_DEFAULT` after the move), `specs/econ/SPEC.md` (Goods, Parameters P01), `adr/0023`, `adr/0022` (diagnostics)
- Test: `crates/ceteris-pack/tests/goods.rs`, `crates/ceteris-pack/tests/icio.rs`, `crates/ceteris-pack/tests/econ_build.rs`, `crates/ceteris-trade/src/params.rs` tests

**Interfaces:**
- Produces: good 1 "Crude oil, gas and refined fuels" (ICIO B06 and B09 as `goods::icio_industry` maps them today, plus C19 from good 5); `econ_goods.arrow` records both `theta_arithmetic` and `theta_harmonic` per class; `TradeParams.theta` per good from ADR 0023's rule on the harmonic means; `TradeParams.theta_scale: f64` (register P150, default 1, lane values {0.6, 1, 1.4} for tariff experiments, applied as theta_g * theta_scale in `Kernel::build`); `theta::harmonic_mean(theta: &[f64], weight: &[f64]) -> f64`.

- [x] **Step 1: Failing tests:** `c19_is_in_good_1` (goods.rs: `icio_industry("C19")` is good 1); `good_1_and_5_totals_follow_the_c19_move` (icio.rs, synthetic ICIO: good 1 and good 5 output and VA equal the aggregation with C19 moved, 1e-9); `class_theta_is_harmonic` (goods.rs, two-line toy class: theta_class = (sum w_l / theta_l)^-1); `welfare_first_order_at_harmonic_theta` (goods.rs, ADR 0101's toy test); `theta_scale_multiplies_every_class` (trade params, kernel factors at scale 1.4 equal those at theta x 1.4 by bits).
- [x] **Step 2: Run** `cargo test -p ceteris-pack --test goods --test icio` and `cargo test -p ceteris-trade theta_scale`; expect failures.
- [x] **Step 3: Implement**; widen the P01 prior to cover the harmonic values; update `fixture_theta_follows_the_classes` and `h2_prior_bands_follow_the_standard_errors` to the new class values.
- [x] **Step 4: Rebuild the pack econ tables** (`pack build --econ-only`), run `cargo test -p ceteris-pack --release --test econ_build` (update `pack_econ_tables_match_golden`), re-run the ADR 0022 gravity diagnostics with `cargo run --release --bin ceteris -- gravity --pack $CETERIS_PACK` and record them in ADR 0022.
- [x] **Step 5: Commit** `feat(econ): move refined fuels into good 1 and use harmonic class theta (adrs 0092, 0101)`.

### Task B2: Gateway masses and nu per theta class, fitted to the cross-border share (ADR 0092)

Carries ADR 0092's gateway mass weights, moved here from Task A3 by owner decision (2026-10-26) after H2 distance missed at -0.536 with masses alone. The Rust mass code was built in af90fa4 and removed in 1662fab; recover the Rust parts (`kernel.rs`, `network.rs`, `pack.rs`, `inputs.rs`, `fixture.rs`, `export.rs`) from af90fa4 rather than rewriting them, onto the kernel as it stands now (f64 K, reciprocals, per-country log weights). af90fa4's Python parts (the pack's mass column, the reference reading it) are written anew in `ceteris-pack` and `ceteris-reference`.

**Files:**
- Modify: `crates/ceteris-trade/src/network.rs` (`gateway_mass`), `crates/ceteris-trade/src/kernel.rs` (masses folded into access and crossing legs, per-class nu), `crates/ceteris-trade/src/params.rs` (`border_nu: [f64; N_CLASSES]`, with `N_CLASSES = 4` and the good to class map), `crates/ceteris-econ/src/pack.rs`, `inputs.rs`, `fixture.rs`, `export.rs`, `world.rs` (read the mass column; encode the four nu), `crates/ceteris-pack/src/econ/gateways.rs` (`Gateway.mass`, `columns`), `crates/ceteris-pack/src/econ/icio.rs` and `build.rs` (new table `econ_home_shares.arrow`), `crates/ceteris-pack/src/read/icio.rs` (domestic-origin absorption per good), `crates/ceteris-reference/src/io.rs`, `model.rs`, `sinkhorn.rs` (mirror the masses; `dense_kernel` with the class nu), `crates/ceteris-calibrate/src/border.rs` (new: the nu fit), `crates/ceteris-calibrate/src/lib.rs`, `steady.rs` (call the fit before stage 1), `write.rs` (write nu into the calibration tables), `crates/ceteris-cli/tests/p2_gate.rs` (H2 per class bands, kernel control), `specs/trade/SPEC.md`, `specs/econ/SPEC.md` (Pack additions: the home-share table), `tests/golden/*`
- Test: `crates/ceteris-trade/src/source.rs` tests, `crates/ceteris-pack/tests/gateways.rs`, `crates/ceteris-pack/tests/icio.rs`, `crates/ceteris-pack/tests/econ_build.rs`, `crates/ceteris-cli/tests/reference_cli.rs`, `crates/ceteris-calibrate/tests/steady_state.rs`, `crates/ceteris-cli/tests/p2_gate.rs`

**Interfaces:**
- Produces:
  - `Network.gateway_mass: Vec<f64>`, summing to 1 over each country's gateways; pack column `gateways.arrow: mass` = the gateway's share of its country's 2000 trade throughput (port and airport tonnage or value where the pack has it, else 1/k_A). Masses are folded into the access and crossing legs (K_oa m_a K_ab m_b K_bd), so the low-rank evaluation and the stored reciprocals are unchanged, and access no longer grows with the number of gateway pairs.
  - `econ_home_shares.arrow`: one row per (pack country, traded good), `home_share` = ICIO 2000 domestic-origin use over total use (intermediate plus final), `absorption` (million USD), `icio: bool` (false for countries outside ICIO; their share is NaN and they carry no weight). Read by `ceteris_calibrate::border` and, in C7, by `ceteris_calibrate::benchmark`.
  - `ceteris_calibrate::border::fit_border_nu(inp: &Inputs, u_star: &[f64], target: &[f64; 4], limits: Limits) -> Result<[f64; 4], CalibrationError>`: one nu per class, a monotone 1-D root (bracketed, Brent or bisection to 1e-9 in nu) on the absorption-weighted cross-border share of the class (ICIO countries only), each evaluation stages 1 and 2 of ADR 0089 on the mass-weighted kernel; `border::target_shares(pack: &Path) -> Result<[f64; 4], String>` from `econ_home_shares.arrow`. Written into the calibration tables and read into `TradeParams.border_nu` (P05 becomes four calibrated values).
  - *As built (2026-10-29, erratum):* `fit_border_nu(inp: &Inputs, target: &[f64; 4], icio: &[bool], limits: Limits, progress: &mut dyn FnMut(&str)) -> Result<border::Fit, CalibrationError>` (no `u_star`: stage 1 is held at the data state, controller ruling R3, and does not use it; the ICIO flags select the countries of the moment; `Fit` carries the nu, the target, the shares and the evaluation count) and `target_shares(pack: &Path, theta_class: &[usize]) -> Result<([f64; 4], Vec<bool>), String>` (also the ICIO flag of every pack country). C7's `benchmark` reads `econ_home_shares.arrow` the same way. `nu_fit_matches_cross_border_share` checks the shares relative to the target (1e-6), not absolute: class 3's share on the fixture moves by 2.7e-7 per unit nu.
- H2 per class gate (`p2_gate.rs::h2_theta_by_class`): each class's PPML estimate within 2 s.e. of theta_c (ADR 0092), replacing the P01-wide band. The kernel-control regression (model's own kernel instead of ln distance) is printed beside it. `h2_distance_elasticity` is re-run after the nu fit: masses alone moved it from -0.753 to -0.536 (band -1.1 to -0.7).

- [x] **Step 1: Failing tests:** `duplicating_a_gateway_with_half_mass_changes_no_flow` (source.rs, 1e-12); `gateway_mass_sums_to_one` (gateways.rs, on the built pack, per country 1e-12); `home_shares_follow_the_icio_aggregation` (icio.rs, synthetic two-economy ICIO: domestic over total use, 1e-12); `routing_matches_rust` and `routing_matches_rust_on_the_pack` (reference_cli.rs) updated to the mass-weighted kernel; `nu_fit_matches_cross_border_share` (steady_state.rs: on the fixture, the simulated share at the fitted nu equals the target per class, 1e-6); `nu_fit_is_monotone_and_bracketed` (steady_state.rs: a target outside the reachable range is an error naming the class).
- [x] **Step 2: Run** the named tests (`cargo test -p ceteris-trade -p ceteris-pack -p ceteris-calibrate -p ceteris-cli`); expect failures.
- [x] **Step 3: Implement** the masses (recovered from af90fa4) and their reference mirror, the home-share table, then the per-class nu fit.
- [x] **Step 4: Rebuild the pack econ tables** (mass column, home shares), re-export the fixture goldens, recalibrate (`ceteris calibrate --pack`) and install the tables; run `cargo test --release --workspace` with `CETERIS_PACK` and `CETERIS_PACK_EXPORT`; run `p2_gate.rs` (`h2_distance_elasticity`, `h2_theta_by_class`) after the nu fit and record both in the trade SPEC.
- [x] **Step 5: Repeat the gateway-count measurement** of Task A5 (ADR 0097, As built) with the masses in: 300 against 600 gateways by the pack's rule (`gateways::TOTAL`), parallel source and dispatch time, |d ln Phi_d| at the same prices, whether the 300 pack calibrates and H2 there. Record it in ADR 0097 and the trade SPEC (Open questions); propose a count only if Phi_d moves less than 1e-3.
- [x] **Step 6: Commit** `feat(trade): weigh gateways by mass (adr 0092)`, `feat(pack): icio home shares per country and good (adr 0092)`, `feat(calib): fit border friction per theta class (adr 0092)`, then the re-bless alone.

### Task B2a: Selection on trade links, buyers pay P_d (ADR 0200)

**Files:**
- Modify: `crates/ceteris-trade/src/source.rs` (`Sourced`: per-leg selection, `selected` tally beside `lost`; `source` sets q_d = P_d), `crates/ceteris-trade/src/kernel.rs` (Gamma per good from the s_dd = 1 normalization), `crates/ceteris-econ/src/world.rs` (`delivered_prices`, `base_delivered_prices`; the goods invariant in `end_tick` gains the selection term), `crates/ceteris-econ/src/production.rs` or `systems.rs` (enterprise input use per unit sold to d scaled by 1 / s_od), `crates/ceteris-reference/src/model.rs` (`Sourcing`: q = P_d, `Flows` with the selection term), `crates/ceteris-calibrate/src/steady.rs` (stage 1 check that s_dd = 1 reproduces the data state), `specs/trade/SPEC.md` (Deviations: the buyer price row replaced; Equations: Sourcing; Invariants), `specs/econ/SPEC.md` (enterprise input use, goods invariant), `adr/0200` (as built), `tests/golden/*`
- Test: `crates/ceteris-trade/src/source.rs` tests, `crates/ceteris-econ/tests/p2_properties.rs`, `crates/ceteris-reference/tests/equilibrium.rs`, `crates/ceteris-cli/tests/reference_cli.rs`, `crates/ceteris-calibrate/tests/steady_state.rs`

**Interfaces:**
- Produces: per active kernel leg s_od = Gamma_g^-1 (K_od / pi_od)^(1/theta_g) (one `libm::pow` per leg per tick), so alpha_d = sum_o pi_od s_od / c_od = 1 / P_d and buyers pay q_d = P_d = `source::price_index(..)`. A unit shipped from o arrives in d as s_od delivered units; `Sourced.selected: f64` tallies the delivered units added by selection, as `lost` tallies melt. Gamma_g and the base unit set so that s_dd = 1 at the 2000 data state; the ADR 0089 steady state is unchanged. Trade SPEC buyer price row: "Buyers of a traded good pay the CES index P_d = Gamma Phi_d^(-1/theta); each leg carries the selection multiplier s_od (ADR 0200)", replacing the q_d = 1 / alpha_d row and its rationale. No register entry.
- Consumes: B2's mass-weighted kernel and class nu.

- [x] **Step 1: Failing tests** (ADR 0200, Tests): `two_country_welfare_equals_acr` (source.rs, two-country one-good toy: welfare change equals lambda_dd'^(-1/theta), 1e-12); `raising_tau_never_lowers_the_buyer_price` (source.rs, proptest over small kernels: tau on one link x (1, 2], q_d non-decreasing at fixed origin prices); `selection_off_reproduces_the_status_quo` (p2_properties.rs: an autarky toy with every s = 1 gives hashes equal to the status quo); `goods_invariant_holds_with_selection` (p2_properties.rs, 120 ticks of the fixture, every tick); `inactive_leg_has_no_selection_term` (source.rs, Review Focus 6); `buyers_price_traded_goods_at_the_delivered_unit_price` (p2_properties.rs) rewritten as `buyers_pay_the_ces_index`; `steady_state_unchanged_by_selection` (steady_state.rs: stage 1 on the fixture reproduces the data state with s_dd = 1, 1e-12); the reference mirror in `equilibrium.rs::trade_money_matches_dense_route_oracle` and `base_prices_are_the_reference_sourcing_at_prices_one` updated to q = P_d, and `reference_cli.rs::routing_matches_rust` (1e-12). The benchmark cases of ADR 0200 (Japan, Brexit) are in B2b.
- [x] **Step 2: Run** `cargo test -p ceteris-trade -p ceteris-econ -p ceteris-reference -p ceteris-calibrate -p ceteris-cli`; expect failures.
- [x] **Step 3: Implement** s_od in `source` and `dispatch` (hoisted per leg, as the reciprocals), the input-use scaling, the invariant term, then the reference mirror.
- [x] **Step 4: Run** `cargo test --release --workspace` with `CETERIS_PACK`; re-export the fixture goldens; record the change in the trade and econ SPECs and ADR 0200 (As built).
- [x] **Step 5: Commit** `feat(trade): buyers pay the ces index with selection on links (adr 0200)`, then `test(golden): re-bless for selection on trade links (adr 0200)` alone.

### Task B2b: The benchmark harness on main (ADR 0200, ADR 0207)

**Files:**
- Create: `crates/ceteris-calibrate/src/shock.rs` (parse and apply shocks), `crates/ceteris-calibrate/src/benchmark.rs` (country aggregates, home shares, welfare, ACR), `crates/ceteris-calibrate/tests/benchmark.rs`, `crates/ceteris-calibrate/tests/data/benchmark/` (`border-1.1.shocks`, `border-10.shocks`, `brexit-soft.shocks`, `brexit-hard.shocks`)
- Modify: `crates/ceteris-econ/examples/shock.rs` (cherry-pick 90a6940 from `origin/claude/project-thread-cy0p06`, then reduce it to a wrapper over `ceteris_calibrate::shock` and `benchmark`, writing the same four CSVs), `crates/ceteris-calibrate/src/lib.rs`
- Test: `crates/ceteris-calibrate/tests/benchmark.rs`

**Interfaces:**
- Produces:
  - `shock::Shock { Tariff { importer: usize, exporter: usize, group: Option<usize>, add: f32 }, Ntb { exporter: usize, importer: usize, factor: f64 }, Border { factor: f64, good: Option<usize> }, Prod { country: usize, good: usize, factor: f64 } }`; `shock::parse(text: &str, codes: &[String]) -> Result<Vec<Shock>, String>` with countries by ISO3 code (an unknown code is an error naming it); `shock::apply(inputs: &mut EconInputs, shocks: &[Shock], trade: &TradeParams)` as the harness's `apply` (an empty list reproduces the default kernel bit for bit).
  - `benchmark::Outcome { income: Vec<f64>, omega: Vec<[f64; N_GOODS]>, buyer_price: Vec<[f64; N_GOODS]>, home_share: Vec<[f64; N_GOODS]>, absorption: Vec<[f64; N_GOODS]>, max_dlnp_last: f64, ticks: u32 }` from `benchmark::run(pack: &Path, shocks: &[Shock], ticks: u32) -> Result<Outcome, String>` (frictionless, from the calibrated steady state, as the harness); `benchmark::welfare(base: &Outcome, shocked: &Outcome) -> Vec<f64>` (d ln income less the omega-weighted d ln buyer prices, the artifact's definition); `benchmark::acr(base: &Outcome, shocked: &Outcome, theta: &[f64; N_GOODS]) -> Vec<f64>` (-sum over traded g of omega_cg / theta_g d ln lambda_cg); `benchmark::foreign_share(o: &Outcome, goods: &[usize]) -> f64` (world absorption-weighted 1 - lambda).
- Consumes: B2a's buyer price; `econ_home_shares.arrow` (B2).

- [x] **Step 1: Failing tests:** `empty_shock_reproduces_the_default_kernel` (fixture, kernel factors equal by bits); `shock_files_name_countries_by_code` (parse rejects "XXX"); `acr_is_exact_on_the_two_country_toy` (the B2a toy through `benchmark::acr` and `welfare`, 1e-12); pack-gated (skip without `CETERIS_PACK`, release): `japan_loses_from_costlier_borders` (`border-1.1.shocks`: JPN welfare < 0) and `eu_loses_from_soft_brexit` (`brexit-soft.shocks`, crossing tau x 1.0277 both ways between GBR and each EU27 member: EU27 income-weighted welfare <= 0, GBR < 0).
- [x] **Step 2: Run** `cargo test -p ceteris-calibrate --release --test benchmark`; expect failures (the pack-gated two fail on main's buyer price without B2a; with B2a they are the check).
- [x] **Step 3: Implement**; the hard Brexit file adds EU MFN tariffs on GBR as the 28 October run did and is reported only.
- [x] **Step 4: Run** with `CETERIS_PACK`; record JPN, GBR and EU27 welfare in ADR 0200 (As built) beside the 28 October values (JPN +0.6%, EU27 +0.47% hard, +0.06% soft). A pack-gated failure stops work and goes to the owner.
- [x] **Step 5: Commit** `feat(calib): benchmark harness with welfare and acr in rust (adrs 0200, 0207)`.

### Task B3: Exact local fill and uncapped wage search (ADR 0093)

**Files:** `crates/ceteris-econ/src/local.rs` (`fill`, `split`), `crates/ceteris-econ/src/labour.rs` (`clearing_wage` caller in `systems.rs`), mirror in `crates/ceteris-reference/src/model.rs` (local output) and `crates/ceteris-calibrate/src/steady.rs` (`local_output`); tests in `crates/ceteris-econ/tests/p2_properties.rs`, `crates/ceteris-calibrate/tests/steady_state.rs`.

**Interfaces:**
- Produces: `local::fill_exact(users: &LocalUsers, stock: &[f64; N_LOCAL]) -> [f64; N_LOCAL]` enumerating the at most 2^3 regimes, keeping the largest feasible fill; the 32-pass bound and ad hoc clearing removed. The frictionless search finds the root of sum L*_ig(w) = (1 - u*) L^s on uncapped targets and caps only `allocate`.

- [x] Failing tests: `exact_fill_equals_converged_iteration` (every fixture case and proptest small provinces, 1e-12); `local_chain_terminates_on_cycles` settles in one solve; `frictionless_wage_matches_closed_form_when_one_target_reaches_supply`; `local_output_is_nonnegative_and_clears_locally` (steady_state.rs) still passes. Implement, run `cargo test -p ceteris-econ -p ceteris-calibrate`, commit `fix(econ): exact local fill and uncapped wage search (adr 0093)`.

### Task B4: PIGL aggregation across pops (ADR 0094)

**Files:** `crates/ceteris-econ/src/demand.rs`, `crates/ceteris-econ/src/world.rs` (`household_preferences`), `crates/ceteris-reference/src/demand.rs` (`lognormal_sigma`, `invert`), `crates/ceteris-calibrate/src/steady.rs` (stage 4, `demand::invert_at`), `crates/ceteris-calibrate/src/summary.rs` (the s_w = 0 list).

**Interfaces:**
- Produces: within-pop dispersion `s_w^2 = max(0, s_nat^2 - Var_k(ln m_k))` per country (population-weighted); inversion at the calibrated buyer prices q_d over the country's pops by a 1-D root on lambda; countries with s_w = 0 listed in the calibration summary.

- [x] Failing tests: `aggregate_data_state_shares_match_omega` (steady_state.rs, per country, 1e-12); `one_pop_inversion_equals_old_at_prices_one` (reference.rs); `between_and_within_variance_add_up` (reference.rs); `pigl_matches_rust` (reference.rs) on the new aggregation. Implement simulator and reference, run, commit `feat(econ): invert pigl over each country's pops (adr 0094)`.

### Task B5: Clip outside pi^e, wider clip for storables, i128 aggregates (ADR 0106)

**Files:** `crates/ceteris-trade/src/market.rs` (`reprice`), `crates/ceteris-econ/src/labour.rs` (`wage_step`), `crates/ceteris-trade/src/params.rs` (`c_max_storable` = 0.3, register P151), `crates/ceteris-core/src/ledger.rs` (i128 account class), `crates/ceteris-core/src/snapshot.rs` (`Encoder::i128`, `Decoder::i128`; the header already carries a format version, line 152: bump it), `crates/ceteris-econ/src/world.rs` (pools, mint, markets, gateways on i128; pops and enterprises stay i64), `crates/ceteris-calibrate/src/steady.rs` (`calibrate_from`), `crates/ceteris-calibrate/tests/steady_state.rs` (multi-start), trade SPEC Stability region.

**Interfaces:**
- Produces: `dlnp = pi^e + clip(gap terms, +-c_max_g)`, `dlnw = pi^e + clip(kappa_w (x + u*), +-c_max)` then the rigidity factor when negative; `clip_ticks` counts the gap clip. `Ledger` gains `money_wide(a) -> i128` and an account kind chosen at `open`; `transfer` between kinds checks i64 range on the narrow side and panics loudly on overflow. Decode of another format version is `DecodeError` naming it. `steady::calibrate_from(inp: &Inputs, u_star: &[f64], limits: Limits, start: &Mat, progress: &mut dyn FnMut(&str)) -> Result<Steady, CalibrationError>` (start prices; `calibrate_with` calls it at the data state).

- [x] Failing tests: `clip_lets_expected_inflation_through` (pi^e 0.2, zero gap: dlnp = 0.2); `storable_goods_clip_at_0_3`; `mint_survives_scale_1000` (the overflow at about x802 is gone); `old_codec_version_is_a_decode_error`; `calibration_unique_from_eight_starts` (steady_state.rs, fixture, prices +-50%, Philox seeds 0 to 7, same fixed point to 1e-9; a failure is a finding for the owner). Implement, run, commit `feat(core): i128 aggregate accounts and pi^e outside the clip (adr 0106)`.

### Task B6: Split price rule with the traded-market anchor (ADR 0090, owner redesign)

**Files:** cherry-pick `origin/p2-adr0090` 56b8e7f as the starting point, keeping its Rust and golden changes and porting its Python changes (`stability.py` to `crates/ceteris-calibrate/src/stability.rs`, `calibrate.py` to `steady.rs`, `io.py` and `model.py` to `crates/ceteris-reference/src/io.rs` and `model.rs`; `jax_model.py` is dropped); `crates/ceteris-trade/src/market.rs`, `crates/ceteris-trade/src/params.rs` (`kappa_r_traded` 0.25, `kappa_r_local` 0.05 as P03a split into P03a-t and P03a-l, `kappa_n` P143 0.2, `delta` per good P56), `crates/ceteris-econ/src/world.rs` (one world reduction per tick, `fixed_sum`); trade SPEC Origin market and Stability region; ADR 0090 "Anchor redesign". The special-case golden `tests/golden/p2-as-built-rule.txt` is already on main (43021e1).

**Interfaces:**
- Produces: with l_o = ln((D_o + delta_g S*_o) / Q_o), T the active traded markets,
  `lbar_T = ln(sum_T p_o (D_o + delta_g S*_o) / sum_T p_o Q_o)` (0 when the denominator is 0), and
  `dlnp_o = pi^e + clip((kappa_r,o / E_o)(l_o - lbar_T) + (kappa_n / h) lbar_T - (kappa2 / E_o)(S'_o - S*_o)/S*_o, +-c_max_g)`,
  E_o = theta_g + 1 traded, 1 local; h the steady-state household share of traded orders (fixed at load); kappa2 by the midpoint rule at the new gains (`ceteris calibrate stability`). The Log|Linear switch of the trial stays, with the special-case golden `p2-as-built-rule.txt`.

- [ ] **Step 1: Failing tests:** `anchor_ignores_a_local_shortage` (fixture: a local good short by ln 2 in one province leaves lbar_T at 0 and moves only that price); `anchor_is_zero_without_traded_supply`; `nominal_shock_returns_within_30_ticks` (5% money shock, index within 1%); `special_case_matches_as_built_rule` (hashes equal `p2-as-built-rule.txt` over 120 ticks); `crates/ceteris-trade/tests/price_rule_stability.rs` Jury checks re-derived for the normalized rule; `calibrate_stability_prints_the_fixture_table` (`ceteris-cli/tests/calibrate_cli.rs`) on the new table.
- [ ] **Step 2:** run, expect failures. **Step 3:** implement.
- [ ] **Step 4: Stop rule.** Run the fixture data state and the pack data state, default and frictionless, 1,200 ticks (`cargo run --release --bin ceteris -- run --pack $CETERIS_PACK --ticks 1200 --data-state [--frictionless] --out DIR`); record the world index at 0, 36, 120, 360, 1,200 against the as-built rule (2.38 at 120, 2.02 at 360 on the pack). If either index exceeds 10 or the run overflows, stop and send the table to the owner before changing any gain.
- [ ] **Step 5:** commit `feat(trade): split price rule with a traded-market anchor (adr 0090)`.

### Task B7: Competitive storage at origin markets (ADR 0091)

**Files:** `crates/ceteris-trade/src/market.rs` (storage orders at T3), `crates/ceteris-econ/src/world.rs` (per-market `pi_e_market` state, encode and restore), `crates/ceteris-trade/src/params.rs` (`psi` P144, `zeta` P145, `g_s` P146, `k_g` money cost and `delta_g` P56 for goods 0, 1, 2, 5), mirror in `crates/ceteris-reference/src/model.rs` (storage at the fixed point) and `crates/ceteris-calibrate/src/steady.rs` (stage 5 stocks), ADR 0091 as built.

**Interfaces:**
- Produces: `pi_e_og <- pi_e_og + g_s (dlnp_og - pi_e_og)` after T4; `x_og = pi_e_og + y_g(S / (s_g D)) - k_g - delta_g`, `y_g(z) = (k_g + delta_g) z^-zeta_g`; storage orders `dS = delta_g S + psi_g D x`, floored so S' >= 0; the kappa2 term is off for goods 0, 1, 2, 5. Spoilage is a tallied goods flow.

- [x] Failing tests: `storage_orders_equal_spoilage_at_steady_state`; ADR 0089's fixed-point test with storage (`stage5_state_is_stationary_under_the_stock_rules`, steady_state.rs); `storage_never_sells_below_zero`; `stockout_raises_price_more_than_surplus_lowers_it` (30% harvest cut vs surplus on agriculture); `storage_conserves_goods_and_money`; linearized region re-derived with psi and zeta on the toy market (`crates/ceteris-calibrate/src/stability.rs`). The SPEC records the weak sources behind delta_g and k_g as an open question for P9. Implement, run, commit `feat(trade): competitive storage for storable goods (adr 0091)`.

### Task B8: V1 cell rule, recalibration and the P2 gate

Runs on the Rust-built pack (owner decision 9).

**Files:** `crates/ceteris-cli/tests/p2_gate.rs` (`v1`), `crates/ceteris-econ/tests/p2_acceptance.rs`, `crates/ceteris-reference/src/uniqueness.rs` (one-market J2 per cell at the run's gains), `specs/econ/SPEC.md` (P2 gate, Open questions), `tests/golden/*`, ADRs 0090 to 0095, 0101, 0106, 0200 (status, as built).

**Interfaces:**
- Produces: `uniqueness::cells_outside_region(inp: &Inputs, gains: &Gains) -> Vec<(usize, usize)>` (province, good), with `uniqueness::Gains { kappa_r_traded: f64, kappa_r_local: f64, kappa_n: f64, kappa2: f64, c_max: f64 }` read from the export's params; `v1` in `p2_gate.rs` asserts the 1% tolerances on every other cell and prints the excluded cells with their gaps; the test asserts the excluded count is at most 1% of active cells (else it is a finding).

- [x] **Step 1:** failing `v1_excludes_cells_outside_region` (`p2_gate.rs`, fixture variant with one cell pushed outside J2; runs without a pack) and `cells_outside_region_is_empty_on_the_fixture` (`ceteris-reference/tests/reference.rs`).
- [x] **Step 2:** rebuild the pack econ tables, recalibrate (ADR 0089, `cargo run --release --bin ceteris -- calibrate --pack $CETERIS_PACK`) with B1 to B7 and B2a, install the tables, re-export (`ceteris export --pack`), re-bless goldens in their own commit.
- [x] **Step 2a: main's whole provinces (ADR 0251).** Main 2d41a65 is merged into B (trial merge `trial/p3-merge-main`, 4 textual conflicts, fixture tests green). The rebuild in Step 2 is a full pack build, not `--econ-only`, since the provinces change. On the rebuilt pack:
  - [x] Recalibrate at format 7; check `econ_calib_cells.arrow` carries `block_scale_default` and `block_scale_frictionless`.
  - [x] Confirm `econ_build.rs` build stats: gateway kinds 286/171/143 (gateway masses do not change kinds), 4,368 provinces with value added, imputed VA share 0.0754 (predicted); fix and record any difference.
  - [x] Re-bless `tests/golden/p1-pack.txt` and `p2-pack.txt` (own commit); check `p1-geo.txt` stays main's.
  - [x] Re-measure ADR 0267's blocks (scaled count per mode, Albania, Sudan, Lesotho, France) and record them under ADR 0267 As built; replace the split-pack Sudan province ids.
  - [x] Main's open misses on its unsplit pack, now on this branch's code: V1 from the perturbed steady state at seeds 20 to 22 (Albania; the case that motivated ADR 0267) and `pack_calibrated_state_holds_to_2e_6_and_4_minor_units` (Pitcairn's tertiary pop money at 1.49 of the 4-minor-unit floor; P156 does not move the floor).
  - [x] Update the figures in the trade SPEC and CHANGELOG measured on the split pack (H2 distance, provinces with value added).
- [x] **Step 3: Full P2 gate** with the commands in the econ SPEC P2 gate section: `CETERIS_PACK=$PACK CETERIS_PACK_EXPORT=$EXPORT cargo test -p ceteris-cli --release --test p2_gate -- --nocapture` (V1 from the perturbed steady state; the V1 stress test `v1_full_pack_from_the_data_state_stays_open`, which fails when V1 from the data state passes: then report to the owner before inverting it; `h2_distance_elasticity`; `h2_theta_by_class` with B2's bands and the kernel control: the kernel-control regression, the model's own kernel in place of ln distance, printed beside the per-class estimate, was not built in B2 and is built here), `CETERIS_PACK=$PACK cargo test -p ceteris-econ --release --test p2_acceptance` (100 years, 1 vs 8 threads, V0), `cargo test -p ceteris-calibrate --release --test steady_state --test benchmark`, `cargo bench -p ceteris-econ` against 8 and 35 ms. Record every number in the SPEC. A miss stops work and goes to the owner.
- [x] **Step 4:** If the V1 stress test still fails, record the diagnosis (Mali and Albania pops, market losses; `data_state_write_off_census`) and ask the owner whether to revisit producer stock (decision 3).
- [ ] **Step 5:** commits, PR.

---

## Branch C: `pre-p3/audit` (Section 3)

Starts from main after B merges. Every pack change adds `crates/ceteris-pack/config/fetch.toml` pins and `sources.toml` rows and passes `ceteris pack check-sources --profile release`. Readers of new sources go in `crates/ceteris-pack/src/read/`, their builders in `crates/ceteris-pack/src/econ/`, and each table is listed in `econ::build::TABLES`.

### Task C1: Price levels and source corrections (ADR 0098)

**Files:** `crates/ceteris-pack/src/econ/icio.rs` (1999-2001 rate rescale per country block; `impute` region override table), `crates/ceteris-pack/src/econ/people.rs` (SWIID first, WDI fallback, `gini_source`; SAH population from WDI; VEN income rule), `crates/ceteris-pack/src/read/wdi.rs` (`read_rates`, PA.NUS.PPPC.RF and PA.NUS.FCRF), new `crates/ceteris-pack/src/econ/ras.rs` (donor RAS to WDI sector VA and BACI trade by good, rest-of-world block constraint, 1e-10, at most 200 iterations), `crates/ceteris-pack/src/econ/build.rs` (`price_level` column), `crates/ceteris-econ/src/pack.rs`, `inputs.rs`, `world.rs` (`base_delivered_prices`, local base price p_c), `crates/ceteris-reference/src/io.rs` (read p_c), `crates/ceteris-calibrate/src/steady.rs` (world price index over traded goods only).

**Interfaces:** `econ_countries.arrow` gains `price_level` (WDI PA.NUS.PPPC.RF / PA.NUS.FCRF, 2000 or nearest ICP-anchored year, flag `price_level_imputed`) and `gini_source`; local-good base price p_c in `ProvinceData::base_price` and in coef = a p_o / q; `ras::balance(seed: &[[f64; N_GOODS]; N_GOODS], rows: &[f64], cols: &[f64], tol: f64, max_iter: usize) -> Result<[[f64; N_GOODS]; N_GOODS], String>`.

- [ ] Failing tests: `rate_rescale_keeps_within_block_ratios` (icio.rs); `ras_matches_targets` (new `crates/ceteris-pack/tests/ras.rs`, 1e-10); `price_level_values_unchanged` (`econ_build.rs`: t = 0 values, shares, national accounts unchanged); `missing_source_falls_back_with_flag` (people.rs); `region_override_places_an_unclassified_country` (icio.rs) extended to AIA and MSR. Data caveats from P2 fixed here: SAH population from WDI (not the 811-person WorldPop sum), a region override table for dependencies (AIA, MSR take Latin America and Caribbean donors), VEN income from WDI where PWT and WDI disagree by more than a factor of 3 (flagged). Implement, rebuild the pack (`pack build --econ-only`), commit per concern (`feat(pack): ...`, `feat(econ): local base price p_c (adr 0098)`).

### Task C2: Trade imbalances as transfers (ADR 0100)

**Files:** `crates/ceteris-pack/src/econ/build.rs` (`ca_2000`, CA path from WDI BN.CAB.XOKA.CD 2000-2024, `econ_countries.arrow` columns), `crates/ceteris-calibrate/src/steady.rs` (remove r_g; absorption = income + D_c; stage 1 docs), `crates/ceteris-reference/src/model.rs` (D_c in absorption), `crates/ceteris-econ/src/world.rs` (world transfer account, i128, nets to zero each tick, asserted in `end_tick`), `crates/ceteris-econ/src/params.rs` (switch `Transfers::{Off, FixedShares, ObservedPath}`, register 17.2 entry).

- [ ] Failing tests: `transfers_net_to_zero_every_tick` (p2_properties.rs); `off_switch_reproduces_balanced_trade` (p2_properties.rs, hashes equal); `r_g_removed_and_residual_reported` (steady_state.rs: the summary reports the residual, no r_g column). Implement, commit `feat(econ): trade imbalances as transfers (adr 0100)`.

### Task C3: Three final demanders and the saving rate (ADR 0099)

**Files:** `crates/ceteris-pack/src/read/icio.rs` (`CountryIo`: HFCE, GGFC plus NPISH and GFCF vectors of 10 in place of `final_demand_shares`), `crates/ceteris-pack/src/econ/icio.rs` (`country_columns`: g_c, s_c), `crates/ceteris-econ/src/demand.rs`, `systems.rs` (fiscal and investment accounts per country; each spends in full each tick), `params.rs` (accelerator beta, register P152, default 1), `crates/ceteris-calibrate/src/steady.rs` stage 4 (PIGL on HFCE; G and I fixed in the Sinkhorn targets), `crates/ceteris-reference/src/model.rs` and `demand.rs`.

- [ ] Failing tests: `final_demand_splits_into_three_demanders` (icio.rs, synthetic ICIO, sums equal the old final demand, 1e-12); `money_stock_unchanged_by_fiscal_and_investment_accounts` (p2_properties.rs); `beta_one_reproduces_fixed_share`; `pigl_inverted_on_hfce_only` (steady_state.rs). Implement, commit `feat(econ): household, government and investment demand (adr 0099)`.

### Task C4: Sector labour shares, wealth-weighted and foreign dividends (ADR 0104)

**Files:** new pack tables `econ_labour_shares.arrow` (WIOD SEA 2000 relative shares rescaled so the VA-weighted mean equals PWT), `econ_wealth.arrow` (WID decile shares, region fallback), `econ_fdi.arrow` (IMF CDIS 2009 inward positions; f_c,g capped at 0.8), built by new `crates/ceteris-pack/src/econ/factors.rs` from readers in `crates/ceteris-pack/src/read/` (`sea.rs`, `wid.rs`, `cdis.rs`) and `read::people::read_pwt`; `crates/ceteris-econ/src/production.rs` (`s_L` per enterprise), `systems.rs` (E4 dividends by wealth share; foreign share via C2's transfer account), `crates/ceteris-calibrate/src/steady.rs`, `crates/ceteris-reference/src/io.rs` (`ent_s_l` per enterprise) and `model.rs`. Download terms checked per source before fetch; a source behind a form or a bot check goes to the owner.

- [ ] Failing tests: `labour_share_va_weighted_mean_equals_pwt` (new `crates/ceteris-pack/tests/factors.rs`, 1e-12); `dividend_shares_integrate_wealth_over_income` (p2_properties.rs); `foreign_dividends_net_to_zero_worldwide` (p2_properties.rs); `missing_source_falls_back_with_flag` (factors.rs). Implement, commit `feat(econ): sector labour shares and wealth-weighted dividends (adr 0104)`.

### Task C5: Initial drift (ADR 0103)

**Files:** `crates/ceteris-pack/src/econ/build.rs` (`g_A`, `g_K` from PWT rtfpna 1995-2000 via `read::people::read_pwt`, relative to the world, empirical-Bayes shrinkage to the regional mean; capital-output gap), `crates/ceteris-econ/src/params.rs` (`drift: bool`, half-life h register P153, prior 3 to 15 years), `systems.rs` (A and F grow by the faded drift each tick), `crates/ceteris-econ/tests/p2_steady_state.rs` and `crates/ceteris-cli/tests/p2_gate.rs` (run with drift off).

- [ ] Failing tests: `drift_off_keeps_the_steady_state` (`fixture_calibrated_state_holds_to_1e_9` under `drift = false`); `drift_fades_with_half_life`; `drift_is_zero_at_the_first_tick`. Implement, commit `feat(econ): initial drift off the steady state (adr 0103)`.

### Task C6: Recalibrate and re-run the gate

- [ ] Rebuild the pack econ tables and recalibrate on the Rust-built pack (Global Constraints commands), install, re-export, re-bless goldens (own commit), run the full gate as B8 Step 3, re-measure budgets, update the SPECs and ADRs 0098 to 0100, 0103, 0104 (status, as built).

### Task C7: Rerun the 28 October benchmark (ADR 0207 gate)

Closes pre-P3's economics. D and E do not move the economics (E reproduces the goldens at `L = 1`); if either moves a golden, this task reruns at the end of E.

**Files:**
- Modify: `crates/ceteris-calibrate/tests/benchmark.rs` (the three criteria), `crates/ceteris-calibrate/src/benchmark.rs` (if a metric is missing), `specs/econ/SPEC.md` (P2 gate: a "Pre-P3 benchmark" entry with the numbers and the command), `specs/trade/SPEC.md` (Test plan: the benchmark cases; Open questions on a miss), `adr/0207` (a line under As built)

**Interfaces:**
- Consumes: B2b's `shock` and `benchmark` modules; `econ_home_shares.arrow`.

- [ ] **Step 1: Failing tests** (pack-gated, release): `openness_within_1_5_of_icio` (baseline run: for each theta class and for all traded goods, `foreign_share` over ICIO countries in [1/1.5, 1.5] times the ICIO share from `econ_home_shares.arrow`; prints per-country ratios and world goods exports over income against ICIO's 0.14); `acr_formula_holds_under_costlier_borders` (`border-1.1.shocks` against the baseline: sign agreement for every ICIO country with |ACR| > 1e-4, median model/ACR in [0.8, 1.25], correlation >= 0.9); `tenfold_border_shock_converges` (`border-10.shocks`, 1,200 ticks: no invariant violation, `max_dlnp_last` <= 1e-6, world price index finite).
- [ ] **Step 2: Run** `CETERIS_PACK=$PACK cargo test -p ceteris-calibrate --release --test benchmark -- --nocapture`. They are expected to pass after C6; any failure is the finding, not a test to adjust.
- [ ] **Step 3: Record** every number (class and world foreign shares against ICIO, the ACR ratio distribution and correlation, JPN, GBR and EU27 welfare, the x10 convergence path every 50 ticks and the final index) in the econ SPEC beside the 28 October values (openness about 3 times the data; median 0.66 of ACR, correlation 0.57; x10 stopped at tick 45). A miss of any criterion stops work and goes to the owner with the table; no gain or friction is changed to pass (CLAUDE.md, no silent tuning).
- [ ] **Step 4: Commit** `test(calib): rerun the 28 october benchmark as the pre-p3 gate (adr 0207)` and the records, then the PR for C.

---

## Branch D: `pre-p3/p3-prereqs` (Section 4 and the P2 minors)

Starts from main now, in parallel with C, and rebases after C6 (owner decision 12); D5 can go any time. Not yet rewritten for the migration: D1 and D2's ADR is the next free number (0243 at 7219de2), not 0113; D4's `python/tests/test_reference.py:59` and D5's `pytest python/tests/test_p2_gate.py` read as `crates/ceteris-reference/tests/reference.rs` and `cargo test -p ceteris-cli --release --test p2_gate` with `CETERIS_PACK_EXPORT` (ADR 0113).

### Task D1: Snapshots across network changes

As built (RFC 0285, branch `claude/project-thread-qd8p06`): `Sim::intervene` logs between-tick edits and seek replays them; snapshots name the network by its content digest (not a version number) and carry both parameter sets (format 3); the static digest keeps its meaning. D3's list is in RFC 0285, As built (ADR 0097 is not edited: its file predates the RFC split). D4: the Review Focus references at SPEC Equations and Test plan are replaced; `test_reference.py:59` no longer exists after the migration.

**Files:** `crates/ceteris-econ/src/world.rs` (`set_network`, `encode`, `restore`, `static_digest`), `crates/ceteris-core/src/sim.rs`, new `adr/0113-interventions-and-snapshot-state.md`.

**Interfaces:** the static digest leaves out the network and keeps the inputs digest of everything else; `EconWorld` keeps an append-only `networks: Arc<Vec<Arc<Network>>>` (shared across restores, not state) and a `network_version: u32` that is state; `restore` rebuilds the kernel when the snapshot's version differs from the live one.

- [x] Failing tests: `seek_across_network_change_replays` (snapshot at t, `set_network` at t + 5, seek to t + 2, run to t + 20: hashes equal the continuous run); `null_intervention_reproduces_parent` (same network set again: identical hashes). Implement, commit.

### Task D2: Structural parameters as ex-post interventions

**Files:** `crates/ceteris-econ/src/world.rs` (`set_params`, `set_trade_params`), `equilibrium.rs` (`from_equilibrium` documented path), `bin/ceteris.rs` (guard hint without a CLI flag name), `specs/econ/SPEC.md` (Initial state describes the guard and the intervention path), ADR 0113.

**Interfaces:** `EconWorld::set_trade_params(&mut self, trade: TradeParams)` and `set_params(&mut self, params: EconParams)` between ticks; a structural change rebuilds the kernel; both parameter sets are encoded in the snapshot (codec version bump) and leave the static digest. Load stays guarded: calibrated tables start only under the structural parameters they were built with. An edit of the PIGL parameters moves the world off its steady state like any other structural edit, since the calibrated weights were inverted at the defaults; the SPEC says so.

- [x] Failing tests: `seek_across_param_edit_replays`; `theta_sweep_starts_from_baseline_steady_state` (load the fixture calibration, `set_trade_params` with theta x 1.4 at tick 0, tick 0 state equals the baseline's); `load_guard_still_panics_with_other_structural_params`. Implement, commit.

### Task D3: Warm-start audit

- [x] List every iterative solve whose result enters state (`clearing_wage` warm-starts from the last wage, which is state; B7's storage and B6's anchor are closed form) and assert in `seek_across_*` tests that a cold-restored child matches. Record the list in ADR 0097 as built. Commit `test(econ): warm starts are snapshot state (adr 0097)`.

### Task D4: P2 minors

- [x] One batch: the SPEC Initial state guard text (if D2 left any), "Review Focus" references at `python/tests/test_reference.py:59` and SPEC lines near 407 and 821 replaced by SPEC section names, an assert in `from_pack` that the pack `coverage` column equals `TradeParams::coverage` (or drop the column), `--data-state` rejected on uncalibrated packs, `demand.rs` near line 126 wording. Commit `chore(econ): p2 review minors`.

### Task D5: Nightly pack gate on a self-hosted runner

**Files:** `.github/workflows/pack-gate.yml`, `scripts/pack-gate.sh`, `docs/pack-runner.md`.

- [ ] Workflow: `on: schedule` (nightly, 03:00 local) and `workflow_dispatch` only, never `pull_request`; `runs-on: [self-hosted, macOS, ARM64, pack]`; `CETERIS_PACK` from a runner environment variable; steps: export, `cargo test -p ceteris-econ --release --test p2_acceptance`, `pytest python/tests/test_p2_gate.py` with `CETERIS_PACK_EXPORT`, upload the logs. `docs/pack-runner.md` gives the owner the registration steps (runner download, `./config.sh --labels pack`, `./svc.sh install`). The owner registers it; the controller does not handle the token. Verify with one `workflow_dispatch` run after registration. Commit `ci: nightly full-pack gate on a self-hosted runner`.

---

## Branch E: moved to P3 Branch L (ADR 0288)

Kept here for its requirements, which P3 Branch L builds; it starts once D merges, so kernel signatures are final. This branch gets its own plan then (signatures written now would be stale after B and C). The retrofit generic over `Real` and the `Dual<N>` gradients (ADR 0096) moved to P9 (ADR 0207); E does not introduce a `Real` trait, and its lane layout must not preclude adding one later. Its fixed requirements:

- Economic kernels and the route-choice solve generic over `const L: usize` (ensemble lanes, ADR 0024), lane innermost after goods; read-only data stored once; lanes differ only in their parameters and random streams (common random numbers per lane as ADR 0024 sets them).
- Release builds instantiate `L = 1`; goldens reproduce bit for bit, at 1, 3 and all threads.
- Lane l of an `L = 8` run reproduces, bit for bit, the `L = 1` run with lane l's parameters and seed.
- Criterion benchmarks of `L = 8` against `L = 1` set the ensemble budget in the econ SPEC.

---

## Branch F: `pre-p3/design-snapshot` (Section 6, independent)

### Task F1: Design doc and DESIGN.md snapshot

- [ ] Apply the econ SPEC "Changes for the next design-doc snapshot" list to the Claude Docs design doc (paginated read; see the project memory for the link) and refresh `specs/DESIGN.md` from it, re-grepping line numbers first. One commit, `docs(design): refresh the snapshot with p2 as built`. P2 UI (map modes, province panel, time controls) is a separate plan and is not touched.

---

## Order and parallelism

A is merged (PR #28). F runs any time. D5 can run as soon as the owner registers the runner. B starts now from main 7219de2 (owner decision 9); within B, B1 and B3 to B7 are independent of each other, B2 follows B1 (class values), B2a follows B2, B2b follows B2a, and B8 runs last. Then C (C1 to C5 in order, C6, C7), with D in parallel from main (owner decision 12), rebased after C6; each PR is reviewed by the owner. Pre-P3 closes when C7's three criteria pass (or the owner accepts a recorded miss) and D has merged; E is P3 Branch L (ADR 0288).
