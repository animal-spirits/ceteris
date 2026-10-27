# Pre-P3 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring P2 as built (main fc7ce3d) to the state P3 (counterfactual workbench, DESIGN 21.1 item 4) needs: enforced determinism and the dispatch speed plan, the P2 economics review (ADRs 0090 to 0095, 0101), main's assumption-audit ADRs (0098, 0099, 0100, 0103, 0104, 0106), P3 prerequisites (snapshots across interventions, structural edits, lanes and Real), the open P2 issues, and the records.

**Architecture:** Five code branches, each a PR off the previous one's merge (A, then B, C, D, E), plus one records branch (F) that is independent. A changes speed and the kernel; B changes the price rule, storage and the trade frictions, then recalibrates and re-runs the P2 gate; C changes the calibration data and the demand and income blocks, then recalibrates and re-runs the gate; D adds what P3's seek, branching and sweeps need; E retrofits the kernels generic over `Real` and a lane count once the economics have settled. The Python GE reference mirrors every model change, because V1 compares against it.

**Tech Stack:** Rust 2024 workspace (`ceteris-core`, `-geo`, `-trade`, `-econ`; rayon, libm, arrow), Python 3 via `uv` (`python/ceteris`: pack builders, GE reference, PPML, calibration), GitHub Actions.

**Spec:** `specs/econ/SPEC.md`, `specs/trade/SPEC.md` (as built), ADRs 0089 to 0112, the P2 ledger `.superpowers/sdd/PLAN/progress.md` and `.superpowers/sdd/PLAN/final-review.md` (main checkout), and the owner request of 2026-10-26 (thread "Prepare the simulator for P3").

## Owner decisions (2026-10-26)

1. Re-bless order: 0097 lints (no hash change), then gather dispatch proven bit-identical to the scatter (no re-bless), then one `Crossing` rewrite carrying the stored reciprocals, ADR 0093's kernel range and log-domain weights and ADR 0092's gateway mass weights, with one re-bless (as built: the masses moved to Task B2 by owner decision, 2026-10-26, and A re-blessed three times, ADR 0097). The economics re-bless again at each recalibration.
2. ADR 0090's anchor is redesigned as lbar_T = ln(sum_T p (D + delta S*) / sum_T p Q) over traded markets only; kappa_r starts traded 0.25, local 0.05.
3. ADR 0091 storage lives at origin markets; producer stock (`origin/p2-r3-producer-stock`) stays parked, revisited only if the V1 stress test still fails after B.
4. All of 0106, 0100, 0098, 0103 (drift switch off in the P2 steady-state test and V1), 0099 and 0104 land before P3.
5. One retrofit, generic over `Real` (ADR 0096) and a const lane count `L` (ADR 0024); f64 with `L = 1` reproduces the goldens bit for bit.
6. A structural parameter change on a calibrated pack is an ex-post intervention from the baseline steady state; the load guard stays.
7. A self-hosted GitHub runner on the owner's Mac runs a nightly full-pack gate; the owner registers it.
8. V1 excludes cells whose one-market J2 fails at the run's gains, and reports them separately.

## Controller rulings in this plan (cost if wrong in brackets)

