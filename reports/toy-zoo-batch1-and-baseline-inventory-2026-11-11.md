# Toy zoo batch 1 status (W2) and literature baseline inventory (W4), 11 November 2026

Follows `reports/toy-models-and-literature-baseline-2026-11-11.md` (sections 2, 3.1, 4). Status as of
this PR; the inventory is a code scan (field names and ADR references), not a run.

## 1. W2: batch 1 status

Simulator tier needs a steady state to compare against. `ToyWorld` carries none (`steady: None`);
a 2-country 2-good toy started from its data state settles within about 1% of prices 1 and
drifts slowly after 120 ticks, so a before/after comparison on it is good to a few percent at
best. Comparative-statics toys (Z2, Z3, Z5, Z6, Z7, Z19, Z20) therefore need a toy steady-state
calibration (stages 1 to 5 on `ToyWorld`) first. That is the larger half of W2 and is not in this PR.

| Toy | Status | Note |
|---|---|---|
| Z35 Lotka | done, passes | `crates/ceteris-econ/tests/zoo_pop.rs`; growth within 0.003/yr, band shares within 0.01 |
| Z36 momentum | done, finding | simulator momentum 1.158 at 150 y against 1.238 analytic (6.4% low, pass number 2%); test ignored until explained. The cohort model's growth at replacement fertility is -0.0003/yr (rounding, mixed residence), so a 600-year read decays to 1.012. Horizon change was made after seeing results and is recorded in the test |
| Z2, Z3, Z5, Z6, Z7, Z19, Z20 | waits on toy steady state | as above |
| Z10 Lerner | not expressible | `Wedges` has import tariffs and NTBs only, no export tax |
| Z17 quantity theory | not expressible | no edit target that scales the money stock |
| Z13 Hume, Z11 cobweb, Z14 beer game | open | Z13 needs money held fixed across two countries (E3/B1 spike); Z11 overlaps `p2_price_rule_sweep`; Z14 needs the self-input chain (T3) |

Sea legs and storables are not added: no listed toy needs them yet.

## 2. W4: reconciliation inventory

Classes: **exists** (a parameter or switch reverts it on the default code path), **addable** (code
exists for the literature form or it is one branch, default path untouched), **build-time** (set
when the pack or calibration is built, not at run), **not revertible**, **unchecked** (no ADR
reference found in code; needs a read of the spec).

| ADR | Reconciliation | Class | Evidence / note |
|---|---|---|---|
| 0274 | per-class thetas | exists | `TradeParams.theta`, `theta_class`, `theta_scale`; all-4 is a data change |
| 0092 | fitted border nu | exists | `TradeParams.border_nu`, edit `trade.border_nu.<g>` (0 reverts); calibration reads it |
| 0272 | log-distance friction | unchecked | no ADR reference in crates |
| 0200 | EK-index buyer price | unchecked | used in `world.rs`, `inputs.rs`; no switch seen |
| 0247 | bulk rate | exists (probable) | `bulk_rate()` is an Option on `bulk_sea`, `bulk_port`; confirm zero classes turns it off |
| 0263, 0267 | per-market and block caps | addable | only `c_max`, `c_max_storable` are parameters; caps are in code |
| 0338 | money anchor | exists | `TradeParams.anchor: Option<PriceAnchor>`, None = aggregate rule bit for bit |
| 0340 | bounded gap | unchecked | `market.rs`, `p5_price_anchor.rs`; no switch seen |
| 0343, 0344 | chained index, pay on sales, credit line | exists | one switch, `TradeParams.e3` (off by default) |
| 0106 | widths | unchecked | `c_max` is a value, not a switch |
| 0256, 0270, 0281 | oil technology, extraction column, rents to VA | build-time | pack ICIO build (`ceteris-pack` icio); fixture and toys unaffected |
| 0266 | input-capped employment | addable | in `world.rs`/`systems.rs`, no switch |
| 0094 | country weight inversion | not revertible | needed to calibrate at all (report 3.1) |
| 0112 | loss write-off | exists | `EconParams.write_off_months = INFINITY` |
| 0100, 0308 | transfers and RAS | exists / build-time | `Transfers::Off`; RAS in `ceteris-calibrate` balance, build-time. The r_g rescale was removed, not revertible without old code |
| 0280 | storage partial adjustment | exists (probable) | `storage_gain`; confirm 1 gives the ADR 0091 form |
| 0109 | admission caps | addable | `admission.rs`, no switch seen |

Counts: 6 exists plus 2 probable, 3 build-time, 4 addable, 4 unchecked, 2 not revertible
(0094, the r_g rescale). Not covered: the other of the 41 audited alterations of 30 October
(this table follows the 3.1 list only), and ADRs after 0344.

Next for W5: read the four unchecked specs; confirm the two probables with a one-line test each;
decide with Antonio (open question 1 of the baseline report) whether the four addable switches
need an RFC or an issue (research-only, off by default, citing the ADR: an issue, per his
11-11 ruling).
