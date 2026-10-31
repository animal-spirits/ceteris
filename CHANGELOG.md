# Changelog

## Unreleased

### BEHAVIOUR: border friction per good in theta class 3 (ADR 0276, proposed)

- P05 is one value per good (`TradeParams::border_nu`, `EconInputs::border_nu`, reference
  `Inputs::border_nu`). `ceteris calibrate --pack` fits it over friction groups
  (`border::friction_groups`): the theta classes, with class 3 split into good 1 (crude oil, gas
  and refined fuels) and good 2 (coal and other mining), each clamped at nu 0 where its ICIO
  target is above its share there (ADR 0092's rule). On the rebuilt pack the class value 0.0524,
  set by crude oil, took coal and mining below its own share and the calibration failed.
- Export format 15 (`params.json` `border_nu` per good), calibration format 10 (`econ_calib.arrow`
  metadata `border_nu` per good): exports and calibration tables of earlier formats are refused.

Goldens re-blessed:

- `tests/golden/p2-fixture/`: export format 15 in every table, `border_nu` of ten values and a
  new static digest in `params.json`, `reference/solution.json` and the calibration (format 10);
  every column of every table is bit for bit the same (`ceteris pack compare` against 0e25521).
  The fixture keeps 0.3 for every good, so no price, quantity or money moves.
- `tests/golden/p2-econ.txt`: every hash (the static digest carries ten border frictions, not
  four). The hashes are the same at 1, 3 and all threads. p0-toy.txt and the core goldens do not
  move.

### Changed: B7 review round on competitive storage (ADRs 0091, 0267)

- The block map takes a stored cell's revenue from its buyers' orders (o / r, the merge read
  R (x + q + e), r times too large) and carries its stock and expectation as states (trade
  SPEC, Block gain). The calibration builds it at the export's storage parameters, and the
  structural digest now carries P56's money cost, P144, P145 and P146 beside P56's delta, so a
  run under storage parameters other than its calibration's is refused. A storage loss without
  speculation is refused (`TradeParams::check_storage`).
- With storage off (the default) nothing moves: the fixture's limits, its calibration and every
  golden hash are unchanged.
- The storage loop's toy test moved to `ceteris-calibrate/tests/storage_loop.rs` and runs on
  `stability::storage_matrix`; the conservation test asserts the spoilage tally and the release
  floor exactly.

### Changed: V1's horizon is 6,000 ticks where competitive storage is on (ADR 0091)

- `v1_fixture_frictionless_matches_reference` runs 6,000 ticks when any good stores, 1,200
  otherwise (owner, 2026-10-31): from the data state the stored goods converge slowly and not
  monotonically (flows 1.5% off at 3,600 ticks, 2.1% at 4,800, 1.4e-4 at 6,000). Storage is off by
  default, so the test and every golden are unchanged.

### Changed: B7 competitive storage (ADR 0091) merged into pre-P3 B, calibration format 9

- Merges claude/storage-bounds (edc2e8d, ee79611, ae1473d: B7 with g_s 0.006). Both sides had bumped the calibration
  format to 8 (ADR 0267's limit gains here, competitive storage there); the merged tables are
  format 9 (`ceteris_calibrate::steady::FORMAT`, `ceteris_econ::calibration::FORMAT`): format-8
  tables of either branch must be recalibrated.
- ADR 0267's country map (wage, local cells, households, pool) carries ADR 0091's storage
  response r_g on a stored good's own demand term, its cross terms and its routes' order shares,
  and on the per-market J (`MarketElasticity::j`); the pool's revenue is sales value, p Q / (1 +
  delta s) at a stored origin; the limit gains are computed at the inputs'
  P56 loss with P144 and P145 at their defaults. B7's guard that dropped the block scale at
  storage parameters off the defaults is gone with the scale: a run with storage on keeps the
  calibration's limit, as at other settings off the defaults (trade SPEC, Block gain).
- Storage stays off by default (`TradeParams::default`: psi and delta 0).

Goldens regenerated (`ceteris calibrate --inputs tests/golden/p2-fixture`):

- `tests/golden/p2-fixture/calibration/`: format 9 in every table's metadata, B7's static digest
  in `econ_calib` (with g_s 0.006), the block limit columns of ADR 0267; every column is bit for bit the
  pre-merge `pre-p3/econ-review` tables (`ceteris pack compare`), and against B7's (ae1473d) only the
  block columns differ (scales there, limits here). p2-econ.txt is B7's (ae1473d); p0-toy.txt and the core
  goldens do not move.

### BEHAVIOUR: storage expectation gain g_s 0.006 (P146, ADR 0091)

- `TradeParams::storage_gain` 0.08 to 0.006, P45's start (Milani 2007; owner approval
  2026-10-31): at 0.08 agriculture's storage loop leaves the stability region, at 0.006 the
  approved values pass the fixture's perturbation test. Storage stays off by default, so no
  price, quantity or money moves.
- ADR 0091's Tests amended (owner, 2026-10-31): `storage_smooths_a_short_cut_below_flow_clearing`
  and `cut_draws_the_stock_to_a_pipeline_floor` replace the ignored
  `stock_hits_zero_only_on_the_cut_side`; the storage loop's toy-market points are re-chosen for
  g_s 0.006.

Goldens re-blessed:

- `tests/golden/p2-fixture/params.json`, `calibration/econ_calib.arrow`,
  `reference/solution.json`: the static digest and `storage_gain` only; every column is bit for
  bit the same (`ceteris pack compare`).
- `tests/golden/p2-econ.txt`: every hash (static digest).

### Added: competitive storage for storable goods, off by default (ADR 0091)

- `Markets::store` at T3: the origins of goods 0, 1, 2, 5 lose delta S of their stock (destroyed,
  tallied) and the merchants order b = max(delta S + psi D x, -(1 - delta) S), x the expected
  return of holding with a convenience yield in cover; the price step reads D + b with kappa2 off
  there, and each such market keeps an expectation pi^e_og (gain g_s). Spoilage replacement in
  the calibration (stage 1 profits, stage 2 sales targets, stage 5 stocks at s D) and the GE
  reference; J and the block map carry the storage order's response. Parameters P56 (delta, k),
  P144 (psi), P145 (zeta), P146 (g_s) in DESIGN 17.
- Off by default (owner ruling 2026-10-31): psi and delta default to 0, since at the approved
  values (`TradeParams::with_storage`) the fixture's default mode grows by 1.0069 a tick from a
  perturbed steady state and frictionless V1 fails. With storage off every price and quantity is
  as before; the storage tests run with it on.
- `Markets` carries `expectation` and `storage` in the snapshot; export format 14
  (`goods.storage_loss`, the storage parameters in `params.json`); calibration format 8 (every
  calibrated pack is recalibrated). `ceteris calibrate stability --storage` prints the storage
  loop's kappa1 limits at the approved values.

### BEHAVIOUR: golden hashes move with the snapshot layout and the static digest (ADR 0091)

- No price, quantity or money moves (storage is off by default); the state hash covers the two
  new market vectors and the static digest the five storage parameters.

Goldens re-blessed:

- `tests/golden/p2-fixture/`: export format 14, `goods.storage_loss` (0), the storage parameters
  in `params.json`, a new static digest; every other value is bit for bit the same.
- `tests/golden/p2-fixture/calibration/` and `reference/`: calibration format 8 and the new
  static digest; every column is bit for bit the same (`ceteris pack compare` against 7ca3a2b).
- `tests/golden/p2-econ.txt`: every hash (snapshot layout and static digest).

### BEHAVIOUR: crude oil and gas from rents produce with the world extraction column (ADR 0270)

