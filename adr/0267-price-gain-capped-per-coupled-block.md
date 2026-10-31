# 0267. Price gain capped per coupled block of commodity markets

Status: proposed (owner ruling 2026-10-30T23:19Z, option 1A: extend the ADR 0263 cap to coupled
country blocks of commodity markets by the block spectral radius of the price-step Jacobian)
Date: 2026-10-30
Design sections: 8.2; amends ADR 0263 and the trade SPEC price rule (P03a, P03d, P157; Stability
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

Option 1 (owner, 2026-10-30T23:19Z), on the linearized tick of each country (the block review's
extension of the price-step Jacobian with wages, local prices and income, within the ruling).
Trade SPEC, Stability region, Block gain:

- The linearized tick of a country, at the calibrated steady state with pi^e = 0 and kappa2 = 0
  as for J2, has the price deviations x of its cells (active traded cells and local cells with
  output), the output deviations q of its enterprises, the log wages of its provinces, their
  wage bills and the country's profit pool. It carries every coupling of the tick between
  them: substitution between the country's origins of a good (E_ij = -(1 + theta) w_j T2_ij /
  t_i, the off-diagonal of (1 + theta)(I - Pi), on the Pi_cell leg convention); input orders
  (an enterprise's orders move with its output through the order-up-to position, H = 1 + cover
  + L times the change on top of the use, and with the change of L, which moves with the
  origins' shares); input costs (its output answers last tick's buyer prices of its traded
  inputs and posted prices of its local ones through eps_k times the cost share); the wage (its
  output answers it through beta = s_L sigma_FL / (1 - s_L); frictionless the province's wage
  clears its labour market within E1, under the wage rule it steps on last tick's excess
  demand, at the gain of a rise and of a cut); local draws (a local market's demand from its
  users' output this tick); households (each pop's PIGL and CES demand at this tick's buyer
  prices and its money, this tick's wages and last tick's dividends, by central differences of
  `demand::shares`); and the profit pool (market revenue less wages and purchases, paid at E4).
  One market with one enterprise at given wages is the SPEC's single-market map, whose -1 root
  is J2.
- The pool and the households join every cell of a country: a block is a country. Its gain
  scale b is 1 where the map at gains kappa_cell / m is stable, else the lower end of a
  bisection of (0, 1] to 2^-12, stable at b kappa_cell / m and unstable a step above (not
  necessarily the largest stable grid value: the stable set need not be an interval). The
  calibration checks the map stable at the gains b kappa_cell themselves and stops otherwise.
- Stability is a measured spectral radius at most 1 + 2e-3 (P157), the growth rate of the map's
  iterates over the second half of 4,000 ticks from a fixed start vector (Gelfand's formula).
  There is no linear algebra library in the workspace, the blocks reach 1,787 cells and 3.7
  million couplings, and a growth rate needs only the sparse map. The tolerance decides which
  blocks are capped, so it is registered; ITERATIONS and BISECTIONS are constants of the
  measurement.
- The calibration stores each capped block's limit gain, L_cell = b kappa_cell / m (infinite for
  a block within its margin), computed at the default kappa1, m, eta and wage parameters, for
  the default mode and frictionless (`econ_calib_cells.arrow`, calibration format 8). A run caps
  each cell at the share m of the limit of its wage regime:
  kappa1_cell = min(kappa1, 2 m / J_cell, m L_cell). At the defaults this is b kappa1 min(1,
  J_ref / J_cell); off them it is continuous (a larger kappa1 leaves a capped block at its cap, a
  larger m raises the cap with it, another eta moves the per-market term only), which keeps the
  cap in kappa1 and m sweeps and in the P3 prior ensembles. Refusing those runs, or dropping the
  cap there as first built, were the alternatives (block review, I2).

## Consequences

- The steady state, the calibration and the reference do not move: the gains enter only the
  dynamics. The calibrated fixture caps no block in either mode (largest radius at gains / m
  0.974 at eta 0.3, 0.982 at eta 1, a slow mode of the pool and the households near 1), so its
  runs and goldens do not move; its calibration tables change the two columns to the limits,
  every other column bit for bit the same.
- On this branch's scratch pack (k263, recalibrated at format 8): in the default mode 11 of 233
  blocks are capped, Sudan to 0.244 and Lesotho to 0.333, whose radius at the per-market gains
  themselves is 1.359 and 1.197, then Zimbabwe 0.459, Benin 0.460 and Togo 0.492 (radius 1.034,
  1.032 and 1.007 there), Tanzania, Madagascar, Zambia, Switzerland, Singapore and Belgium.
  Frictionless 41 are capped, from Sudan 0.341 and Lesotho 0.432 to Albania 0.706 (radius 1.247
  at gains / m), France 0.721 (842 cells), Armenia 0.970 and Bosnia 0.998. The map is the tick's:
  on the fixture a coupled block beyond its limit grows in `EconWorld` at the map's radius to
  within P157, both for substitution and for input orders and in both modes, and the map of the
  first build, at given wages and without the move of L, read the substitution mode 0.05 too
  unstable (`block_map_growth_matches_the_tick`). The default-mode claim of ADR 0263 (no market
  slowed in the default mode) does not hold on this pack for blocks either; B8 lists the capped
  blocks on the rebuilt pack for the owner.
