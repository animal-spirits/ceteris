# 0267. Price gain capped per coupled block of commodity markets

Status: proposed (owner ruling 2026-10-30T23:19Z, option 1A: extend the ADR 0263 cap to coupled
country blocks of commodity markets by the block spectral radius of the price-step Jacobian)
Date: 2026-10-30
Design sections: 8.2; amends ADR 0263 and the trade SPEC price rule (P03a, P03d; Stability
region, Block gain)

## Context

ADR 0263 caps each market's gain at the share m (P03d, 0.5) of its own J2 limit. J2 is a
one-market bound, and the ADR left coupled modes open: on main's unsplit pack Albania's goods 1,
2 and 5 cells each sit under the single-market limit (J 16.9 to 28.2 against J_ref 20, gains 0.035
to 0.05) but oscillate together, a coupled price-rule loop (about 23 ticks in Tiranë, period 3 in
mining g2 at provinces 62 and 65), and V1 at seed 20 misses by 1.85e-2 in prices and 1.52e-2 in
wages over ticks 600 to 1,200. Scaling those three goods' gains by 0.8 in Albania gives 8.4e-6
(other goods alone do not: goods 0, 3, 4, 6 at 0.8 leave 1.3e-2, good 2 alone 5.2e-3); kappa1 0.04
everywhere gives 2.1e-4 but slows every market (v1-diagnosis, section 2). ADR 0263's own
instability map lists the same family: Albania, Armenia and Bosnia fail on coupled modes.

## Options