- `world_oil_gas_column` sums the ICIO extraction industries (B06, B09) only, which
  `aggregate` now keeps per economy (`CountryIo::extraction`, `extraction_output`). With
  refining in good 1 (ADR 0092) the good-1 column has a_VA 0.418 on 2000 data; extraction has
  0.666, as before refining moved. WDI rents measure extraction, so the merged column gave rent
  economies 1.59 times the gross output and 5 times the own use of good 1.
- `econ_io` moves for the non-ICIO economies with a rent, and so do the calibration and the
  `p2-pack` goldens (to re-bless with the B8 rebuild); the fixture goldens do not move.

### BEHAVIOUR: the block cap linearizes each country's tick and caps at its limit gain (ADR 0267, block review)

- The block map now carries the wage (frictionless clearing, the wage rule at the gains of a
  rise and a cut), local markets and their draws, the households' PIGL and CES demand on this
  tick's wages and last tick's dividends, the profit pool and the move of each slot's months in
  transit with the origins' shares (`ceteris_calibrate::block`): a block is a country, local
  cells included. On the fixture a coupled block beyond its limit grows in `EconWorld` at the
  map's radius to within 2e-3 (`block_map_growth_matches_the_tick`); the first build's map read
  it 0.05 too unstable.
- The calibration stores each capped block's limit gain, b kappa_cell / m (infinite elsewhere),
  in `block_limit_default` and `block_limit_frictionless` (calibration format 8: format-7 tables
  must be recalibrated), and a run caps each cell at min(kappa1, 2 m / J, m L)
  (`MarketElasticity::price_gain`). The cap is continuous in kappa1, P03d and eta: the first
  build dropped it at any setting off the defaults (kappa1 0.0500000001 ran uncapped).
- The calibration checks each capped block stable at its own gains and stops otherwise; the
  stability tolerance 2e-3 is registered (P157).
- Behaviour moves only in capped blocks of calibrated packs; the steady state does not move. On
  the k263 scratch pack (format 8) 11 of 233 blocks are capped in the default mode (Sudan
  0.244, Lesotho 0.333) and 41 frictionless (Sudan 0.341, Albania 0.706, France 0.721). Default
  mode, seed 20: Sudan 2.06 to 1.3e-2 (the first build took it from 1.17 to 2.16). Regression
  against the first build: frictionless Sudan at seed 20 stays far off, 10.75 to 8.37 (0.34
  under the first build's cap of traded cells only). The pack steady-state test now fails on
  the wage's boundedness (Pitcairn, 0.10 of its tolerance, 2.35 times its early drift), no
  longer on the world price index.

Goldens re-blessed:

- `tests/golden/p2-fixture/calibration/`: calibration format 8, the block columns renamed to the
  limit gains (all infinite on the fixture, which caps no block); every other column is bit for
  bit the same (`ceteris pack compare` against the format-7 tables). p2-econ.txt, p0-toy.txt
  and the core goldens do not move.

### Changed: the dispatch-mass tolerance carries the rounding below the normal range (owner ruling on x10, ADR 0090)

- A product below 2^-1022 rounds by up to 2^-1074 absolute, and the route factors after it scale
  that error into the units, so dispatched units and sales could differ by far more than 1e-12
  of either at normal flows (x10 border run: 1.3e-9 at sales of 1.5e-297, from a fill times an
  origin weight of 2.8e-310; the reach rescale was 0 throughout). `source` and `dispatch` now
  carry a bound on that rounding (`Sourced::orders_rounding`, `Sourced::shipped_rounding`), and
  `end_tick` allows 1e-12 of the larger plus that bound plus 2^-1022 (trade SPEC, Invariants).
- No hash moves: the bound is scratch and only widens the check. The x10 run now stops at tick 61
  on a payment that does not fit the ledger (block scratch pack), beyond P2's range (ADR 0090).

### BEHAVIOUR: main's whole first-level provinces (ADR 0251) merged into pre-P3 B

- Merges main 2d41a65 (ADR 0251; 27 files, geo only, since main 6fe4c2f). The geo layer is
  main's: 4,594 whole first-level provinces (4,796 split), so every province index moves. The
  econ layer is this branch's: gateway masses (ADR 0092), refined fuels in good 1, calibration
  format 7 with the block scales (ADR 0267).
- Neither side's pack is valid for the merge: main's pack has no gateway mass and calibration
  format 3, this branch's scratch pack (k263) has the split provinces. The merged pack is rebuilt
  and recalibrated at B8 (pre-P3 PLAN, B8 Step 2a); until then every pack-dependent test skips
  or reads a stale pack, and the numbers below that need the pack are not yet measured.
- `crates/ceteris-pack/tests/econ_build.rs` pins main's gateway kinds (286 port, 171 airport,
  143 land crossing) and value-added count (4,368) with this branch's imputed share 0.0754. The
  share is predicted from this branch's change on the split pack (0.0759 to 0.0754), not
  measured on the merged pack.
- Figures this branch measured on the split pack are stale after the merge: ADR 0267's scaled
  blocks (54 of 233 frictionless, 12 default; Albania 0.636, Sudan 0.324, Lesotho 0.348) and
  the Sudan provinces 3464 and 3474; the 4,555 provinces with value added and H2 -0.753 in
  earlier entries. B8 re-measures them.
- Goldens not re-blessed here: `tests/golden/p1-pack.txt` and `p2-pack.txt` carry main's
  hashes, built without this branch's pack changes, and are re-blessed at B8. `p0-toy.txt`,
  `p2-econ.txt`, `p2-as-built-rule.txt` and `tests/golden/p2-fixture` do not read the pack and
  do not move (core and econ golden tests pass).

### Changed: the pack's steady-state money tolerance scales with income (P156)

- `p2_steady_state.rs`: pop income and money hold to max(4 minor units, 2e-6 y_k) on the full
  pack, y_k the pop's steady monthly income (was 2e-6 of the quantity itself; owner approval
  2026-10-30T23:19Z, DESIGN 17 P156). The fixture keeps 1e-9 relative. On the k263 scratch pack
  (format 7) pop money reads 0.511 of its tolerance (1.130 under the old rule); pop income 0.734
  under both.

### BEHAVIOUR: price gain capped per coupled block of commodity markets (ADR 0267)

- Each country's traded cells are linearized together at the calibrated steady state: cross-price
  substitution between its origins of a good, input orders of its enterprises through the
  order-up-to position, and their output's answer to last tick's input prices. A connected
  block whose map is unstable at gains kappa_cell / m (m = P03d) is scaled by the largest b on a
  2^-12 grid that makes it stable there; kappa1_cell = b kappa1 min(1, J_ref / J_cell)
  (`ceteris_calibrate::block`, `world::price_gains`, `MarketElasticity::block_scale`).
- b is computed at the default kappa1 and m for the default mode and frictionless and written to
  `econ_calib_cells.arrow` (`block_scale_default`, `block_scale_frictionless`); calibration
  format 7, so format-6 tables (every calibrated pack, `data/packs/2000` included) must be
  recalibrated. It loads into `EconInputs::market_elasticity` (inputs digest). A run at another
  eta, kappa1 or m keeps the per-market cap alone.
- Behaviour moves only in scaled blocks of calibrated packs. The steady state does not move. On
  the k263 scratch pack 54 of 233 blocks are scaled frictionless (Albania 0.636) and 12 in the
  default mode (Sudan 0.324, Lesotho 0.348); the calibrated fixture scales none.

Goldens re-blessed:

- `tests/golden/p2-fixture/calibration/`: calibration format 7 and the two new columns (all 1 on
  the fixture); every existing column is bit for bit the same (`ceteris pack compare` against the
  format-6 tables).
- `tests/golden/p2-econ.txt`, `p0-toy.txt` and the core goldens do not move (the fixture runs from
  its data state, without a calibration, and the calibrated fixture scales no block).
  `p2-pack.txt` waits for the pack recalibration (pre-P3 B8).

