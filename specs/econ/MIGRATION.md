# P2 Migration to ADRs 0015 to 0022 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the pack-free P2 core on `p2-econ` (PLAN.md Tasks 1 to 12, 7b) onto the review's model: destination-side gravity with origin markets cleared across ticks, route-choice costs, sector thetas, expected inflation, PIGL demand, putty-clay coefficients with a same-tick local chain, and the uniqueness-checked gate; then resume PLAN.md Part D.

**Architecture:** The trade crate keeps the two-level kernel but runs it backwards: destinations spread expenditure over origins (Eaton-Kortum shares on K p_o^-theta), origins clear orders against output and merchant inventory, buyers pay at dispatch and receive after transit through a per-buyer pipeline. Every behaviour change is mirrored in the Python GE reference in the same task, so V1 on the fixture holds after each task. Route choice is a new `ceteris-geo` module whose per-good leg factors the trade network can carry; the fixture keeps direct legs (S05 least-cost).

**Tech Stack:** as PLAN.md (Rust 1.93, rayon, libm, arrow 60.0.0, criterion, proptest; Python 3 with uv, numpy, scipy, pyarrow, pytest, hypothesis).

**Spec:** ADRs 0015 to 0022 (each "Mapping to P2 as built" section is binding), `specs/DESIGN.md` rev 226 (5.4, 7.2, 8.2, 8.3, 8.6, 8.7, 17, 19.2, 21.1, 25), `specs/econ/SPEC.md`, `specs/trade/SPEC.md`. Rulings in `.superpowers/sdd/PLAN/progress.md` stand unless a task below replaces them: intermediate users first in rationing, input orders on capacity output, `WEIGHT_FLOOR`, fixture sea legs at 0.002 USD/t-km, i64 money headroom (about 300x one month of world income).

## Global Constraints

- Everything in `.superpowers/sdd/PLAN/global-constraints.md` (= PLAN.md Global Constraints), with register IDs as renumbered in Task 1.
- Each task updates the affected sections of `specs/trade/SPEC.md` and `specs/econ/SPEC.md` (deviation tables, equations, parameters, system order, tests) in the same commit, and moves its ADR's status line to `implemented on p2-econ (<task>)`.
- Each task that changes behaviour mirrors it in `python/ceteris/reference` in the same commit.
- Golden discipline during the migration: from Task 1 until Task 10 the golden-bound tests are `#[ignore = "re-blessed in MIGRATION Task 10 (ADRs 0015-0022)"]` (Rust) or `pytest.mark.skip(reason=...)` with the same text (Python): `p2_fixture_matches_golden_hashes`, `fixture_export_is_current`, `test_fixture_reference_is_current`. Nothing under `tests/golden/` is rewritten before Task 10.
- V1 must hold after every task. Tests that read the fixture export or its reference take the directory from `CETERIS_FIXTURE_DIR` (default `tests/golden/p2-fixture`, added in Task 1). Every task ends with the **WIP check**:

```bash
cargo run -p ceteris-econ --release --bin ceteris -- export --fixture --out target/p2-wip
uv run --project python python -m ceteris.reference solve --inputs target/p2-wip --out target/p2-wip/reference
CETERIS_FIXTURE_DIR=$PWD/target/p2-wip cargo test -p ceteris-econ --release --test p2_acceptance --test p2_stability -- --include-ignored --skip p2_fixture_matches_golden_hashes --skip fixture_export_is_current
CETERIS_FIXTURE_DIR=$PWD/target/p2-wip uv run --project python --locked pytest python/tests
CETERIS_FIXTURE_DIR=$PWD/target/p2-wip cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all --check
```

  Expected: all pass. V1 is never xfailed; if it fails, debug (superpowers:systematic-debugging), no parameter changes to pass, and report to the controller if the cause is the model rather than the code.
- New register entries: P44 route-choice dispersion theta_r (Task 4), P45 expectation gain g (Task 6), P46 PIGL price parameter and P47 PIGL Engel parameter only if P06 and P07 cannot carry them (Task 7 decides, see there).

## Review Focus

1. A destination with no reachable active origin for a traded good (island without a producer, Phi_d below the floor): its delivered price index is infinite, households must buy nothing of that good and spend the rest, enterprises with that input stay input-capped, no NaN. Test in Task 2 (`unreachable_destination_buys_nothing`) and Task 3.
2. Origin prices spanning many orders of magnitude: origin weights `(p_o / p_min)^-theta` relative to the good's lowest active price, floored at `WEIGHT_FLOOR`, shares finite and adding up. Test in Task 2 (`extreme_prices_stay_finite`).
3. Goods in flight at a snapshot: the per-buyer pipeline is part of the encoded state, and replay from every snapshot stays bit-identical. Test in Task 3 (`replay_from_every_snapshot`, already in `p2_acceptance.rs`, must cover a tick with every lag slot non-empty).
4. A pop far poorer or richer than its country's base (PIGL shares leave [0, 1]): shares are clamped at 0 and the rest rescaled to sum to 1, spending still equals money. Test in Task 7 (`pigl_shares_stay_in_unit_interval_far_from_base`).
5. A cycle among local goods (services feeding services and construction feeding services): the same-tick chain terminates within its bound and Leontief input use still equals a Y exactly. Test in Task 8 (`local_chain_terminates_on_cycles`).