- In the default mode the cap now helps the case the first build made worse: Sudan at seed 20
  improves from 2.06 to 1.3e-2 (the first build's scale took it from 1.17 to 2.16), and Lesotho,
  Zimbabwe, Benin and Togo all fall below 4.1e-5 (As built). Frictionless, Sudan stays far off
  at seed 20 (10.75 to 8.37; the first build's map, which capped traded cells only, gave 0.34),
  its local good 9 at province 3474: the regression against the first build is in this one
  case.
- Albania V1 at seeds 20 to 22 on the scratch pack, already passing under ADR 0263, improves
  (As built). Main's unsplit pack cannot be read by this branch's code (its gateways carry no
  mass column, ADR 0092; calibration format 3), so the case that motivated the ADR is checked at
  B8 on the rebuilt pack.
- Cost: the calibration builds the blocks in about 7 s and measures both modes in about 140 s on
  14 cores, against about 4 minutes for the steady state. A run pays nothing.
- Big blocks slow together: the cap applies to the whole country, local markets included, so a
  country (France, 842 cells, 0.721) slows every market for its most coupled loop. A finer
  partition (per loop) is left open.
- Left out of the map, each stated in the SPEC: rebates (held), expected inflation (0, as J2),
  local input orders (at their zero kink at the steady state; along a mode at radius r they
  fill only where output passes its past peak, about (1 - 1/r) of the move, 0 at the limit) and
  foreign prices and wages (held: a block is one country, and the couplings between countries
  are not bounded).

## As built (2026-10-30, pre-P3 B; block review round)

- `ceteris_calibrate::block` (`Block`, `Wage`, `blocks`, `limits`, `Block::spectral_radius`,
  `Block::apply`), called at the end of `steady::calibrate`; `Steady::block_limit`; columns
  `block_limit_default` and `block_limit_frictionless` (format 8); `ceteris_econ::inputs::
  MarketElasticity::limit` (inputs digest), `MarketElasticity::price_gain` and
  `world::price_gains`. `steady::COVER` is `systems::COVER` (P149), the cover of the run's order
  rule, so H follows it.
- The move of L (the slot's expected months in transit) was found by the tick test: without it
  the map read the fixture's substitution mode 0.05 too unstable (one-step orders
  of a mining cell -6.50 in the tick against -8.27 in the map). Under selection every route
  delivers spending / q_d units (ADR 0200), so L moves with the spending shares, at -theta a_os
  (lag - L) per origin.
- Measured on the scratch pack (k263 recalibrated at format 8), largest price and wage gap over
  the world index at tick 1,200 from the 10 percent perturbation, with the block cap and without
  it (limits set infinite in memory):
  - Frictionless, Albania: seed 20 5.2e-4 / 6.1e-4 (9.8e-4 / 1.17e-3 without), seed 21 2.0e-5 /
    2.1e-5, seed 22 2.1e-5 / 2.3e-5. Sudan at seed 20 10.75 to 8.37 (local good 9, province
    3474; 1.7e-3 at seed 21, 1.2e-2 at seed 22), Lesotho 3.7e-2 to below 2.8e-3 (the eighth
    country, Tajikistan, 2.7e-3).
  - Default mode, seed 20: Sudan 2.06 to 1.3e-2 (local good 9, province 3464), Lesotho 3.8e-2,
    Zimbabwe 3.4e-2, Benin 1.6e-2 and Togo 8.8e-3 to below 4.1e-5 (the eighth country, Uganda,
    4.0e-5).
  - The runs without the cap read higher than at 7ca3a2b (Albania seed 20 1.17e-3 against
    7.4e-4, Sudan default 2.06 against 1.17): the round of ADR 0093 fixes and the dispatch-mass
    tolerance (0227e72 to 8aef501) moved the runs between.
  - Unperturbed default mode (`pack_calibrated_state_holds_to_2e_6_and_4_minor_units`, 120
    ticks): every quantity holds its tolerance (at most 0.73 of it, pop income), and the world
    price index's boundedness check, which failed under the first build, passes (4.3e-14 against
    3.0e-14). The wage's fails: 0.101 of its tolerance over ticks 61 to 120 against 0.043 before
    (2.35 times; the check allows 2), at Pitcairn (province 3015), a country no block caps, and
    prices drift 1.9 times at Macedonia's cells. It stays open for B8 on the rebuilt pack.
- p2_acceptance on the scratch pack: 10 passed, 1 ignored.
- Competitive storage (ADR 0091; B7 review round, I1 and I2): at a stored good's origin the map
  carries the storage order's factor r on the order terms, takes revenue from the buyers'
  orders o / r (the merge's R (x + q + e) overstated it by r), and has the cell's stock and
  expectation as states, as the Storage loop does. The limits are computed at the export's
  storage parameters, and the structural digest carries them (P56 delta and money cost, P144,
  P145, P146), so a run under other storage parameters refuses the calibration's limits rather
  than keeping them. With storage off nothing changes, bit for bit. On the fixture calibrated
  with storage on, the default mode caps no block and frictionless caps 1 of 4 (57 cells,
  radius 1.0077 at gains / m; 1.152 without the two states).

## First build (2026-10-30, superseded by the review round above)

The first build linearized the price step alone, at given wages, without households, the pool,
local markets or the move of L, scaled traded cells only, and dropped the cap at any other eta,
kappa1 or m. On the same pack it capped 54 blocks frictionless (Albania 0.636, France 0.744 on
539 traded cells) and 12 in the default mode (Sudan 0.324, Lesotho 0.348), and in the default
mode it made Sudan at seed 20 worse (1.17 to 2.16), the review's I1.
