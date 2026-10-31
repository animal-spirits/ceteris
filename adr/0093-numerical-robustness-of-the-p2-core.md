# 0093. Numerical robustness of the P2 core

Status: proposed (partly built, see As built); the frictionless wage search amended by ADR 0266
(input-capped labour targets)
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
  - the last pass's map: within 1,024 margins of f, or beyond where g lies on that pass's
    pieces (amended 2026-10-30, task B3 fill round 4, below);
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
    bounds the two- and three-pass moves. The one-pass move beyond TAIL needs g on the pass's
    pieces (amended 2026-10-30, below).
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

## Amendment (2026-10-30): bounded descent and the Leontief tolerance of local draws

**Bounded descent.** "Lowers it by rounding units until the fit holds" had no bound. Where the
closed form puts the root above a piece on which U is flat, the evaluated fit fails by rounding
along the whole piece, and the descent walks it one unit at a time. On the installed pack,
frictionless, under the B6 split rule, a province's construction producer had no capacity
(R = 0) and a user sat at its stock threshold, so good 8's root came out at 1 with the fit
holding only below 0.9776. That is about 2e14 units: one tick ran over 40 minutes, and the pass
cap did not bound it, since the walk is inside a single step. The step now descends at most 8
units, then bisects the bit patterns of [0, x]: at most 64 more evaluations. The result is the
same, the largest f64 at most the root at which the good fits as evaluated. The evaluated fit
is monotone in the fill: each need is a product of a min over the fills, and rounded products,
maxima and sums are monotone; the producer's output does not depend on its own good. 0 always
fits, and positive f64 order as their bits. No golden moves. Regression:
`local::tests::a_root_above_a_flat_piece_descends_in_bounded_work`, the captured province.

**Leontief tolerance of local draws.** "That is the Leontief invariant's own tolerance" was not
so. The invariant compared each user's use with 1e-12 of its own coef Y, but two roundings of
the good's flows land on single users:
- where the passes stop short, a good is short by at most FIT = 1e-12 of its flows,
  U - R <= FIT (U + R), so R / U >= (1 - FIT) / (1 + FIT) exactly. A user drawing pro rata then
  misses (n - b)(1 - R / U) <= 2 FIT / (1 + FIT) (n - b) < 2 FIT of its own need n: the 1.1e-12
  to 2e-12 breaches under the B6 rule. This bound is per user, so it scales with the user's
  need, not the market's;
- the users draw from the market account one after the other. Each draw rounds its balance by
  at most eps / 2 of what the market held, M; with the split's own sums that is up to
  2 (N_GOODS + 1) roundings, (N_GOODS + 1) eps M in all, and the user at which the market runs
  dry absorbs them. The market cap binds only where the market ends empty, so M is the units
  drawn D plus those roundings: M <= D / (1 - (N_GOODS + 1) eps) < 2 D. The proptest seed of
  `conservation_over_120_ticks` (tick 43, enterprise 41, good 7) misses 1.8e-31 of a need of
  1.5e-22, which is 1.6e-17 of D = 1.1e-14.

The invariant now allows, for a local good, (1e-12 + 2 FIT) of max(coef Y, available) plus
2 (N_GOODS + 1) eps D (4.9e-15 D), where D is the units of the good drawn at the province market
that tick. Traded inputs keep 1e-12. A first version of this amendment scaled the chain-stop term
by D as well (2 FIT (1 + FIT) D); that let a user whose need is below 2e-12 of its market's draws
get nothing and pass, so the term is per user
(`a_tiny_local_user_left_without_its_input_breaks_leontief`).

