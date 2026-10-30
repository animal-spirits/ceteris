# 0266. Labour capped at what inputs allow

Status: proposed (owner ruling A, 2026-10-30T20:08Z, approves the econ SPEC change)
Date: 2026-10-30
Design sections: 8.5, 8.7, 15.3 (V1), 17.1 (no new parameter); econ SPEC Labour; amends ADR
0089 (E1 labour targets, the frictionless wage at the natural rate) and ADR 0093 (the frictionless
wage search on uncapped targets); ADRs 0106, 0112

## Context

`conservation_over_120_ticks` drew a frictionless case (fixture, no calibration, prices x0.5 to
2 per cell) that overflowed the ledger at tick 91; it was kept as the ignored known-defect test
`frictionless_labour_spiral_overflows_the_ledger` (1b8911d). The diagnosis (pre-p3 ledger,
`ovf-diagnosis.md`):

- Province 10's local sectors start at low price indices and hire little; its same-tick chain
  collapses, and all nine of its origins reach Q = 0 by tick 18.
- The price rule at Q < Q_floor pushes an origin with no output up every tick (spec'd, and it
  steers buyers away from dead origins): good 5 goes from 7 to 6.3e6 by tick 91.
- Frictionless E1 values labour at that posted price on targets with no input bound: from tick
  21 all of the province's labour (1.629e6, 8 x L0) goes to the good-5 enterprise, which has no
  output. The wage follows (6.7e8 at tick 91); the enterprise pays it from debt (ADR 0112 writes
  off only what pops hold), money is created, the world price index reaches 9e3, and a wage
  payment of 1.09e19 minor units exceeds i64.
- On 40 draws each: default mode no panic (index at most 17); frictionless 1 panic, the passing
  cases up to 4.5e4.

The defect is E1's: production is Leontief (econ SPEC, Production), so an enterprise without its
inputs produces nothing whatever labour it hires, yet E1 hires on the notional target alone.
Widening the ledger (against ADR 0106) or holding prices at Q = 0 (breaks the island test and
the stability tests) would hide it.

## Options

- **A. Cap the labour target at the labour the inputs allow** (traded stocks after T1, local
  inputs from the last chain; the frictionless wage clears on the capped targets). Owner ruling A.
- **B. Value output at its realised price** (sales, not the posted price): needs a restart rule
  for an enterprise that sells nothing.

## Decision

A. Fixed-coefficient production bounds labour demand by the scarcest input: quantity rationing
(Clower 1965, the dual-decision hypothesis; Barro and Grossman 1971; Malinvaud 1977). At E1:

```
Y_in = min over inputs k != g with coef_k > 0 of (M_k + Q_ik [k local]) / coef_k
L_in = VA^-1(a_VA Y_in)
L*   = min(L*_notional, L_in)
```

M_k is the enterprise's stock (traded inputs after T1 delivery), Q_ik the province's last output
of local good k (the market's Q from the last same-tick chain): what this tick's chain can give
it at most if output holds. Its own good is left out (its own output serves it first). VA^-1 is
the inverse of the normalized CES (econ SPEC, Production): L0 [((VA / A VA0)^rho - (1 - s_L)) /
s_L]^(1/rho), infinite beyond VA's asymptote for rho < 0, 0 below VA(0) for rho > 0;
Cobb-Douglas L0 (VA / A VA0)^(1/s_L). The default wage rule reads excess demand on the capped
targets.

Frictionless, the wage clears sum_g min(L*_ig(w), L_in,ig) = (1 - u*) L^s. Where no cap binds at
the root of the uncapped demand, that root is kept, bit for bit as before. Where every enterprise
that would hire is at its cap, demand is flat in w and the root is not unique: every wage up to
w_top = min over them of p_VA dVA/dL(L_in) hires the same labour. The wage is then min(w_{t-1},
w_top): the last wage where it still clears, else the highest that does. A search on the flat
demand ends at its lower limit instead: on the fixture's data state, whose one month of stocks
caps every enterprise at L0 exactly, wages near 1e-58 at tick 0. Frictionless unemployment is u*
except where the caps leave more labour unhired.

**Local inputs: the province's last output, not the enterprise's last fill.** The ruling named
the enterprise's last chain fill. Built that way (Y_local = last output where the enterprise's
smallest local fill was below 1, unbounded where it was 1), the spiral case still overflowed:
an enterprise cut to no output hires no one, its need vanishes, its fill reads 1, and it hires
the whole province again the next tick, every other tick, at the inflating posted price
(overflow near tick 76). The province's last output of the good plus the enterprise's stock is
the chain's own bound on what it can give, does not vanish with the enterprise's need, and
needs no new state.

No new parameter. The steady state is unchanged: stocks hold (1 + COVER + L) months of use
(P149) and Q_ik exceeds any one user's use, so L_in > L* there; the reference model and the
calibration do not compute the cap.

## Consequences

- An enterprise does not hire labour it cannot equip, in both modes; the frictionless wage no
  longer prices labour for an enterprise with no output.
- The P2 golden moves (the fixture's data state starts with the caps at L0 exactly).
- The local chain binds less, since users no longer hire beyond what the province's local output
  supplies: `exact_fill_equals_converged_iteration` covers 285 binding province ticks of its
  default runs (410 before) and adds a frictionless cycle run (66) to keep its coverage.
- Frictionless runs with prices off their base point fire fewer write-offs:
  `write_off_runs_are_thread_independent` runs its prices in the default mode, where they still
  fire.
- The Q = 0 price rule, the storable clip and kappa2 = 0 are unchanged; they contributed to the
  spiral but each does a job (ovf-diagnosis).

## As built

`systems::input_output` (Y_in), `production::labour_for_value_added` (VA^-1),
`production::marginal_value_added` (w_top), `systems::labour` (cap, frictionless clearing).

Tests: `frictionless_labour_spiral_is_bounded` (the overflowing case: 120 ticks, world price
index at most 1.7, against the ADR 0106 scale of about 1e4); `frictionless_price_sweep_stays_bounded`
(40 frictionless draws, 0.5 to 2 per cell, proptest's fixed seed: highest index 18.0, no panic;
at HEAD the same draws reach 4.6e3); `labour_is_capped_at_what_inputs_allow` (no traded stock or
no local input: hires no one; ample stocks: wage and employment the same at x100 and x1e4);
`rationed_frictionless_wage_is_the_last_one_that_clears`; the inverse and the marginal product
against `value_added` and `labour_target_uncapped`;
`frictionless_unemployment_is_the_natural_rate` (u* everywhere but province 24 at tick 1, whose
first tick used its stocks with its orders in transit, and never below u*);
`unreachable_destination_buys_nothing_in_the_world` in both modes unchanged.

Sweeps on the diagnosis draws (40 each): default 0.5 to 2 no panic, highest index 1.63;
frictionless 0.5 to 2 no panic, 2.47; frictionless 0.9 to 1.1 no panic, 1.11.

Steady state: the fixture's calibrated state holds to 1e-9 (`fixture_calibrated_state_holds_to_1e_9`)
and the fixture reference (V1) passes. On the branch's scratch pack (b2c recalibration, format 6,
pre-0256), runs from the calibrated steady state are bit for bit those of HEAD through tick 72 in
both modes (snapshot hashes); the first cap binds at tick 76 (default; frictionless by tick 84),
at province 3474 (Sudan) good 1, in the known pre-0256 drift (target 4.6 x L0 against an input
cap of 1.8 x L0). `pack_calibrated_state_holds_to_2e_6_and_4_minor_units` fails there before and
after (worst price 2.96 relative at tick 108 before, 0.95 at tick 106 after), as it did on this
pack since B2b.