---

## File Structure

```
crates/ceteris-trade/
  src/params.rs     TradeParams: theta [f64; 10] (P01), theta_r (P44), expectation_gain (P45), time_cost (P43)
  src/kernel.rs     Kernel::build per good with theta_g; origin access sums (v', u', Phi)
  src/source.rs     NEW source(): destination expenditure over origins, forward pass with fills
  src/pipeline.rs   NEW Pipeline: per-buyer goods in flight (replaces transit.rs)
  src/market.rs     origin clearing with priority classes, reprice with pi^e, clip counter
  src/route.rs      DELETED in Task 3
crates/ceteris-geo/src/route_choice.rs   NEW (I - A_r)^-1 factors, expected hours, spectral radius
crates/ceteris-econ/
  src/params.rs     EconParams: register IDs P39-P42, rationing (S11), technology (S09), pigl
  src/demand.rs     pigl() replaces ces()
  src/labour.rs     wage_step with pi^e
  src/local.rs      NEW same-tick local-goods chain
  src/systems.rs    schedule rewired (Tasks 3, 6, 8)
python/ceteris/reference/  model.py (sourcing, PIGL), uniqueness.py (NEW)
CHANGELOG.md        NEW in Task 10 (BEHAVIOUR entry)
```

---

### Task 1: Register renumbering and the migration harness

**Files:**
- Modify: `crates/ceteris-econ/src/params.rs`, `src/labour.rs:13`, `src/inputs.rs:109`, `src/export.rs:361-365,711-712`, `crates/ceteris-trade/src/params.rs:29`, `specs/econ/SPEC.md`, `specs/trade/SPEC.md`, `specs/econ/PLAN.md` (lines naming P37 to P41)
- Modify: `crates/ceteris-econ/tests/{golden,p2_acceptance,p2_stability}.rs`, `python/tests/test_reference.py` (directory override, ignores)

**Interfaces:**
- Produces: register IDs P39 kappa_w, P40 delta_w, P41 eta, P42 e_k, P43 time cost lambda_g (old P37 to P41 in that order); `CETERIS_FIXTURE_DIR` read by every test that opens the fixture export or its reference (Rust helper `fn fixture_dir() -> PathBuf` in each test file; Python module constant `FIXTURE` from `os.environ.get`).

- [ ] **Step 1:** Rename the IDs in doc comments, the export's parameter-ID table and both specs (Parameters tables, equations, "P41" in the H2 gate text). `grep -rn -E "P3[7-9]|P4[01]" crates python specs/econ specs/trade` must then show only the new meanings.
- [ ] **Step 2:** Add the directory override and the three ignores with the Global Constraints reason text.
- [ ] **Step 3: Run** the WIP check; expected: PASS (the export now differs from the golden only in `params.json` IDs).
- [ ] **Step 4: Commit** `refactor(econ): renumber p2 register entries to p39-p43`

### Task 2: Destination sourcing, origin markets and the pipeline (trade crate)

ADRs 0015 and 0016. Adds the new primitives beside `route.rs`; Task 3 switches the world over and deletes the old path.

**Files:**
- Create: `crates/ceteris-trade/src/source.rs`, `src/pipeline.rs`
- Modify: `src/kernel.rs` (origin-side access sums), `src/market.rs`, `src/lib.rs`, `tests/trade_nesting.rs`

