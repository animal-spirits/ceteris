# 0091. Competitive storage in P2 with per-market price expectations

Status: proposed (owner chose the options, 2026-10-25)
Date: 2026-10-25
Design sections: 8.2, 12.2, 17 (P02, P56, S04); trade SPEC Origin market; ADRs 0016, 0019, 0029
Amended by ADR 0278 (proposed, 2026-10-31): the storage order is a partial adjustment toward a target stock, b = delta S + lambda (S* - S).

## Context

ADR 0029 (design branch, proposed) adds competitive storage for grains, other storable crops,
crude oil and metals, with storage cost and convenience yield in P56. P2's origin markets
already hold merchant stock targeted at S* = s_g D, so taking 0029 later would leave two
inventory models for the same goods. The owner chose to bring competitive storage into P2
(2026-10-25). Storage needs an expected price per good; P2 has only the world pi^e (ADR 0019).

## Options

Where storage applies: (1) the 0029 goods only, with ADR 0090's kappa2 rule for the rest;
(2) every traded good. Which expectation it reads: (a) a per-market adaptive expectation;
(b) a model-consistent price within the tick; (c) world pi^e plus mean reversion.

## Decision

Options (1) and (a) (owner, 2026-10-25).

**Goods.** Competitive storage replaces the stock rule for agriculture (0), crude oil, gas and
refined fuels (1, with C19 per ADR 0092), coal and other mining (2), and chemicals, plastics,
minerals and metals (5). Food (3), light manufacturing (4) and machinery (6) keep ADR 0090's
kappa2 rule. Good 5 is the closest P2 has to "metals"; the list is revisited when goods split
in P8.

**Expectation.** Each origin market (o, g) keeps an expected log-price change

```
pi^e_og <- pi^e_og + g_s ( dlnp_og - pi^e_og )        (P146, prior as P45)
```

updated after T4. S04 stays adaptive-only.

**Storage rule.** Merchants at the origin market buy into or sell out of stock at the posted
price. The expected net return of holding one more unit for a month is

```
x_og = pi^e_og + y_g(S / (s_g D)) - k_g - delta_g
y_g(z) = (k_g + delta_g) z^(-zeta_g)
```

with k_g the money cost and delta_g the physical loss per month (P56), and y_g the convenience
yield (Working-Kaldor), falling in cover z. Storage demand added to orders at T3:

```
dS_og = delta_g S_og + psi_g D_og x_og,   floored so that S' >= 0
```

psi_g (P144) is the speculation intensity, zeta_g (P145) the convenience-yield elasticity.
At a steady state pi^e = 0, so x = 0 at z = 1: stock sits at P02 cover, and P02 keeps its
meaning. A rising price expectation pulls stock in, and a stockout (S = 0) lets the price spike,
the asymmetry of Deaton and Laroque (1992). The kappa2 term of ADR 0090 is off for these goods;
the relative and aggregate terms apply unchanged.

## Tests

- Steady state: at pi^e = 0 and z = 1, storage orders equal spoilage delta_g S; ADR 0089's fixed-point test holds with storage.
- Linearized stability: the storage loop adds one state per market; re-derive the region with
  psi and zeta and check it on the toy market.
- Stockout asymmetry: a 30% harvest cut in agriculture raises the price more than a 30% surplus
  lowers it.
- Smoothing (amended 2026-10-31, owner): a 3-month 30% cut raises the price by less than half the
  flow-clearing price without stock, Deaton and Laroque's benchmark; and a 12-month cut draws the
  stock below half its target while it stays above zero. This replaces "stocks hit zero only on
  the cut side": the Working-Kaldor yield z^-zeta grows without bound as cover falls, so
  merchants keep a pipeline stock (Working 1949), and the stock rule with storage off is not the
  benchmark, since at kappa2 = 0 its price does not read the stock. A mean-reverting expectation
  (option c) is left to P9, where psi, zeta and g_s are fitted.
- Conservation: storage purchases and releases pass through the ledger; spoilage is tallied.

## Consequences