### Changed: a user's stock is a floor under the local fill (ADR 0093, amendment of 2026-10-30)

- Defect fix of the as-built chain rule: each user's output was cap min_k F_k, the common fill
  applied even to users whose own stock covers their need, against the split's own rule (the
  stock covers the remainder). Now Y = cap min_k G_k(F_k), G = s + (1 - s) F with s the share of
  the user's need at capacity its stock covers: F is the share of each user's uncovered need that
  the good's output serves. A user fully covered by its stock is not cut, and with no output of
  the good a user runs on its stock. A user without stock is as before, bit for bit.
- Each user's uncovered need is computed from its levels, not as coef Y - b, which cancels to the
  rounding of b where a user runs mostly on its stock (133 of 100,000 stocked random provinces
  stalled that way). The split draws it.
- No golden moves: the chain never binds on the fixture's golden runs, and the steady state never
  rations. Outputs move wherever a stocked user of a short local good was cut below its stock
  (pack: an Algerian enterprise with 2.2 months of every local input in stock produced nothing).
- Each user's floor of each local good is computed once, when the province is built
  (`Province::new`, `Province::from_parts`), not in the chain's inner loops: on 100,000 random
  provinces `local::fills` takes 0.43 us per province with stocks (0.80 before) and 0.37 without
  (0.42), the passes the same. A province whose `cap`, `coef` or `stock` changes after it is
  built is rebuilt (`Province::rebuilt`).
- Fill tests: the translated collapses take producers' floors for the constant stock thresholds
  gave (`translate`, `translated_original`, `translatable`), with the sample and full run
  re-measured; pinned seeds that the floor changes are re-measured or replaced, each with its
  reason; `translated_collapse_stop_refuses_an_off_piece_fixed_point` is removed (no province
  under the floor reaches that case) and replaced by a hand-built regression
  (`local::tests::the_stop_refuses_a_fixed_point_past_a_cover_point`); the stop's result on a
  line of fixed points of gain exactly 1 is pinned and the SPEC records that the fills are not
  unique there (`neutral_good_stop_takes_a_fixed_point_on_its_line`).

### Fixed: the write-off's country total is i128 (ADR 0106)

- `systems::write_off` narrowed the country total to i64 and panicked where the pops' money and
  the excesses passed it (the scratch pack's x10 border run, Taiwan, tick 49). The total moves
  through the wide pool account and stays i128, as does the largest-remainder total (exact long
  division, `mul_div`); each part stays within its weight. No hash moves.

### BEHAVIOUR: inputs ordered by effective demand; the own good caps labour too (ADR 0266)

- E3 orders a traded input whose cap cut the enterprise's labour at E1 on notional output,
  VA(L_notional) / a_VA, and every other input on constrained output, VA(L) / a_VA at this tick's
  employment (Benassy 1975, 1982; Dreze 1975; owner decision 2026-10-30T22:43Z). E1 hands E3 the
  binding inputs (`systems::scarcest_inputs`, `Scratch::short`) and the employment it would
  allocate without the cap (`Scratch::notional`).
- The labour cap counts every traded input, the enterprise's own good included (owner decision
  2026-10-30T22:43Z); the own-good exclusion of the first build covered a lock that came from E3
  planning orders on the output the cap had cut.
- Why the hashes move: in the fixture's default run caps bind from tick 1 as stocks run short, and
  the short inputs are now reordered on notional output; the P2 fixture golden differs from tick
  12. Where no cap binds nothing changes: the fixture's calibrated state and the scratch pack's
  steady start are bit for bit as before (pack: through tick 72, where the first cap binds).
- Stress (ADR 0266, As built): frictionless sweep seeds 0 to 9 no panic (one before), structured
  perturbations 8 and 2 overflows of 24 (11 and 10 before), the spiral case at index 1.55 (2.7).
  `input_orders_plan_on_capacity_output` is now `input_orders_plan_on_notional_output_for_the_short_input`;
  `frictionless_unemployment_is_the_natural_rate` also allows province 20 at ticks 8 and 9 and
  checks the wage where caps leave more than u* unemployed.

### BEHAVIOUR: labour capped at what traded inputs allow (ADR 0266)

- E1 caps each labour target at the labour whose output the enterprise's traded input stocks
  after T1 allow, its own good left out (included since: entry above) (`systems::input_output`,
  `production::labour_for_value_added`). Local inputs set no cap: the same-tick chain bounds
  them, and both bounds from last tick's chain failed (owner decision 2026-10-30; ADR 0266). Both wage rules read the capped targets. Frictionless, the wage
  clears on them; where no cap binds at the uncapped root that root is kept bit for bit, and
  where every enterprise that would hire is at its cap (flat demand) the wage is min(last wage,
  lowest marginal value product at the caps) (owner decision 2026-10-30).
- Why the hashes move: the fixture's data state holds one month of base inputs, so the caps sit
  at L0 from tick 0 and bind wherever stocks run short; the P2 fixture golden differs from tick
  12. Steady starts do not move until a cap binds (scratch pack: see ADR 0266, As built).
- The frictionless ledger overflow is gone: `frictionless_labour_spiral_overflows_the_ledger`
  is now `frictionless_labour_spiral_is_bounded`, and a seeded 40-draw frictionless price sweep
  is added. `frictionless_unemployment_is_the_natural_rate` allows more unemployment where caps
  bind (province 24, tick 1).

### Added: benchmark harness with welfare and ACR in Rust (ADRs 0200, 0207)

- `ceteris_calibrate::shock` parses shock files (countries by ISO3 code) and applies tariff,
  iceberg (ntb, border) and productivity shocks; iceberg shocks fold the bulk legs into route
  taus, so the kernel is the default one but for the shocked crossings.
  `ceteris_calibrate::benchmark` runs a calibrated pack frictionless from its steady state and
  computes welfare, the multi-sector ACR value and the foreign share.
- The comparative-statics example of 28 October (90a6940) moves to
  `crates/ceteris-calibrate/examples/shock.rs` as a wrapper over both, with its four CSVs.
- Benchmark shock files in `crates/ceteris-calibrate/tests/data/benchmark/`. No behaviour moves.
- `benchmark::acr_io`: the ACR prediction with input-output linkages, -Omega' (I - A')^-1
  diag(1/theta) d ln lambda (Costinot and Rodriguez-Clare 2014; Caliendo and Parro 2015), A the
  country's cost shares; ADR 0265 (proposed) makes it ADR 0207's ACR criterion, which C7
  asserts at B8. `acr` and `acr_io` give NaN, not +inf, where a home share vanishes in
  one run only. Hard Brexit adds a tariff only up to the EU common external tariff (CYP, HRV,
  MLT and ROU already met it with GBR in 2000). `EconInputs::fold_bulk_into_route_taus` is the
  one fold of the bulk legs, used by the shocks and `fixture::unreachable_island`.

### BEHAVIOUR: per-market price gain, capped at half each market's stability limit (ADR 0263)

- T4 moves each market by kappa1_cell = kappa1 min(1, J_ref / J_cell), J_ref = 2 m / kappa1 with
  the new P03d m = 0.5 (`TradeParams::gain_share`, `TradeParams::price_gain`) and J_cell = E +
  eta eps / (2 - eta) at the run's eta. `Markets::reprice` takes the gain per cell;
  `Layout::price_gain` holds it, computed once in `EconWorld::new` (`world::price_gains`).
