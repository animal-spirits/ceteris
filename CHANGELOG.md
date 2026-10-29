# Changelog

## Unreleased

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