- One inventory model per good; ADR 0029's storage part is implemented in P2, its export
  restrictions and famine parts stay in their phases.
- Three new register entries (P144 to P146); P56 is used from P2.
- psi, zeta and g_s are not identified in P2 (no shocks); P9 fits them to commodity price and
  stock dynamics (FAO, EIA, LME).
- ADR 0028's OPEC rule reads the same stocks for crude.

## As built (pre-P3 B7, 2026-10-31)

- **Off by default** (owner ruling 2026-10-31, after the stop below): P144 psi and P56 delta
  default to 0, so `TradeParams::stores` is false and goods 0, 1, 2, 5 run the stock rule as
  before this ADR, bit for bit in prices and quantities (the fixture's calibration and GE
  reference are unchanged to the last bit; the snapshot and the export carry the new state and
  parameters). The mechanism is complete and tested at the approved values
  (`TradeParams::with_storage`); turning it on is setting those parameters and recalibrating.
  The ADR stays proposed until a value set passes the stability checks.
- **The stop.** At the approved values the fixture world is unstable: from the GE reference
  perturbed by 1% the default mode grows by 1.0069 a tick (agriculture of the island province,
  whose income feedback leaves an effective E of about 0.3) and frictionless V1 fails (1.5%,
  coal); with g_s = 0 both decay (0.992), with psi = 0 the spoilage term alone grows (1.0004).
  Candidate fixes for the owner: a smaller kappa1 psi for agriculture, a smaller g_s, or a
  mean-reverting expectation (option c above), each needing approval.

- **Parameters** (owner approval 2026-10-30T22:51Z; DESIGN 17 rows P56, P144 to P146): per month,
  delta 0.003, 0.0005, 0.001, 0 and k 0.009, 0.011, 0.007, 0.0067 (interest 0.0047 included) for
  goods 0, 1, 2, 5; zeta 1.8, 13, 3.5, 3.5; psi 20, 1.0, 3.571, 5.714 (derived from a
  stock-persistence target, not estimated; good 5's 5.714 is the source sheet's 40 at zeta 0.5
  rescaled to zeta 3.5, 40 x 0.5 / 3.5, where the formula psi = lambda s / (zeta (k + delta))
  gives 5.54: approved as rescaled); g_s 0.08 at first, 0.006 since the B7 follow-up (below);
  s_g unchanged (3, 1, 2, 2). The weak
  sources are an open question for P9 (trade SPEC). `TradeParams::storage_loss`, `storage_cost`,
  `speculation`, `convenience`, `storage_gain`; storage applies to `goods::STORABLE`.
- **The order** (`Markets::store`, T3 before clearing): the stock S carried in loses delta S
  (destroyed in the ledger, tallied); b = max(delta S + psi D x, -(1 - delta) S) with D this tick's
  buyer orders and x = pi^e + (k + delta)(z^-zeta - 1), the convenience yield written relative to
  its value at z = 1 so that x is 0 bit for bit at the steady state. D = 0 orders delta S; S in z
  is floored at Q_floor, so a stockout orders a finite, very large b that the P151 clip bounds.
  The order enters the price step as orders (dlnp = pi^e + clip(kappa1_cell (D + b - Q) / Q)) and
  the kappa2 term is off for these goods. The stock stays what the market does not sell (buyers
  first, S11 unchanged), so the order is filled through the price: at a steady price D + b = Q and
  S' - S = psi D x. pi^e_og (`Markets::expectation`) moves by g_s toward the cell's own step after
  T4. Both vectors are in the snapshot.
