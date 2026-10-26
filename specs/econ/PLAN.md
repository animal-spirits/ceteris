# P2 Economic Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the P2 economic core on the full globe with 2000 data and pass the P2 gate (V0, V1, H2, 100 years, budgets).

**Architecture:** `ceteris-trade` (markets, two-level kernel, logit routing, transit) and `ceteris-econ` (pops, enterprises, labour, demand, world, schedule) are pure Rust over a graph-independent `EconInputs`, so everything is built and tested first on a generated small world. The Python pack pipeline adds the 2000 economic tables; a Rust `ceteris` binary exports inputs and runs to Arrow; the Python GE reference model reads the export, calibrates productivity and checks V1 and H2.

**Tech Stack:** Rust 1.93 (edition 2024, rayon, libm, arrow 60.0.0, criterion, proptest); Python 3 with uv (numpy, pyarrow, scipy, pytest, hypothesis).

**Spec:** `specs/econ/SPEC.md` and `specs/trade/SPEC.md` (approved 2026-10-23); ADRs 0012 to 0014. Read both specs before any task: equations and parameter values are there and are not repeated here.

## Global Constraints

- `unsafe_code = "forbid"`; `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass after every task.
- Determinism (19.4): transcendental functions through `libm` (`libm::pow`, `libm::exp`, `libm::log`); f64 state; sums over entities whose order could depend on threads use `ceteris_core::reduce::fixed_sum`; per-entity parallel work writes to fixed slots; no float atomics; no `HashMap` in tick code.
- Goods are the innermost dimension: arrays indexed `[entity * N_GOODS + g]`, `N_GOODS = 10`.
- Money: ledger `i64`, 1 minor unit = 1e-4 numeraire; amounts round half to even (`f64::round_ties_even`).
- Pack-dependent tests skip without a pack, printing the notice used in `crates/ceteris-geo/tests/common/mod.rs`; CI stays pack-free.
- Golden files are rewritten only with `CETERIS_BLESS=1`; a commit that changes one says why.
- Do not edit `app/` or `crates/ceteris-app` (the native app is a separate effort, ADR 0011); P2's UI is a later plan once the native app reaches parity.
- Commits: Conventional Commits, imperative, lowercase, no period, no attribution lines, no em dashes anywhere in code, docs or messages.
- New parameters exist only with their register ID in a doc comment (`/// P39`), values from the spec's Parameters tables.

## Review Focus

1. A province that consumes a traded good it never produces (all imports, Q = 0): the stock-flow rule must use the spec's Q floor and stay finite. Test in Task 3.
2. Price ranges far from 1 (p^theta underflows for theta = 5 when p < 1e-62): routing must compute p^theta relative to the good's maximum active price so shares stay finite. Test in Task 2.
3. A chokepoint closed mid-run (graph version moves): the kernel rebuilds (T0) and the run continues without invariant violations. Test in Task 17.
4. A country with one province and one gateway (islands, microstates): routing, gateways and wage root still work. The fixture includes such a country (Task 6).
5. Nominal scale: multiplying all prices, wages and money by 1e6 must not overflow `i64` money and must leave real outcomes identical. Test in Task 7.

---

## File Structure

```
crates/ceteris-core/src/sim.rs           State::restore (static inputs survive snapshots)
crates/ceteris-trade/
  src/lib.rs        goods.rs (N_GOODS, TRADED, names, tariff group)
  src/params.rs     TradeParams (P01, P02, P03a-c, P05, P43)
  src/network.rs    Network, Leg: graph-independent leg costs
  src/kernel.rs     Kernel::build, access sums (v, u, Pi)
  src/route.rs      route(): flows, payments, tariffs, losses, netback
  src/transit.rs    TransitWheel
  src/market.rs     Markets: clear, reprice
  benches/trade.rs
crates/ceteris-econ/
  src/lib.rs        params.rs, production.rs, labour.rs, demand.rs
  src/inputs.rs     EconInputs (static data) + from_pack
  src/world.rs      EconWorld (dynamic state, accounts, Encode, State)
  src/systems.rs    E1-E5 and trade system wrappers, schedule()
  src/fixture.rs    small_world(): generated 3+1 country world
  src/export.rs     Arrow export of inputs, kernel, run results
  src/bin/ceteris.rs  export | run
  tests/p2_properties.rs  tests/golden.rs  tests/p2_acceptance.rs  benches/econ.rs
python/ceteris/packs/econ/  goods.py icio.py people.py downscale.py tariffs.py gateways.py build.py
python/ceteris/reference/   io.py model.py solve.py sinkhorn.py calibrate.py ppml.py uniqueness.py __main__.py
python/ceteris/calibrate/   gravity.py (direct estimation, ADR 0022)
python/tests/  test_econ_pack.py test_reference.py test_p2_gate.py
tests/golden/  p2-econ.txt  p2-pack.txt  p2-fixture/ (export + reference solution)
```

---

## Part A: Trade core (pack-free)

### Task 1: Trade crate, goods, parameters, network and kernel

**Files:**
- Create: `crates/ceteris-trade/Cargo.toml`, `src/lib.rs`, `src/goods.rs`, `src/params.rs`, `src/network.rs`, `src/kernel.rs`
- Test: unit tests in `src/kernel.rs`