- ADR 0111's rational-expectations storage switch for 0091 is deferred to P9, where it is fitted and could become the default; P2 keeps adaptive expectations (owner choice D5b). [one task in P9]
- ADR 0103's drifts are not netted against ADR 0108's catch-up term: the catch-up equation is not in the model before P4, so nothing is counted twice yet; the netting lands with 0108. [drift values move when 0108 lands]
- ADR 0102 is not in scope (not requested); ADR 0092's nu fit runs conditional on the current P04. [nu refit when 0102 lands]
- Parameters of a run become snapshot state (Task D2), because an ex-post edit of them must survive a restore. [codec bump, done once with 0106's]
- New ADR 0113 records snapshots across interventions and parameters as state (D1, D2). The anchor redesign is a section of ADR 0090, not a new ADR.

## Global Constraints

- Determinism: identical state hashes at 1 thread, 3 threads and all threads; parallel float sums only through `ceteris_core::reduce::fixed_sum` or fixed slots combined in index order; no `HashMap`/`HashSet` reaching state.
- Money is exact: i64 transfers rounded half to even (pops and enterprises), i128 for aggregate accounts after B5; every invariant in `EconWorld::end_tick` keeps holding.
- `CETERIS_PACK` is an absolute path; the pack is `data/packs/2000`.
- Calibrate: `uv run --project python --locked python -m ceteris.reference calibrate` writes `target/calibration/<pack>`; install the tables into the pack yourself.
- Python reference mirrors the Rust model: routing and payments to 1e-12 relative (today 3e-16), the fixture equilibrium within the existing test tolerances.
- Goldens (`tests/golden/p2-econ.txt`, `p2-pack.txt`, `p2-fixture/`) change only in a commit whose subject says re-bless and why.
- Every new parameter takes a register ID: next free P150. Reserved: P143 kappa_n, P144 psi, P145 zeta, P146 g_s (0091), P147 M_F (held), P56 storage cost.
- ADR numbers: next free 0113; check origin/main and open design PRs before adding one.
- Budgets (M4 Pro, full pack, criterion medians): `source_dispatch_parallel` 8 ms, whole tick 35 ms. Re-measure at the end of A, B and C.
- Never bypass bot checks; browser-only sources are downloaded by the owner. Public downloads are approved; pin each in `fetch.toml` with sha256 and a `sources.toml` licence row.
- Delete files by moving them to the Trash.
- Conventional Commits, no attribution, no em dashes. Comments only for counterintuitive logic; no process references (task numbers, briefs) in code.
- Code PRs need the owner's review; ADR-only PRs may be merged once green.

## Review Focus

1. **Seek across an intervention.** A snapshot taken before `set_network` or a parameter edit, restored after it, must replay to the same hashes as the continuous run (D1, D2: `seek_across_network_change_replays`, `seek_across_param_edit_replays`).
2. **A good with no traded activity in the anchor.** If sum_T p Q is 0 (no active traded market, or all output zero after a shock), lbar_T must not be NaN or infinite; the aggregate term is 0 that tick (B6: `anchor_is_zero_without_traded_supply`).
3. **Old snapshots after the codec bump.** Decoding a version-N snapshot after B5 (i128) or D2 (params) returns `DecodeError` naming the version, never a panic or a silent misread (B5: `old_codec_version_is_a_decode_error`).
4. **Storage at a stockout.** With S = 0 and a falling price expectation, storage sales cannot take S' below 0, and the price may move up to c_max 0.3 in one tick (B7: `storage_never_sells_below_zero`, `stockout_raises_price_more_than_surplus_lowers_it`).
5. **Countries missing a new source.** A country without an ICP price level, a current account, a WID wealth series or a SEA labour share takes the documented fallback with its flag set, never NaN (C1, C2, C4: `missing_source_falls_back_with_flag` in each pack test).

---

## Branch A: `pre-p3/speed` (Section 1, ADR 0097)

### Task A1: Determinism lints

**Files:**
- Create: `clippy.toml`, `crates/ceteris-core/tests/clippy_bans.rs` (fixture, see Step 1)
- Modify: `crates/ceteris-econ/src/calibration.rs:290-300`, `crates/ceteris-econ/src/export.rs:133`, `crates/ceteris-core/src/snapshot.rs:4,193`, `crates/ceteris-econ/src/world.rs:1330-1360`, every other site the lint flags
- Test: `scripts/check_clippy_bans.sh`, `.github/workflows/ci.yml`

**Interfaces:**
- Produces: workspace `clippy.toml` with `disallowed-methods` and `disallowed-types` exactly as ADR 0097 lists them, each `reason` naming ADR 0097.

- [ ] **Step 1: Write the failing fixture.** `scripts/check_clippy_bans.sh` copies a throwaway crate (`scripts/clippy-bans/`, outside the workspace members, with its own `Cargo.toml` pointing at the workspace `clippy.toml` via `CLIPPY_CONF_DIR`) that calls each banned rayon method, names `HashMap` and `HashSet`, and calls `f64::algebraic_add`; the script runs `cargo clippy -- -D warnings` on it and exits 0 only if clippy fails with one `disallowed` diagnostic per item (count them). The algebraic methods are unstable: if clippy cannot resolve their path, keep them in `clippy.toml` (ADR 0097 says an unresolvable path fails the build; check whether current clippy errors or warns, and record which in the ADR as built).
- [ ] **Step 2: Run it, expect failure** (no `clippy.toml` yet, so clippy passes the fixture and the script exits 1).
- [ ] **Step 3: Add `clippy.toml`; swap the metadata maps in `calibration.rs` and `export.rs` to `BTreeMap`; add `#[expect(clippy::disallowed_types, reason = "keyed lookup only, never iterated (ADR 0097)")]` on the `SnapshotStore` blob map; replace the journal scan's `par_chunks(1 << 16).fold(..).reduce(..)` in `world.rs` with fixed slots: one `Vec<i64>` per chunk index written in parallel, combined in chunk order.**
- [ ] **Step 4: Run** `scripts/check_clippy_bans.sh` (exit 0), `cargo clippy --workspace --all-targets -- -D warnings` (clean), `cargo test --release --workspace` (goldens unchanged), and add the script to CI.
- [ ] **Step 5: Commit** `build(lint): ban order-dependent reductions and hash maps (adr 0097)`.

### Task A2: Gather-parallel dispatch, bit-identical

**Files:**
- Modify: `crates/ceteris-trade/src/source.rs` (`dispatch`, the origin and domestic passes), `crates/ceteris-trade/src/kernel.rs` (`ByColumn` if a chunk API is needed)
- Test: `crates/ceteris-trade/src/source.rs` tests, `crates/ceteris-econ/benches/trade.rs`

**Interfaces:**
- Consumes: `Kernel::crossings_by_target`, `access_by_gateway`, `domestic_by_destination` (`ByColumn::column(c) -> &[(u32, u32)]`, row order).
- Produces: `dispatch(k: &Kernel, g: usize, price: &[f64], fill: &[[f64; 2]], out: &mut Sourced)` unchanged in signature; the old body kept as `#[cfg(test)] fn dispatch_scatter(..)` for the property test.

- [ ] **Step 1: Write the failing property test** `gather_dispatch_equals_scatter_bit_for_bit`: proptest over random small kernels (2 to 4 countries, 1 to 3 gateways each, random prices and fills including zeros); every field of `Sourced` written by `dispatch` equals `dispatch_scatter`'s by `to_bits()`, including `lost`.
- [ ] **Step 2: Run it**; it passes trivially while `dispatch` is still the scatter. Make it meaningful by first switching `dispatch` to call the new gather body, then run: expect failure until the gather is written.
- [ ] **Step 3: Implement the gather** as ADR 0097 step 2 describes, without the reciprocals: per target, sum incoming legs over the `ByColumn` lists in row order, targets in fixed chunks in parallel; `lost` as per-chunk partials combined with `fixed_sum`. Where the bit order of `lost` or of `paid[d]`, `received[o]` cannot be kept, stop and report: the owner chose a bit-identical gather.
- [ ] **Step 4: Run** the property test, `cargo test --release --workspace` (goldens unchanged at 1, 3 and all threads: `RAYON_NUM_THREADS`), and `cargo bench -p ceteris-econ --bench trade` (record `source_dispatch_parallel` and one-good dispatch).
- [ ] **Step 5: Commit** `perf(trade): gather dispatch by target (adr 0097)`.

### Task A3: One `Crossing` rewrite and its re-bless

**Files:**
- Modify: `crates/ceteris-trade/src/kernel.rs` (`Entry`, `Crossing`, `Kernel::build`, `origin_access_into`), `crates/ceteris-trade/src/source.rs` (`origin_weights`, `price_index`, `source`, `dispatch`), `crates/ceteris-trade/src/network.rs` (gateway mass), `python/ceteris/packs/econ/gateways.py` and `build.py` (mass column), `crates/ceteris-econ/src/pack.rs` (read it), `python/ceteris/reference/model.py` (mirror), `specs/trade/SPEC.md` (Kernel, Dispatch, rounding), `tests/golden/*`
- Test: `crates/ceteris-trade/src/kernel.rs`, `source.rs` tests, `python/tests/test_reference.py`, `python/tests/test_econ_pack.py`

**Interfaces:**
- Produces:
  - `Crossing { col: u32, ln_k: f32, inv_duty: f64, inv_loss: f64, lag: u8 }` with `inv_duty = 1 / (1 + t)`, `inv_loss = 1 / (tau * border_tau)`; `Entry { col: u32, ln_k: f32, inv_tau: f64 }`. ln K in f32 (ADR 0093 option one; f64 K only if a test shows f32 ln K loses more than 1e-7 relative in K).
  - `Network.gateway_mass: Vec<f64>`, summing to 1 over each country's gateways; pack column `gateways.arrow: mass` = the gateway's share of its country's 2000 trade throughput (port and airport tonnage or value where the pack has it, else 1/k_A).
  - Origin weights stored as `ln_w: Vec<f64>`, shifted per country by its maximum; gateway sums by log-sum-exp; reachability tested on ln Phi_d; the 1e-100 floor removed except for K = 0 (no path).
- Consumes: A2's gather.

- [ ] **Step 1: Failing tests:** `wide_price_spread_keeps_both_countries_reachable` (two countries, price ratio 1e12, theta 10.57; flows match an f64 dense reference to 1e-12 relative); `kernel_round_trip_at_extreme_tau` (tau 1e4, theta 64.85; ln K finite, K exp'd matches libm to 1e-7); `duplicating_a_gateway_with_half_mass_changes_no_flow` (1e-12); `test_gateway_mass_sums_to_one` (pack); Python `test_routing_matches_rust` updated to the mass-weighted kernel.
- [ ] **Step 2: Run, expect failures.**
- [ ] **Step 3: Implement** the struct changes, log-domain weights, mass weights (fold m into access and crossing legs so the low-rank evaluation is unchanged), reciprocals in `dispatch` and the access loops; mirror in `model.py`.
- [ ] **Step 4: Rebuild the pack econ tables** (gateway mass), re-export the fixture goldens, recalibrate (structural digest moves) and install the tables; run `cargo test --release --workspace` with `CETERIS_PACK`, `pytest python/tests` with `CETERIS_PACK_EXPORT`, `cargo bench -p ceteris-econ`. Record the tick and `source_dispatch_parallel` against 35 and 8 ms in the trade SPEC.
- [ ] **Step 5: Commit** code and records, then `test(golden): re-bless for reciprocals, log-domain weights and gateway mass (adrs 0092, 0093, 0097)` alone.

### Task A4: Typed, compressed history

**Files:**
- Modify: `crates/ceteris-econ/src/history.rs`, `crates/ceteris-econ/src/bin/ceteris.rs`, `Cargo.toml` (one compression dependency), `sources.toml` or `LICENSES` record if the repo keeps crate licences
- Test: `crates/ceteris-econ/src/history.rs` tests, `crates/ceteris-econ/tests/history_cli.rs`

**Interfaces:**
- Produces: `History` buffers with implicit tick, province and good keys and native widths (u32 keys, f64 values); on-disk format `run_history_*.arrow` unchanged in columns; `ceteris run --history-format csv` writes the same tables as CSV.

- [ ] **Step 1:** Licence check: `cargo info alp` (or crates.io) for the `alp` crate's licence and maintenance; acceptable licences are those `deny.toml` (or the workspace's licence policy) allows. If none fits, use Arrow IPC's own zstd buffer compression (`arrow-ipc` `zstd` feature) with byte-stream-split, which needs no new crate. Record the choice in ADR 0097 as built.
- [ ] **Step 2: Failing tests:** `history_round_trips_losslessly` (decode equals every recorded f64 by bits), `history_memory_per_market_row_at_most_49_bytes` (buffered bytes over rows), `history_csv_matches_arrow`.
- [ ] **Step 3: Implement.** Recording only reads state: the existing `recording_leaves_the_run_unchanged` and `history_is_deterministic` must still pass.
- [ ] **Step 4: Run** history tests and a 120-tick full-pack run with history; record peak memory (was 1.88 GB).
- [ ] **Step 5: Commit** `perf(econ): typed and compressed run history (adr 0097)`.

### Task A5: Fused multi-good pass (conditional)

- [ ] **Step 1:** If after A3 `source_dispatch_parallel` is within 8 ms and one good's dispatch is no longer the critical path (profile: it is if the slowest good exceeds 80% of the stage), record that and skip this task.
- [ ] **Gateway count (deferred here from Task 19b):** with A3's mass weights, access no longer scales with the number of gateways, so the count can be tuned for speed. Measure `source_dispatch_parallel` and the H2 distance coefficient at the current 600 gateways and at a smaller count (land crossings joined only to their neighbour, ledger Open questions); propose a count to the owner only if it buys at least 1 ms with Phi_d moving under 1e-3.
- [ ] **Step 2 (if needed):** per-crossing per-good `ln_k` stored goods-innermost; inner arithmetic as a fixed-width loop over goods, each good keeping its own operation order. Test: state hashes and goldens unchanged at 1, 3 and all threads. Commit `perf(trade): fused multi-good dispatch (adr 0097)`.

### Task A6: Records and PR

- [ ] ADR 0097 gets an "As built" section (lints result, gather numbers, reciprocal rounding, history format and crate, A5 outcome) and status accepted; ADR 0093 and 0092 note which parts landed in A3. Trade SPEC budgets updated. Open the PR to main.

---

## Branch B: `pre-p3/econ-review` (Section 2, plus 0106)

Starts from main after A merges.

### Task B1: Refined petroleum into good 1 and harmonic class theta (ADRs 0092, 0101)

**Files:**
- Modify: `python/ceteris/packs/econ/goods.py`, `icio.py`, `theta.py`, `build.py`; `crates/ceteris-trade/src/params.rs` (`TradeParams` theta defaults, `theta_scale`), `crates/ceteris-econ/src/params.rs` if goods names live there; `specs/econ/SPEC.md` (Goods, Parameters P01); `adr/0023`
- Test: `python/tests/test_icio.py`, `test_econ_pack.py`, Rust param tests

**Interfaces:**
- Produces: good 1 "Crude oil, gas and refined fuels" (ICIO B05, B06, C19); pack metadata records both the arithmetic and the harmonic class means; `TradeParams.theta` per good from ADR 0023's rule on the harmonic means; `TradeParams.theta_scale: f64` (new register P150, default 1, lane values {0.6, 1, 1.4} for tariff experiments, applied as theta_g * theta_scale in `Kernel::build`).

- [ ] **Step 1: Failing tests:** `test_c19_in_good_1` (good 1 and good 5 totals equal the ICIO aggregation with C19 moved, 1e-9); `test_class_theta_is_harmonic` (theta_class = (sum w_l / theta_l)^-1 on a two-line toy class); `test_welfare_first_order_at_harmonic_theta` (ADR 0101's toy test); Rust `theta_scale_multiplies_every_class`.
- [ ] **Step 2:** run, expect failures. **Step 3:** implement; widen the P01 prior to cover the harmonic values. **Step 4:** rebuild the pack econ tables, run the pack tests, re-run the ADR 0022 gravity diagnostics on the new flows (`python -m ceteris.reference` diagnostics as in Task 18a) and record them in ADR 0022. **Step 5:** commit `feat(econ): move refined fuels into good 1 and use harmonic class theta (adrs 0092, 0101)`.

### Task B2: Gateway masses and nu per theta class, fitted to the cross-border share (ADR 0092)

Carries ADR 0092's gateway mass weights, moved here from Task A3 by owner decision (2026-10-26) after H2 distance missed at -0.536 with masses alone. The mass code was built in af90fa4 and removed in 1662fab; recover the mass parts from af90fa4 (`git show 1662fab` lists them) rather than rewriting them, onto the kernel as it stands after Branch A (f64 K, reciprocals, per-country log weights).

**Files:**
- Modify: `crates/ceteris-trade/src/network.rs` (`gateway_mass`), `crates/ceteris-trade/src/kernel.rs` (masses folded into access and crossing legs, per-class nu), `crates/ceteris-trade/src/params.rs` (`border_nu: [f64; N_CLASSES]`), `crates/ceteris-econ/src/pack.rs`, `inputs.rs`, `fixture.rs` and `export.rs` (read the mass column, export it), `python/ceteris/packs/econ/gateways.py` (mass column), `python/ceteris/reference/io.py`, `model.py`, `sinkhorn.py` (mirror the masses), `python/ceteris/reference/calibrate.py` (new stage before stage 1), `python/tests/test_p2_gate.py` (H2 per class bands), `specs/trade/SPEC.md`, `specs/econ/SPEC.md`, `tests/golden/*`
- Test: `crates/ceteris-trade/src/source.rs` tests, `python/tests/test_econ_pack.py`, `test_reference.py`, `test_calibrate.py`, `test_p2_gate.py`

**Interfaces:**
- Produces:
  - `Network.gateway_mass: Vec<f64>`, summing to 1 over each country's gateways; pack column `gateways.arrow: mass` = the gateway's share of its country's 2000 trade throughput (port and airport tonnage or value where the pack has it, else 1/k_A). Masses are folded into the access and crossing legs (K_oa m_a K_ab m_b K_bd), so the low-rank evaluation and the stored reciprocals are unchanged, and access no longer grows with the number of gateway pairs.
  - `calibrate.fit_border_nu(model, theta_c) -> np.ndarray` (one nu per class, monotone 1-D root on the absorption-weighted cross-border share of the class in ICIO 2000, on the mass-weighted kernel); written into the calibration tables and read into `TradeParams.border_nu` (P05 becomes three calibrated values).
- H2 per class gate: each class's PPML estimate within 2 s.e. of theta_c (ADR 0092), replacing the P01-wide band. The kernel-control regression (model's own kernel instead of ln distance) is reported beside it. The H2 distance check is re-run after the nu fit: masses alone moved it from -0.753 to -0.536 (band -1.1 to -0.7).

- [ ] **Step 1: Failing tests:** `duplicating_a_gateway_with_half_mass_changes_no_flow` (1e-12) and Python `test_gateway_mass_sums_to_one` (pack), both dropped from A3 with the masses; Python `test_routing_matches_rust` updated to the mass-weighted kernel; `test_nu_fit_matches_cross_border_share` (1e-6 per class on the fixture).
- [ ] **Step 2: Run, expect failures.**
- [ ] **Step 3: Implement** the masses (recovered from af90fa4) and their Python mirror, then the per-class nu fit.
- [ ] **Step 4: Rebuild the pack econ tables** (mass column), re-export the fixture goldens, recalibrate and install the tables; run `cargo test --release --workspace` with `CETERIS_PACK`, `pytest python/tests` with `CETERIS_PACK_EXPORT`; run the H2 distance check and the per-class gate after the nu fit and record both in the trade SPEC.
- [ ] **Step 5: Repeat the gateway-count measurement** of Task A5 (ADR 0097, As built) with the masses in: 300 against 600 gateways by the pack's rule, parallel source and dispatch time, |d ln Phi_d| at the same prices, whether the 300 pack calibrates and H2 there. Record it in ADR 0097 and the trade SPEC (Open questions); propose a count only if Phi_d moves less than 1e-3.
- [ ] **Step 6: Commit** `feat(trade): weigh gateways by mass (adr 0092)`, `feat(calib): fit border friction per theta class (adr 0092)`, then the re-bless alone.

### Task B3: Exact local fill and uncapped wage search (ADR 0093)

**Files:** `crates/ceteris-econ/src/local.rs` (`fill`, `split`), `crates/ceteris-econ/src/labour.rs` (`clearing_wage` caller in `systems.rs`), mirror in `python/ceteris/reference/model.py`; tests in `crates/ceteris-econ/tests/p2_properties.rs`.

**Interfaces:**
- Produces: `local::fill_exact(users: &LocalUsers, stock: &[f64; N_LOCAL]) -> [f64; N_LOCAL]` enumerating the at most 2^3 regimes, keeping the largest feasible fill; the 32-pass bound and ad hoc clearing removed. The frictionless search finds the root of sum L*_ig(w) = (1 - u*) L^s on uncapped targets and caps only `allocate`.

- [ ] Failing tests: `exact_fill_equals_converged_iteration` (every fixture case and proptest small provinces, 1e-12); `local_chain_terminates_on_cycles` settles in one solve; `frictionless_wage_matches_closed_form_when_one_target_reaches_supply`. Implement, run, commit `fix(econ): exact local fill and uncapped wage search (adr 0093)`.

### Task B4: PIGL aggregation across pops (ADR 0094)

**Files:** `crates/ceteris-econ/src/demand.rs`, `crates/ceteris-econ/src/world.rs` (`household_preferences`), `python/ceteris/reference/demand.py`, `calibrate.py` (stage 4).

**Interfaces:**
- Produces: within-pop dispersion `s_w^2 = max(0, s_nat^2 - Var_k(ln m_k))` per country (population-weighted); inversion at the calibrated buyer prices q_d over the country's pops by a 1-D root on lambda; countries with s_w = 0 listed in the calibration report.

- [ ] Failing tests: `aggregate_data_state_shares_match_omega` (per country, 1e-12); `one_pop_inversion_equals_old_at_prices_one`; `between_and_within_variance_add_up`. Implement Rust and Python, run, commit `feat(econ): invert pigl over each country's pops (adr 0094)`.

### Task B5: Clip outside pi^e, wider clip for storables, i128 aggregates (ADR 0106)

**Files:** `crates/ceteris-trade/src/market.rs` (`reprice`), `crates/ceteris-econ/src/labour.rs` (`wage_step`), `crates/ceteris-trade/src/params.rs` (`c_max_storable` = 0.3, register P151), `crates/ceteris-core/src/ledger.rs` (i128 account class), `crates/ceteris-core/src/snapshot.rs` (`Encoder::i128`, `Decoder::i128`, codec version), `crates/ceteris-econ/src/world.rs` (pools, mint, markets, gateways on i128; pops and enterprises stay i64), `python/tests/test_p2_gate.py` (multi-start), trade SPEC Stability region.

**Interfaces:**
- Produces: `dlnp = pi^e + clip(gap terms, +-c_max_g)`, `dlnw = pi^e + clip(kappa_w (x + u*), +-c_max)` then the rigidity factor when negative; `clip_ticks` counts the gap clip. `Ledger` gains `money_wide(a) -> i128` and an account kind chosen at `open`; `transfer` between kinds checks i64 range on the narrow side and panics loudly on overflow. Snapshot header carries a codec version byte; decode of another version is `DecodeError`.

- [ ] Failing tests: `clip_lets_expected_inflation_through` (pi^e 0.2, zero gap: dlnp = 0.2); `storable_goods_clip_at_0_3`; `mint_survives_scale_1000` (the overflow at about x802 is gone); `old_codec_version_is_a_decode_error`; Python `test_calibration_unique_from_eight_starts` (prices +-50%, seeds 0 to 7, same fixed point to 1e-9; a failure is a finding for the owner). Implement, run, commit `feat(core): i128 aggregate accounts and pi^e outside the clip (adr 0106)`.

### Task B6: Split price rule with the traded-market anchor (ADR 0090, owner redesign)

**Files:** cherry-pick `origin/p2-adr0090` 56b8e7f as the starting point; `crates/ceteris-trade/src/market.rs`, `crates/ceteris-trade/src/params.rs` (`kappa_r_traded` 0.25, `kappa_r_local` 0.05 as P03a split into P03a-t and P03a-l, `kappa_n` P143 0.2, `delta` per good P56), `crates/ceteris-econ/src/world.rs` (one world reduction per tick, `fixed_sum`), mirror in `model.py`; trade SPEC Origin market and Stability region; ADR 0090 "Anchor redesign".

**Interfaces:**
- Produces: with l_o = ln((D_o + delta_g S*_o) / Q_o), T the active traded markets,
  `lbar_T = ln(sum_T p_o (D_o + delta_g S*_o) / sum_T p_o Q_o)` (0 when the denominator is 0), and
  `dlnp_o = pi^e + clip((kappa_r,o / E_o)(l_o - lbar_T) + (kappa_n / h) lbar_T - (kappa2 / E_o)(S'_o - S*_o)/S*_o, +-c_max_g)`,
  E_o = theta_g + 1 traded, 1 local; h the steady-state household share of traded orders (fixed at load); kappa2 by the midpoint rule at the new gains. The Log|Linear switch of the trial stays, with the special-case golden `p2-as-built-rule.txt`.

- [ ] **Step 1: Failing tests:** `anchor_ignores_a_local_shortage` (fixture: a local good short by ln 2 in one province leaves lbar_T at 0 and moves only that price); `anchor_is_zero_without_traded_supply`; `nominal_shock_returns_within_30_ticks` (5% money shock, index within 1%); `special_case_matches_as_built_rule` (hashes equal `p2-as-built-rule.txt` over 120 ticks); `price_rule_stability.rs` Jury checks re-derived for the normalized rule.
- [ ] **Step 2:** run, expect failures. **Step 3:** implement.
- [ ] **Step 4: Stop rule.** Run the fixture data state and the pack data state, default and frictionless, 1,200 ticks; record the world index at 0, 36, 120, 360, 1,200 against the as-built rule (2.38 at 120, 2.02 at 360 on the pack). If either index exceeds 10 or the run overflows, stop and send the table to the owner before changing any gain.
- [ ] **Step 5:** commit `feat(trade): split price rule with a traded-market anchor (adr 0090)`.

### Task B7: Competitive storage at origin markets (ADR 0091)

**Files:** `crates/ceteris-trade/src/market.rs` (storage orders at T3), `crates/ceteris-econ/src/world.rs` (per-market `pi_e_market` state, encode and restore), `crates/ceteris-trade/src/params.rs` (`psi` P144, `zeta` P145, `g_s` P146, `k_g` money cost and `delta_g` P56 for goods 0, 1, 2, 5), mirror in `model.py`, ADR 0091 as built.

**Interfaces:**
- Produces: `pi_e_og <- pi_e_og + g_s (dlnp_og - pi_e_og)` after T4; `x_og = pi_e_og + y_g(S / (s_g D)) - k_g - delta_g`, `y_g(z) = (k_g + delta_g) z^-zeta_g`; storage orders `dS = delta_g S + psi_g D x`, floored so S' >= 0; the kappa2 term is off for goods 0, 1, 2, 5. Spoilage is a tallied goods flow.

- [ ] Failing tests: `storage_orders_equal_spoilage_at_steady_state`; ADR 0089's fixed-point test with storage; `storage_never_sells_below_zero`; `stockout_raises_price_more_than_surplus_lowers_it` (30% harvest cut vs surplus on agriculture); `storage_conserves_goods_and_money`; linearized region re-derived with psi and zeta on the toy market. The SPEC records the weak sources behind delta_g and k_g as an open question for P9. Implement, run, commit `feat(trade): competitive storage for storable goods (adr 0091)`.

### Task B8: V1 cell rule, recalibration and the P2 gate

**Files:** `python/tests/test_p2_gate.py`, `crates/ceteris-econ/tests/p2_acceptance.rs`, `python/ceteris/reference/uniqueness.py` (one-market J2 per cell at the run's gains), `specs/econ/SPEC.md` (P2 gate, Open questions), `tests/golden/*`, ADRs 0090 to 0095, 0101, 0106 (status, as built).

**Interfaces:**
- Produces: `uniqueness.cells_outside_region(model, gains) -> list[(province, good)]`; V1 asserts the 1% tolerances on every other cell and reports the excluded cells with their gaps; the test asserts the excluded count is at most 1% of active cells (else it is a finding).

- [ ] **Step 1:** failing `test_v1_excludes_cells_outside_region` on a fixture variant with one cell pushed outside J2.
- [ ] **Step 2:** recalibrate (ADR 0089) with B1 to B7, install the tables, re-bless goldens in their own commit.
- [ ] **Step 3: Full P2 gate** with the commands in the econ SPEC P2 gate section: V0, V1 (perturbed steady state), V1 stress (data state, still `xfail(strict)` unless it now passes, in which case report to the owner before removing the marker), H2 distance, H2 per class (B2's bands plus the kernel control), 100 years, 1 vs 8 threads, benchmarks against 8 and 35 ms. Record every number in the SPEC. A miss stops work and goes to the owner.
- [ ] **Step 4:** If the V1 stress test still fails, record the diagnosis (Mali and Albania pops, market losses) and ask the owner whether to revisit producer stock (decision 3).
- [ ] **Step 5:** commits, PR.

---

## Branch C: `pre-p3/audit` (Section 3)

Starts from main after B merges. Every pack change adds `sources.toml` rows and passes the release-profile licence check.

### Task C1: Price levels and source corrections (ADR 0098)

**Files:** `python/ceteris/packs/econ/icio.py` (1999-2001 rate rescale per country block), `people.py` (SWIID first, WDI fallback, `gini_source`), a new `python/ceteris/packs/econ/ras.py` (donor RAS to WDI sector VA and BACI trade by good, rest-of-world block constraint, 1e-10, at most 200 iterations), `build.py` (`price_level`), `crates/ceteris-econ/src/pack.rs`, `inputs.rs`, `world.rs` (`base_delivered_prices`, local base price p_c), `python/ceteris/reference/calibrate.py` (index over traded goods only).

**Interfaces:** `econ_countries.arrow` gains `price_level` (WDI PA.NUS.PPPC.RF / PA.NUS.FCRF, 2000 or nearest ICP-anchored year, flag `price_level_imputed`) and `gini_source`; local-good base price p_c in `ProvinceData::base_price` and in coef = a p_o / q.

- [ ] Failing tests: `test_rate_rescale_keeps_within_block_ratios`; `test_ras_matches_targets` (1e-10); `test_price_level_values_unchanged` (t = 0 values, shares, national accounts unchanged); `missing_source_falls_back_with_flag`. Data caveats from P2 fixed here: SAH population from WDI (not the 811-person WorldPop sum), a region override table for dependencies (AIA, MSR take Latin America and Caribbean donors), VEN income from WDI where PWT and WDI disagree by more than a factor of 3 (flagged). Implement, rebuild the pack, commit per concern.

### Task C2: Trade imbalances as transfers (ADR 0100)

**Files:** `build.py` (`ca_2000`, CA path from WDI BN.CAB.XOKA.CD 2000-2024), `calibrate.py` (remove r_g; absorption = income + D_c), `crates/ceteris-econ/src/world.rs` (world transfer account, i128, nets to zero each tick, asserted in `end_tick`), `crates/ceteris-econ/src/params.rs` (switch `Transfers::{Off, FixedShares, ObservedPath}`, register 17.2 entry).

- [ ] Failing tests: `transfers_net_to_zero_every_tick`; `off_switch_reproduces_balanced_trade`; `test_r_g_removed_and_residual_reported`. Implement, commit.

### Task C3: Three final demanders and the saving rate (ADR 0099)

**Files:** `icio.py` (HFCE, GGFC+NPISH, GFCF vectors of 10, g_c, s_c), `crates/ceteris-econ/src/demand.rs`, `systems.rs` (fiscal and investment accounts per country; each spends in full each tick), `params.rs` (accelerator beta, register P152, default 1), `calibrate.py` stage 4 (PIGL on HFCE; G and I fixed in the Sinkhorn targets), `model.py`.

- [ ] Failing tests: `money_stock_unchanged_by_fiscal_and_investment_accounts`; `beta_one_reproduces_fixed_share`; `pigl_inverted_on_hfce_only`. Implement, commit.

### Task C4: Sector labour shares, wealth-weighted and foreign dividends (ADR 0104)

**Files:** new pack tables `econ_labour_shares.arrow` (WIOD SEA 2000 relative shares rescaled so the VA-weighted mean equals PWT), `econ_wealth.arrow` (WID decile shares, region fallback), `econ_fdi.arrow` (IMF CDIS 2009 inward positions; f_c,g capped at 0.8); `crates/ceteris-econ/src/production.rs` (`s_L` per enterprise), `systems.rs` (E4 dividends by wealth share; foreign share via C2's transfer account), `calibrate.py`, `model.py`. Download terms checked per source before fetch; a source behind a form or a bot check goes to the owner.

- [ ] Failing tests: `labour_share_va_weighted_mean_equals_pwt` (1e-12); `dividend_shares_integrate_wealth_over_income`; `foreign_dividends_net_to_zero_worldwide`; `missing_source_falls_back_with_flag`. Implement, commit.

### Task C5: Initial drift (ADR 0103)

**Files:** `build.py` (`g_A`, `g_K` from PWT rtfpna 1995-2000 relative to the world, empirical-Bayes shrinkage to the regional mean; capital-output gap), `crates/ceteris-econ/src/params.rs` (`drift: bool`, half-life h register P153, prior 3 to 15 years), `systems.rs` (A and F grow by the faded drift each tick), `p2_steady_state.rs` and `test_p2_gate.py` (run with drift off).

- [ ] Failing tests: `drift_off_keeps_the_steady_state` (the existing 1e-9 test under `drift = false`); `drift_fades_with_half_life`; `drift_is_zero_at_the_first_tick`. Implement, commit.

### Task C6: Recalibrate and re-run the gate

- [ ] Recalibrate, install, re-bless goldens (own commit), run the full gate as B8 Step 3, re-measure budgets, update the SPECs and ADRs 0098 to 0100, 0103, 0104 (status, as built), PR.

---

## Branch D: `pre-p3/p3-prereqs` (Section 4 and the P2 minors)

Starts from main after C merges, except D5, which can go any time.

### Task D1: Snapshots across network changes

**Files:** `crates/ceteris-econ/src/world.rs` (`set_network`, `encode`, `restore`, `static_digest`), `crates/ceteris-core/src/sim.rs`, new `adr/0113-interventions-and-snapshot-state.md`.

**Interfaces:** the static digest leaves out the network and keeps the inputs digest of everything else; `EconWorld` keeps an append-only `networks: Arc<Vec<Arc<Network>>>` (shared across restores, not state) and a `network_version: u32` that is state; `restore` rebuilds the kernel when the snapshot's version differs from the live one.

- [ ] Failing tests: `seek_across_network_change_replays` (snapshot at t, `set_network` at t + 5, seek to t + 2, run to t + 20: hashes equal the continuous run); `null_intervention_reproduces_parent` (same network set again: identical hashes). Implement, commit.

### Task D2: Structural parameters as ex-post interventions

**Files:** `crates/ceteris-econ/src/world.rs` (`set_params`, `set_trade_params`), `equilibrium.rs` (`from_equilibrium` documented path), `bin/ceteris.rs` (guard hint without a CLI flag name), `specs/econ/SPEC.md` (Initial state describes the guard and the intervention path), ADR 0113.

**Interfaces:** `EconWorld::set_trade_params(&mut self, trade: TradeParams)` and `set_params(&mut self, params: EconParams)` between ticks; a structural change rebuilds the kernel; both parameter sets are encoded in the snapshot (codec version bump) and leave the static digest. Load stays guarded: calibrated tables start only under the structural parameters they were built with. An edit of the PIGL parameters moves the world off its steady state like any other structural edit, since the calibrated weights were inverted at the defaults; the SPEC says so.

- [ ] Failing tests: `seek_across_param_edit_replays`; `theta_sweep_starts_from_baseline_steady_state` (load the fixture calibration, `set_trade_params` with theta x 1.4 at tick 0, tick 0 state equals the baseline's); `load_guard_still_panics_with_other_structural_params`. Implement, commit.

### Task D3: Warm-start audit

- [ ] List every iterative solve whose result enters state (`clearing_wage` warm-starts from the last wage, which is state; B7's storage and B6's anchor are closed form) and assert in `seek_across_*` tests that a cold-restored child matches. Record the list in ADR 0097 as built. Commit `test(econ): warm starts are snapshot state (adr 0097)`.

### Task D4: P2 minors

- [ ] One batch: the SPEC Initial state guard text (if D2 left any), "Review Focus" references at `python/tests/test_reference.py:59` and SPEC lines near 407 and 821 replaced by SPEC section names, an assert in `from_pack` that the pack `coverage` column equals `TradeParams::coverage` (or drop the column), `--data-state` rejected on uncalibrated packs, `demand.rs` near line 126 wording. Commit `chore(econ): p2 review minors`.

### Task D5: Nightly pack gate on a self-hosted runner

**Files:** `.github/workflows/pack-gate.yml`, `scripts/pack-gate.sh`, `docs/pack-runner.md`.

- [ ] Workflow: `on: schedule` (nightly, 03:00 local) and `workflow_dispatch` only, never `pull_request`; `runs-on: [self-hosted, macOS, ARM64, pack]`; `CETERIS_PACK` from a runner environment variable; steps: export, `cargo test -p ceteris-econ --release --test p2_acceptance`, `pytest python/tests/test_p2_gate.py` with `CETERIS_PACK_EXPORT`, upload the logs. `docs/pack-runner.md` gives the owner the registration steps (runner download, `./config.sh --labels pack`, `./svc.sh install`). The owner registers it; the controller does not handle the token. Verify with one `workflow_dispatch` run after registration. Commit `ci: nightly full-pack gate on a self-hosted runner`.

---

## Branch E: `pre-p3/real-lanes` (ADRs 0024, 0096)

Starts after D merges, so kernel signatures are final. This branch gets its own plan then (signatures written now would be stale after B and C); its fixed requirements:

- A `Real` trait in `ceteris-core` (arithmetic, `mul_add`, `exp`, `ln`, `powf`, smooth max and min) implemented by `f64` and `Dual<N>` (N up to 16, contiguous tangents).
- Economic kernels and the route-choice solve generic over `R: Real` and `const L: usize`, lane innermost after goods; read-only data stored once.
- Release builds instantiate `f64`, `L = 1`; goldens reproduce bit for bit, at 1, 3 and all threads.
- Dual gradients match central finite differences under common random numbers to 1e-6 relative on the P2 golden scenario.
- Criterion benchmarks of `Dual<10>` and of `L = 8` against `f64, L = 1` set the calibration and ensemble budgets in the econ SPEC.

---

## Branch F: `pre-p3/design-snapshot` (Section 6, independent)

### Task F1: Design doc and DESIGN.md snapshot

- [ ] Apply the econ SPEC "Changes for the next design-doc snapshot" list to the Claude Docs design doc (paginated read; see the project memory for the link) and refresh `specs/DESIGN.md` from it, re-grepping line numbers first. One commit, `docs(design): refresh the snapshot with p2 as built`. P2 UI (map modes, province panel, time controls) is a separate plan and is not touched.

---

## Order and parallelism

F runs now, in parallel with A. D5 can run as soon as the owner registers the runner. A, B, C, D, E run in order, each PR reviewed by the owner before the next branch starts from main; while a PR waits, the next branch may start stacked on it and rebase after the merge.