**Jump ahead to the last whole cycle.** The first amendment has a jump that leaves the pieces
"jump to the last whole cycle before that". The doubling (second amendment) instead ended at its
last check, up to half the remaining cycles short, so the next passes stayed on the same pieces
and a further jump was needed before the kink. The doubling now bisects the cycles between its
last two checks (adding 2^i cycles for i from k - 2 down to 0 where the cycle from there stays on
the pieces, the cycles that stay taken to be the first ones, as the doubling takes them) and
then takes that whole cycle, at most 64 more units of the shared budget. The next pass crosses
a kink, so where every jump applies the passes are at most 2 + 3 + 2K, K the kinks the fall can
cross: users' stock thresholds inside (0, 1), local goods beyond the first that start to fall,
and pairs of local goods some user takes both of (econ SPEC, Production; asserted per province
tick in `local_chain_terminates_on_cycles`, at most 6 passes there against bounds of 12 and up).
On a fall of gain 0.995 to a kink 1,500 cycles on, 7 passes become 4
(`a_jump_ahead_ends_after_the_last_whole_cycle_on_the_pieces`). On 200,000 random provinces of
each of five families, 21 of 1,000,000 take more than their kinks allow, against 140 before.
No golden moves: the chain does not bind on the golden runs. The cycle fixture's run
moves from tick 22, where province 10's fills (about 5e-19) land 300 units in the last place
apart, in 4 passes instead of 5, and the difference grows from there.

Collapses take fewer passes, so `translated_collapses_sample`, which translated the collapses of
12 or more passes, found 7 of 300,000 draws (43 before) and `translated_collapses_full` 62 of
3,000,000 (420). That selection used the code under test; the next paragraph replaces it.

**Chain bound, accounted (2026-10-30, task B3 fill round 3).** The bound 2 + 3 + 2K holds where
every jump applies, and the 21 exceedances above were described by cause without evidence. The
fills now report the passes outside the accounting (`Fills::uncovered`): a pass after which no
jump moved and one did not apply (its cycle's map undefined or singular, its fall within
rounding, or the budget spent); two for a pass that cuts a good by rounding alone while another
falls resolvably (it changes the pieces where no kink lies and ends the previous pass's jump,
as a kink costs two); a pass a stop moves on from; at the stall rule every pass since the last
move, and every pass after a stall. `chain_passes_within_their_kinks` asserts passes <=
2 + 3 + 2K + uncovered on random provinces of the five families; it held on 3,000,000 probed.
The exceedances, rerun at this code (22 of 1,000,000: the 21 and one that stalls, which the
first count left out), classify as:

| Cause | Cases | Example (k of k x 0x9E3779B97F4A7C15) |
|---|---|---|
| A good cut by rounding every other pass: at its limit with U = R along the rest of the fall, each cut ends the last pass's jump after a few cycles | 10 | 175527: 88 passes, bound 25, 39 cuts; 3779: 68, bound 27 |
| Stall rule at 257 passes: pieces that repeat over 4 or more passes, or binding fills switching every pass, so no jump of up to 3 passes applies | 11 | 2267: period 4; 99418: goods 0 and 1 switch 204 times; 73799 stalls, 792 passes |
| A step's root rounded onto a kink every other pass (good 0 lands on good 2's fill), so the cycle's map is undefined | 1 | 170254: 14, bound 11 |

Some causes are mixed (noted 2026-10-30, round 5): 135641 and 12446, listed as rounding cuts
(28 and 15), also have falls no jump resolves (19 and 9); 175527 and 56969 also have a pair whose
binding switches three and two times; 73799 is within the bound only by the charge for every
pass after a stall (521 over without it). `chain_passes_and_uncovered_are_pinned` pins
(passes, uncovered) for five of the 22 and for four provinces with no uncovered pass, since the
counter is generous by design (a stall charges 256 at once) and would absorb added passes.
None exceeds the bound where every jump applies, so the derivation stands under its stated
condition (`chain_passes_beyond_their_kinks_are_uncovered` keeps the 22). The pair term remains
an assumption: a pair of goods can switch its binding more than once (175527, goods 0 and 1,
three times early in the fall; 23 provinces with no uncovered pass, within 6 of their bound,
have such a pair), and the allowance of two passes per kink has held each one. About 11% of
provinces have an uncovered pass; those without stay at least 1 pass inside the bound. Neither
this property nor the fixture's bound tells apart the jump that ended at the doubling's last
check (with it, 141 of 1,000,000 exceed 2 + 3 + 2K, all with uncovered passes); the unit test
above is the guard.