- The calibration computes E (own-price elasticity of the cell's orders, Leontief buyers at
  Gamma P_d) and eps (the largest supply elasticity of the cell's enterprises; ADR 0263 amended
  from the output-weighted mean as built) at the calibrated point
  (`ceteris_calibrate::elasticity`) and writes them to `econ_calib_cells.arrow`
  (`demand_elasticity`, `supply_elasticity`); calibration format 6, so format-5 tables (every
  calibrated pack, `data/packs/2000` included) must be recalibrated. They load into
  `EconInputs::market_elasticity` (inputs digest); P03d joins the static digest only for inputs
  that carry them. Inputs without a calibration run kappa1 in every market.
- Behaviour moves only where a market's J exceeds 20: frictionless runs of a calibrated pack
  (the 0256 pack: 298 traded cells), and in the default mode wherever a cell's J exceeds 20.
  The study found none on main's 0256 pack (J at most 16.9), and the fixture has none (12.5;
  frictionless 13.8, so no fixture run moves). That is not established for the rebuilt pack:
  this branch's scratch pack (pre-0256) caps 383 traded cells in the default mode, J up to 32.4
  at Lesotho g1 (eps 84.81 at a_VA 0.021309, from Senegal's donor good-1 column, which 14
  economies share: BEN CAF COM GIN GMB GNB LSO MDG SDN SEN TGO TZA ZMB ZWE), and ADR 0256
  replaces that column only for economies with a rent. Pending B8, which lists the
  default-capped cells on the rebuilt pack, and an owner ruling if any remain (ADR 0263, As
  built).
- A traded cell that reaches no buyer with spending gets E = 1 + theta (Pi 0), as in the study.
- A data-state start of calibrated inputs under structural parameters other than the
  calibration's is refused (`EconWorld::new`), as a steady start is, since its gains would be
  stale. `EconWorld::set_network` keeps the calibrated gains.
- `params.json` of an export gains `gain_share` (P03d).
- `conservation_over_120_ticks`'s frictionless case that overflows the ledger is now the ignored
  test `frictionless_labour_spiral_overflows_the_ledger` (known defect: E1 hires labour it cannot
  equip; owner decision pending), so the property test is green.

Goldens re-blessed:

- `tests/golden/p2-fixture/calibration/`: calibration format 6 and the two new columns; every
  existing column is bit for bit the same (`ceteris pack compare` against the format-5 tables).
- `tests/golden/p2-fixture/params.json`: `gain_share` 0.5 under P03d; the static digest does not
  move. The fixture's other export tables and its reference do not move.
- `tests/golden/p2-econ.txt`, `p0-toy.txt` and the core goldens do not move (the fixture runs
  from its data state, without a calibration). `p2-pack.txt` waits for the pack recalibration
  (pre-P3 B8).

### Changed: H2 distance reported, H2 per class on the model's kernel (ADR 0264)

- `p2_gate.rs::h2_distance_elasticity` prints the distance coefficient against -1.1 to -0.7 and
  fails only on a non-finite or non-negative one (owner ruling A, 2026-10-30).
- `h2_theta_by_class` controls for the model's country-pair kernel net of tariffs
  (`model::ln_country_kernel`, sourcing weights) and gates theta_c within max(2 pair-clustered
  s.e., 1% of theta_c); the ln distance version is printed. New `ppml::class_tariff_fit`.
- No golden moves.

### BEHAVIOUR: bulk freight rate on for theta class 3 by default (ADR 0247, owner ruling 2026-10-30)

- P154 0.026 and P155 0.40 for class 3 (goods 1 and 2), 1 for classes 0 to 2, in
  `TradeParams::default`. ADR 0247's criterion 3 was ill-posed (H2 distance was outside its band
  before the rate, -0.509 off, -0.463 on); criteria 1 and 2 pass. Class 3 now has a border
  friction root on the pack (nu 0.0078 on the B2 scratch pack), so ADR 0092's clamp no longer
  applies to it.
- The fixture builds its own bulk legs: its six sea crossings at P154 times its sea cost per
  tonne-km (it has no ports, so P155 scales nothing); every other leg is unchanged.
  `TradeParams::bulk_off` gives the rate off, for networks without bulk legs: hand-built test
  networks, the autarky world and `fixture::unreachable_island` (route taus exclude bulk legs,
  so it folds class 3's bulk legs into its route taus: its kernel is the default fixture's but
  for the cut).

Goldens re-blessed:

- `tests/golden/p2-fixture/`: only class 3 moves. `legs` gains the bulk costs and hours (NaN
  before), `kernel_crossing` changes for goods 1 and 2 only (k up to 2.6e4 times for coal on
  the sea lanes, inv_loss by up to 82%), in `provinces` `base_price_1` and `base_price_2`
  fall by up to 12% and 45%, and `gamma_1` and `gamma_2` move by up to 1.9e-4; `params.json`
  sets the class-3 bulk fields, and the static digest changes. Every other value is bit for bit
  the same.
- `tests/golden/p2-fixture/calibration/`: goods 1 and 2 move by up to 1.4% in price and output;
  coal's input coefficient and stock (`coef_2`, `stock_2`) by up to 80%, in units, since its
  delivered price falls by up to 44%; the pipeline by up to 80% (coal). Every other good moves
  by 1e-5 or less, through the general equilibrium.
- `tests/golden/p2-fixture/reference/`: the frictionless GE equilibrium moves with the cheaper
  coal and oil crossings: cross-border coal flows by up to 9.8 times, oil 47%, other goods 2.6%
  or less; prices by up to 1.3%, wages 0.75%.
- `tests/golden/p2-econ.txt`: every hash (the inputs digest carries the fixture's bulk legs).
  `p0-toy.txt`, `p1-geo.txt` and `p1-pack.txt` do not move; `p2-pack.txt` waits for the pack
  rebuild (pre-P3 B8).

### BEHAVIOUR: buyers pay the CES index with selection on trade links (ADR 0200)

- Buyers of a traded good pay q_d = Gamma_d,g P_d, the CES index times a selection constant per
  cell fixed where the world is calibrated (owner ruling 2026-10-30, option A): there q_d is the
  delivered unit price of the physical arrivals, 1 / alpha_d, as before. Every route delivers
  what the buyer paid over q_d, whatever the origin: shipped units melt by the route's physical
  loss and selection adds (or, where the physical cost is below q_d, takes) the rest.
  `Sourced` gains `buyer_price`, `home` (origin units bought at home), `survived` (by arrival
  slot) and the `selected` tally beside `lost`; `source` takes Gamma per province.
- T2b books selection in the ledger: home purchases arrive as delivered units at the buyer,
  and each transit account is brought to the pipeline, creating what selection adds and
  destroying the melt. A new invariant checks every tick that what T2b creates less what it
  destroys is the selection term less the melt.
- `ProvinceData::gamma` (in the inputs digest): the data state's 1 / (alpha_d P_d) at prices 1,
  or the calibration's q_d / P_d at the calibrated prices. Calibration format 5
  (`econ_calib_cells.arrow` gains `gamma`); export format 13 (`provinces` gains `gamma_g`). The
  calibration prices buyers at 1 / alpha_d at every pass (`Model::physical`); the reference
  model prices them at Gamma P_d where its inputs carry Gamma.
- The lead and the calibrated pipeline count arrivals by value over q_d instead of by
  physical units, so the arrival slots of a destination are weighted by what it pays on each
  route.

Goldens re-blessed:

- `tests/golden/p2-fixture/`: `provinces` gains `gamma_0` to `gamma_9`, and `params.json` its
  format and static digest. Every other value of the export, base prices included, is bit for
  bit the same (the other tables change only in their `export_format` metadata).
- `tests/golden/p2-fixture/calibration/`: stage 1's values (output value, value added, sales,
  rebates, prices, coefficients, PIGL weights) are bit for bit the same; the lead moves by up
  to 0.165 months (52% relative in one cell; the sum over cells +0.8%) and the pipeline by up to
  52% in one entry (+1.4% in total units), because arrivals are now split over slots by value;
  `gamma` is new.
