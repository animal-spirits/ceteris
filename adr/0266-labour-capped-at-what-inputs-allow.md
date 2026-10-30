# 0266. Labour capped at what inputs allow

Status: proposed (owner ruling A, 2026-10-30T20:08Z, approves the econ SPEC change; owner
decisions 2026-10-30T21:51Z: the cap on traded inputs only, and the rationed frictionless wage rule)
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

A, on traded inputs only (owner decision 2026-10-30T21:51Z, on the controller's recommendation;
it narrows owner ruling A, which also named local inputs). Fixed-coefficient production bounds
labour demand by the scarcest input: quantity rationing (Clower 1965, the dual-decision
hypothesis; Barro and Grossman 1971; Malinvaud 1977). At E1:

```
Y_in = min over traded inputs k != g with coef_k > 0 of M_k / coef_k     (infinite if none)
L_in = VA^-1(a_VA Y_in)
L*   = min(L*_notional, L_in)
```

M_k is the enterprise's stock of k after T1 delivery. VA^-1 is the inverse of the normalized CES
(econ SPEC, Production): L0 [((VA / A VA0)^rho - (1 - s_L)) / s_L]^(1/rho), infinite beyond VA's
asymptote for rho < 0, 0 below VA(0) for rho > 0; Cobb-Douglas L0 (VA / A VA0)^(1/s_L). The
default wage rule reads excess demand on the capped targets.

**No cap from local inputs.** The same-tick chain already bounds local inputs within the tick,
and its zero-output state needs no extra lock. Both bounds from last tick's chain were built and
failed:

- From the enterprise's last fill, as the owner's ruling named it (last output where its
  smallest local fill was below 1, none where it was 1): an enterprise cut to no output hires no
  one, its need vanishes, its fill reads 1, and it hires the whole province again the next tick,
  every other tick, at the inflating posted price. The fixture's spiral case still overflowed
  the ledger, near tick 76.
- From the enterprise's stock plus the province's last output of the good (committed in
  b762792): a province whose local goods all reach zero together stays there, every cap being
  0 / coef. On the scratch pack the frictionless benchmark base locked province 3464 (Sudan) at
  zero by tick 220; its local prices rose every tick under the Q = 0 price rule (good 8: 7.6e5
  by tick 380), expected inflation carried the rise to every market, and a payment overflowed
  the ledger at tick 1,186 (world price index 6e9). Without the cap that run completes.

**No cap from the enterprise's own good.** A stock of it short of what output needs cuts the
output that refills it: the same lock. Capped on it too, the fixture's default run inflates
(world price index 1.1e7 at tick 100) and overflows the ledger before tick 150.

Frictionless, the wage clears sum_g min(L*_ig(w), L_in,ig) = (1 - u*) L^s. Where no cap binds at
the root of the uncapped demand, that root is kept, bit for bit as before (a search on the capped
demand reaches it by another path and so to other last bits; on the scratch pack that moved a
wage at tick 2). Where every enterprise that would hire is at its cap, demand is flat in w and
the root is not unique: every wage up to w_top = min over them of p_VA dVA/dL(L_in) hires the
same labour. **The wage is then min(w_{t-1}, w_top)**: the last wage where it still clears, else
the highest that does (owner decision 2026-10-30T21:51Z). Without it a search on
the flat demand ends at its lower limit: on the fixture's data state, whose one month of stocks
caps every enterprise at L0 exactly, wages fall to about 7e-59 in some provinces at tick 0
(`rationed_frictionless_wage_is_the_last_one_that_clears` fails then). Frictionless unemployment
is u* except where the caps leave more labour unhired.

No new parameter. The steady state is unchanged: stocks hold (1 + COVER + L) months of use (P149),
so L_in > L* there; the reference model and the calibration do not compute the cap.

## Consequences

- An enterprise does not hire labour its traded inputs cannot equip, in both modes; the
  frictionless wage no longer prices labour for an enterprise with no output.
- An enterprise short of a local input or of its own good still hires on its notional target;
  the chain cuts its output in the tick. That residual is what the owner's local clause aimed
  at; no form of it tried so far keeps the runs bounded.
- The P2 golden moves (the fixture's data state starts with the caps at L0 exactly).
- The Q = 0 price rule, the storable clip and kappa2 = 0 are unchanged; they contributed to the
  spiral but each does a job (ovf-diagnosis).
- The write-off's i64 bound (systems.rs, "write-off within i64") is not addressed: the x10
  border run and one frictionless sweep draw in 400 still reach it.

## As built

`systems::input_output` (Y_in), `production::labour_for_value_added` (VA^-1),
`production::marginal_value_added` (w_top), `systems::labour` (cap, frictionless clearing).
Commits b762792 (with the stock-plus-last-output local bound) and the fix that narrows it to
traded inputs.

Tests: `frictionless_labour_spiral_is_bounded` (the overflowing case: 120 ticks, world price
index at most 2.7, against the ADR 0106 scale of about 1e4); `frictionless_price_sweep_stays_bounded`
(40 frictionless draws, 0.5 to 2 per cell, seed 0: highest index 120, no panic; 2.6e3 before the
cap); `labour_is_capped_at_what_inputs_allow` (no traded stock: hires no one; no local input in
stock or in the province's last output: no change; ample stocks: wage and employment the same at
x100 and x1e4); `rationed_frictionless_wage_is_the_last_one_that_clears`; the inverse and the
marginal product against `value_added` and `labour_target_uncapped`;
`frictionless_unemployment_is_the_natural_rate` (u* everywhere but province 24 at tick 1, whose
first tick used its stocks with its orders in transit, and never below u*);
`unreachable_destination_buys_nothing_in_the_world` in both modes unchanged.

Sweeps, 0.5 to 2 per cell, seeds 0 to 9 (400 runs each): frictionless 1 panic (seed 7, case 31,
the write-off's i64 bound), highest passing index 1.9e3, none above 1e4; before the cap 3 panics
and 4 runs above 1e4 (highest passing 8.1e4). Default: no panic, highest 1.85 (34 before).

Steady state: the fixture's calibrated state holds to 1e-9 (`fixture_calibrated_state_holds_to_1e_9`)
and the fixture reference (V1) passes. On the branch's scratch pack (b2c recalibration, format 6,
pre-0256), runs from the calibrated steady state are bit for bit those without the cap through
tick 72 in both modes (snapshot hashes); the first cap binds at tick 76 in the default mode, at
province 3474 (Sudan) good 1, in the known pre-0256 drift (target 4.6 x L0 against a cap of
1.8 x L0, from its traded stocks). `pack_calibrated_state_holds_to_2e_6_and_4_minor_units` fails
there with and without the cap (worst price 2.96 relative at tick 108 without, 0.95 at tick 106
with), as it has on this pack since B2b.

Benchmark on the scratch pack (`benchmark_cases_are_reported`): the 1,200-tick frictionless base
completes (world price index 1.000 at every 100th tick; max |dlnp| over the last 50 ticks 0.105).
Openness by class 1.109, 1.039, 1.125, 0.998 of ICIO. ACR x1.1 with input-output linkages:
median model/ACR 1.058 (p10 0.821, p90 1.282), correlation 0.856, no wrong sign (B2b, without
the cap: 1.064, 0.865). Hard Brexit: GBR -0.638%, EU27 -0.111%, IRL -0.177%. Border x10 still
panics at the write-off's i64 bound.