**Interfaces:**
- Produces:
  - `Kernel::origin_access_into(&self, g, weight: &[f64], out: &mut Access)`: with `weight[o] = (p_o / p_min_active)^-theta` on active origins (0 elsewhere, below `WEIGHT_FLOOR` treated as 0): `v'_a = sum_{o in A} K_oa w_o`, `u'_b = sum_a v'_a K_ab`, `Phi_d = sum_{o in B} K_od w_o + sum_b K_bd u'_b` (domestic includes K_dd = 1)
  - `source(k: &Kernel, g, price: &[f64], active: &[bool], spend: &[[f64; 2]], out: &mut Sourced)` where `spend[d] = [intermediate, household]` expenditure in numeraire. `Sourced { orders: Vec<[f64; 2]> /* per origin, units at p_o by class */, price_index: Vec<f64> /* P_d = Phi_d^(-1/theta) in price units, f64::INFINITY when Phi_d = 0 */, .. }`. Backward Markov split: d sends `E_d K_bd u'_b / Phi_d` to b, b splits by `K_ab v'_a / u'_b`, a by `K_oa w_o / v'_a`.
  - `dispatch(k: &Kernel, g, price: &[f64], fill: &[[f64; 2]], out: &mut Sourced)`: the forward pass with per-origin fill ratios by class; fills `Sourced { shipped: Vec<f64> /* per origin, units */, arrive: [Vec<[f64; 2]>; 5] /* slot 0 home (now), 1 domestic (t+1), 2..4 crossings by lag, destination units by class */, paid: Vec<f64> /* per destination, numeraire incl. tariff */, received: Vec<f64> /* per origin, net of tariff */, tariff: Vec<f64> /* per destination */, domestic_value, access_in_value, crossing_value, access_out_value /* aligned with the kernel CSRs, as in Routed */, lost: f64 }`. Arrivals use the per-leg factor K_leg / tau_leg (units arriving per unit of value at the origin price), so arrivals = sum over routes of X / (p_o tau_route).
  - `Pipeline::new(n_buyers)`; `push(slot: usize, buyer: usize, g: usize, units: f64)`; `pop_due(&mut self) -> &[f64] /* [buyer * N_GOODS + g] */`; `on_order(&self, buyer, g) -> f64` (sum over slots); `in_flight(g) -> f64`; `Encode`/`Decode`. Slots: 4 future ticks (t+1 to t+4); home deliveries do not enter it.
  - `Markets`: `supply` = this tick's output (traded) or local output; `demand: Vec<[f64; 2]>`-equivalent split into `demand_int` and `demand_hh` (units ordered at the origin); `clear(&mut self, rationing: Rationing, fill: &mut [[f64; 2]])` with `Rationing::{Priority, Proportional}` (Priority: intermediate orders filled first, households from the rest; Proportional: one ratio for both); inventory S' = S + Q - sales at the origin; `active` now means "has a market": traded cells with output capacity (an enterprise with VA0 > 0), local cells as before.
- Rules: prices of inactive traded cells are not used anywhere (no netback). `paid` = units shipped x p_o x (1 + t) on crossings, x p_o domestic; tariff = t x p_o x units to the importer; origin receives p_o x units.

- [ ] **Step 1: Write failing tests** in `source.rs`: `sourcing_spends_destination_expenditure` (sum over origins of orders x p_o = sum of spend at d, 1e-12 relative), `three_province_toy_matches_eaton_kortum` (one country, K = tau^-5 on three pairs, hand-computed shares `K_od p_o^-5 / sum_k K_kd p_k^-5`, 1e-12), `sourcing_matches_dense_kernel` (random 2-country world, 10 provinces, 3 gateways: orders per origin equal the dense sum over enumerated gateway routes, 1e-10), `shares_homogeneous_in_prices` (prices x lambda for lambda in {1e-6, 1e3}: order shares unchanged to 1e-12, ADR 0015), `forward_pass_conserves_goods` (shipped - lost = arrivals over all slots, 1e-12), `fills_scale_shipments_by_class`, `unreachable_destination_buys_nothing` (Review Focus 1), `extreme_prices_stay_finite` (origin prices 1e-80 to 1e80; Review Focus 2), `price_index_is_ces_of_delivered_prices` (P_d^-theta = sum_o K_od p_o^-theta, dense, 1e-12); in `pipeline.rs`: `pipeline_delivers_each_buyer_after_its_slot`, `pipeline_round_trips_encode`; in `market.rs`: `origin_clears_orders_against_output_and_inventory`, `excess_supply_goes_to_inventory`, `priority_fills_intermediate_first`, `proportional_fills_one_ratio`; in `trade_nesting.rs`: `two_level_vs_direct_report` rewritten for sourcing (report median log difference, assert both spend E exactly).
- [ ] **Step 2: Run** `cargo test -p ceteris-trade`; expected: FAIL (missing items).
- [ ] **Step 3: Implement** the items above; `market.rs` `reprice` unchanged in form (D = total orders at the origin, Q = output, S = merchant inventory).
- [ ] **Step 4: Run** `cargo test -p ceteris-trade`; expected: PASS. Run `cargo test --workspace`; expected: PASS (econ still on `route.rs`).
- [ ] **Step 5: Commit** `feat(trade): source destination spending over origins and clear at the origin`

### Task 3: Wire sourcing into the world, mirror it in the reference

ADRs 0015 and 0016, second half.

**Files:**
- Modify: `crates/ceteris-econ/src/{world,systems,export,flows,equilibrium}.rs`, `tests/p2_properties.rs`, `tests/p2_stability.rs`, `tests/p2_acceptance.rs`; `python/ceteris/reference/{io,model,sinkhorn}.py`, `python/tests/test_reference.py`; `specs/trade/SPEC.md` (Routing, Payments, Active destinations, Province market, System order, Deviations: remove FOB, delivered pricing and pro rata rows, add the 0015/0016 rows), `specs/econ/SPEC.md`
- Delete: `crates/ceteris-trade/src/route.rs`, `src/transit.rs`

