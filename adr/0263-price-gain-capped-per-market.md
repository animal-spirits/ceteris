# 0263. Price gain capped at a share of each market's stability limit

Status: accepted
Date: 2026-10-30
Design sections: 8.2; amends the trade SPEC price rule (P03a)

## Context

The origin price rule moves each market by kappa1 (D - Q) / Q a tick (trade SPEC 8.2). One market
is stable while kappa1 J < 2 (J2, Stability region), J = E + eta eps / (2 - eta): E the
elasticity of the market's orders to its own price, eps the supply elasticity of its labour
target. kappa1 is one number for every market, so the market with the largest J sets the limit.

The failures are in the frictionless mode only (eta 1), which V1 runs: it puts the whole supply
response eps into the loop. In the default mode (eta 0.3) J tops out at 16.9 and a 10 percent
perturbation decays everywhere (largest gap 1.3e-4 at tick 1,200).

On the 0256 pack (frictionless, eta 1) traded J has median 12.5, p99 21.0, max 42.0 (São Tomé and
Príncipe g4, the period-2 cycle the SPEC lists); 298 cells are above 20 and 191 above 30. Albania's
g5 cells sit at J 36.3 to 36.9 (labour share 0.83, eps 25.3), 92 percent of the limit at kappa1
0.05; after ADR 0249 lowered Fier's share of Albania's oil purchases (0.96 to 0.34) Fier's E rose
from 0.4 to 7.6 and coupled g5's Leontief demand into price-responsive markets, and Albania settled
on a growing period-5 mode that saturates near 1 percent (V1 fails at seeds 20 to 22). Lowering
kappa1 for all markets to 0.04 passes V1 but slows every market by 20 percent; scaling every market
to the median J (12.5) destabilizes the local-goods and money loop (V1 3.5e-2 at Tiranë g8).

## Options

1. kappa1 0.04 for all (register entry).
2. kappa1_cell = kappa1 J_ref / J_cell for all cells (normalized gain).
3. kappa1_cell = kappa1 min(1, J_ref / J_cell): only markets above J_ref slow down.

## Decision

Option 3 (owner, 2026-10-30, per-market gain), with J_ref = 2 m / kappa1: no market's loop gain
kappa1 J exceeds the share m of its own stability limit. m = 0.5 (new P03d), so J_ref = 20 at
kappa1 0.05: 298 traded cells slow, the rest keep today's speed.

- J_cell = E_cell + eta eps_cell / (2 - eta), with the run's eta, so in the default mode (J at
  most 16.9 on the 0256 pack) no cell is capped and the live dynamics are unchanged. E_cell = 1 + theta_g - (theta_g + 1) Pi_cell, Pi the
  cell's share of its buyers' purchases weighted by the units it sells to each (Leontief inputs,
  sigma_d 0); local cells E = sigma (P06). eps_cell is the largest over the cell's enterprises of
  s_L sigma_FL / ((1 - s_L) a_VA) (amended as built, below; the draft said the output-weighted
  mean).
- Computed from the pack at calibration, at the calibrated prices, as a calibrated quantity: two
  sparse passes over the kernel legs per traded good (one destination sum, one with squared leg
  factors), the cost of one dispatch. Recomputed when the network, the thetas or the structural
  parameters change, never mid-run.

## Consequences

- On the 0256 pack: Albania stays at its steady state (3e-11 at tick 800, unperturbed), V1 at seed
  20 passes at 7.1e-5 (North Darfur g9), the São Tomé cycle goes.
- Cost: none measured. From a 10 percent perturbation the capped rule converges as fast as today
  (default mode p99 below 0.1 percent at tick 446 in both; uniform 0.04 takes 513), and the Suez
  closure reroutes the same (dependent value share 0.0417 to 0.0310 by t60; world index 1.000300
  in both).
- Instability map (unperturbed, frictionless, 1,200 ticks): uniform kappa1 fails ALB and STP at
  0.05, adds SVN at 0.06, HKG and ARM at 0.08; capped, nothing fails at 0.05, ALB alone at 0.06,
  ALB and ARM at 0.08, BIH at 0.10. The cap removes the single-market cases (STP, SVN, HKG) but
  J2 is a one-market bound: coupled modes across goods (Albania's g5 feeding g1 and g2, Armenia,
  Bosnia) remain, so Albania's margin at 0.05 is under 20 percent. A bound on coupled modes is left
  open.
- The pre-P3 B branch changes demand elasticities (ADRs 0200, 0247, gateway masses), so the code
  lands after it, and B8 recomputes J and reruns the gate.
- P03d is a new free parameter (DESIGN 17 register): m in (0, 1), start 0.5.

## As built (2026-10-30, pre-P3 B)

- eps_cell is the enterprise maximum, not the output-weighted mean the Decision first named: the
  study measured the maximum, and every number above rests on it (298 cells above 20, Albania's g5
  at 36.3 to 36.9, São Tomé at 42.0). The mean gives a lower J, so it caps fewer cells than the
  measurements that justified m = 0.5, and in a cell whose enterprises differ it understates the
  loop gain of the most elastic one, which J2 is about.
- E_cell and eps_cell are written by the calibration to `econ_calib_cells.arrow` (columns
  `demand_elasticity`, `supply_elasticity`; calibration format 6), read into
  `EconInputs::market_elasticity` (in the inputs digest), and combined with the run's eta in
  `EconWorld::new` (`TradeParams::price_gain`); P03d is `TradeParams::gain_share`, in the static
  digest when the inputs carry a calibration. Inputs without a calibration (the generated
  fixture, an uncalibrated pack) run kappa1 in every market and keep their digest, so their
  hashes do not move; a data-state start of calibrated inputs keeps the calibrated gains.
- On the calibrated fixture no cell is capped in either mode (largest J 12.5 at eta 0.3, 13.8 at
  eta 1), so its runs do not move either; only the calibration tables gain the two columns.
- Pi_cell is computed per leg with squared leg factors (the study's form): a pair joined by
  several routes leaves out the cross terms between them.