`translated_collapses_*` now choose the collapses without the code under test: the plain fill
iteration (bisection, fills below 1e-200 flushed to 0) ends with at least two fills of 0, about
23% of draws. The degenerate-limit heuristic is checked against the plain iteration on the
translated province: a good where they disagree (seed 1122501757669556625 at 0.3) gets the loose
check of lying in [t, 1] only. Some limits turn on rounding whatever the solver: a good whose
balance with its own fill raised to 1 is within 1e-11 of its flows of 0 can stay at 1 or fall
to t (seeds 1097310483361641164, 7641237549142102837, 13946236107313884973 at 0.1, where the
fills and the plain iteration end on different sides); where a fill is off its confirmed limit
and such a good is present, every good of the case gets the loose check. The sample is 5,000
draws (3,552 cases, 2,290 held to every heuristic limit, about 14 s in debug) and the full run
300,000 (207,054 cases); the minimums are these counts. The named regression seeds are all held
to their exact limits.

**Oracle loosening at the plain limit (2026-10-30, task B3 fill round 5).** The rule above read
the fills under test: whether a fill was off its limit and whether some good's balance was
rounding were both evaluated at the solver's output, and a review's mutation probe (a good the
plain iteration confirms at 1, lowered to t: it still fits) found 3,898 of 6,418 such wrong
fills accepted (seed 9896657790763124343 at 0.1, 0.3 and 0.5, [t, t, t] for [1, t, t]). The
loosening is now decided at the plain iteration's limit and per good (`loosened`): a good is
unresolved where its balance with its own fill raised to 1 is within 1e-11 of its flows of 0
there; a good is loosened where it is unresolved, or where, with each unresolved good at t or
at 1 and the rest at the plain limit, its own such balance is within 1e-11 of 0 or changes
sign (its choice between staying at 1 and a cut turns on where they end). Loosened goods get the
check of lying in [t, 1]; every other good keeps its confirmed limit to 1e-11. Where the plain
iteration does not converge every good is loosened, as before. The cross rule is needed: with
the unresolved goods alone loosened, 25 of the 207,054 cases fail, seed 13946236107313884973
among them (good 1 fits at 1 once good 2 stays at 1). Loosening every good whose users or
producer take an unresolved good, the structural reading, accepts 2,188 of 6,418 wrong fills on
the 30,000-draw sample, all at goods that fit at 1 by a wide margin under every end. The named
rounding seeds (1097310483361641164, 7641237549142102837, 13946236107313884973 at 0.1,
1122501757669556625 at 0.3) all pass under the rule; none is held by name.

Mutation result, as `translated_sample` now counts it (full run, 207,054 cases): of 64,098
wrong fills (a good confirmed at 1 and kept at 1 by the fills, lowered to t), 51,078 are
rejected and 13,020 accepted, every accepted one at a good whose own balance is rounding at the
plain limit, which the sample asserts. Those cannot be rejected by an oracle that admits the
solver: holding such goods to their confirmed limits fails 74 of the 20,565 cases of the
30,000-draw sample, both ways and between (seed 7641237549142102837 cuts good 2 to t where the
plain iteration keeps 1; seed 13997107944263473172 at 0.3 stops good 0 at 0.73). The sample
counts are 3,552 cases, 1,107 held to every heuristic limit, 858 mutants rejected (239 at
unresolved goods); the full run 207,054, 65,073, 51,078 (13,020). Held counts fall from 2,290
and 132,459 because a case is held only where no good is loosened. The named regression seeds
of `translated_collapses_stop_at_their_limit` are held to every heuristic limit directly, the
loosened goods included; `translated_oracle_rejects_a_good_lowered_from_one` keeps the review's
examples.