**Interfaces:**
- Consumes: Task 2.
- Produces: `EconWorld.price_index: Vec<f64>` (`[province * N_GOODS + g]`, P_d for traded goods, the local price for local goods), refreshed in E3 from `Kernel::origin_access_into` at current prices, read by E1 of the next tick; schedule IDs 201 to 210 in this order: T1 deliver (pipeline arrivals to buyers: households consume on arrival, enterprises stock), E1 labour (p_VA = (p_ig - sum_k a_kg P_ik) / a_VA with last tick's `price_index`), E2 produce (output to the origin market at p_o, SALE), E3 demand (household CES at P_d for traded goods; enterprise orders `(2 a_kg Y_cap - stock_k - on_order_k).max(0)`, spend = units x P_dk), T2 source (orders per origin), T3 clear (origin clearing, `Rationing::Priority` from `EconParams.rationing`), T2b dispatch (shipments, pipeline, payments d -> b -> a -> o as today, tariff to rebate; home flow delivered now), T4 reprice, E4 distribute, E5 assert. Buyers pay at dispatch for what is filled; unfilled spending stays with them.
- Export: `run_prices.arrow` gains `price_index` per (province, good); `run_flows.arrow` keeps (exporter_country, importer_country, good, value, units) with value = origin value of shipped units.
- Reference: unknowns ln p over origin cells with a market plus ln w; traded clearing is origin output = sum_d X_od / p_o with X_od = E_d K_od p_o^-theta / Phi_d; demand at P_d; `model.evaluate` returns `price_index`. `sinkhorn.balanced` unchanged; it is the S02 full solve.

- [ ] **Step 1: Write failing tests** Rust: `buyers_pay_origin_price_at_dispatch_in_minor_units` (replaces `trade_is_paid_at_delivered_prices_in_minor_units`; per buyer and tick, paid = sum of filled units x p_o x (1 + t), half-even minor units, gateways net zero), `origins_without_capacity_have_no_market` (replaces `inactive_cells_use_netback_price`), `input_orders_count_goods_on_order` (an enterprise with a full pipeline orders 0), `homogeneity_of_degree_zero` (kept; horizon as today), `replay_from_every_snapshot` (kept; add an assertion that the tick snapshotted has all four pipeline slots non-empty; Review Focus 3), `equilibrium_is_a_fixed_point` (tightened: from the reference, crossing lags forced to 0, 24 ticks: every price and wage within 1e-6 of the reference, ADR 0016's S02 comparison), `small_perturbation_decays` (kept); Python: `test_sinkhorn_and_dense_kernel_reproduce_equilibrium_gravity` (adapted: balanced Sinkhorn on K with the equilibrium's origin supplies and destination expenditures reproduces the sourcing flows, 1e-8), `test_two_province_closed_form` (adapted to sourcing), `test_trade_money_matches_dense_route_oracle` (adapted: buyer pays p_o (1 + t) per shipped unit).
- [ ] **Step 2: Run** `cargo test -p ceteris-econ` and `uv run --project python pytest python/tests/test_reference.py`; expected: FAIL.
- [ ] **Step 3: Implement** the world and reference changes; delete `route.rs` and `transit.rs`.
- [ ] **Step 4: Measure stability** with `small_perturbation_decays` instrumented (print the per-tick growth factor between ticks 300 and 600) at (kappa1, kappa2) in {0.02, 0.05, 0.1, 0.2} x {0, 0.05}, lags as built and all at 3. Record the table in the `TradeParams` kappa doc comments, replacing the Task 7b numbers. If the default (0.02, 0) no longer decays, stop and report the table to the controller before changing anything.
- [ ] **Step 5: Run** the WIP check; expected: PASS, V1 within 1%.
- [ ] **Step 6: Commit** `feat(econ): source trade from destination spending and clear origin markets across ticks`

### Task 4: Route-choice trade costs

ADR 0017, with the separate dispersion theta_r (P44).

**Files:**
- Create: `crates/ceteris-geo/src/route_choice.rs`, `crates/ceteris-geo/examples/route_choice_radius.rs`
- Modify: `crates/ceteris-geo/src/lib.rs`, `crates/ceteris-trade/src/{network,kernel,params}.rs`, `crates/ceteris-geo/tests/pack_properties.rs`, `specs/geo/SPEC.md` (route-choice section), `specs/trade/SPEC.md` (Leg factors, S05)

**Interfaces:**
- Produces:
  - `route_choice::EdgeFactors::new(g: &TransportGraph, weight: f64, time_cost_per_day: f64, theta_r: f64) -> EdgeFactors` with `t_kl = 1 + c_kl weight + time_cost_per_day hours_kl / 24` from `graph::price`, `a_kl = t_kl^-theta_r`, closed edges a = 0
  - `EdgeFactors::spectral_radius(&self) -> f64` (power iteration on the symmetric A, at most 10,000 iterations, relative change 1e-10)
  - `route_choice::solve_from(f: &EdgeFactors, source: u32, nodes: Option<&[u32]> /* subgraph */, warm: Option<&[f64]>) -> RouteTau { z: Vec<f64> /* [(I - A)^-1]_source,. */, hours: Vec<f64> /* expected route hours */ }`: conjugate gradient on I - A (symmetric positive definite when rho < 1), relative residual 1e-12; hours from the second solve with A o H: `T_sd = [(I - A)^-1 (A o H) z]_d / z_d`
  - `tau_sd = z_d^(-1/theta_r)`; panics with the radius when `spectral_radius() >= 1` (never clipped)
  - `ceteris_trade::network::LegTaus { domestic: Vec<[f64; N_GOODS]>, access: Vec<[f64; N_GOODS]>, crossings: Vec<[f64; N_GOODS]>, hours: same three }` and `Network.route_taus: Option<LegTaus>`; `Kernel::build` uses them when present (K = tau^-theta_g, lag from the good's hours), else the least-cost `Leg` costs (S05; the fixture)
  - `TradeParams.theta_r: f64 /* P44 */`
- The pack loader that fills `route_taus` is PLAN.md Task 17 (`EconInputs::from_pack`); this task builds and tests the module and measures theta_r.

- [ ] **Step 1: Write failing tests** `converges_to_least_cost_as_theta_r_grows` (6-node toy graph with two routes: tau from route choice -> Dijkstra cost factor as theta_r goes 10, 100, 1000, monotone, gap < 1e-3 at 1000), `derivative_matches_finite_difference` (d tau / d c on one edge, central difference h = 1e-6, 1e-6 relative), `closing_an_edge_is_continuous` (scale one edge's cost by 1 .. 1e6 in 20 steps: tau changes monotonically with no jump above 5% between steps), `two_parallel_routes_add_access` (closed form for two disjoint routes), `radius_above_one_panics` (`#[should_panic]`), `kernel_uses_route_taus_when_present` (trade); pack test (skips without a pack) `route_choice_radius_below_one_at_theta_r` for every traded good.
- [ ] **Step 2: Run** `cargo test -p ceteris-geo route_choice && cargo test -p ceteris-trade`; expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Research the prior** for theta_r: route-choice dispersion estimates in Allen and Arkolakis (2022) and related work (open the papers; cite pages). Record prior and start value as P44 in `specs/trade/SPEC.md` with sources.
- [ ] **Step 5: Measure** `cargo run -p ceteris-geo --release --example route_choice_radius` on the P1 pack: per traded good (w_g from the Task 13 report table, lambda_g from P43), the smallest theta_r with rho < 0.99 (bisection on log theta_r), and the build time of all domestic and gateway solves at that value. **If any good's minimum lies above the prior's upper end, stop and report the table to the controller** (owner decision pending, ADR 0017).
- [ ] **Step 6: Run** the WIP check (fixture unaffected); expected: PASS.
- [ ] **Step 7: Commit** `feat(geo): add route-choice trade costs with a separate route dispersion`

### Task 5: Sector trade elasticities

ADR 0018; writes ADR 0023 for the class boundaries.

**Files:**
- Create: `adr/0023-trade-elasticity-classes.md`, `python/ceteris/packs/econ/theta.py`
- Modify: `crates/ceteris-trade/src/{params,kernel,source}.rs`, `crates/ceteris-econ/src/export.rs`, `python/ceteris/reference/{io,model}.py`, `python/ceteris/reference/ppml.py`, `python/tests/{test_econ_pack,test_reference}.py`, `specs/trade/SPEC.md` (P01 row)

**Interfaces:**
- Produces: `TradeParams.theta: [f64; N_GOODS]` (local goods 0, unused), `TradeParams.theta_class: [u8; N_GOODS]`; kernels keyed per good as now (weight classes are goods; the class map is carried for later merging, ADR 0018 mapping); export `goods.arrow` gains `theta` and `theta_class`; `theta.caliendo_parro_by_good(baci_2000: Path, hs_isic: Path, isic_bridge: Path) -> np.ndarray[10]` (trade-value-weighted arithmetic mean of Caliendo and Parro (2015) sector estimates over the HS lines of each good, through the Task 13 concordance); `ppml.trade_elasticity(flows, exporter, importer, tariff, distance) -> Elasticity` (coefficient on ln(1 + t) is -theta).
- Class rule (agent default, ADR 0023): sort the seven traded thetas by log value and split into the fewest classes (3 to 5) whose within-class spread in log theta is at most ln 1.5; class theta is the trade-weighted mean.

- [ ] **Step 1: Write failing tests** `test_caliendo_parro_values_transcribed` (the table in `theta.py` equals the paper's, checked against the values cited in ADR 0023 with page), `test_theta_by_good_weighted_by_trade` (hand-made 3-line BACI sample), `test_classes_follow_rule`, `test_ppml_recovers_trade_elasticity` (synthetic gravity with tariffs, theta 6: within 0.05); Rust `kernel_uses_theta_per_good` (two goods with theta 2 and 8: K_leg = tau^-theta_g), `shares_homogeneous_in_prices` (kept, per good).
- [ ] **Step 2: Run** the pytest selection and `cargo test -p ceteris-trade`; expected: FAIL.
- [ ] **Step 3: Implement;** compute the vector from the BACI cache, write the values and the classes into `TradeParams::default()` with doc comments citing ADR 0023, widen the P01 prior in the spec to the range of the class values.
- [ ] **Step 4: Note** in `specs/econ/PLAN.md` Task 20 that H2 adds `test_h2_theta_by_class` (PPML on simulated flows recovers each class's theta within its prior band).
- [ ] **Step 5: Run** the WIP check; expected: PASS.
- [ ] **Step 6: Commit** `feat(trade): give each traded good its own trade elasticity`

### Task 6: Expected inflation, stability region and clip counts

ADR 0019.

**Files:**
- Create: `crates/ceteris-trade/tests/price_rule_stability.rs`
- Modify: `crates/ceteris-trade/src/{market,params}.rs`, `crates/ceteris-econ/src/{labour,world,systems,export}.rs`, `crates/ceteris-econ/tests/p2_properties.rs`, `specs/trade/SPEC.md` (8.2 rule, new "Stability region" subsection, P03c, P45), `specs/econ/SPEC.md` (wage rule)

**Interfaces:**
- Produces: `Markets::reprice(&mut self, p: &TradeParams, expected_inflation: f64)` adds pi^e before the clip; `Markets.clip_ticks: Vec<u32>` per cell (cumulative, encoded); `labour::wage_step(w, excess, expected_inflation, p, c_max) -> f64` with `dlnw = clip(pi^e + kappa_w x)`, rigidity on the nominal step when negative; `EconWorld.wage_clip_ticks: Vec<u32>`; `EconWorld.expected_inflation: f64`, updated after T4: `pi^e <- pi^e + g (ln(I_t / I_{t-1}) - pi^e)` with I the world price index of `price_index_weights` (moved from `p2_acceptance.rs` into `world.rs`, used by V1 too); `TradeParams.expectation_gain: f64 /* P45 */`; export `run_prices.arrow` and `run_wages.arrow` gain `clip_ticks`.
- Linearized origin market (one good, deviation x = ln p - ln p*, s = (S - S*)/S*, employment lag eta): excess demand (D - Q)/Q = -E x_t - (q_t) with q the supply deviation, q_{t+1} = q_t + eta (eps_S x_t - q_t), x_{t+1} = x_t - kappa1 (E x_t + q_t) - kappa2 s_t, s_{t+1} = s_t + (E x_t + q_t) / s_bar, with E the demand elasticity (theta for sourcing among origins at the margin, bounded by the CES sigma for total demand; the spec states which) and eps_S the supply elasticity from the labour target. The region is the set of (kappa1, kappa2) where the 3x3 transition matrix has spectral radius < 1 (Jury conditions written out in the spec).

- [ ] **Step 1: Write failing tests** `stability_region_matches_simulation` (linearized toy market: 20 (kappa1, kappa2) pairs on a grid, 10 inside and 10 outside the derived region by at least 10%: simulated deviation after 2,000 steps shrinks inside and grows outside), `reprice_adds_expected_inflation` (D = Q, S = S*: dlnp = pi^e exactly), `wage_rigidity_applies_to_nominal_cuts`, `clip_ticks_count_bindings`, `homogeneity_of_degree_zero` (kept: pi^e is a log difference, unchanged by the nominal scale).
- [ ] **Step 2: Run** `cargo test -p ceteris-trade --test price_rule_stability && cargo test -p ceteris-econ`; expected: FAIL.
- [ ] **Step 3: Implement;** research the gain's prior (adaptive-expectations gains used for firm price setting in Poledna et al. 2023 and the 12.2 heuristics; cite), record P45. Place the fixture's default (kappa1, kappa2) on the derived region in the spec; if the default lies outside, stop and report to the controller.
- [ ] **Step 4:** The GE reference is unchanged (pi^e = 0 at a steady state); say so in its module docstring.
- [ ] **Step 5: Run** the WIP check; expected: PASS.
- [ ] **Step 6: Commit** `feat(trade): add expected inflation to price and wage rules and derive their stability region`

### Task 7: PIGL demand

ADR 0021.

**Files:**
- Modify: `crates/ceteris-econ/src/{demand,params,inputs,fixture,systems,export}.rs`, `python/ceteris/reference/{io,model}.py`, `python/tests/test_reference.py`, `specs/econ/SPEC.md` (Final demand, Deviations: remove the homothetic CES row, Parameters), `specs/econ/PLAN.md` (Task 15: add the 2000 Gini column)

**Interfaces:**
- Produces: `demand::pigl(weights: &PiglWeights, price: &[f64; N_GOODS], money: f64, sigma_income: f64, p: &EconParams, out: &mut [f64; N_GOODS])`; `PiglWeights { phi: [f64; N_GOODS], nu: [f64; N_GOODS] }` per country, from `demand::invert_pigl(omega: &[f64; N_GOODS], base_spend: f64, sigma_income: f64, p: &EconParams) -> PiglWeights` (exact inversion at prices 1 and the country's base spending per pop); `Countries.gini: Vec<f64>`; pop income lognormal with mean m and `sigma = sqrt(2) Phi^-1((G + 1) / 2)`; fixture Gini 0.30, 0.40, 0.50 for the mainland countries, 0.45 for the island.
- Form: Alder, Boppart and Muller (2022) multi-sector PIGL. Step 1 transcribes their indirect utility, the share equation and the parameter roles into the econ SPEC with equation numbers. P06 and P07 carry the price and Engel parameters if the paper's form has exactly one of each (rename the rows); otherwise add P46 and P47. Pop spending on g is the share equation integrated over lognormal income using E[y^(1-eps)] = exp((1 - eps) mu + (1 - eps)^2 sigma^2 / 2).
- Shares outside [0, 1] at a pop's income are clamped to 0 and the rest rescaled (Review Focus 4); the reference does the same.

- [ ] **Step 1: Transcribe** the PIGL form into `specs/econ/SPEC.md` (cite the paper; open it).
- [ ] **Step 2: Write failing tests** `pigl_pop_demand_matches_quadrature` (64-node Gauss-Hermite over the lognormal: 1e-10 relative), `pigl_adds_up` (spending = money, 1e-14 relative), `pigl_inversion_reproduces_base_shares` (prices 1, base spending: shares = omega, 1e-12), `engel_curves_on_the_fixture` (across fixture pops ordered by income: food share falls, services share rises), `pigl_shares_stay_in_unit_interval_far_from_base` (money x 1e-3 and x 1e3), `homogeneity_of_degree_zero` (kept); Python `test_pigl_matches_rust` (shares on 100 random (price, money, sigma) draws, 1e-12) and `test_two_province_closed_form` (adapted).
- [ ] **Step 3: Run**; expected: FAIL.
- [ ] **Step 4: Implement** in Rust and the reference; research start values from the paper's estimates and record them.
- [ ] **Step 5: Note** in `specs/econ/PLAN.md` Task 15: `people.country_labour` also returns the 2000 Gini (WDI SI.POV.GINI nearest year 1995-2005, else WIID; imputed flag), and `econ_countries.arrow` gains `gini`.
- [ ] **Step 6: Run** the WIP check; expected: PASS.
- [ ] **Step 7: Commit** `feat(econ): replace homothetic ces with pigl demand over lognormal pop income`

### Task 8: Putty-clay coefficients, rationing switch and the same-tick local chain

ADR 0020.

**Files:**
- Create: `crates/ceteris-econ/src/local.rs`
- Modify: `crates/ceteris-econ/src/{inputs,params,world,systems}.rs`, `tests/p2_properties.rs`, `specs/econ/SPEC.md` (Production, System order, Deviations), `specs/trade/SPEC.md` (rationing row, S11)

**Interfaces:**
- Produces: `EntData.coef: Vec<[f64; N_GOODS]>` (inputs per unit of the enterprise's output, from its country's ICIO column at load); `EconInputs::coef(e, k) -> f64` replaces `a(c, k, g)` at every enterprise call site; `EconParams.technology: Technology::{PuttyClay, FixedLeontief}` (S09; FixedLeontief reads the country row); `EconParams.rationing: Rationing` (S11, default Priority); `Rationing::Mixed` added beside Task 2's `Priority` and `Proportional` (owner, 2026-10-24): read Pichler et al. (2022, JEDC) for its mixed rule, transcribe the definition with a citation into the trade SPEC rationing row first, then implement it in `Markets::clear`; `local::chain(world: &mut EconWorld, i: usize)` run inside E2 per province.
- Chain algorithm (per province, local goods L = {7, 8, 9}, fixed order 7, 8, 9):

```
Y_g = min(capacity_g, traded-input cap from stocks)            for g local
repeat up to N_LOCAL = 32 times:
    D_k = sum_g coef_kg Y_g                                     for k in L (all local and traded users)
    f_k = min(1, (Y_k + buffer_k) / D_k)                        buffer_k = local input stocks held
    Y'_g = Y_g * min over k in L with coef_kg > 0 of f_k
    stop when max_g |Y'_g - Y_g| <= 1e-12 Y_g; Y = Y'
inputs: each user draws coef_kg Y_g of k from this tick's local output first, its buffer second
households buy the remaining local output in T3
```

  Y is non-increasing, so the loop terminates; after the bound, the last f_k are applied once and any residue is drawn from the buffer, so Leontief use equals coef Y exactly. Traded goods keep their stocks and transit lags.

- [ ] **Step 1: Write failing tests** `putty_clay_one_vintage_equals_fixed_leontief` (120 ticks, both S09 settings: identical state hashes every tick), `local_shortage_reaches_downstream_same_tick` (cut services labour 50% at tick 10: construction output falls at tick 10), `traded_shortage_still_lags` (cut coal output at tick 10: downstream output unchanged at tick 10, lower once stocks run out), `local_chain_respects_intermediate_priority`, `local_chain_terminates_on_cycles` (coefficients with services -> services 0.3 and construction <-> services 0.2: under 32 iterations, Leontief invariant holds; Review Focus 5), `proportional_rationing_switch` (S11 Proportional: one fill ratio for both classes), `mixed_rationing_matches_definition` (S11 Mixed on a hand-computed two-class shortage, values from the transcribed definition).
- [ ] **Step 2: Run** `cargo test -p ceteris-econ`; expected: FAIL.
- [ ] **Step 3: Implement.** The GE reference is unchanged (the chain changes dynamics, not the steady state).
- [ ] **Step 4: Run** the WIP check; expected: PASS.
- [ ] **Step 5: Commit** `feat(econ): add putty-clay coefficients and a same-tick chain for local goods`

### Task 9: Uniqueness check and the gate

ADR 0022.

**Files:**
- Create: `python/ceteris/reference/uniqueness.py`
- Modify: `python/tests/test_reference.py`, `crates/ceteris-econ/tests/p2_acceptance.rs` (V1 docs), `specs/econ/SPEC.md` (Equilibrium nesting, P2 gate), `specs/econ/PLAN.md` (Tasks 18 and 20)

**Interfaces:**
- Produces: `uniqueness.spectral_bound(theta: np.ndarray, agglomeration: float = 0.0, migration: float | None = None) -> float` per Allen, Arkolakis and Li (2024) (the paper's matrix of elasticities for a multi-sector gravity model; transcribe the theorem into the spec with its number); `uniqueness.v1_mode(bound: float) -> Literal["tolerance", "report"]` (`"tolerance"` when the bound is at most 1).

- [ ] **Step 1: Write failing tests** `test_bound_reproduces_paper_examples` (the paper's worked cases: unique and non-unique parameter sets give bounds on the right side of 1), `test_trade_only_is_unique` (P2 defaults, no agglomeration or migration: bound below 1), `test_v1_mode`.
- [ ] **Step 2: Run**; expected: FAIL. **Step 3: Implement.**
- [ ] **Step 4: Rewrite** the P2 gate text: V1 runs in tolerance mode where the bound holds, otherwise both models start from the data state and the test reports which equilibrium each reaches; comparisons with CDP and KLR are sign and rank only.
- [ ] **Step 5: Move** direct estimation into `specs/econ/PLAN.md` Task 18 as its first step: PPML structural gravity on 2000 BACI with least-cost (S05) costs between countries (from the pack built in Task 17; route choice deferred past P2, ADR 0017) estimates theta by class (tariff coefficient), border friction P05 and mode cost ratios P04 in `python/ceteris/calibrate`, outputs with provenance "data"; it first reproduces the published estimates of Yotov, Piermartini, Monteiro and Larch (2016) on their sample (licence row in `sources.toml`). Productivity calibration then runs at the estimated values.
- [ ] **Step 6: Run** the WIP check; expected: PASS.
- [ ] **Step 7: Commit** `feat(reference): check equilibrium uniqueness before v1 and restate the p2 gate`

### Task 10: Goldens, changelog and records

**Files:**
- Create: `CHANGELOG.md`
- Modify: `tests/golden/p2-econ.txt`, `tests/golden/p2-fixture/**`, the three ignored or skipped tests (remove the markers), `adr/0015` to `0023` (status lines), `specs/econ/PLAN.md` (Task 19 trade budget 5 ms per tick on the full pack; `route` bench renamed `source_dispatch`; Task 21 register list: P39 to P47 instead of P37 to P41), `.superpowers/sdd/PLAN/progress.md` (migration done, resume at Task 13 review and Part D). The planned PLAN.md Task 17 amendment (`from_pack` fills `Network.route_taus` at P44) is dropped: route choice is deferred past P2 (ADR 0017, owner 2026-10-24), and Task 4 already wrote into Task 17 that `route_taus` stays `None`.

- [ ] **Step 1: Re-bless** `cargo run -p ceteris-econ --release --bin ceteris -- export --fixture --out tests/golden/p2-fixture`, `uv run --project python python -m ceteris.reference solve --inputs tests/golden/p2-fixture --out tests/golden/p2-fixture/reference`, `CETERIS_BLESS=1 cargo test -p ceteris-econ --release --test golden`.
- [ ] **Step 2: Remove** the ignore and skip markers; **run** `cargo test --workspace --release` and `uv run --project python --locked pytest python/tests` with no override; expected: PASS.
- [ ] **Step 3: Write** `CHANGELOG.md` with an "Unreleased" heading and one BEHAVIOUR entry listing ADRs 0015 to 0023, the goldens re-blessed (`p2-econ.txt`, `p2-fixture/`), the V1 maxima from the run, and the stability table from Task 3.
- [ ] **Step 4: Commit** `test(econ): re-bless the p2 goldens after the adr 0015-0022 migration` (body: why each golden changed).

---

## Out of this plan

- Theta estimation and the trade budget measurement: PLAN.md Tasks 17 to 20 as amended above. Full-pack route-choice taus are deferred past P2 (ADR 0017).
- Refreshing the design doc for the register additions (P39 to P47) and ADR 0023: PLAN.md Task 21.