- `tests/golden/p2-fixture/reference/`: the frictionless GE equilibrium moves (prices by up to
  2.0%, wages 2.0%, cross-border flows up to 16%): it lies away from the data state, where
  buyers now pay Gamma P_d rather than 1 / alpha_d.
- `tests/golden/p2-econ.txt`: every hash. The inputs digest carries Gamma. The fixture run
  starts at the data state, where its Gamma is fixed, so buyers pay 1 / alpha_d at tick 0; the
  data state is not a steady state, so prices move from tick 1 and q_d then follows P_d rather
  than 1 / alpha_d. Arrivals are also split over slots by value from tick 1.
- `p0-toy.txt`, `p1-geo.txt` and `p1-pack.txt` do not move. `p2-pack.txt` waits for the pack
  rebuild (pre-P3 B8).

### BEHAVIOUR: bulk freight rate per theta class, built and off; class 3 clamped at nu 0 (ADRs 0247, 0092)

- A theta class can be shipped at a bulk freight rate: its legs are least-cost paths on the
  graph repriced with the sea cost per tonne-km times P154 and the listed and minor port fees
  times P155 (`TransportParams::bulk`, `Network::bulk`, built at load where a class is off 1;
  in the inputs digest only when present). The sourced class-3 values are 0.026 and 0.40, from
  2000 tanker and dry-bulk voyage rates (UNCTAD Review of Maritime Transport 2001 and 2002).
- Off by default (1 for every class): ADR 0247's ablation test failed on H2 distance (-0.463
  with the rate on, band -1.1 to -0.7).
- The border friction fit clamps a class at nu = 0 when its ICIO target is above its share at
  nu 0, and reports it (`Fit::clamped`), instead of stopping (ADR 0092, Amendment 2026-10-30);
  on the 2000 pack class 3 is clamped (target 0.2964, share 0.2007).
- Export format 12: `legs` gains `bulk_cost_per_t` and `bulk_hours`, `params.json` gains
  `bulk_sea`, `bulk_port` and `bulk_classes`.

Goldens re-blessed:

- `tests/golden/p2-fixture/`: the export's tables for format 12 (schema metadata, the two new
  leg columns, NaN on the fixture, which has no bulk legs) and `params.json` (the new fields). The
  fixture's kernel, digest, reference solution and calibration do not move.
- `tests/golden/p2-econ.txt`, `p0-toy.txt`, `p1-geo.txt` and `p1-pack.txt` do not move.
  `p2-pack.txt` waits for the pack rebuild (pre-P3 B8).

### BEHAVIOUR: calibration passes mixed in relative units

- The Anderson mixing of the calibration passes holds the rebates and the PIGL scales relative to
  their block's largest magnitude, the units `change` measures them in, instead of in money: its
  least squares weighed the rebates' residual (about 1e7) against the prices' (about 1) and fitted
  the rebates alone. On the full pack with the B1 tables the passes wandered for 100 passes; the
  pass map itself cycles there. No equation or fixed point changes.

Goldens re-blessed:

- `tests/golden/p2-fixture/calibration/`: every table moves by at most 3.0e-12 relative (the
  pipeline units; prices and coefficients by at most 6e-13), and the fixture now settles in 24
  passes instead of 21. The fixed point is the same to the 1e-12 the passes stop at; the
  different path ends at a different point inside that tolerance.
- `tests/golden/p2-econ.txt`, the fixture export and reference, `p0-toy.txt`, `p1-geo.txt`,
  `p1-pack.txt` and `p2-pack.txt` do not move.

### BEHAVIOUR: gateway masses and the border friction per theta class (ADR 0092)

- Every gateway carries a mass, its share of its country's 2000 trade throughput (1/k for a
  country's k gateways: the pack has no throughput per gateway); a crossing a to b weighs
  m_a m_b, so access no longer grows with the number of gateway pairs. `gateways.arrow` gains
  `mass`; a pack without it is refused at load with an error naming the column.
- P05 is one nu per theta class (`TradeParams::border_nu: [f64; 4]`); the default stays 0.3 for
  every class. `ceteris calibrate --pack` fits them to ICIO 2000's cross-border share of each
  class (the new pack table `econ_home_shares.arrow`) before the steady state, records them in
  the calibration (format 4), and a calibrated world runs under them
  (`EconInputs::trade_params`). The fixture keeps the default.
- Export format 11 (`gateways.mass`, the masses in the crossings' k, `border_nu` a list); the
  static digest encodes nu as a list.
- The P2 gate's H2 per class asks that each class's theta_c lie within 2 pair-clustered
  standard errors of its PPML estimate (ADR 0092), in place of the prior bands.
- Calibration's stage 2 scaling may run 100,000 sweeps (was 20,000; a slow mode on the full pack
  at theta 16.91), and an Anderson-extrapolated step that leaves f64 restarts the mixing from the
  plain sweep it extrapolated instead of failing. Neither moves a golden.

Goldens re-blessed:

- `tests/golden/p2-econ.txt`: every hash (the masses change the fixture's crossing factors, and
  nu enters the digest as a list). The hashes are the same at 1, 3 and all threads.
- `tests/golden/p2-fixture/`: the export in every table (format 11), the reference solution and
  the calibration (format 4, `border_nu` in its metadata) at the mass-weighted kernel.
- `tests/golden/p0-toy.txt`, `p1-geo.txt` and `p1-pack.txt` do not move. `p2-pack.txt` waits for
  the pack rebuild (pre-P3 B8): the installed pack has neither the mass column nor the home
  shares.

### BEHAVIOUR: PIGL inverted jointly over each country's pops at within-pop dispersion (ADR 0094)

- Every pop spends at its country's within-pop log income dispersion s_w, s_w^2 = max(0, s_nat^2 -
  Var_k ln m_k) over the pops' base incomes per person, instead of the national Gini's s_nat.
- The PIGL weights are inverted per country jointly over its pops, each at its own buyer prices
  and income, so that their spending-weighted shares are the 2000 final demand shares exactly:
  at prices 1 and the base incomes at load, at the province buyer prices of each pass in
  calibration stage 4 (which no longer inverts at country unit-value prices).
- The weights carry s_w: `pigl_sigma` in `econ_calib_countries.arrow`, and in the inputs digest.
  The calibration summary lists the countries with s_w = 0 (`within_sigma_zero`).
- The calibration passes stop at a relative change of 1e-12 (`PASS_TOL`, was 1e-11), so the
  returned state is a fixed point to the 1e-12 its checks claim; the fixture now takes 21 passes
  (20 before this change).

Goldens re-blessed:

- `tests/golden/p2-econ.txt`: every hash (the weights and s_w). The hashes are the same at 1, 3
  and all threads.
- `tests/golden/p2-fixture/`: `pigl_check.arrow`, the calibration (the PIGL weights, `pigl_sigma`,
  the state at the new household shares, 21 passes at the new pass tolerance) and the reference
  solution.
- `tests/golden/p0-toy.txt`, `p1-geo.txt`, `p1-pack.txt` and `p2-pack.txt` do not move.

### Changed: one exact fill per local good (ADR 0093)

- The same-tick chain gives each local good one fill, the limit of the fill iteration from 1,
  solved exactly with the work bounded (ADR 0093, amendments of 2026-10-29); the 32-pass bound and
  the clearing of what it left short are gone. It changes outputs wherever the chain binds. No
  golden moves: the fixture's default and frictionless runs never bind it.