The full run finds one solver defect: seed 13067305529229119429 at 0.3 ends at [0.267, 0, 0]
against t = 0.3 (the plain iteration gives t to rounding, and no good of a translated province
can fall below t). The third pass reaches t to rounding on the pieces below t; at that stop
`settle` takes the last pass's fixed point wherever it lies, [0.267, 0, 0], where every good
fits and the confirming passes return it. Holding that fixed point to 9.3e-10 of f, as the
cycles of two and three passes are, fixes the case and fails
`near_singular_cycle_keeps_its_limit`, whose real loop of gain 1 - 1e-15 needs the far fixed
point. Telling the two apart is a change to the solver, open pending a ruling
(`translated_collapse_settles_below_its_limit`, ignored, since renamed
`translated_collapse_stop_refuses_an_off_piece_fixed_point`; `translated_collapses_full` fails
on it).

**Off-piece fixed point (2026-10-30, task B3 fill round 4).** The two cases differ in where the
last pass's pieces hold, not in the gain. In the near-singular cycle (gain 1 - 1e-15) the pass
has the same pieces at f = [0.5, 1, 1] and at its fixed point [0.270, 0.540, 1]: the passes run
on them all the way, and the fixed point is their limit. In seed 13067305529229119429 at 0.3 the
fills stop at t to rounding on a pass whose good-0 step sits on a piece (user 2 short of good 0
above its stock threshold) that holds only at f: just below t that user's stock covers its need
and good 0 fits; goods 1 and 2 have a gain of 1 - 1e-16 and no constant, so the fixed point
takes them to 0 and good 0 to its constant, 0.267. Every good fits there, so the passes confirm
it, but no pass from f goes there.

