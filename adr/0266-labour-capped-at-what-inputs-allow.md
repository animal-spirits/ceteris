# 0266. Labour capped at what inputs allow

Status: proposed (owner ruling A, 2026-10-30T20:08Z, approves the econ SPEC change; owner
decisions 2026-10-30T21:51Z: the cap on traded inputs only, and the rationed frictionless wage rule;
owner decisions 2026-10-30T22:43Z: the enterprise's own good caps too, and E3 orders inputs by
effective demand)
Date: 2026-10-30
Design sections: 7.3, 8.5, 8.7, 15.3 (V1), 17.1 (no new parameter); econ SPEC Labour and
Production (the E3 order rule); amends ADR 0089 (E1 labour targets, the frictionless wage at the
natural rate) and ADR 0093 (the frictionless wage search on uncapped targets), and refines the E3
order rule of the owner decisions of 2026-10-24; ADRs 0106, 0112

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
Y_in = min over traded inputs k with coef_k > 0 of M_k / coef_k     (infinite if none)
L_in = VA^-1(a_VA Y_in)
L*   = min(L*_notional, L_in)
```

M_k is the enterprise's stock of k after T1 delivery, its own good g included (owner decision
2026-10-30T22:43Z, below). VA^-1 is the inverse of the normalized CES
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

**The enterprise's own good caps too** (owner decision 2026-10-30T22:43Z). As first built (96bac69)
the cap left it out: capped on it, with E3 as it was, the fixture's default run inflated (world
price index 1.1e7 at tick 100, 5.3e7 when the ledger overflowed at tick 104) and 232 of 400 default
sweep draws panicked. The reason then given, that a short own good cuts the output that refills
it, does not hold for a traded own good, which is bought at the origin markets like any other
input. The lock came from E3 (spec review of 96bac69, I1; confirmed by the E3 probe, As built): E3
planned orders on Y_cap = VA(L) / a_VA, and once the cap cut L, Y_cap was the output the shortage
had cut, so the short input was reordered on its own shortage, the ratchet the owner decisions of
2026-10-24 ruled out. With E3 ordering by effective demand (next) the own good caps like any traded
input and the default run stays at a world price index of 1.19 over 1,200 ticks.

**E3 orders inputs by effective demand** (owner decision 2026-10-30T22:43Z, "follow the
literature's picks"). A rationed agent's demand on each market reflects the constraints it meets on
the other markets but not the one on that market itself: the dual decision (Clower 1965; Barro and
Grossman 1971), in the effective-demand form of Benassy (1975, 1982) and the fixed-price equilibria
of Dreze (1975). For an enterprise held by input k this means: it orders k on what it would produce
without that constraint, and its other inputs on what it can produce with it. At E1, after the
cap:

```
short_e = { traded k with coef_k > 0 : L*_free,e > L_in,e  and  M_k / coef_k <= Y_in,e (1 + 1e-9) }
L_notional = allocate(L*_free, L_{t-1}, eta, L^s)       per province, at the wage the targets were set at
```

L*_free is the target without the input cap (with the L^s cap of the target), at the wage the
capped targets were set at: w_{t-1} under the wage rule, w_t frictionless. allocate is the step
toward the targets and the pro-rata L^s scaling of E1. The 1e-9 margin takes in ties to the last
bits (the data state holds one month of every input). Then at E3:

```
order_k = max(0, (1 + cover + L_k) coef_k Y_plan,k - stock_k - on_order_k)
Y_plan,k = VA(L_notional) / a_VA   if k in short_e
         = VA(L) / a_VA            otherwise (local inputs included)