- Rules that can change outputs where the chain binds:
  - No fall below rounding (4,096 eps f per cycle) is extrapolated.
  - A pass at what f64 resolves settles at a confirmed fixed point of the last passes, or stops
    with every good fitting to 1e-12 of its flows (the Leontief tolerance).
  - After 256 passes, the fills move to the last pass's fixed point, clamped to [0, f], or the
    stall is reported (`Scratch::chain_stalled`).
  - A call ends at 1,024 passes (`local::PASS_CAP`) on a counted fallback: the fills as they
    stand where every good fits to 1e-12 of its flows, else the goods that do not set to 0.
  - Markets balanced exactly at the limit keep their fills; before, the doubling took them to 0.

### BEHAVIOUR: refined fuels in good 1, four harmonic theta classes, theta scale (ADRs 0092, 0101, 0023)

- ICIO C19 (refined petroleum) and its HS lines move from good 5 into good 1, renamed "Crude oil,
  gas and refined fuels"; good 5 is renamed "Chemicals and metals". `read::icio::icio_good` now
  reads `goods::icio_industry`, so the pack has one industry map.
- Class theta is the trade-weighted harmonic mean over HS lines, on the Caliendo-Parro
  replication vector (Auto and Other Transport 8.22; owner ruling, ADR 0023 amendment). The rule
  draws four classes: 2.91 (food; chemicals and metals), 4.14 (machinery, electronics and
  vehicles), 7.68 (agriculture; light manufacturing) and 16.91 (crude oil, gas and refined fuels;
  coal and other mining). `TradeParams::default()`, `gravity::CLASSES` and `THETA_DEFAULT` follow;
  the P01 prior is 2.91 to 16.91.
- `TradeParams.theta_scale` (P150, default 1) multiplies every theta where the kernel is built. It
  enters the static and structural digests and `params.json` (export format 10), and the reference
  applies it to the exported theta.
- The transit invariant allows 1e-12 of the good's market turnover beside its 1e-9 relative
  tolerance: markets fill the transit account by differences of their holdings, and tiny shipments
  vanish against large stocks.
- `econ_goods.arrow` records each good's class with the class's harmonic and arithmetic theta.
  The installed 2000 pack is not rebuilt here: its econ tables and calibration still have refined
  petroleum in good 5 and the old classes until the pack is rebuilt and recalibrated.
- Side effects of the move (ADR 0092, As built): refined fuels take the primary tariff group
  (group 0, with good 1); good 5 stays storable (P151) without refined fuels; good 1's P02
  coverage stays 1 month; PIGL category M is unchanged for goods 1 and 5.

Goldens re-blessed:

- `tests/golden/p2-econ.txt`: every hash (the new thetas and P150 in the static digest). The
  hashes are the same at 1, 3 and all threads.
- `tests/golden/p2-fixture/`: every table (export format 10), `params.json` (thetas, classes,
  P150), the reference solution and the calibration, all at the new thetas.
- `tests/golden/p0-toy.txt`, `p1-geo.txt`, `p1-pack.txt` and `p2-pack.txt` do not move.

### BEHAVIOUR: i128 aggregate accounts, pi^e outside the clip, calibrated nominal level (ADR 0106)

- The snapshot codec moves to format version 2: the ledger writes each account's kind and its
  balance as `i64` (pops, enterprises, transit) or `i128` (markets, gateways, rebate and profit
  pools, the mint). A snapshot of format 1 is a `DecodeError` naming its version.
- The price and wage rules clip the gap terms only; expected inflation passes through. Storable
  commodities (goods 0, 1, 2, 5) clip at 0.3 (P151, `TradeParams::c_max_storable`), carried in
  the snapshot header beside P148, outside the static digest, and listed in `params.json`.
- Calibration rescales the traded r each pass so world traded value added equals the data's;
  the passes alone fixed the r only up to one common factor, which depended on the start. The
  fixture's calibrated nominal values (VA0, output, w0, money, pipeline, rebates) move by 1.9%
  and its PIGL scales by 1.4% (the factor to the power eps); a recalibration of the 2000 pack
  moves them by 2.56% (r from 0.98795 to 1.01322 for agriculture). The installed pack tables are
  unchanged and still load until the next recalibration of the pack.

Goldens re-blessed:

- `tests/golden/p0-toy.txt`, `tests/golden/p2-econ.txt`: every hash (format 2 and the account
  kinds in the canonical bytes; where the clip binds and pi^e != 0 the step is pi^e larger,
  elsewhere the sum rounds differently). The hashes are
  the same at 1, 3 and all (14) threads.
- `tests/golden/p2-fixture/params.json` (P151) and `tests/golden/p2-fixture/calibration` (the
  nominal level). The fixture reference is unchanged.
- `tests/golden/p1-geo.txt`, `p1-pack.txt` and `p2-pack.txt` do not move.
### BEHAVIOUR: provinces are whole first-level regions (ADR 0251)

- `data/packs/2000` is rebuilt without the 250,000 km² split: 4,594 provinces (from 4,796; the
  295 pieces of 93 regions are whole again), largest 3,053,172 km² (from 402,129). Counties
  (53,084, 357 unassigned), cells (78,483), sea zones (725), ports (1,001), airports (1,154) are
  unchanged; nodes 40,187 (40,534), edges 133,907 (134,810), minor ports 1,233 (1,283), coastal
  provinces 1,864 (1,934), county area outside its province 0.0274 (0.0302). Gateways stay 600
  (286 port, 171 airport, 143 land crossing; were 287, 168, 145); provinces with value added
  4,368 (4,555); `va_imputed_share` 0.0759.
- Recalibrated by `ceteris calibrate --pack` with ADRs 0256 and 0263 (41 passes, Sinkhorn
  residual 8.3e-14).
- Every province index moves, so every province-indexed table, the calibration tables and the
  least-cost trees move. `crates/ceteris-pack/tests/data/pixel_cells.txt` maps each old index to
  the whole province's new index; the build stats pinned in `tests/econ_build.rs` follow the
  new gateway kinds, value-added count and imputed share.
- P2 gate on the new pack: H2 distance -0.745 (s.e. 0.015, pseudo-R2 0.982, 47,306 pairs)
  passes; H2 per class passes (food 1.662, s.e. 0.176; machinery 3.502, 0.525; the five-good
  class 9.173, 0.323). 100 years: no violation, world price index 1.000000000000006. 1 vs 8
  threads, 120 ticks: identical hashes. V1 from the data state stays open (price 0.907, wage
  0.749).
- V1 from the perturbed steady state FAILS on flows: seed 20 price 9.27e-3, wage 4.10e-3, flow
  units 2.71e-2, value 2.53e-2 (Albania to Italy, coal and other mining); seed 21 8.34e-3,
  2.62e-3, 2.02e-2, 1.87e-2; seed 22 1.157e-2 (ALB-1524), 4.23e-3, 3.78e-2, 3.73e-2. Sudan
  (SDS-871 Unity, SDN-882 South Kordufan) now converges under every seed. The miss is main's:
  main's split pack, built and calibrated fresh, fails seed 20 in Albania by more (price
  1.047e-2, wage 1.191e-2, flow units 3.10e-2). Albania's pops hold 0.22 ticks of income and
  their money moves 10 to 16% after the perturbation.
- `p2_steady_state::pack_calibrated_state_holds_to_2e_6_and_4_minor_units` FAILS: pop money
  reaches 1.493 times its tolerance at tick 10 (0.729 after tick 60), on Pitcairn's tertiary pop
  (1,700 units, against the floor of 4 minor units); every other series stays within 0.6. Main
  passes it.

Goldens re-blessed:

- `tests/golden/p1-pack.txt`, `tests/golden/p2-pack.txt`: every province-indexed table
  (provinces, cells, counties, nodes, edges, gateways, `econ_provinces`, `econ_pops`), every
  calibration table and the web fills, attributes and tile layers. Against main, `sea_zones`,
  `countries`, `chokepoints`, `econ_goods`, `econ_countries`, `econ_io` and `econ_tariffs` are
  unchanged.