Rule: the last pass's fixed point g beyond TAIL of f is taken only where g lies on that pass's
pieces: from g, each step that cut in the pass is on its piece (`on_piece`, the roles and the
producer's binding fill the jumps check) with its map's value at least 0, and each good the pass
leaves fits to 1e-12 of its flows (the maps' rounding, as in `follow`). The shortages are not
checked at g, where they vanish. A condition on one fill (a stock threshold, a covered user's
min) that holds at f and at g holds along any monotone fall from f to g, and the passes only
lower the fills, so the passes from f run on the pieces towards g, which is their limit. That is
first-order (corrected 2026-10-30, task B3 fill round 5, which earlier named the covered user's
min as the gap; the min is harmless under a monotone fall). The real gap has two parts. The
passes follow g + M^k (f - g), not the segment, except where f - g lies along one near-unit
direction: so it is at a rounding stop (the other components are below margin / (1 - lambda2)),
but not necessarily at the stall rule, where two slow directions can coexist. And the
comparisons between two fills (which fill binds a user or the producer, `OnStep` m >= x,
`On(k)` f_k <= x) are not kept by a monotone fall; they hold trivially at ties, as at a collapse
to 0, where every acceptance a review probed (10 in 1,000,000 random provinces) occurred. Within
TAIL a move is taken as before. It applies at a stop and at the stall rule alike. Where g is
refused, the period-1 move is partial: the goods whose g lies within TAIL of f still move there
(each chosen on its own), and the move is kept if the pass confirms it; only where none moves or the
confirmation fails does a stop try the two- and three-pass cycles, then take the fills as they
are; a stall is then reported. Seed 13067305529229119429 at 0.3 now stops at t to rounding; the
near-singular cycle keeps its limit.

**Pass cap.** A jump's confirming passes (up to PERIOD = 3) came after the cap check, so a call
could run 1,027 passes. The fallback now starts at 1,021 passes, and no call runs more than
1,024.

**Descent.** The step's bounded descent counts its fit evaluations (at most 8 + 1 + 64, asserted
in debug builds and in its regression test), and asserts in debug builds that the good's
producer does not take its own good, which the monotone fit rests on.

## Amendment (2026-10-30): a user's stock is a floor under the fill (defect fix)

**Defect.** The first amendment's rule set each user's output to cap_e min_k F_k, the common fill
applied to every user of k, including users whose own stock covers their need. That contradicts
the same amendment's split ("each user takes what its stock cannot cover", the stock "covering
the remainder"): a user cut to F_k cap_e by the fill draws less than its stock alone could
supply. On the scratch pack (v1 diagnosis, 2026-10-30) an Algerian enterprise with 2.2 months of
every local input in stock produced nothing once one local producer stopped, and the collapse
spread through the province. The rule as built was a defect against its own stated intent, so
this amendment fixes it without a new decision.

**Rule.** For user e of local good k, with need at capacity n_ek = a_ek cap_e and stock b_ek, let
s_ek = min(b_ek / n_ek, 1), the share of that need its stock covers. Then

    Y_e = cap_e min_{k : a_ek > 0} G_ek(F_k),    G_ek(F) = s_ek + (1 - s_ek) F   (1 at F = 1).

F_k is the share of each user's uncovered need at capacity, (n_ek - b_ek)^+, that k's output
serves: where k binds e, e's uncovered need is exactly F_k (n_ek - b_ek)^+. A user fully covered
by its stock is not cut by k (s = 1), and with no output of k at all a user runs on its stock
(G(0) = s). U_k, R_k, the fit and the split are unchanged; a user without stock (s = 0) has
G = F exactly, so a province where no user holds stock gives the same fills and outputs bit for
bit.

**Solver.** Each user's level G_ek is affine in F_k on [0, 1), so a step's U_k stays continuous,
nondecreasing and piecewise linear in its fill, a pass and a cycle stay affine maps on their
pieces, and the evaluated fit stays monotone in the fill (G is s plus a constant times F, then a
min with 1; the value at F = 1 is 1 exactly, so no unit in the last place is lost there). The
kinks change: a user k binds contributes (n - b) F_k with no stock threshold inside (0, 1); the
kink is where its level of k meets its smallest other level, F = (m - s) / (1 - s); a user whose
smallest other level m is at most s is covered whatever F_k is. Users bound on another good k'
contribute a_ek cap_e G_ek'(F_k') - b_ek, affine in F_k'.

**Uncovered need without cancelling.** Evaluated as coef_ek Y_e - b_ek, a user's uncovered need
cancels to the rounding of b where it runs mostly on its stock (a small fill: Y_e near cap_e
s_ek), so its relative error grows as eps s / ((1 - s) F); past about F = 1e-6 the maps' rounding
exceeds the 1e-12 the jumps allow, and on random provinces with stocks 133 of 100,000 stalled
(before the floor 7 stalled over five families of 1,000,000 each) and a fall to a limit near
1e-17 took 97 passes. The chain
therefore computes it from the levels: u_ek = coef_ek cap_e max(min_k' (G_ek' - s_ek), 0), with
G_ek - s_ek = (1 - s_ek) F_k for the good itself and (s_ek' - s_ek) + (1 - s_ek') F_k' for the
user's other goods, at most its need n_ek, and n_ek exactly for a user without stock. The split
draws u_ek where it drew max(n - b, 0). s is rounded down until coef (cap s) <= b as evaluated,
so that at F = 0 every uncovered need is exactly 0 and 0 always fits. With this no stocked
province of 200,000 per family stalls but one (scarce stocks, 792 passes, as before), and the
first family takes at most 14 passes.