```

The short input is ordered on notional output, so its shortage does not cut its own reorder; the
complementary inputs scale with the binding shortage, which is how disruptions propagate in the
data (input shortages cut the use of complementary inputs: Boehm, Flaaen and Pandalai-Nayar 2019;
Carvalho et al. 2021) instead of piling up stocks the enterprise cannot use. Ordering up to a target
position stays the target-stock (order-up-to) policy (Arrow, Harris and Marschak 1951; Scarf 1960;
on inventories and output smoothing at the firm, Blinder 1986; Ramey and West 1999). Where no cap
binds, short_e is empty and both plans are Y_cap: the steady state is unchanged.

The alternative probed, ordering every input on notional output (N2 below), plans the complementary
inputs on output the enterprise cannot reach.

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

No new parameter (the 1e-9 tie margin is rounding, not behaviour). The steady state is unchanged:
stocks hold (1 + COVER + L) months of use (P149), so L_in > L* there and short_e is empty; the
reference model and the calibration do not compute the cap.

## Consequences

- An enterprise does not hire labour its traded inputs cannot equip, in both modes; the
  frictionless wage no longer prices labour for an enterprise with no output.
- An enterprise short of a local input still hires on its notional target; the chain cuts its
  output in the tick. That residual is what the owner's local clause aimed at; no form of it
  tried so far keeps the runs bounded.
- An enterprise held by a traded input orders that input on notional output and its other
  inputs on constrained output; E1 hands E3 the binding set and the notional employment through
  `Scratch` (within the tick, not in the snapshot).
- The P2 golden moves (the fixture's data state starts with the caps at L0 exactly).
- The Q = 0 price rule, the storable clip and kappa2 = 0 are unchanged; they contributed to the
  spiral but each does a job (ovf-diagnosis).
- The write-off's i64 bound (systems.rs, "write-off within i64") is not addressed: the x10
  border run still reaches it (the frictionless sweep draw that did, seed 7 case 31, no longer
  does).

## As built

`systems::input_output` (Y_in, own good included), `systems::scarcest_inputs` (the binding set,
margin `systems::SCARCEST` = 1e-9), `production::labour_for_value_added` (VA^-1),
`production::marginal_value_added` (w_top), `systems::labour` (cap, frictionless clearing, and
`Scratch::short` and `Scratch::notional` for E3), `systems::demand` (the split order rule).
Commits b762792 (with the stock-plus-last-output local bound), 96bac69 (traded inputs only, own
good left out), then the round of the owner decisions of 2026-10-30T22:43Z (own good in, E3 split).

Tests: `frictionless_labour_spiral_is_bounded` (the overflowing case: 120 ticks, world price index
at most 1.55, bound 3; 2.7 at 96bac69); `frictionless_price_sweep_stays_bounded` (40 frictionless
draws, 0.5 to 2 per cell, seed 0: highest index 324, no panic; 2.6e3 before the cap, 120 at
96bac69); `labour_is_capped_at_what_inputs_allow` (no traded stock, own good included: hires no
one; no local input in stock or in the province's last output: no change; ample stocks: wage and
employment the same at x100 and x1e4); `rationed_frictionless_wage_is_the_last_one_that_clears`;
`short_input_is_ordered_on_notional_output_and_the_rest_on_constrained_output` (unit: half a
month of one input against a month and a half of the rest; caught both the order rule as built at
96bac69 and notional output for every input); `input_orders_plan_on_notional_output_for_the_short_input`
(was `input_orders_plan_on_capacity_output`; also checks the binding set against
`scarcest_inputs`); the inverse and the marginal product against `value_added` and
`labour_target_uncapped`; `frictionless_unemployment_is_the_natural_rate` (u* everywhere but
province 24 at tick 1, whose first tick used its stocks with its orders in transit, every cap 0
and the wage halved, and province 20 at ticks 8 and 9, every hiring enterprise held by good 2 and
the caps summing below (1 - u*) L^s, the wage min(w_{t-1}, w_top); never below u*; the wage
checked at each); `unreachable_destination_buys_nothing_in_the_world` in both modes unchanged.
On the cycle fixture the chain binds in 483 province ticks, at most 4 passes (547 and 6 at 96bac69);
`exact_fill_equals_converged_iteration` compares 480 of 483.

**Measurements of the order rule** (E3 probe, then this round; the same probe tests for every
variant, the committed code reproduces the probe's S hashes bit for bit). V5 is no cap (a51988f);
V0 the cap as at 96bac69 (own good out, E3 on Y_cap); V4 own good in with E3 on Y_cap; N2 own good
in, every input ordered on notional output; S own good in, the split adopted here. Sweeps: prices
x0.5 to 2 per cell, 120 ticks, proptest seeds 0 to 9 x 40 draws per mode. Structured: alternating
cells at x lo (0.2, 0.3, 0.4, 0.5) and x hi (2, 2.5, 3), period 2 or 3, 120 ticks, 24 cases per
mode. Pack: the scratch pack (b2c recalibration, format 6, pre-0256), frictionless benchmark base
from the calibrated state, 1,200 ticks.

| Measure | V5 | V0 | V4 | N2 | S |
|---|---|---|---|---|---|
| Fixture default, 1,200 ticks, max index | 1.169 | 1.148 | overflow t104 (5.3e7) | 1.189 | 1.188 |
| Fixture frictionless, 1,200 ticks | 1.104 | 1.103 | 1.104 | 1.103 | 1.103 |
| Spiral case, max index | overflow t91 | 2.70 | write-off panic t59 | 1.69 | 1.55 |
| Sweep frictionless: panics, runs > 1e4, max passing | 3, 4, 8.1e4 | 1, 0, 1.9e3 | 49, 122, 2.8e17 | 0, 0, 2.3e3 | 0, 0, 1.9e3 |
| Sweep frictionless, seed 0 max | | 120 | | 67 | 324 |
| Sweep default: panics, runs > 1e4, max | 0, 0, 34 | 0, 0, 1.85 | 232, 148, 2.9e17 | 0, 0, 1.90 | 0, 0, 1.85 |
| Seed 7 case 31, frictionless | | panic (write-off) t70 | | 1.61 | 1.49 |
| Structured overflows of 24, default / frictionless | 20 / 7 | 11 / 10 | 12 / 9 | 12 / 5 | 8 / 2 |
| Fixture calibrated state, 1,200 ticks, both modes | | bit for bit | bit for bit | bit for bit | bit for bit |
| Pack base: completes, max index | yes, 1.000369 | yes, 1.000285 | yes, 1.000314 | yes, 1.000307 | yes, 1.000328 |
| Pack base: max abs dlnp, last 50 ticks | 0.173 | 0.105 | 0.254 | 0.182 | 0.367 |

The owner's adoption test (S as stable as N2: no sweep panic, no run above 1e4, structured
overflows no worse than N2's by more than 2 per mode, pack base clean) holds: S is better than N2
on the spiral, the frictionless sweep maximum and the structured cases. On the pack every variant
with the cap is bit for bit V0 through tick 72 (the first cap binds at tick 76, Sudan); the late
price movement is in Sudan (country 188, provinces 3464 and 3474: good 5 at 3474 moves 0.367 in S,
0.182 in N2; good 9 at 3464 moves 0.105 in V0), the known pre-0256 drift, which main's ADR 0256
pack removes. The structured cases of the first labour round (another generator: alternating cells
x0.2 to 0.5 and x2 to 3) gave 8 / 2 overflows before the cap and 5 / 3 at 96bac69, where the
frictionless count rose by one; on the probe's generator above the cap lowered both, and S lowers
them further.

Steady state: the fixture's calibrated state holds to 1e-9 (`fixture_calibrated_state_holds_to_1e_9`)
and the fixture reference (V1) passes. On the scratch pack, runs from the calibrated steady state
are bit for bit those without the cap through tick 72 in both modes (snapshot hashes); the first cap
binds at tick 76 in the default mode, at province 3474 (Sudan) good 1, in the known pre-0256 drift.
`pack_calibrated_state_holds_to_2e_6_and_4_minor_units` fails there with and without the cap, as it
has on this pack since B2b.

Benchmark on the scratch pack with S (`benchmark_cases_are_reported`): the 1,200-tick frictionless
base completes (max |dlnp| over the last 50 ticks 0.367, Sudan, above). Openness by class 1.110,
1.039, 1.125, 0.998 of ICIO. ACR x1.1 with input-output linkages: median model/ACR 1.066 (p10
0.820, p90 1.272), correlation 0.855, no wrong sign (96bac69: 1.058, 0.856; B2b without the cap:
1.064, 0.865). Hard Brexit: GBR -0.626%, EU27 -0.104%, IRL -0.186%. Border x10 still panics at the
write-off's i64 bound.