- **Steady state** (with storage on). Output covers sales and spoilage, Q = (1 + delta s) D, and the market's cash
  flow -p delta S goes to the country's profit pool (merchant rule, ADR 0112). The calibration
  targets sales value V / (1 + delta s) in stage 2, charges the spoilage to profits in stage 1 and
  holds stocks at s D in stage 5; the GE reference orders D (1 + delta s) at stored origins and
  charges p Q delta s / (1 + delta s) to profits. delta joins the structural digest. Export format
  14 (`goods.storage_loss`, the storage parameters in `params.json`), calibration format 8 (9 after the merge
  into pre-p3/econ-review with ADR 0267's limit gains): every calibrated pack is recalibrated (B8). With storage on the fixture's calibration moves by up to
  0.8% (output, stocks, VA), and its stage 5 state is stationary under the stock and storage
  rules (`stage5_state_is_stationary_under_the_stock_rules` calibrates the fixture with storage
  on beside the default worlds). No test runs `EconWorld` from a storage-on calibration:
  `fixture_calibrated_state_holds_to_1e_9` runs the default calibration, storage off (the first
  As built said otherwise; B7 review I4), and a storage-on calibration does not load into
  `EconInputs`, whose `trade_params` carry no storage parameters (gate below).
- **J includes the storage response** (ADR 0263 interaction): b moves with every change in the
  buyers' orders through z, so the static elasticity of the market's orders is E (1 + psi zeta (k
  + delta)) / (1 + delta s): 1.42, 1.15, 1.10 and 1.13 times E for goods 0, 1, 2, 5. The per-market
  J uses it at the run's parameters (`MarketElasticity::j`), and the block map of ADR 0267 scales
  a stored good's order terms by the same factor (`block::blocks`); with storage off the factor
  is 1. With storage on, on the calibrated fixture no
  cell is capped per market (largest J 14.3 default, 15.6 frictionless, against 20). The first
  build's block map (traded cells only) scaled 3 of its 4 blocks in both modes (largest radius
  at gains / m 1.075 and 1.208). The B7 review round (I1, I2) corrected the merged map: a
  stored cell's revenue follows its buyers' orders, o / r, not the storage order (the merge
  read R (x + q + e), overstating it by r), and its stock and expectation are states of the map
  (trade SPEC, Block gain). On the fixture calibrated with storage on at g_s 0.006
  (`block_limits_with_storage_on`) the default mode caps no block (largest radius at gains / m
  0.994) and frictionless caps 1 of 4 (57 cells, 1.0077); without the two states the map reads
  1.022 and 1.152.
- **Stability** (trade SPEC, Storage loop): the 4 x 4 map of (x, q, sigma, pi^e_og), checked by
  Schur-Cohn and against the toy market (`stability_with_storage_matches_simulation`). At the
  first g_s 0.08 (superseded by the g_s 0.006 bullet below): over the
  fixture's eps the default kappa1 0.05 is inside for every good; agriculture's limit in the
  default mode is 0.054. Over the pack's eps (down to 0.09) agriculture's limit is 0.0425 in both
  modes: an agriculture market facing total demand (E near 0.6) with an inelastic supply has a
  slow growing root (1.003), the momentum loop kappa1 psi = 1.0 beating the stock's pull. The
  per-market cap does not reach it (it bounds J2). Recorded for B8; no value changed.
- **Stockout asymmetry** (at g_s 0.08; the g_s 0.006 values are in the last bullets and in
  Tests): a 30% harvest cut held 12 months raises agriculture's price by 0.73 in
  logs, a 30% surplus lowers it by 0.35 (`stockout_raises_price_more_than_surplus_lowers_it`).
  The second claim does not hold at these values: the stock does not reach zero on the cut side
  (lowest 0.89 of 3 months), since the convenience yield and the momentum make merchants hold back
  as the stock falls, and the price overshoots after the shock (stock to 3.4 times its target,
  price -0.31 by tick 60). Without storage the same cut raises the price by 0.21 and a 24-month
  cut empties the stock. `stock_hits_zero_only_on_the_cut_side` was kept and ignored as a finding
  for the owner (replaced by the amended Tests, 2026-10-31).
