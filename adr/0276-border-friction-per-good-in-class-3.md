# 0276. Border friction per good in theta class 3

Status: proposed
Date: 2026-10-31
Design sections: 5.4, 8.3, 17 (P05); amends ADR 0092; trade SPEC Two-level kernel, Parameters;
econ SPEC Calibration

## Context

ADR 0092 fits one border friction nu_c per theta class to the class's ICIO 2000 cross-border
share. Class 3 holds two goods with one theta (16.91): good 1 (crude oil, gas and refined fuels)
and good 2 (coal and other mining). Their ICIO targets differ, and on the pack rebuilt after the
main merge (ADR 0256's oil and mining imputation, ADR 0251's whole provinces, ADR 0270's
extraction column; scratch `b8/pack`) the class value is set by crude oil and imposed on mining:

- Class 3: the model's share is 0.3360 at nu 0 and 0.2104 at nu 10, the target 0.2964, so nu_3
  0.05237 (0.00777 on the pack before the merge, share at nu 0 0.3037 there).
- Good 1 alone: target 0.3031, share 0.3569 at nu 0, root nu 0.06591.
- Good 2 alone: target 0.2583, share 0.2198 at nu 0. Its share is below its own target with no
  friction at all, so ADR 0092's clamp (Amendment 2026-10-30) would put it at nu 0; the class
  value pushes it further, to 0.2058.

The calibration then fails. Passes fall to 1.53e-2 by pass 12, and pass 13's scaling hits the
100,000-sweep cap (residual 1.19e-13 against 1e-13). The change after pass 8 is carried by ln q of
good 2 in Rwanda, Nicaragua, Sao Tome and Ethiopia, which flips between export parity (q about
0.2) and import parity (q about 2) by 1.5 to 2.3 a pass. At nu 0.052 those markets are
near-closed blocks (Rwanda's output over demand 1.175 with 3 origins, Nicaragua 1.200). Values
are fixed by the calibration, so inside the autarky band a block's price level is pinned only by
a tiny trade flow: Sinkhorn's slow mode and an outer map that jumps between the two parities.
Storage, the block map, ADR 0270 and this branch's code are ruled out (B8 diagnosis: the old
pack repeats its old pass history digit for digit).

Controlled runs on the same pack, classes 0 to 2 at their fitted nu:

| Class 3 | Passes | Seconds | Residual |
|---|---|---|---|
| both goods at 0.0078 | 68 | 1,251 | 8.29e-13 |
| good 1 at 0.0524, good 2 at 0.0078 | 75 | 1,878 | 8.77e-13 |
| both at 0.0524 from the 0.0078 state | does not settle | | |
| good 1 at 0.06591, good 2 at 0 | 68 | 1,583 | 3.35e-13 |

Classes 0 and 2 mix goods too. At the class fit the per-good shares against the per-good targets
are: good 3 (food) 0.1736 against 0.0992 and good 5 (chemicals and metals) 0.1411 against
0.1806; good 0 (agriculture) 0.1072 against 0.0776 and good 4 (light manufacturing) 0.1605
against 0.1723. Every one of these goods has a root of its own (its share at nu 0 is above its
target: 0.7158, 0.6845, 0.3113, 0.4709), and the calibration settles with the class values.

## Options

1. Per good in class 3 only; the other classes keep one value.
2. Per good wherever a class has more than one good (classes 0, 2 and 3; class 1 is good 6 alone).
3. One class value fitted to a target that does not choke good 2 (good 1 alone gives 0.066,
   higher, and fails as now).
4. Solver work for near-closed blocks at the class value.
5. Data: RAS small-country mining value added to WDI (C1); it does not remove the coupling.

## Decision

Option 1 (proposed; the owner rules between A, B, C and D of the B8 report, this is A).

- P05 is stored per good (`TradeParams::border_nu`, one per good; local goods never cross).
- The fit runs over friction groups (`ceteris_calibrate::border::friction_groups`): each theta
  class is a group, except class 3 (`PER_GOOD_CLASS`), each of whose goods is a group of its own.
  Each group's moment is ADR 0092's, the absorption-weighted ICIO cross-border share of its goods,
  and its root is found as before (stage 2 at the data state's stage 1, Brent per group, shared
  evaluations).
- ADR 0092's clamp applies per group: a good of class 3 whose target is above its share at nu 0
  sits at nu = 0, with the miss reported.
- Why class 3 alone: it is the only class where the class value takes a good past its own corner
  (good 2's share is below its target at nu 0) and the only one where the calibration fails.
  Classes 0 and 2 miss their goods' own shares in both directions but each good has an interior
  root and they settle; fitting them per good is a further model change (nu of food and
  agriculture up, chemicals and metals and light manufacturing down) that no failure forces and
  that is not measured to settle. The smaller change is the decision; option 2 stays open.
- theta and the kernel sharing stay per class: only the border friction splits.

## Consequences

- Each class-3 good matches its own ICIO share, or clamps at nu = 0 below it, which is ADR 0092's
  rule applied to a finer moment, not tuning.
- On the fixture nothing moves: it keeps its default nu (0.3 for every good). Export format 15
  (`params.json` `border_nu` per good) and calibration format 10 (`econ_calib.arrow` metadata
  `border_nu` per good); the static digest carries ten values, so every fixture golden hash moves
  without a price, quantity or money moving.
- Good 2 at nu 0 is more open, so its near-closed blocks get more trade; good 1 at 0.066 is a
  little more closed than at the class's 0.052, which may tighten good-1 blocks (the DR Congo's
  interior was a good-1 cycle on an earlier pack). H2 per class measures class 3 with both goods
  pooled; the gate reports it by good as well.
- Register P05 is five values on the 2000 pack (classes 0 to 2, goods 1 and 2).
- If the owner picks another option this ADR is withdrawn and its commits revert cleanly (ADR and
  specs, tests, code, re-bless are separate).
