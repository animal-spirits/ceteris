# 0093. Numerical robustness of the P2 core

Status: proposed (partly built, see As built)
Date: 2026-10-25
Design sections: 19.4; trade SPEC Active markets; econ SPEC Production, Labour

## Context

Four numerical shortcuts in P2 fail at the edges:

- **Global weight scale.** Origin weights are w_o = (p_o / p_min)^-theta with one world p_min
  and a floor of 1e-100. At theta 10.57 a price spread of about 3e9 (1e8 at the first thetas'
  12.55) makes a destination unreachable, through scaling alone. DESIGN 19.4's
  "no log-domain needed" does not hold.
- **Local chain bound.** The same-tick chain runs up to 32 passes, then clears what is left by
  ad hoc cuts that change which users are rationed.
- **Frictionless wage search.** Capping each labour target at L^s makes the wage indeterminate
  below the root wherever one target reaches L^s.
- **f32 kernel.** K = tau^-theta in f32 underflows once tau^theta exceeds about 1e38: tau
  above about 3,900 at theta 10.57 (1,050 at 12.55), or 3.8 at theta 64.85.

## Options

Keep the shortcuts and measure on the pack (open questions now), or fix them now.

## Decision

Fix them now. None changes a result away from the edge cases.

- **Log-domain weights.** Store ln w_o. Shift per country by its maximum, carry the shift
  through the gateway sums with log-sum-exp over at most 600 terms, and test reachability on
  Phi_d in log space. The floor applies only to K = 0 (no path).
- **Exact local fill.** Per province, the fills f_k of the three local goods solve a monotone
  piecewise-linear system. Enumerate the at most 2^3 regimes of which users are stock-covered,
  solve each, and keep the largest feasible f. No iteration bound and no ad hoc clearing remain.
- **Uncapped wage search.** Find the root of sum_g L*_ig(w) = (1 - u*) L^s on uncapped targets;
  cap only the allocation.
- **Kernel range.** Store ln K in f32 (or K in f64), and exponentiate after the per-country
  shift.

## Tests

- A 1e12 price spread between two countries leaves both reachable; flows match the f64 dense
  reference.
- The exact fill equals the converged iteration on every fixture case and on random small
  provinces (1e-12); the cycle cases of `local_chain_terminates_on_cycles` settle in one solve.
- The frictionless wage matches the closed form where one target reaches L^s.
- Kernel round trip at tau 1e4 and theta 64.85.

## Consequences

- DESIGN 19.4 is corrected. Sourcing cost rises by one log-sum-exp per gateway per good per
  tick, negligible against the budget.
- State hashes change only where the old code hit a floor or the chain bound.

## As built

Built in the pre-p3/speed branch: the log-domain weights and the kernel range. Weights are ln
w_o = -theta ln(p_o / p_A), relative to the cheapest active price of the origin's own country,
with a shift per country. The abroad part of each destination's access takes its country's
destination shift, the largest shift among the foreign countries linked to it (a gateway with
access and a crossing with K > 0 into it), and a table of exp(shift difference) per country
pair, at most 1, carries the shift through the gateway sums in place of a log-sum-exp per term.
The home part keeps the country's own origin shift. Each destination combines the two parts in
log space. Where the two scales are more than 709.78 apart the cross term is formed in log
space, and where spending per unit of access is beyond f64 it is carried with a binary scale
(trade SPEC, Active markets). `WEIGHT_FLOOR` is gone; a destination is reached when its access
is at least `f64::MIN_POSITIVE`. The kernel stores K in f64, which makes the f32 row scaling
unnecessary. The price spread of 1e12 between two countries, a wide-spread reachability test,
the kernel round trip at extreme tau, a scale gap beyond 709.78 and spending per unit of access
beyond f64 are tests.

The uncapped wage search is built in pre-p3/econ-review: the frictionless wage is the root on
the uncapped targets (`production::labour_target_uncapped`, infinite at the corners where no
finite profit maximum exists, as the GE reference's), and only the allocation caps at L^s.

Not built, and still open: the exact local fill.
