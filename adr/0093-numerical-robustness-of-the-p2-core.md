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

The exact solve above has no bound on its work, and rounding can mislead it. A review timed
1,000,000 random provinces. A handful took thousands of passes or seconds, all of them provinces
whose fills fall towards 0:
- one falls on a single pass map at a rate within 1e-5 of 1, and following its cycles one at a
  time replayed up to 100,000 cycles after every pass (2.97 s);
- another switches irregularly between two pass maps, which no cycle of up to three passes
  captures (15,678 passes).

A second review found that extrapolating a fall over many cycles turns rounding into fills of 0.
Where a local market balances exactly along a ray of fills (the calibrated form, once a shock has
used up the stocks), rounding leaves a good short by a unit in the last place. Its pass map then
has a gain of 1 to rounding and a fixed point of 0. The rules below bound the work and keep
rounding from being extrapolated.

- **Resolvable falls.** A fill falls resolvably over a pass or a cycle when it falls by more than
  4,096 units in its last place (4,096 eps f, relative 9.1e-13). A pass's map sums a few rounded
  terms, and a step's descent moves its fill by a few units, so rounding alone gives a few units.
  The margin is three orders above that and still below the 1e-12 at which the fills are compared
  with the iteration. A jump is taken only where every good its cycle lowers falls resolvably
  over the cycle's first application.
  - A fall below the margin is treated as no fall, whatever its source. Most are rounding, but a
    real loop whose gain is within about 9e-13 of 1 falls that slowly too, and it is not followed.
    Example: a two-good loop with no constant and gain 1 - 8e-13, whose exact limit is [0, 0, 1],
    stays at [0.4999999999992, 0.9999999999984, 1] after 2 passes. The good left short is short
    by 4e-13 of its flows, within the fit below.
- **Cycles at once.** A jump follows the cycle's maps one cycle at a time for at most 64 cycles.
  It then takes 2^k cycles at once (the composed map squared k times, k up to 64), checked at
  those points.
  - At the doubled points, a good the cycle lowers must be strictly short, with no rounding
    allowance. A good it leaves must fit up to 1e-12 of its flows, the maps' rounding.
  - A piece left between two checked points and entered again before the next is not seen.
  - All jumps of one `fills` call share a budget of 4,096 cycles and map applications. The
    work of a call is at most twice that (`local::WORK_BOUND`).
- **At the resolution of f64.** A pass whose falls are all below the margin is taken to have
  reached what f64 resolves (see the margin's caveat above). The fills then move to a fixed point g, clamped to [0, f], for the goods g
  would lower by more than the margin. The fixed point comes from the last passes, kept across
  jumps:
  - the last pass's map, wherever g lies;
  - else a cycle of the last two or three passes, but only within 1,024 margins of f (relative
    9.3e-10, the rest of a fall below the margin per pass at a gain up to 1 - 1/1024). A cycle
    spanning pieces may have fixed points the passes never reach: unbounded, one moved a fixture
    province from 0.043 to 0.012.

  Each move is confirmed by running the cycle's passes from g, which must return it to 1e-12
  of its largest fill.
  - Where nothing moves, the fills are taken as they are. Two cases: the fills are at g to
    rounding, or the map falls to 0 only by rounding (no constant, so g = 0, with no resolvable
    fall).
  - **Fit up to rounding.** Taken as they are, a good may be short by rounding, at most 1e-12 of
    its flows. That is the Leontief invariant's own tolerance, and the split shares it pro rata.
    It holds by construction. Each good fit at its own step in the last pass. Later in that pass
    its R fell by at most the relative fall of its producer's binding fill, below 9.1e-13, and
    falls of other fills only lower its U. Every return of `fills` follows a pass.
    The tests check fit to 1e-12 of the flows (`fits_up_to_rounding`, in every fill test of
    `p2_properties.rs`).
  - Exact fit is not required, because it is not reachable there. Passes continued past this
    point turn rounding into whole cuts. Seed 7778631857305059895, translated to a limit at 0.1:
    good 1, which fits in exact arithmetic, was cut from 1 to 0.1 once the other goods reached
    0.1 to rounding.
- **Degenerate limits.** Where supply equals need at the limit (U = R at F = t), U - R vanishes
  linearly in F - t. Its rounding, eps times the gross flows, therefore decides whether a good
  fits within about eps times the gross flows over the slope of U - R of t. The plain iteration's
  own result there depends on rounding. So the tests compare with the exact limit, not with the
  plain iteration.
  - The bound is empirical. The test cap is 1e-11, and the worst observed over the 1,254 to
    1,260 translated cases is 1.81e-12.
  - The per-case formula has no bound where a good's U - R is about 0 along the ray. On seed
    7252145407610438004 at 0.3, good 0's slope is 1.1e-12, which would give about 1e-3, yet the
    other goods pin its fill within 1.03e-12 of t.
  - Classification is a heuristic, read from the province's construction. It is not a logical
    consequence. `translate` makes U - R of every good homogeneous in F - t, but the province is
    not the original shifted:
    - users with stock become fully covered;
    - producers without local inputs get a constant;
    - an extra user is added where needed.

    The rule is: goods the original province's bisection iteration takes to 0 (below 1e-200
    taken as 0) are given limit t, and goods it never cuts are given limit 1. It holds on the
    sampled provinces. Any other good is checked only for fit and for lying in [t, 1], a loose
    check.
  - The 1e-200 threshold leaks into the classification. In seed 2172124891379086981, good 1
    ends at 1.02e-200 in the original, a collapse, but is classified as neither.
  - Every other province keeps the comparison with the plain iteration at 1e-12. That holds for
    the proptest's family. On ill-conditioned provinces the plain iteration itself drifts by
    about eps / (1 - gain): in the calibrated form it drifts by up to 1.4e-10 over thousands of
    passes, which is the reference's error, not the fills'.