1. Block cap: linearize each country's traded cells together and scale a block's gains so that it
   keeps the margin m of its own stability limit (the owner's 1A).
2. kappa1 0.04 for every market (register entry).

## Decision

Option 1 (owner, 2026-10-30T23:19Z). Trade SPEC, Stability region, Block gain:

- The linearized step of a country's traded cells, at the calibrated steady state with pi^e = 0
  and kappa2 = 0 as for J2, has the cell price deviations x, the output deviations q of the
  country's enterprises and the buyer prices y of their input slots (province, traded good). It
  carries the three couplings of the price step: substitution between the country's origins of a
  good (E_ij = -(1 + theta) w_j T2_ij / t_i, the off-diagonal of (1 + theta)(I - Pi), on the
  Pi_cell leg convention), input orders (an enterprise's orders move with its output through the
  order-up-to position, H = 1 + cover + L times the change on top of the use) and input costs (its
  output answers last tick's buyer prices of its inputs through eps_k times the cost share). The
  timing is the tick's: E1 at this tick's own price and last tick's buyer prices, E3 orders,
  T4 reprices. One market with one enterprise is the SPEC's single-market map, whose -1 root is
  J2.
- A block is a connected set of a country's cells under these couplings. Its gain scale b is the
  largest value on a 2^-12 grid in (0, 1] at which the map is stable at gains b kappa_cell / m,
  1 if it is already stable there: the block keeps the margin m that ADR 0263 gives each market,
  and kappa1_cell = b kappa1 min(1, J_ref / J_cell), the per-market cap staying the bound of each
  cell. Only blocks beyond their margin change.
- Stability is measured as the growth rate of the map's iterates over the second half of 4,000
  ticks from a fixed start vector (the spectral radius by Gelfand's formula), at most 1 + 2e-3.
  These are numerical constants of the measurement, not parameters: there is no linear algebra
  library in the workspace, the blocks reach 1,088 cells and 3.1 million couplings in all, and a
  power-iteration growth rate needs only the sparse map.
- b is a calibrated quantity, like E and eps: the calibration computes it at the default kappa1
  and m for the default mode's eta (0.3) and frictionless (1) and writes both to
  `econ_calib_cells.arrow` (calibration format 7). A run multiplies the per-market gain by the
  b of its mode; at another eta, kappa1 or m it keeps the per-market cap alone.

## Consequences

- The steady state, the calibration and the reference do not move: the gains enter only the
  dynamics. The calibrated fixture scales no block in either mode (largest radius at gains / m
  0.820 at eta 0.3, 0.942 at eta 1), so its runs and goldens do not move; its calibration tables
  gain the two columns, every existing column bit for bit the same.
- On this branch's scratch pack (k263, recalibrated at format 7): frictionless, 54 of 233 blocks
  are scaled, Albania to 0.636 (radius 1.324 at gains / m), with Lesotho 0.505, Sudan 0.530,
  Zambia 0.567, Tanzania 0.643, Zimbabwe 0.646, France 0.744 and Armenia 0.949 and Bosnia 0.971
  among them; in the default mode 12, led by Sudan (0.324) and Lesotho (0.348), whose radius at
  the per-market gains themselves is 1.23 and 1.17: the linearization predicts the default-mode
  Sudan oscillation the pre-P3 B pack shows (late max |d ln p| 0.367, all Sudan) and the Lesotho
  cells on Senegal's donor good-1 column that ADR 0263 (As built) already caps. The default-mode
  claim of ADR 0263 (no market slowed in the default mode) does not hold on this pack for blocks
  either; B8 lists the scaled blocks on the rebuilt pack for the owner.
- Albania V1 at seeds 20 to 22 on the scratch pack, already passing under ADR 0263, improves
  (As built, below). Main's unsplit pack cannot be read by this branch's code (its gateways
  carry no mass column, ADR 0092; calibration format 3), so the case that motivated the ADR is
  checked at B8 on the rebuilt pack.
- Cost: the calibration builds the blocks in about 7 s and measures them in about 20 s (default)
  and 110 s (frictionless) on 14 cores, against about 4 to 6 minutes for the steady state. A run
  pays nothing.
- Big blocks slow together: the scale applies to the whole connected set, so a country whose
  traded cells are all joined (France, 539 cells, 0.744) slows every one of them for its most
  coupled loop. A finer partition (per loop) is left open.
- The linearization is at given wages and leaves out households' income, the local goods' markets
  and the wage response (frictionless the wage clears each province), as J2 does; it is a bound
  on the loop as the per-market cap is, not a model of the whole economy.

## As built (2026-10-30, pre-P3 B)

- `ceteris_calibrate::block` (`Block`, `blocks`, `scales`), called at the end of
  `steady::calibrate`; `Steady::block_scale`; columns `block_scale_default` and
  `block_scale_frictionless`; `ceteris_econ::inputs::MarketElasticity::block` (inputs digest),
  `MarketElasticity::block_scale` and `world::price_gains`.
- Measured on the scratch pack (k263 recalibrated at format 7; the block scale off by running at
  kappa1 0.0500000001, which the scale does not apply to), largest price and wage gap over the
  world index at tick 1,200 from the 10 percent perturbation:
  - Frictionless, Albania: seed 20 8.8e-5 / 9.4e-5 (7.0e-4 / 7.4e-4 without the block scale),
    seed 21 1.03e-4 / 1.04e-4, seed 22 1.05e-4 / 1.14e-4. Sudan 10.2 to 0.34 (its local good 9,
    province 3464), Lesotho 1.6e-2 to 5.8e-3, Iraq 5.5e-3 to 6.6e-4, Kenya 5.2e-3 to below
    6.8e-4.
  - Default mode, seed 20: Sudan 1.17 to 2.16 (local good 9, province 3474), Lesotho 4.0e-2 to
    4.5e-2; Zimbabwe 3.6e-2 and Benin 1.7e-2 to below 5.6e-3.
  - Unperturbed default mode (`pack_calibrated_state_holds_to_2e_6_and_4_minor_units`, 120
    ticks): without the block scale Sudan leaves the steady state from rounding (price 1.78
    relative at tick 113); with it every quantity holds its tolerance and only the world price
    index's boundedness check fails (1.56e-13 relative over ticks 61 to 120 against 2.9e-14
    before). Over 480 ticks the Sudan mode grows with the scale too (price 3.9 relative at tick
    302). Sudan's default-mode loop runs through its local goods and wages, which the block
    leaves out (Consequences); it stays open for B8.
