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
  (Amended 2026-10-29: one fill per good, the limit of a stated iteration; see Amendment.)
- **Uncapped wage search.** Find the root of sum_g L*_ig(w) = (1 - u*) L^s on uncapped targets;
  cap only the allocation.
- **Kernel range.** Store ln K in f32 (or K in f64), and exponentiate after the per-country
  shift.

## Tests

- A 1e12 price spread between two countries leaves both reachable; flows match the f64 dense
  reference.
- The exact fill equals the converged iteration on every fixture case and on random small
  provinces (1e-12); the cycle cases of `local_chain_terminates_on_cycles` settle in one solve.
  (Amended: the iteration is the fill iteration of the Amendment, run to convergence.)
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

The exact local fill is built in pre-p3/econ-review as the Amendment states (`local::fills`,
`local::Province`); `exact_fill_equals_converged_iteration` compares it with the fill iteration
run to convergence by bisection on every province tick of the fixture runs and on random small
provinces.

## Amendment (2026-10-29): one fill per local good

Owner ruling (2026-10-29, option 2A of the first attempt). The Decision's "largest feasible f"
has no unique meaning: raising one fill raises both the output of the goods its users make and
their demand for other local goods, so the feasible fills are not closed under the
componentwise maximum. The rule is instead the limit of one stated iteration, solved exactly.

**Rule.** In a province, enterprise e has output cap_e (capacity and traded input stocks),
local coefficients a_ek (0 for its own good, which its own output serves first), local stocks
b_ek, and the producer p_k of good k keeps the share a_kk of its output. Each local good k has
one fill F_k in [0, 1], and

    Y_e = cap_e min_{k : a_ek > 0} F_k        (cap_e if e uses no other local good).

Good k fits at F when what its users' stocks cannot cover fits what its producer leaves:

    U_k(F) = sum_e max(a_ek Y_e - b_ek, 0)  <=  R_k(F) = (1 - a_kk) Y_{p_k}     (0 without a producer).

The fill iteration starts at F = 1 and runs passes over k = 7, 8, 9 in order; each step sets
F_k to the largest x in [0, F_k] at which k fits, the other fills as they stand. F never rises,
so the passes converge; the fills are their limit, and every good fits there. Draws from output
are then split as before (output first; short, each user takes what its stock cannot cover and
the rest pro rata to its usable stock).

**Exact solve.** Each step is exact: U_k is continuous, nondecreasing and piecewise linear in
F_k, with kinks where a user's stock stops covering it (a_ek cap_e x = b_ek) and where another
of its fills becomes its smallest; the step finds the root on the right piece in closed form,
then lowers it by rounding units until the fit holds as evaluated in floating point. Passes run
until one changes nothing. A fill below the smallest normal f64 is 0, as a destination's access
is in the Decision as built, so a fill falling geometrically to 0 does not crawl through the
subnormals.

A cycle (goods feeding each other's producers) converges only geometrically, so the passes are
accelerated. On its piece each step is affine in the other fills, so a pass is an affine map
of the fills at its start, and so is a cycle of the last one, two or three passes. Its fixed
point g on the goods the cycle lowers, the others held, is the limit wherever the passes,
repeated, stay on the same pieces. The passes the maps give are followed from the current fills
(a few multiplications each): every step on its piece, no fill raised, every good a pass leaves
still fitting where its step comes, every good it lowers short there (comparisons allow 1e-12
of the flows for the maps' rounding). Where they reach g to 1e-15 of the distance, the cycle's
passes are run from g and must return it to 1e-12; that is the limit. Where they leave a piece
first, the fills jump to the last whole cycle before that, which the passes reach in exact
arithmetic, and the passes go on from there. The shortest cycle that reaches a limit is taken.
No clearing step remains; F only falls, so the passes end. (The work is bounded by the second
amendment, Bounded work.)

**Ties and cycles.** A user's output depends only on the smallest of its fills, so ties among
its fills do not change Y; a piece takes the lowest good, and the comparisons that place a step
on a piece are not strict, since both sides of a tie or a stock threshold give the same U. On
the fixture with services and construction feeding each other (`local_chain_terminates_on_cycles`)
every province tick settles in at most 6 passes; the old chain ran to its bound of 32 there. On
random small provinces with coefficients up to 0.6, nearly all settle within 12 passes and a
loop gain near 1 can take about 200.

**Why the old chain differs.** The old chain multiplied each enterprise's output by the
smallest fill of its local inputs in each pass, so its total cut was the product of the
per-pass minima, not one fill per good. The two differ wherever goods bind in different passes:
with goods A (producer pA, users pB, u1, u2) and B (producer pB, users pA, u1), A short first
and B short after pB is cut, u1 ends at the product of both goods' cuts and u2 at A's alone, so
u1 runs below min(F_A, F_B). The old bound of 32 passes then left cycles still converging and
cleared them by cutting the users that make no local good, which changed who was rationed.

**Consequences.** State hashes move wherever the chain binds. On the fixture's default and
frictionless runs it never binds (1,200 ticks), so the goldens do not move; it binds in the
cycle fixtures, which have no golden. The steady state never rations, so the calibration and
the GE reference, whose local output solves the unrationed Leontief system, do not change.

## Amendment (2026-10-29): bounded work

The exact solve above has no bound on its work. A review timed 1,000,000 random provinces: a
handful take thousands of passes or seconds, all of them provinces whose fills fall towards 0.
One falls on one pass map at a rate within 1e-5 of 1, so following its cycles one at a time
replays up to 100,000 cycles after every pass (2.97 s). Another switches irregularly between two
pass maps, which no cycle of up to three passes captures (15,678 passes). One such province in a
tick costs about 100 times the tick's budget, so the work is bounded, by three rules.

- **Cycles at once.** A jump follows the cycle's maps one cycle at a time for at most 64 cycles,
  and for at most 4,096 cycles within one `fills` call, all jumps together. Then it takes 2^k
  cycles at once, the composed map squared k times, for k up to 64. It checks the pieces, the
  falls and the fits at those points only. A fall at a rate near 1 then reaches its fixed point
  in about 60 steps, a collapse towards 0 included, since the composed map is linear there and
  its fixed point is exactly 0. A piece left between two checked points and entered again before
  the next is not seen; at the points checked, the conditions are the exact ones.
- **Stalled falls.** After 256 passes since the last such cut, the goods that the last pass's
  map takes to 0 are set to 0. These are the goods it lowers whose fixed point is exactly 0, as
  in a collapse; where there are none, every good it lowered is set to 0. The passes then go on
  until one changes nothing. Each such cut zeroes at least one good for good, since F never
  rises, so a province takes at most about 4 x 256 passes. This can change a result only where
  the iteration still falls after 256 passes without any cycle of up to three passes reaching
  its limit. In the 1,000,000 random provinces this happened twice. Both provinces fall
  irregularly towards 0, and both end where the unbounded iteration ends: [0, 0, 0] in one,
  [0.1243, 0, 0] in the other, to 3e-17. On the fixture, no province comes near it.
- **Zero, not subnormal.** Every fill below the smallest normal f64 is 0: at a step, after its
  rounding descent, and wherever a jump lands.

`collapsing_provinces_settle_in_bounded_passes` holds both review seeds to a pass bound.