- **g_s 0.006** (owner approval 2026-10-31, B7 follow-up): P146 moves from 0.08 to P45's start, the
  constant learning gain of Milani (2007), 0.0183 a quarter. det(I - M) of the storage loop is
  c eta g_s kappa psi (E + eps) / s > 0, so it leaves the region through a slow complex pair that the
  momentum drives; g_s scales that loop and leaves the stock's pull alone. Agriculture's psi limit
  (E down to 0.3, the pack's eps, per-market cap applied) rises from 14.7 to 43.5, and the approved
  psi of goods 0, 1, 2, 5 sit at 46%, 48%, 16% and 22% of theirs. The fixture's perturbed run
  decays (0.991 frictionless, 0.992 default). Storage stays off by default: V1 from the data state
  fails at 1,200 ticks at every value set tried (slow convergence, about 0.9986 a tick; prices
  within 0.06% at 3,600; superseded by the V1 horizon bullet below). The stockout claim is restated in Tests (the stock stops at 0.23 of
  target at the 12-month cut; a 3-month cut moves the price by 0.10 against 0.59 without stock).
- **V1 horizon** (owner, 2026-10-31): from the data state the fixture with storage on converges
  slowly and not monotonically (flows 1.5% off at 3,600 ticks, 2.1% at 4,800, 1.4e-4 at 6,000),
  so V1 runs 6,000 ticks where storage is on (econ SPEC, P2 gate) instead of raising psi for
  goods 1, 2 and 5 to pass at 1,200. With g_s 0.006 the approved values pass the stability checks
  and V1; turning storage on by default is the owner's call.
  On the 2000 pack (B8 tables, storage-on calibration of a copy) the first rule failed (stv1: five
  of eight runs panicked on ledger overflow by tick 82, the default perturbed runs ended 2.2% to
  3.3% off in flows). Under ADR 0278's partial adjustment (B8f) every run reaches 6,000 ticks;
  the default mode passes from seeds 20 to 22 and the data state (largest gap 9.7e-5, 5.9e-5,
  4.4e-5 and 3.2e-3 at 1,200, at most 2.3e-7 from 3,600 on, with or without Pitcairn), and
  frictionless passes seeds 21 and 22 (3.6e-5, 2.4e-3 at 1,200) but misses seed 20 (a good-1
  cycle in Switzerland, 0.48, 0.39, 0.48, 0.25 at 1,200, 3,600, 4,800, 6,000) and the data state
  (France's province 1094, 2.79 at 1,200 and 6.99 at 6,000; the storage-off frictionless data
  state also fails, B8e). The V1 gate item stays unmet in the frictionless mode (ADR 0278, As built).
- **psi limit** (`stability::psi_limit`): reproduces agriculture's 43.5 at g_s 0.006. It binds at
  a corner no single fixture market has (E 0.3 with eps 30.4); at E 0.2 it is 42.5, and without
  the per-market gain cap (ADR 0263) it is 23.9, so the cap is what makes psi 20 safe.
- **Storage parameters are structural** (B7 review round, I2; ADR 0267): the calibration builds
  the block map at the export's storage parameters (`goods.storage_loss`, and the P56 money
  cost, P144, P145 and P146 of `params.json`), and `structural_digest` carries all five, so a
  run under storage parameters other than its calibration's is refused at a calibrated start
  and at a data-state start of calibrated inputs: storage-off limits never cap a storage-on run.
  Recomputing at the run, or a second storage-on column, were the alternatives; refusing keeps
  one map per calibration, and turning storage on is recalibrating anyway. A storage loss
  without speculation (delta > 0, psi = 0), an untargeted stock that spoils (root 1.0004 above),
  is refused (`TradeParams::check_storage`, in `EconWorld::new` and the calibration).
- **Gate for switching storage on** (B7 review I5; none of these is met yet): an ablation test
  (DESIGN 15.7) with a falsification target (price skew, stockout spikes, stock persistence);
  the P9 fit against the 2011 to 2018 holdout (ADR 0107); 100-year invariants with storage on;
  V1 from the data state at 6,000 ticks (V1 horizon above); 1 against 8 threads with storage on;
  a storage-on calibration that loads
  (`EconInputs::trade_params` carrying the calibration's storage parameters) and a
  calibrated-state test from it; and the review's I1 and I2, fixed above.