**Interfaces:**
- Produces:
  - `goods::{N_GOODS: usize = 10, TRADED: [bool; 10], NAME: [&str; 10], tariff_group(g) -> usize}` (0 primary for goods 0 to 3, 1 manufactured for 4 to 6)
  - `TradeParams { theta, coverage: [f64; 10], kappa1, kappa2, c_max, border_nu, time_cost: [f64; 10] }`, `Default` = spec start values (theta 5; coverage [3,1,2,2,2,2,2,0,0,0]; kappa 0.2, 0.1; c_max 0.1; nu 0.3; time_cost [0.002,0.002,0.002,0.005,0.010,0.010,0.010,0,0,0])
  - `Leg { from: u32, to: u32, cost_per_t: f64, hours: f64 }`
  - `Network { province_country: Vec<u16>, gateway_country: Vec<u16>, domestic: Vec<Leg>, access: Vec<Leg>, crossings: Vec<Leg> }` (domestic o to d, o != d, same country, both directions; access province to gateway of its country; crossings gateway to gateway of different countries, both directions)
  - `Wedges { tariff: Vec<f32> }` indexed `[(importer * n_countries + exporter) * 2 + group]`
  - `Kernel::build(net: &Network, weight: &[f64; 10], wedges: &Wedges, p: &TradeParams) -> Kernel`; per traded good CSR rows over origins (domestic), over provinces (access), over gateways (crossings), each entry `k: f32`, `tau: f64`; crossing entries also `lag: u8` and `border_tau` for the iceberg border loss
  - `Kernel::access(&self, g, price_theta: &[f64]) -> Access { v: Vec<f64>, u: Vec<f64>, pi: Vec<f64> }` (spec "Low-rank evaluation")

- [ ] **Step 1: Write failing tests** `leg_factor_matches_spec` (tau = 1 + c w + lambda T/24 days; K = tau^-theta), `crossing_includes_border_and_tariff` ((tau (1+nu)(1+t))^-theta, lag = round(hours/730) capped at 3), `pi_matches_dense_kernel` (random 2-country world, 10 provinces, 3 gateways: Pi from `access` equals the dense sum over K_od built by explicit enumeration of gateway routes, relative 1e-12), `home_kernel_is_one`.
- [ ] **Step 2: Run** `cargo test -p ceteris-trade`; expected: compile failure.
- [ ] **Step 3: Implement** the files above; add the crate to the workspace (members already glob `crates/*`).
- [ ] **Step 4: Run** `cargo test -p ceteris-trade`; expected: PASS.
- [ ] **Step 5: Commit** `feat(trade): add goods, trade parameters and the two-level kernel`

### Task 2: Routing, payments and transit

**Files:**
- Create: `crates/ceteris-trade/src/route.rs`, `src/transit.rs`
- Test: unit and proptest in those files

**Interfaces:**
- Consumes: `Kernel`, `Access`, `TradeParams` (Task 1).
- Produces:
  - `route(k: &Kernel, g: usize, supply: &[f64], price: &[f64], active: &[bool], out: &mut Routed)`
  - `Routed { home: Vec<f64>, arrive: [Vec<f64>; 4] /* by lag, destination units */, fob_paid: Vec<f64> /* per destination */, fob_received: Vec<f64> /* per origin, exports only */, access_out_value: Vec<f64>, crossing_value: Vec<f64>, access_in_value: Vec<f64> /* aligned with kernel CSR, for gateway transfers */, tariff: Vec<f64> /* per destination */, shipped: Vec<f64> /* per origin, non-home units */, lost: f64, netback: Vec<f64> /* per origin */ }`
  - `TransitWheel::new(provinces) ; push(lag, g, &[f64]) ; pop_due(&mut self) -> &[f64] /* [province * N_GOODS + g] */ ; Encode/Decode`
- Rules: inactive destinations get no flow; an origin with Pi = 0 keeps its whole supply at home; p^theta is computed as `(p / p_max_active)^theta` per good (Review Focus 2); domestic arrivals use lag 0 (delivered next tick), crossings lag 0 to 3 (delivered t + 1 + lag); home flow is immediate.

- [ ] **Step 1: Write failing tests** `routing_conserves_origin_mass` (proptest: sum of home + shipped = supply, 1e-12 relative), `markov_aggregation_matches_dense` (arrivals per destination equal dense X_od / tau_route, 1e-10), `fob_balances` (sum fob_paid + tariff-free = sum fob_received; gateway in-values equal out-values per gateway), `tariff_is_rate_times_fob_crossing`, `extreme_prices_stay_finite` (prices spanning 1e-80 to 1e80: shares finite, mass conserved), `wheel_delivers_after_lag_and_wraps`, and in `crates/ceteris-trade/tests/trade_nesting.rs` `two_level_vs_direct_report` (on a planar world with direct cross-border leg costs: print the median absolute log difference between gateway and direct-kernel flows; asserts only that both conserve mass, per the trade SPEC invariant).
- [ ] **Step 2: Run** `cargo test -p ceteris-trade route transit`; expected: FAIL.
- [ ] **Step 3: Implement** `route` with the gateway Markov split of the spec; iceberg losses per leg plus border loss, summed into `lost`; netback = sum X p_d / tau / sum X over destinations including home.
- [ ] **Step 4: Run** the same command; expected: PASS.
- [ ] **Step 5: Commit** `feat(trade): route supply by posted-price logit over the gateway kernel`

### Task 3: Province markets

**Files:**
- Create: `crates/ceteris-trade/src/market.rs`
- Test: unit tests in the file

