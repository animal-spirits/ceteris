# Literature baseline on the fixture (W5, first cut), 11 November 2026

Follows `reports/toy-models-and-literature-baseline-2026-11-11.md` section 3 and the W4 inventory.
Harness: `crates/ceteris-econ/tests/literature_baseline.rs` (ignored matrix, research only; no
default path or golden changes). All numbers below are one run each, 1,200 ticks, fixture from the
data state (the reference steady state is hashed to main's parameters, so it cannot be settled
under a reverted switch). `index_drift` is the log change of the world price index; ~13 means the
index hit its bound.

## Switches built (each cites the ADR it reverts)

Theta 4 for every traded good (0274), border nu 0 (0092), no write-off (0112), transfers off
(0100), no anchor (0338). Not switches on the fixture: bulk rate 0247 (the network carries the bulk
legs: build-time), 0343/0344 (anchor `e3`, off by default). Confirmed by test: rho = 1 is the
ADR 0091 form (0280), `bulk_off` zeroes the bulk rate.

## Results

| config | default none | default x10 | frictionless none | frictionless x10 |
|---|---|---|---|---|
| main | ok, +0.016 | fails 1130 | ok, +0.016 | fails 601 |
| literature (all five) | ok, -0.018 | survives, 11.1 | ok, -0.019 | fails 723 |
| main + Theta | ok, +0.012 | fails 1083 | ok, +0.012 | fails 606 |
| main + BorderNu | ok, -0.011 | survives, 10.7 | ok, -0.011 | fails 702 |
| main + WriteOff | ok, +0.016 | fails 1005 | ok, +0.016 | fails 601 |
| main + Transfers | identical to main | | | |
| main + NoAnchor | identical to main | | | |
| literature minus Theta | as BorderNu alone | | | |
| literature minus BorderNu | as Theta + WriteOff | fails 1107 | | fails 606 |
| literature minus WriteOff, Transfers, NoAnchor | identical to literature | | | |

## Reading

- Transfers and NoAnchor change nothing: the fixture has no transfer shares and the anchor is
  already off by default. They matter only on the pack (W6).
- No-shock runs survive in every configuration; the literature form drifts the index about 2%
  down instead of 1.5% up. Nothing here fails through patterns 1 or 2 in the no-shock case, so
  the prediction that the fixture literature baseline dies within a few hundred ticks
  (report 3.3) is **not borne out without a shock**.
- Under the x10 border shock every frictionless configuration fails (ledger overflow, tick 601 to
  723); in default mode main fails at 1130, literature survives. Fitted border nu is the one
  switch that changes the outcome (reverting it removes the failure in default mode; frictionless
  it delays it from 601 to 702). Theta and write-off move the failure tick by 10% or less.
- Single run per cell on one fixture; failure tick differences of tens of ticks are not
  distinguishable from path noise without perturbation draws.

## Not done (needs a decision or the Mac)

- Both calibrations (joint as built, independent per source): the calibration machinery runs on
  the pack, not the fixture.
- The 10% perturbation start and the free moments (distance elasticity, Brexit, ACR-IO,
  openness): need the pack.
- Switches not added: 0272, 0200, 0340, 0106, 0263/0267 caps, 0266, 0109 (four unchecked specs
  and four addable branches from the W4 inventory). Each is a code branch in `world.rs` or
  `trade`; per Antonio's 11-11 ruling an issue suffices (research-only, off by default).