- `tests/golden/p1-geo.txt`: every source, since the network changes; `IND-2447.1` is now
  `IND-2447`.
- `tests/golden/p0-toy.txt` and `tests/golden/p2-econ.txt` do not read the pack and are
  unchanged.

### BEHAVIOUR: world technology for crude oil and gas set by rents (ADR 0256)

- Outside ICIO, crude oil and gas VA set from WDI rents now produces with the ICIO world input
  column (output-weighted) instead of the donor's, which could have next to no such sector
  (Sudan took Senegal's a_VA of 0.072). `econ_io` and the calibration move; `tests/golden/p2-pack.txt` is re-blessed from the owner-side build.

### BEHAVIOUR: partial WDI sector shares and oil and gas from rents (ADR 0249)

- Outside ICIO, a missing agriculture, industry or services share no longer drops its VA: the
  goods it covers take GDP times the ICIO median VA over GDP less the known shares. Kuwait goes
  from 0.14 to about 35 bn USD (GDP 37.7 bn); BMU, ERI, KEN, LBY, MAC, MDG, PNG and TUV rise to
  the median too.
- Where WDI has oil or gas rents (two new WDI series), crude oil and gas VA is GDP times the
  rents times the ICIO ratio of crude oil and gas VA to rents, out of its group and at most the
  largest ICIO share of oil and gas in industry, instead of the donor's split (LBY, DZA, VEN, KWT had near-zero oil).
- `econ_countries`, `econ_provinces` and the six calibration tables move (40 passes);
  `econ_io` and the other econ tables are unchanged. The manifest's `va_imputed_share` goes from
  0.0743 to 0.0759.
- The P2 gate passes. Moved: enterprises 39883 to 38912; Suez-dependent share of trade in units
  0.0424 to 0.0433 (with the closure 0.0320 to 0.0321), in value with the closure 0.0305 to
  0.0302; world price index with the closure 1.00017 to 1.00004. The V1 fixture deviations and
  the 100-year run's violation counts (all zero) are unchanged.

Goldens re-blessed:

- `tests/golden/p2-pack.txt`: `econ_countries`, `econ_provinces` and the six calibration
  tables.
- `tests/golden/p2-econ.txt` and the other goldens do not read these tables and are unchanged.

### BEHAVIOUR: the 2000 pack and its calibration are built in Rust (ADR 0113, step 4f)

- `data/packs/2000` is regenerated by `ceteris pack build` (GEOS 3.15, h3o centres) and
  recalibrated by `ceteris calibrate --pack`; 25 files are replaced (every table, the manifest,
  both fill files, the tiles archive), `web/provinces.json` and `web/counties.json` are equal
  byte for byte. Schemas, metadata and pack format 3 are unchanged.
- The bytes move because arrow-rs writes other bytes than pyarrow. The values move only where
  ADR 0113 step 3 expects: GEOS 3.15 moves `counties.area_km2` (7.8e-11) and `edges.length_km`
  (1.1e-12), as the Python build on GEOS 3.15 shows against the old pack; h3o's cell centres move
  cell areas (3.5e-11), terrain means (1.1e-10), split provinces' and their counties' areas
  (1.2e-8, 4.8e-8) and sea node positions (2.9e-12). The econ tables are equal (0).
- The kernels move by at most 1.4e-12 (from `edges.length_km`), so the export's static digest
  moves; the calibration tables agree with the old ones within 7.0e-11 (46 passes).
- The P2 gate numbers are unchanged at their printed precision (econ SPEC, P2 gate).

Goldens re-blessed:

- `tests/golden/p1-pack.txt`, `tests/golden/p2-pack.txt`: every digest but the two attribute
  files.
- `tests/golden/p1-geo.txt`: every source, because the edge lengths move in the last bits.
- `tests/golden/p0-toy.txt` and `tests/golden/p2-econ.txt` do not read the pack and are
  unchanged.

### BEHAVIOUR: loss write-off beyond three months of base sales (ADR 0112, Task 20b)

- After E4's distribution, an enterprise's retained loss beyond M = 3 months of its base sales
  (P148; VA0 / a_VA, the calibrated steady-state sales value) is written off against its
  country's pops in proportion to their positive money: no pop goes below zero, a shortfall is
  shared over the enterprises and carried, and both sides are exact i64 largest-remainder splits
  moved through the country's pool account (flow `WRITE_OFF`, not income).
- Province markets keep the signed rule: limited liability with a write-off still inflates the
  fixture's run from the data state (world price index 13 at tick 1,200).
- `EconParams::write_off_months` is new and outside the static digest; the snapshot header carries
  it after the digest (checked by `restore`) and `params.json` lists it with P148, so every state
  hash and the fixture golden move. An infinite value turns the write-off off. The homogeneity
  property test runs with it off, since the cap is nominal.
- Steady states and the pack's default run are unchanged apart from the header: no balance is
  negative there.

### BEHAVIOUR: enterprises have limited liability (owner decision 2026-10-26, Task 20)

- At E4 an enterprise pays its balance into its country's profit pool only when the balance is
  positive. A loss stays on the enterprise's own account as retained losses (a negative balance
  at the clearing facility, no interest), and later cash flow repays it before anything is paid
  out. Enterprises never pay a negative dividend; before, losses were pooled and could reach pops
  as negative dividends.
- `EconWorld::profit` of an enterprise is now its balance before the payout: this tick's cash
  flow less any loss it carries.
- Province markets still pay their signed cash flow. Applying the same rule to them, as ruled,
  makes the fixture's run from the data state inflate without bound (merchants' stock purchases
  stay as retained losses while pops keep the money: world price index 15 at tick 1,200 and 129
  at 2,400). It is held for the owner (Task 20 fix round 1); ADR 0112 records the outcome.
- The calibrated steady state is unchanged (every enterprise earns (1 - s_L) VA0 > 0); the
  fixture golden `tests/golden/p2-econ.txt` moves because enterprises run losses early in the
  run from the data state.

### BEHAVIOUR: runs start from the ADR 0089 calibrated steady state (Task 18e)

- The data state is the steady state. A world whose inputs carry the calibration tables of
  `python -m ceteris.reference calibrate` (`EconInputs::calibrate`; `EconInputs::from_pack` when the
  pack holds them) reads A, VA0, L0, a_VA, the coefficients (both S09 settings), w0 and the PIGL
  weights from them and starts from their steady state: posted prices, the last D, Q and sales,
  merchant and input stocks, the pipeline and pop money. The loader checks `calibration_format` 3
  and the static digest of the uncalibrated inputs. `ceteris export --pack` writes the
  uncalibrated world (`from_pack_data`); `ceteris run --pack` starts calibrated.
- Natural-rate wage wedge: dlnw = clip(pi^e + kappa_w (x + u*), -c_max, c_max), the frictionless
  wage clears (1 - u*) L^s, and the base point and initial employment use u*
  (`econ_countries.u_star`; a generated world takes its 2000 rate). The GE reference clears labour
  at (1 - u*) L^s too.
- National profit ownership (owner, 2026-10-25): at E4 enterprises and province markets pay
  their cash flow into one profit pool per country, paid to its pops by population share. The
  rounding is cumulative shares, half to even, so the split is exact in minor units.
- Export format 8: `countries.u_star`.
- 2000 pack: the calibration tables are installed into `data/packs/2000`, and their sha256 joins
  `tests/golden/p2-pack.txt`.

Goldens re-blessed:

- `tests/golden/p2-econ.txt`: every hash, because of the profit pool accounts, national dividends
  and the wedge.
- `tests/golden/p2-fixture`: format 8, and the reference re-solved at the natural rate under
  national ownership.