**Interfaces:**
- Produces: `Markets { price, inventory, demand, supply, sales: Vec<f64> /* [province * N_GOODS + g] */, active: Vec<bool> }` with `Encode`/`Decode`; `Markets::clear(&mut self) -> Vec<f64>` (fill ratio per cell = min(1, (S + Q) / D), sets sales and new inventory; local goods: unsold perishes, returned as `spoiled` per cell via `clear_local`); `Markets::reprice(&mut self, p: &TradeParams)` (spec 8.2, clip c_max, S* = coverage D, Q floor = 1e-9 of the good's world mean Q).
- Inactive cells are skipped by both (their price is set from netback by the caller).

- [ ] **Step 1: Write failing tests** `pro_rata_fill_when_short`, `inventory_absorbs_glut`, `price_rises_with_excess_demand_and_clips`, `local_good_ignores_inventory_term`, `all_import_province_with_zero_q_stays_finite` (Review Focus 1), `steady_state_is_fixed_point` (D = Q, S = S*: price unchanged bit for bit).
- [ ] **Step 2: Run** `cargo test -p ceteris-trade market`; expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run**; expected: PASS.
- [ ] **Step 5: Commit** `feat(trade): add province markets with the stock-flow price rule`

## Part B: Econ core (pack-free)

### Task 4: Core restore with static inputs

**Files:**
- Modify: `crates/ceteris-core/src/sim.rs`, `src/snapshot.rs`, `src/toy.rs`
- Test: existing `crates/ceteris-core/tests/*` (must stay green, golden unchanged)

**Interfaces:**
- Produces: `trait State: Encode + Sized { fn begin_tick(&mut self); fn end_tick(&self, tick: u32) -> Result<(), Violation>; fn restore(&self, d: &mut Decoder) -> Result<Self, DecodeError>; }`; `snapshot::decode_canonical_with(bytes, |d| ...) -> Result<(u32, W), DecodeError>`; `Sim::seek` calls `self.world.restore`. Toy implements `restore` with its `Decode`. Why: the P2 world's static inputs (kernel, data) are shared through an `Arc` and are not in snapshots; the world's encoding includes a 32-byte BLAKE3 digest of them instead.

- [ ] **Step 1: Run** `cargo test -p ceteris-core` before the change; expected: PASS (baseline).
- [ ] **Step 2: Change the trait and seek.**
- [ ] **Step 3: Run** `cargo test -p ceteris-core && cargo test -p ceteris-core --release --test golden --test p0_acceptance`; expected: PASS, `tests/golden/p0-toy.txt` unchanged.
- [ ] **Step 4: Commit** `refactor(core): restore snapshots through the live world`

### Task 5: Econ crate, production, labour and demand functions

**Files:**
- Create: `crates/ceteris-econ/Cargo.toml` (deps: ceteris-core, ceteris-trade, libm, rayon), `src/lib.rs`, `src/params.rs`, `src/production.rs`, `src/labour.rs`, `src/demand.rs`

**Interfaces:**
- Produces:
  - `EconParams { sigma_demand: 0.6 /* P06 */, sigma_fl: 0.8 /* P08 */, kappa_w: 0.1 /* P39 */, delta_w: 0.5 /* P40 */, eta: 0.3 /* P41 */, edu_eff: [1.0, 1.5, 2.3] /* P42 */, frictionless: false }`; `EconParams::frictionless()` sets `frictionless = true, eta = 1, delta_w = 0`
  - `production::rho(sigma) -> f64`; `value_added(a, va0, l0, s_l, rho, l) -> f64`; `labour_target(p_va, w, a, va0, l0, s_l, rho, l_supply) -> f64` (spec closed form, corners, Cobb-Douglas when |rho| < 1e-9)
  - `labour::wage_step(w, excess, p: &EconParams, c_max) -> f64`; `labour::clearing_wage(w_guess, l_supply, demand: impl Fn(f64) -> f64) -> f64` (safeguarded Newton in ln w, bisection fallback, relative tolerance 1e-13); `labour::allocate(targets, prev, eta, l_supply, out: &mut [f64])`
  - `demand::ces(omega: &[f64; 10], price: &[f64; 10], money: f64, sigma: f64, out: &mut [f64; 10])`

- [ ] **Step 1: Write failing tests** `labour_target_maximizes_profit` (for sigma_fl in {0.5, 0.8, 1.0, 1.2}: profit at L* exceeds profit at L* x (1 +/- 1e-4), golden-section check), `labour_target_corners`, `value_added_at_base_is_a_va0`, `clearing_wage_clears` (demand(w) = l_supply within 1e-12), `wage_step_rigidity_halves_cuts`, `allocate_scales_pro_rata_when_short`, `ces_spends_all_money_and_matches_shares_at_unit_prices`.
- [ ] **Step 2: Run** `cargo test -p ceteris-econ`; expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run**; expected: PASS.
- [ ] **Step 5: Commit** `feat(econ): add production, labour and demand functions`

### Task 6: Inputs, world state and fixture

**Files:**
- Create: `crates/ceteris-econ/src/inputs.rs`, `src/world.rs`, `src/fixture.rs`

**Interfaces:**
- Consumes: Tasks 1 to 5.
- Produces:
  - `EconInputs { network: Network, weight: [f64; 10], wedges: Wedges, countries: Countries { omega: Vec<[f64; 10]>, io: Vec<[f64; 100]> /* a_kg at [k * 10 + g] */, s_l: Vec<f64>, unemployment: Vec<f64> }, provinces: ProvinceData { country: Vec<u16>, population: Vec<f64>, w0: Vec<f64> }, pops: PopData { province: Vec<u32>, edu: Vec<u8>, labour_force: Vec<f64>, population: Vec<f64> }, enterprises: EntData { province: Vec<u32>, good: Vec<u8>, a: Vec<f64>, va0: Vec<f64>, l0: Vec<f64>, va_target: Vec<f64> } }`; `EconInputs::digest(&self) -> [u8; 32]`
  - `EconWorld::new(inputs: Arc<EconInputs>, params: EconParams, trade: TradeParams) -> EconWorld` building the kernel, opening accounts (pop, enterprise, market per province, gateway, transit per good, rebate per country, mint), seeding the data state (prices 1, base wages and employment, 2000 unemployment, inventories and input stocks at target, one month of base income minted to pops)
  - `impl Encode for EconWorld` (static digest, then dynamic state: ledger, markets, wages, employment, transit wheel, last-tick income); `impl State for EconWorld` (`end_tick` runs the spec's invariants; `restore` reuses `Arc<EconInputs>`)
  - `fixture::small_world() -> EconInputs`: 3 countries x 8 provinces on a plane plus a 1-province, 1-gateway island country (Review Focus 4); 2 gateways per mainland country; leg cost 0.02 USD per tonne-km and 40 km/h; synthetic IO (diagonal 0.1, off-diagonal 0.03), omega favouring services, VA in millions so payment rounding stays below 1e-9 relative; fully deterministic (no RNG)

- [ ] **Step 1: Write failing tests** `fixture_world_opens_balanced_ledger` (money sums to zero, goods nonnegative), `encode_restore_round_trip` (encode, restore, re-encode: identical bytes), `digest_changes_with_inputs`.
- [ ] **Step 2: Run** `cargo test -p ceteris-econ world`; expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run**; expected: PASS.
- [ ] **Step 5: Commit** `feat(econ): add econ inputs, world state and the small-world fixture`

### Task 7: Systems, schedule and invariants

**Files:**
- Create: `crates/ceteris-econ/src/systems.rs`
- Test: `crates/ceteris-econ/tests/p2_properties.rs`

**Interfaces:**
- Produces: `systems::schedule() -> Schedule<EconWorld>` in the spec order (T1 deliver, E1 labour, E2 produce, T2 route, E3 demand, T3 clear, T4 reprice, E4 distribute, E5 assert; system IDs 201 to 209); `EconWorld::sim(inputs, params, trade) -> Sim<EconWorld>`; ledger `Flow` tags `WAGE, SALE, PURCHASE, FOB, TARIFF, DIVIDEND, REBATE, MINT` (u16 constants in `world.rs`).
- Parallelism: per-province systems run with rayon over fixed chunks of provinces, writing to their slices; ledger transfers are applied afterwards in stable ID order.

- [ ] **Step 1: Write failing tests** `conservation_over_120_ticks` (proptest over fixture perturbations: initial prices x U(0.5, 2) per cell; no `Violation`), `employment_within_bounds`, `homogeneity_of_degree_zero` (prices, wages and minted money x 1e6: all quantities identical bit for bit after 60 ticks, no overflow; Review Focus 5), `inactive_cells_use_netback_price`.
- [ ] **Step 2: Run** `cargo test -p ceteris-econ --test p2_properties`; expected: FAIL.
- [ ] **Step 3: Implement** the systems; `end_tick` returns the first violated invariant.
- [ ] **Step 4: Run**; expected: PASS.
- [ ] **Step 5: Commit** `feat(econ): run the p2 tick pipeline with accounting invariants`

### Task 8: Determinism and golden

**Files:**
- Create: `crates/ceteris-econ/tests/golden.rs`, `tests/golden/p2-econ.txt`
- Test: `crates/ceteris-econ/tests/p2_acceptance.rs` (fixture part)

**Interfaces:**
- Consumes: `EconWorld::sim`, `fixture::small_world`.

- [ ] **Step 1: Write tests** `p2_fixture_matches_golden_hashes` (1,200 ticks, seed 42, header `# P2 fixture: 4 countries, 25 provinces, seed 42, 1200 ticks. Lines: tick state-hash`, same bless logic as `crates/ceteris-core/tests/golden.rs`), `one_vs_eight_threads_identical` (rayon pools of 1 and 8), `replay_from_every_snapshot`.
- [ ] **Step 2: Bless** `CETERIS_BLESS=1 cargo test -p ceteris-econ --release --test golden`; then run without bless; expected: PASS.
- [ ] **Step 3: Add** `cargo test -p ceteris-econ --release --test golden --test p2_acceptance` to the determinism step in `.github/workflows/ci.yml`.
- [ ] **Step 4: Commit** `test(econ): add the p2 fixture golden and determinism checks`

## Part C: Export and GE reference (pack-free)

### Task 9: Arrow export and the `ceteris` binary

**Files:**
- Create: `crates/ceteris-econ/src/export.rs`, `src/bin/ceteris.rs`; add `arrow-array`, `arrow-ipc`, `arrow-schema` 60.0.0

**Interfaces:**
- Produces:
  - `export::write_inputs(dir: &Path, inputs: &EconInputs, kernel: &Kernel, params: &EconParams, trade: &TradeParams)`: `goods.arrow, countries.arrow, provinces.arrow, pops.arrow, enterprises.arrow, kernel_domestic.arrow (good, o, d, k, tau), kernel_access.arrow (good, province, gateway, k, tau), kernel_crossing.arrow (good, a, b, k, tau, border_tau, lag), params.json`
  - `export::write_run(dir: &Path, world: &EconWorld)`: `run_prices.arrow (province, good, price, active), run_wages.arrow, run_flows.arrow (exporter_country, importer_country, good, value, units)` with flows averaged over the last 12 ticks
  - CLI: `ceteris export --fixture|--pack DIR --out DIR` and `ceteris run --fixture|--pack DIR --ticks N [--frictionless] --out DIR`

- [ ] **Step 1: Write failing test** `export_round_trips_fixture` (write, read back with `ceteris_geo::pack::read_table`: row counts and a sample of values equal).
- [ ] **Step 2: Implement; run** `cargo test -p ceteris-econ export`; expected: PASS.
- [ ] **Step 3: Run** `cargo run -p ceteris-econ --release --bin ceteris -- export --fixture --out tests/golden/p2-fixture` and `... run --fixture --ticks 1200 --frictionless --out tests/golden/p2-fixture/run`; expected: files written.
- [ ] **Step 4: Commit** `feat(econ): export inputs and runs as arrow and add the ceteris binary`

### Task 10: GE reference model

**Files:**
- Create: `python/ceteris/reference/{__init__,io,model,solve,sinkhorn,__main__}.py`; add `scipy` to `python/pyproject.toml`
- Test: `python/tests/test_reference.py`

**Interfaces:**
- Consumes: Task 9 export files.
- Produces: `io.load(dir) -> Inputs`; `model.residual(x: np.ndarray, inp: Inputs) -> np.ndarray` (x = ln p over active cells, then ln w; equations of the econ SPEC "GE reference model", one province's labour equation replaced by the world price index = 1, Walras' law); `model.unpack(x, inp) -> Equilibrium(price, wage, flows_by_country_pair_good, output)`; `solve.equilibrium(inp, x0=None, tol=1e-10) -> Equilibrium` (scipy `newton_krylov` from the data state, Anderson-accelerated tatonnement fallback); `sinkhorn.balanced(K, supply, demand) -> flows`; `python -m ceteris.reference solve --inputs DIR --out DIR`.

- [ ] **Step 1: Write failing tests** `test_residual_zero_at_solution` (fixture export), `test_scale_invariance` (scaling prices and wages leaves residuals of the homogeneous block unchanged), `test_balanced_sinkhorn_matches_logit_at_equilibrium` (on the fixture: balanced Sinkhorn with equilibrium marginals reproduces the logit flows, 1e-8 relative), hypothesis `test_two_province_closed_form` (one good, two provinces, symmetric costs: prices equal and flows match the analytic logit split).
- [ ] **Step 2: Run** `uv run --project python pytest python/tests/test_reference.py`; expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run**; expected: PASS.
- [ ] **Step 5: Commit** `feat(reference): add the p2 ge reference model`

### Task 11: V1 on the fixture

**Files:**
- Create: `tests/golden/p2-fixture/reference/` (solved equilibrium, Arrow)
- Modify: `crates/ceteris-econ/tests/p2_acceptance.rs`, `python/tests/test_reference.py`

**Interfaces:**
- Consumes: Tasks 8 to 10. CI runs Rust and Python in separate jobs, so the chain is pinned by files: Rust checks its fixture export is bit-identical to `tests/golden/p2-fixture/` and that its frictionless run matches `reference/`; Python checks `reference/` solves `tests/golden/p2-fixture/`.

- [ ] **Step 1: Write tests** Rust `fixture_export_is_current`, `v1_fixture_frictionless_matches_reference` (1,200 ticks frictionless: every active price and wage, normalized by the world price index, within 1%; every country-pair x good flow above 1e-6 of world trade within 1%); Python `test_fixture_reference_is_current` (re-solve within 1e-8).
- [ ] **Step 2: Generate** `uv run --project python python -m ceteris.reference solve --inputs tests/golden/p2-fixture --out tests/golden/p2-fixture/reference`.
- [ ] **Step 3: Run** `cargo test -p ceteris-econ --release --test p2_acceptance` and the Python test; expected: PASS. If V1 fails, debug with superpowers:systematic-debugging; do not change parameters to pass.
- [ ] **Step 4: Commit** `test(econ): check v1 on the fixture against the ge reference`

### Task 12: PPML gravity

**Files:**
- Create: `python/ceteris/reference/ppml.py`
- Test: `python/tests/test_reference.py`

**Interfaces:**
- Produces: `ppml.fit(flows, exporter, importer, regressors: dict[str, np.ndarray]) -> dict[str, float]` (Poisson pseudo-maximum likelihood with exporter and importer fixed effects, IRLS on a dense design, convergence 1e-10 on deviance); `ppml.distance_elasticity(run_flows, gravity_table) -> float` using CEPII `distw` and `contig`, domestic pairs excluded.

- [ ] **Step 1: Write failing test** `test_ppml_recovers_known_elasticity` (synthetic gravity, 40 countries, beta -0.9, contiguity 0.5, Poisson noise fixed seed: estimate within 0.02).
- [ ] **Step 2: Implement; run**; expected: PASS.
- [ ] **Step 3: Commit** `feat(reference): add ppml gravity estimation`

## Part D: 2000 data pipeline

All data tasks: register each download in `fetch.toml` with `python -m ceteris.packs.fetch --record` (review sha256 and size), confirm the source row in `sources.toml`, keep `check_sources.py` green, and give every output row an `imputed` flag where the spec says so. Exact URLs are found at the provider pages named in `sources.toml`.

### Task 13: Goods and weights

**Files:** Create `python/ceteris/packs/econ/__init__.py`, `goods.py`; test `python/tests/test_econ_pack.py`

**Interfaces:** Produces `goods.ICIO_MAP: dict[str, int]` (industry code to good, spec Goods table; handles B05_06 as a split marker); `goods.weights_from_baci(baci_2000: Path, hs_isic: Path) -> np.ndarray[10]` (tonnes per USD for goods 0 to 6, 0 for local goods).

- [ ] **Step 1: Write failing tests** `test_every_icio_industry_mapped_once` (2025 edition list), `test_weights_positive_for_traded`, `test_bulk_heavier_than_machinery` (w_coal > w_machinery by over 10x).
- [ ] **Step 2: Fetch** BACI HS92 2000 and an HS to ISIC Rev 4 concordance (record in `fetch.toml`).
- [ ] **Step 3: Implement; run** `uv run --project python pytest python/tests/test_econ_pack.py -k goods`; expected: PASS.
- [ ] **Step 4: Commit** `feat(packs): map icio industries to p2 goods and weigh them from baci`

### Task 14: ICIO aggregation and imputation

**Files:** Create `python/ceteris/packs/econ/icio.py`

**Interfaces:** Produces `icio.aggregate(icio_2000_csv) -> dict[str, CountryIO]` with `CountryIO(output[10], va[10], io[10, 10] /* a_kg */, final_demand_shares[10])`; coal split by EIA 2000 production x Pink Sheet prices when the edition lacks B05/B06; `icio.impute(countries, icio_by_code, wdi) -> dict[str, CountryIO]` (nearest ICIO donor by region then |log GDP per capita|; VA by good from WDI agriculture, industry, manufacturing, services shares, split within groups by donor proportions).

- [ ] **Step 1: Write failing tests** `test_aggregation_preserves_output_totals` (1e-9), `test_io_coefficients_below_one` (column sums < 1), `test_final_demand_shares_sum_to_one`, `test_imputed_country_uses_nearest_donor` (small hand-made table).
- [ ] **Step 2: Fetch** ICIO 2025 edition (1995-2000 package), WDI extracts, EIA production, Pink Sheet annual prices.
- [ ] **Step 3: Implement; run** `-k icio`; expected: PASS.
- [ ] **Step 4: Commit** `feat(packs): aggregate icio 2000 to ten goods and impute non-icio economies`

### Task 15: Population, education and labour

**Files:** Create `python/ceteris/packs/econ/people.py`

**Interfaces:** Produces `people.province_population(worldpop_2000, cells) -> np.ndarray[P]` (H3 cell aggregation as `raster.py` does for elevation); `people.country_labour(ilostat, wic, barro_lee, pwt) -> dict[str, Labour(participation, unemployment, edu_shares[3], labour_share)]`; `people.pops(provinces, labour) -> pa.Table` (province x 3 education rows).

- `people.country_labour` also returns the 2000 Gini (`Labour.gini`, `Labour.gini_imputed`): WDI SI.POV.GINI, the year nearest 2000 within 1995 to 2005, else SWIID (the disposable-income Gini; WIID's licence is non-commercial); imputed flag set for SWIID or donor values. `econ_countries.arrow` gains `gini` (and its flag), which `EconInputs::from_pack` reads into `Countries.gini` (PIGL income dispersion, ADR 0021; MIGRATION Task 7).

- [ ] **Step 1: Write failing tests** `test_population_sums_match_country_totals_within_2pct` (WorldPop vs WDI 2000), `test_edu_shares_sum_to_one`, `test_labour_share_in_unit_interval`, `test_gini_in_unit_interval_with_source_flag`.
- [ ] **Step 2: Fetch** WorldPop 2000 global mosaic (1 km), ILOSTAT bulk (participation, unemployment 2000), WIC reconstruction, Barro-Lee v3, PWT 11, WDI SI.POV.GINI, SWIID.
- [ ] **Step 3: Implement; run** `-k people`; expected: PASS.
- [ ] **Step 4: Commit** `feat(packs): add 2000 population, education and labour by province`

### Task 16: VA downscaling

**Files:** Create `python/ceteris/packs/econ/downscale.py`

**Interfaces:** Produces `downscale.province_va(country_io, provinces, proxies) -> (va[P, 10], imputed[P, 10])` with the spec's proxy order (MapSPAM 2000, GEM extraction and coal mines plus P1 mineral sites, DOSE matched by ISO 3166-2 or exact name, Kummu gridded GDP, population) and the under-1,000-working-age rule; local goods split by population.

- [ ] **Step 1: Write failing tests** `test_downscaling_preserves_national_va` (1e-9 per country and good), `test_empty_provinces_get_no_va`, `test_dose_match_rate_reported`.
- [ ] **Step 2: Fetch** MapSPAM 2000, GEM coal mine and oil and gas extraction trackers, DOSE v2.14, Kummu gridded GDP v2.
- [ ] **Step 3: Implement; run** `-k downscale`; expected: PASS.
- [ ] **Step 4: Commit** `feat(packs): downscale national value added to provinces`

### Task 17: Tariffs, gateways, pack tables and pack format 3

**Files:** Create `python/ceteris/packs/econ/{tariffs,gateways,build}.py`; modify `python/ceteris/packs/build.py` (call econ build after P1 tables; `PACK_FORMAT = 3`), `crates/ceteris-geo/src/pack.rs` (`PACK_FORMAT = "3"`), `crates/ceteris-econ/src/inputs.rs` (`EconInputs::from_pack(pack: &Pack, dir: &Path) -> Result<EconInputs, PackError>`); test `python/tests/test_econ_pack.py`, `crates/ceteris-econ/tests/p2_acceptance.rs`; golden `tests/golden/p2-pack.txt`

**Interfaces:**
- `tariffs.table(wdi, cepii_gravity, wb_dta, countries) -> pa.Table` (importer, exporter, group, rate; zero within agreements and customs unions in force on 1 Jan 2000)
- `gateways.select(nodes, edges, provinces, trade_by_country) -> pa.Table` (node, country, kind): ports and airports clustered by k-medoids on great-circle distance (a change from the spec's path-cost rule, which needs Rust; record it in the spec's open questions), count per country proportional to sqrt(2000 trade from BACI), at least 1, total near 600; land crossings from edges whose endpoints lie in different 2000 countries, merged within 50 km
- Tables per econ SPEC "Pack additions"
- Leg costs stay least-cost (S05) in P2: `Network.route_taus` stays `None` (route choice deferred past P2, ADR 0017)
- `EconInputs::from_pack`: leg costs from `ceteris_geo::paths::shortest` run in parallel from each province and gateway, keeping only same-country province entries, gateway entries and gateway-pair entries (never caching full trees: 4,796 trees would need about 3 GB); reads `econ_calib.arrow` when present, else A = 1
- `EconInputs::network_from_graph(pack: &Pack, gateways) -> Network` (the leg-cost step alone) and `EconWorld::set_network(&mut self, net: Network)` (replaces the inputs `Arc` with the new network, rebuilds the kernel: system T0 of the trade SPEC, triggered by the caller in P2 since shocks arrive in P3)
- Carried from the migration (MIGRATION Task 8): `EconInputs::from_pack` must call `set_coefficients()` (`validate` rejects empty `EntData::coef`), and the export writes only the country `a` table, not `EntData::coef` (exact with one vintage; revisit with vintages)

- [ ] **Step 1: Write failing tests** Python `test_pack_econ_tables_reproducible` (two builds, same sha256), `test_every_country_has_a_gateway`, `test_gateway_count_near_600` (500 to 700); Rust `pack_inputs_load` (skips without pack: every province has a country with omega summing to 1; every enterprise has workers), `kernel_rebuilds_after_chokepoint_closure` (run 12 ticks, close Suez with `set_chokepoint_closed`, `set_network(network_from_graph(...))`, run 24 more ticks: no violation, Suez-dependent flows fall; Review Focus 3).
- [ ] **Step 2: Fetch** CEPII gravity, WB DTA, WDI tariff series (TM.TAX.TCOM.WM.AR.ZS, TM.TAX.MANF.WM.AR.ZS).
- [ ] **Step 3: Implement; build** `cd python && uv run python -m ceteris.packs`; expected: pack format 3 with the new tables; `manifest.json` lists them.
- [ ] **Step 4: Bless** `tests/golden/p2-pack.txt` (table sha256 of the econ tables, same pattern as `p1-pack.txt`); run `uv run --project python pytest python/tests` and `cargo test --workspace`; expected: PASS, P1 goldens unchanged apart from the format bump (say so in the commit).
- [ ] **Step 5: Commit** `feat(packs): add 2000 economic tables, tariffs and gateways (pack format 3)`

## Part E: Full globe and the gate

### Task 18: Calibration

**Files:** Create `python/ceteris/calibrate/{__init__,gravity}.py`, `python/ceteris/reference/calibrate.py`; modify `__main__.py`, `python/ceteris/packs/sources.toml` (the Yotov et al. sample's licence row, added with its fetch)

**Interfaces:** Produces `calibrate.productivity(inp: Inputs) -> (a: np.ndarray, residuals)`: one joint Newton-Krylov solve for ln p, ln w, ln A of traded enterprises (equilibrium VA = `va_target`) and ln A per (country, local good) (national VA = ICIO); `python -m ceteris.reference calibrate --pack data/packs/2000` runs `cargo run --release --bin ceteris -- export --pack ...` to a temp dir, solves, writes `data/packs/2000/econ_calib.arrow`, prints the residual summary (count and largest relative miss).

- [x] **Step 1: Estimate trade costs directly (ADR 0022)** in `python/ceteris/calibrate/gravity.py`, PPML from `reference/ppml.py`. First `test_gravity_reproduces_yotov_2016`: the code reproduces the published PPML estimates of Yotov, Piermartini, Monteiro and Larch (2016, "An Advanced Guide to Trade Policy Analysis: The Structural Gravity Model", WTO and UNCTAD) on their sample, to the printed precision (fetched to the cache with its licence row in `sources.toml`; skips without the cache). Then PPML structural gravity on 2000 BACI per theta class (ADR 0023), exporter and importer fixed effects, with costs between countries the least-cost (S05) costs of the pack built in Task 17 (route choice is deferred past P2, ADR 0017): minus the coefficient on ln(1 + tariff) is the class's theta, a border term on a sample with domestic flows (2000 ICIO output less exports) gives the border friction P05, and the S05 path length by mode gives the mode cost ratios P04. Outputs enter the pack with provenance "data" (not "calibrated") and the P01, P04 and P05 rows of the registers say so. Commit `feat(calibrate): estimate theta, border friction and mode costs by ppml on 2000 baci`. Done (b967494): the Yotov et al. reproduction passes to the printed precision; on 2000 BACI theta 13.36, 13.20, 11.82 by class, P05 0.204, P04 not identified (stop rule). Owner decision 2026-10-24: P2 keeps the literature values (Caliendo-Parro class thetas, P05 0.3, geo SPEC P04); the estimates are a diagnostic recorded in ADR 0022 and in the register rows, and enter neither the pack nor the defaults.
- Note: Steps 2 to 5 (productivity) run at the literature values of P01, P04 and P05.
- [ ] **Step 2: Write failing test** `test_calibration_recovers_fixture_productivity` (perturb the fixture's A, export, calibrate against its own equilibrium VA: recovered A within 1e-8). Productivity calibration runs at the literature trade costs (Step 1 outcome).
- [ ] **Step 3: Implement; run** the test; expected: PASS.
- [ ] **Step 4: Run** calibration on the full pack; record residual counts in `tests/golden/p2-pack.txt` alongside the `econ_calib.arrow` hash (bless).
- [ ] **Step 5: Commit** `feat(reference): calibrate p2 productivity against 2000 value added`

### Task 19: Benchmarks

**Files:** Create `crates/ceteris-trade/benches/trade.rs`, `crates/ceteris-econ/benches/econ.rs`

**Interfaces:** criterion benches on the full pack (skip with a notice without one, fixture fallback so `cargo bench --no-run` and CI work): `source_dispatch` (10 goods per tick, budget 5 ms per tick on the full pack), `clear_reprice` (2 ms), `labour` + `produce` (12 ms), `consume` (10 ms), `kernel_build` (report only).
- Carried from the migration (MIGRATION Tasks 6 and 8): E3 runs one extra unit `source` + `dispatch` per traded good each tick for q_d and the lead time L, and the E2 same-tick chain of local goods runs serially per province; budget both

- [ ] **Step 1: Implement; run** `cargo bench -p ceteris-trade -p ceteris-econ`; expected: medians reported; record them in the commit message and in `specs/econ/SPEC.md` (new "Measured" note under the gate).
- [ ] **Step 2: If a budget is missed,** profile (samply) and fix layout or algorithm per 19.4 before continuing; never relax determinism.
- [ ] **Step 3: Commit** `perf(econ): add p2 system benchmarks`

### Task 20: P2 gate on the full pack

**Files:** Modify `crates/ceteris-econ/tests/p2_acceptance.rs`; create `python/tests/test_p2_gate.py`

**Interfaces:** Consumes everything above.
- Carried from the migration (MIGRATION Tasks 5 and 6): define ADR 0018's per-class prior band for H2 before testing `test_h2_theta_by_class` (none is defined; for example the Caliendo and Parro standard errors aggregated per class), and re-measure the stability region and the growth factors on the pack with the committed harnesses `crates/ceteris-econ/tests/p2_price_rule_sweep.rs` and `p2_expectation_sweep.rs`

- [ ] **Step 1: Write tests** (all skip without a pack) Rust `hundred_years_without_violation` (default run, 1,200 ticks, prices and wages finite and positive), `full_pack_one_vs_eight_threads` (120 ticks); Python `test_v1_full_pack` (first `uniqueness.spectral_bound` at the pack's theta and `uniqueness.value_added_share`, then runs `ceteris run --pack ... --ticks 1200 --frictionless` and solves the reference from the data state; `uniqueness.v1_mode` picks the check: in tolerance mode the V1 tolerances of the econ SPEC, in report mode a report of which equilibrium each model reaches, per the econ SPEC P2 gate), `test_h2_distance_elasticity` (default run, `ppml.distance_elasticity` on the origin value `run_flows.value`, in [-1.1, -0.7]), `test_h2_theta_by_class` (ADR 0018: PPML on simulated flows valued at what the importer pays, `run_flows.value` times 1 + t, `ppml.trade_elasticity` per theta class of ADR 0023, recovers each class's theta within its prior band).
- [ ] **Step 2: Run** `cargo test -p ceteris-econ --release --test p2_acceptance` and `uv run --project python pytest python/tests/test_p2_gate.py`.
- [ ] **Step 3: If V1 fails,** use superpowers:systematic-debugging; no parameter changes to pass. **If H2 misses, stop and report the estimate to the owner before changing anything** (owner decision, ADR 0013).
- [ ] **Step 4: Commit** `test(econ): add the p2 gate on the full pack`

### Task 21: Records

**Files:** Modify `specs/econ/SPEC.md` (status line: gate passed with date and evidence), `specs/trade/SPEC.md` (gateway rule as built); append to the design-doc change list for the next snapshot: register entries P39 to P47, the 21 development-slice change (ADR 0012), 19.1 app stack (ADR 0011 option 3).
- Carried from the migration (MIGRATION Tasks 4 and 7): the DESIGN.md refresh must carry the ADR 0017 deferral (lines 257, 1324 and 1856 still describe route choice as the P2 cost model) and ADR 0021's three PIGL categories (DESIGN 8.6 names five)

- [ ] **Step 1: Write** the gate evidence (commands, measured V1 maxima, H2 estimate with standard error, benchmark medians).
- [ ] **Step 2: Commit** `docs(specs): record p2 gate evidence`

---

## Out of this plan

- P2 UI (map modes, province panel, time controls) in the native app: a separate plan once ADR 0011 stage 2 reaches parity.
- Updating the Claude Docs design doc and refreshing `specs/DESIGN.md`: on the owner's request, in its own commit.