- **Stalled falls.** After 256 passes, a fall that is still resolvable moves to the last pass's
  fixed point g, in the same way. At most 3 such moves are made.
  - g is used only where it lies in [0, f] up to the solve's rounding, and it is clamped there.
  - That is exact where a single map takes the fills to its limit. Since F never rises, the
    passes that follow restore every fit.
  - Confirmation shows that g is stationary under the passes, not that it is their limit. TAIL
    bounds the two- and three-pass moves. The one-pass move rests on the near-singularity of the
    map that stalls.
  - The stall is reported (`Fills::stalled`, `Scratch::chain_stalled`) where g is undefined, lies
    outside [0, f], would lower nothing, or fails its confirmation. The plain passes then go on.
- **Pass cap.** A call ends at 1,024 passes (`local::PASS_CAP`, four stall bounds; a jump's
  confirming passes count) on a deterministic fallback, with the stall reported:
  - the current fills, where every good fits to 1e-12 of its flows;
  - else the goods that do not fit are set to 0, one at a time in good order, until every good
    does. All fills 0 always fit, since no user of a local good then runs.

  Before the cap, a reported stall left the passes unbounded. Five random provinces in 6,000,000
  of P2 size ran 818 to 1,518 passes, all collapses crawling to the smallest normal f64.
- **Zero, not subnormal.** A fill below the smallest normal f64 is 0: at a step, after its
  rounding descent, and wherever a jump lands.
- **The role of a user at a step** follows from comparing its smallest other fill and its stock
  threshold with the ends of the piece. A midpoint of the piece is no longer used: rounded onto
  an end, it took a branch that does not exist and panicked.

The regressions are:
- `collapsing_provinces_settle_in_bounded_passes`, with pass and work bounds;
- `stalled_provinces_end_within_the_pass_cap`: four stalled seeds (17368639463330687291,
  15714615364009314852, 17705091116894527464, 14824132916395666595) and a cycle with 200 kinks
  on its path, all within the cap, fitting and, for the four, reported;
- `balanced_markets_keep_their_fills`, the calibrated form at 1e-12 against the iteration's
  limit;
- `translated_collapses_stop_at_their_limit`:
  - the seed whose midpoint rounded;
  - five degenerate seeds at the resolution bound;
  - 6603813906490728229, whose good 1 has exact limit 1 where the plain iteration rounds it to
    0.3;
- `translated_collapses_sample` (300,000 draws) and `translated_collapses_full` (3,000,000
  draws, 1,260 translated provinces, about 34 s, `#[ignore]`);
- `stop_takes_the_cycle_fixed_point`, seed 13018842184927700166, which stopped 1.7e-12 above its
  limit on the last pass's map alone;
- `near_singular_cycle_keeps_its_limit`, a loop gain of 1 - 1e-15, within the problem's
  conditioning.