**Kinks.** A floor is still a kink, though not where the stock threshold was: a user's stock comes
to cover it where its smallest other level falls to its floor (crossed once, as the fills only
fall). The chain bound 2 + 3 + 2K keeps K as it was, the floors inside (0, 1) in place of the
thresholds. Of 1,000,000 random provinces (200,000 per family), 13 exceed 2 + 3 + 2K, each within
it with the passes outside the accounting: 12 at the stall rule, 1 with a root rounded onto a kink.
The 10 of the 22 before the floor that cut a good by rounding every other pass, at its limit with
U = R where stocks covered needs at their thresholds, are gone: a user bound by a good draws
(1 - s) F of its need, which no stock zeroes along the fall. On the cycle fixture the chain binds
in 482 of 3,000 province ticks (483 before), the most 14 passes against a bound of at least 15
(province 10, tick 76: utilities and construction collapse to 0 across the floors of residues of
earlier collapses, 1e-52 to 1e-12 of their users' needs); 4 before.

**Tests.** `local::tests::stock_is_a_floor_under_the_fill` (a user with two months of every local
input runs at its cap when a local producer makes nothing; a half-stocked user at half; the
unstocked user not at all; nobody draws the good from output) and
`a_stocked_user_is_not_cut_by_the_fill` (the same in the fixture world, with services feeding every
sector: the stocked user at its cap, the other users of services at 0, the Leontief and
conservation invariants holding). The oracles of the fill tests (`outputs_at`, `uncovered_at`)
follow the rule. Test families that rested on the old rule changed, each with its reason in the
test's documentation:
- the translated collapses: a partial stock is a floor that gives the fall a constant, and a
  producer covered on its inputs is not cut, so the originals drop the producers' stocks and raise
  every other stock to full cover (`translated_original`); a good without a producer has fill 0
  whatever its stocks; and where a good's users need less than its producer supplies, balancing it
  at t with an extra user leaves a direction of gain exactly 1, a line of fixed points along which
  the stop may take any (seed 1997589725841679792 at 0.1 ended at [0.093, 0.013, 0.013], below t).
  Such provinces are not translatable. `translate` now moves the constant into the producers: a
  floor r / (1 + r), r = t (A - R) / R, under the local inputs it holds no stock of, and capacity
  (1 + r) cap, which leaves its needs beyond its stocks at cap F and adds R r to its good's
  supply. The family is smaller: about 0.4% of draws against 23%, so the sample takes 75,000 draws
  (969 cases, 546 held to every heuristic limit, 393 mutants rejected) and the full run 3,000,000
  (38,187 cases, 19,947 held, 14,277 mutants rejected, 553 accepted at goods whose own limit is
  rounding; 0 failures). The fills land within 1.08e-13 of t on the goods the oracle holds;
- the pinned seeds of `translated_collapses_stop_at_their_limit`,
  `translated_oracle_rejects_a_good_lowered_from_one` and
  `translated_collapse_stop_refuses_an_off_piece_fixed_point` are not translatable under the
  floor but for one. The first two take new cases from the sample (the longest, the furthest from
  t, the first with rejected mutants). The third is removed: no translated collapse of 3,000,000
  draws and no random province of 1,000,000 has a last pass whose fixed point lies off its pieces,
  so the rule stays without a regression case (`translated_family_leaves_out_a_neutral_good`
  records the exclusion above instead);
- the chain bound's pinned passes and exceedances are re-measured; the collapse and stall cases
  that drew stocks and no longer collapse or stall are replaced by draws that do, with the old
  ones named; the jump-ahead province (`a_jump_ahead_ends_after_the_last_whole_cycle_on_the_pieces`)
  takes its kink from a floor (5 passes, 4 before: the zero-supply good is cut in the first pass);
  the captured flat-piece province no longer has a flat piece (the step's root is exact to a few
  units), so its regression calls the descent from 1 directly.

**Consequences.** Outputs and draws move wherever a stocked user of a short local good is cut
below what its stock alone allows. On the fixture's golden runs the chain never binds, so no golden
moves; the steady state never rations, so the calibration and the GE reference do not change. No
parameter is changed.