- `tests/golden/p2-fixture/calibration`: new.
- `tests/golden/p2-pack.txt`: the six calibration tables' sha256.

Fixed point over 120 ticks with every friction on:

- Fixture: largest relative drift 5.7e-11 (pop money), inside 1e-9.
- Full pack: limited by integer money (1e-4 USD) and carried through each country by national
  dividends and domestic trade (ADR 0089 As built). The test holds:
  - real quantities to 2e-6 relative;
  - money to 2e-6 relative or 4 minor units;
  - world aggregates to 1e-9;
  - bounded drift.
- Full-pack maxima: price 1.9e-7, employment 5.1e-7, pop income 155 minor units (5.2e-9
  relative), world price index 5.4e-15. V1 maxima on the fixture: price 2.3e-6, wage 1.0e-6,
  flow units 5.3e-6, flow value 5.4e-6. Decay per tick is 0.99505 frictionless and 0.99564 in the
  default mode.

### BEHAVIOUR: trade elasticities follow the OECD key (ADR 0023 amendment)

`TradeParams::default()` moves from the classes of the WITS and UNSD route to those the ADR 0023 rule
draws from the OECD BTiGE key with the gold and books adaptations (owner decision, 2026-10-25):

| Good | theta before | theta now |
|---|---|---|
| 0 Agriculture, 1 Crude oil and gas, 2 Coal and other mining | 12.55 | 10.57 |
| 3 Food | 2.62 | 2.62 |
| 4 Light manufacturing | 5.88 | 10.57 |
| 5 Chemicals, refined fuels, metals | 12.55 | 10.57 |
| 6 Machinery, electronics, vehicles | 5.88 | 5.19 |

The class map is [2, 2, 2, 0, 2, 2, 1] (was [2, 2, 2, 0, 1, 2, 1]); the P01 prior is 2.62 to 10.57.
`python/tests/test_econ_pack.py::test_rust_theta_defaults_follow_the_estimator` ties the fixture export
of the defaults and `ceteris.calibrate.gravity.CLASSES` and `THETA_DEFAULT` to the estimator.

Effects:

- Re-blessed `tests/golden/p2-econ.txt` (state hashes of the 1200-tick fixture run) and the fixture
  export `tests/golden/p2-fixture` (`goods`, `params.json`, the three kernels and `provinces`, whose
  base prices and kernels depend on theta) with its GE reference, because every traded good except
  food moved.
- Stability region of the price rule (`specs/trade/SPEC.md`): kappa1 limits rise to 0.141 to 0.169 at
  theta 10.57, 0.250 to 0.307 at 5.19 (food unchanged). The default (kappa1 0.05) sits at 0.10 to 0.30
  of its limit (was 0.10 to 0.40). `price_rule_stability.rs` models the small origin at E = 11.57; three
  of its twenty probe points moved to keep the 10% margin from the boundary.
- V1 maxima: price 8.1e-6, wage 6.5e-6, flow units 1.5e-5, flow value 1.9e-5 (tolerance 1e-2). A small
  deviation decays by 0.99597 per tick frictionless and 0.99636 in the default mode. The uniqueness
  bound is 0.922 (was 0.932).

### BEHAVIOUR: P2 economic core migrated onto ADRs 0015 to 0023

Model changes (each with its ADR; the econ and trade specs carry the equations):

- 0015 Trade flows in value with a dimensionless gravity kernel: destinations spread expenditure
  over origins by Eaton-Kortum shares on K p_o^-theta, buyers pay the origin price (with tariff)
  at dispatch. Tariffs sit inside destination spending.
- 0016 Origin markets clear across ticks: orders are filled against output and merchant
  inventory; unfilled spending stays with the buyer, with no backlog. `route.rs`, `transit.rs` and
  `Kernel::access` are gone.
- 0017 Route-choice trade costs: deferred past P2 (owner, 2026-10-24). S05 least-cost legs stay
  the default and `Network.route_taus` stays `None`; the module stays in the tree, tested and off.
- 0018 and 0023 Sector trade elasticities: P01 is a theta per traded good, in classes food 2.62,
  goods 4 and 6 5.88, goods 0, 1, 2, 5 12.55 (Caliendo and Parro 2015, 99% sample column). ADR
  0023 (the classes and their values) is an agent default under 22.2, not an owner decision.
- 0019 Expected inflation in the price and wage rules (P45 gain 0.006).
- 0020 Putty-clay coefficients and a same-tick chain for local goods; rationing modes include
  S11 Mixed (Pichler and Farmer 2022, Algorithm 2).
- 0021 PIGL household demand over three categories (goods 0 and 3; 1, 2, 4, 5, 6; 7, 8, 9) with
  lognormal pop income (PIGL parameters P46, P47); the consumption rule (P37) is spend-all
  in P2 (m = 1, lambda = 0).
- 0022 Uniqueness check (AAL 2024 Theorem 1) before V1, static nesting gate, direct estimation of
  theta, border friction and mode costs (estimation lands with PLAN Task 18).

Owner decisions (2026-10-24):

- Order-up-to rule on inventory position (stock + on order) with target (1 + cover + L) months
  of use.
- Buyers use the delivered unit price q_d = 1/alpha_d in household demand, input orders and p_VA.
- Route choice deferred past P2 (ADR 0017).
- kappa1 0.05 (P03a) and gain 0.006 (P45); kappa2 0 unchanged.
- S11 Mixed rationing mode added; it fills as Priority until P4 adds essential consumption levels.

Goldens re-blessed (`CETERIS_BLESS=1`): `tests/golden/p2-econ.txt` (every state hash changes:
new state layout, sourcing, markets and pipelines) and `tests/golden/p2-fixture/` (export format
6, per-good theta, delivered-price inputs, PIGL parameters, new `pigl_check.arrow`, and the
reference solution of the new model). The three tests held back since Task 1
(`p2_fixture_matches_golden_hashes`, `fixture_export_is_current`,
`test_fixture_reference_is_current`) run again.

V1 maxima from the blessed run (fixture, frictionless vs reference): price 3.14e-5, wage 2.54e-5,
flow units 5.80e-5, flow value 7.57e-5 (tolerance 1e-2). A small deviation from the equilibrium
decays by 0.99644 per tick frictionless and 0.99676 in the default mode.

Stability of the price rule (specs/trade/SPEC.md, Stability region): first kappa2 that fails, with
the kappa1 limit as kappa2 goes to 0, default mode / frictionless. The default (kappa1 0.05,
kappa2 0) lies on the kappa2 = 0 edge, which counts as inside.

| Good | theta_g | s_g | kappa1 limit | kappa2 limit at kappa1 0.05 (default) | at kappa1 0.02 |
|---|---|---|---|---|---|
| 0 Agriculture | 12.55 | 3 | 0.144 / 0.130 | 0.083 / 0.077 | 0.109 / 0.105 |
| 1 Crude oil and gas | 12.55 | 1 | 0.145 / 0.132 | 0.049 / 0.059 | 0.016 / 0.080 |
| 2 Coal and other mining | 12.55 | 2 | 0.144 / 0.130 | 0.077 / 0.071 | 0.040 / 0.097 |
| 3 Food | 2.62 | 2 | 0.507 / 0.368 | 0.392 / 0.347 | 0.040 / 0.380 |
| 4 Light manufacturing | 5.88 | 2 | 0.278 / 0.230 | 0.189 / 0.173 | 0.040 / 0.202 |
| 5 Chemicals, refined fuels, metals | 12.55 | 2 | 0.143 / 0.124 | 0.077 / 0.068 | 0.031 / 0.095 |
| 6 Machinery, electronics, vehicles | 5.88 | 2 | 0.278 / 0.230 | 0.189 / 0.173 | 0.040 / 0.202 |
