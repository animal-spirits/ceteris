# RFC 0364. Derivative-based border friction fit, seeded rounds, and the joint system deferred

Status: accepted
Date: 2026-11-21
Design sections: 15 (calibration), 17.1; econ SPEC, Calibration, stage 2
Release: 1.0 (P4)
Research: `reports/fit-speedup-research-2026-11-20.md` (#650, #651), item 5; spike note on #653; issue #664

## Summary

The research report ranked five speed-ups for the epsilon fit. Four are on main: the parallel
sweep (#652), the per-good Newton scaling (ADRs 0388 to 0391, 0394 to 0396, #656), the nest
hygiene (ADRs 0392, 0393, #661) and progress lines (#663). This RFC is the fifth: fewer
scalings per trial, where the report proposed one Newton system for nu and s_c.

The spike on #653 changed the picture. On the p2 fixture 94 to 96% of a trial is the scalings
of the nu fit (`border::fit_at_spending`, 80 calls at epsilon 0, 257 at epsilon 0.4); the data
state and `solve_scale` together are 3 to 6%. So the gain is in the nu fit, not in coupling s_c.
And at a fixed spending the nu fit is not a coupled system: goods are independent in the
scaling and groups are disjoint sets of goods, so a group's share moves with its own nu alone
(the comment in `fit_border_nu_seeded` says so, and Brent per group relies on it). It is n
independent monotone roots that share each scaling.

Three changes follow, all inside `border.rs`, none to the model or to what counts as solved:

1. Each group's root is found by Newton's method kept inside Brent's bracket, with the slope
   d share_k / d nu_k from one linear solve with the scaling's Jacobian (the operator the
   Newton scaling of ADR 0388 already applies). Brent to `NU_TOL` = 1e-9 takes 4 to 8 rounds;
   a safeguarded Newton takes 3 to 4.
2. The upper bracket ends are evaluated in one scaling for all groups, not one scaling per
   group, falling back to per group where that scaling fails.
3. Data-state rounds after the first start each group from the previous round's root (2 to 5
   rounds seen; the nu moves by 1e-3, then 1e-6, then 1e-9 between rounds).

The full coupled system (nu, s_c and their corners as one complementarity problem) is deferred
(D1): it is where the corner logic lives, its Jacobian needs the derivative of the whole data
state, RFC 0355 may change the system, and the spike puts at most 6% of a trial behind it.

Nothing here is measured. Evaluations per `fit_at_spending` today: 1 at nu 0, one per fitted
group at the upper end (4 to 5), 4 to 8 Brent rounds, 1 final: 13 to 17 (`Fit::evaluations`,
17 measured cold in `reports/warm-start-spike-2026-11-16.md`).

## Decisions

### D1. How much of the nest is replaced?

Options:

- A. The nu fit only: safeguarded Newton per group at a fixed spending (D2, D3), one scaling for
  the upper ends (D4), seeded rounds (D5). The scale passes, `solve_scale` (Brent, ADR 0376),
  the data-state rounds, `is_pinned`, clamps, held groups and typed infeasibility (ADR 0387)
  stay as they are. The coupled system waits for RFC 0355's structure decision and for a
  full-pack profile showing that `solve_scale` and the data state are worth it.
- B. One semismooth Newton system for (nu, s_c) with their corners (De Luca, Facchinei and
  Kanzow 1996), per trial. Replaces levels 2 to 4 of the nest. Needs the derivative of the data
  state (rebates, PIGL weights, the income fixed point), re-derives every corner case, and
  supersedes ADRs 0092 and 0376.
- C. Neither: stop at what is merged.

Recommendation: A, because the measured share says the time is in the nu fit, the nu fit
separates by group so one-dimensional bracketed Newton is the exact method for it, and every
corner rule stays untouched. B buys at most the 3 to 6% share on the fixture for the largest
correctness risk in the fit, on a system RFC 0355 may redefine.

Decision: A, Antonio, 2026-11-21 (question card, fit speed-up research thread: every recommended option).
Row: ADR 0398.

### D2. Where does the slope come from?

share_k is a function of ln w of the goods in group k and of nu_k directly (through the
crossing factors). With F(ln w; nu) = 0 the scaling's residual, d ln w / d nu_k = -J^-1
dF/d nu_k for each good of the group, and the slope is the chain rule through
`cross_border_parts`.

Options:

- A. Implicit differentiation: one GMRES solve per good of the group per evaluation, with the
  Jacobian product, restart and forcing constants of ADRs 0389 and 0394, to a relative residual
  of 1e-6. dF/d nu_k and the direct term are analytic. About the cost of one Newton step of the
  scaling.
- B. Central finite differences: two more scalings per evaluation. Triples the scalings, and
  the difference of two solves at 1e-13 is noise at any step small enough to be accurate.
- C. No slope: keep a derivative-free method (Brent stays, or ITP, Oliveira and Takahashi
  2020). No gain from this RFC beyond D4 and D5.

Recommendation: A, because the operator exists and is tested, the slope is exact up to the
linear solve, and an inexact slope cannot move the root: it only changes how many evaluations
the bracketed iteration takes (D3). A is checked against B in a test, not replaced by it.

Decision: A, Antonio, 2026-11-21 (question card, fit speed-up research thread: every recommended option).
Row: ADR 0399.

### D3. How is the Newton iteration kept safe, and when does it stop?

Options:

- A. Bracketed Newton (the `rtsafe` scheme, Press et al., Numerical Recipes 9.4). Each group
  keeps Brent's bracket [a, b] with f(a) >= 0 >= f(b), updated by every evaluation. The next
  point is the Newton point if it lies strictly inside the bracket and the last step at least
  halved the one before; otherwise the bisection point. A slope that is not finite or not
  negative (f falls in nu) gives a bisection step. Stop when the bracket is at most `NU_TOL`
  wide, or the Newton step is at most `NU_TOL / 10` in absolute value and the evaluation at the
  stepped point has been made; the root reported is the last evaluated point. `MAX_ROUNDS`
  unchanged. Worst case is bisection: 34 rounds from [0, 10] to 1e-9.
- B. Pure Newton from the bracket's midpoint, falling back to Brent on failure. Fewer rules,
  but a failure costs the Newton rounds plus the whole Brent run.
- C. Brent with the slope used only for the first step.

Recommendation: A, because it can never leave the bracket, never does worse than bisection, and
its stop is a statement about nu in the same units as today's.

Decision: A, Antonio, 2026-11-21 (question card, fit speed-up research thread: every recommended option).
Row: ADR 0400.

### D4. Are the upper bracket ends evaluated together?

Today each fitted group gets its own scaling at (nu_k = `NU_MAX`, others 0), halving on a
scaling failure.

Options:

- A. One scaling at `NU_MAX` for every fitted group. If it fails, fall back to today's
  per-group loop with its halvings, unchanged. Saves 3 to 4 scalings per `fit_at_spending`.
  A group's share at its own nu is the same function either way; the bits differ within the
  scaling's tolerance, because the sweeps before the Newton phase mix goods jointly.
- B. Keep one scaling per group.

Recommendation: A, because the separation it relies on is the one the Brent rounds already
rely on in every shared evaluation.

Decision: A, Antonio, 2026-11-21 (question card, fit speed-up research thread: every recommended option).
Row: ADR 0401.

### D5. Do later data-state rounds start from the previous round's root?

`fit_border_nu_seeded` runs up to `STATE_ROUNDS` rounds; each is a full `fit_at_spending` from
the cold bracket. ADR 0373 allows a seeded bracket only as non-authoritative output, because
its seed came from outside the call (another epsilon, an earlier run).

Options:

- A. Within one `fit_border_nu` call, round r > 1 starts each group's iteration at round
  r - 1's root. The nu 0 scaling is still made (it is every evaluation's start, and it decides
  the clamp). A group that was interior takes its first Newton step from the seed with the
  bracket [0, `NU_MAX`] and unknown upper value; if the first two evaluations do not bracket
  the root within the seed's 2% neighbourhood (the rule of ADR 0373), the group runs cold. A
  group that was clamped or held runs cold. The call stays a pure function of its arguments,
  so ADR 0097 holds, and the result is authoritative. Amends ADR 0373's scope: a seed from
  inside the call is allowed in the authoritative fit; a seed from outside is still not.
- B. A plus seeding across the scale passes of one trial (`trial` calls `fit_border_nu` once
  per pass; 4 calls at epsilon 2 in the 11-18 log). Still a function of the trial's arguments.
  More gain, but s_c moves between passes by up to 0.36, so seeds are often far and the
  fallback fires.
- C. No seeding.

Recommendation: A, because rounds 2 to 5 move nu by 1e-3 or less, so the seed is inside the
neighbourhood where the warm-start spike measured the root within 2e-10 of the cold fit, and
the gain is the largest of this RFC. B only if the spike (D6) shows the fallback rarely fires.

Decision: A, Antonio, 2026-11-21 (question card, fit speed-up research thread: every recommended option).
Row: ADR 0402.

### D6. How is the work sequenced under the freeze?

Options:

- A. One-day spike on an unmerged branch, cloud, p2 fixture, with the go numbers below. Then
  one PR for D2 to D5 with the acceptance list. The fit path changes, so the PR carries a
  switch-on behaviour test (calibration rule of 2026-11-16). No kernel is added or wired.
- B. Straight to the PR.

Recommendation: A, because RFC 0361's spike overturned three constants fixed before any code
ran (RFC 0362), and the development laws ask for a go/no-go number first.

Decision: A, Antonio, 2026-11-21 (question card, fit speed-up research thread: every recommended option).
Row: none (sequencing; no spec row).

### D7. Does the epsilon search change again?

Options:

- A. No. ADR 0384's grid stands and waits for its 16 vCPU spike; until then Brent on epsilon
  stays. A slope d f / d epsilon would need the coupled Jacobian that D1 defers.
- B. Replace Brent on epsilon with ITP now. Perhaps 10 to 14 trials down to 7 to 9 on the Mac
  (estimate), for a reviewed move of `epsilon.json` that the grid would then move again.

Recommendation: A, because one reviewed move is better than two and the larger lever on the
Mac is the per-trial time.

Decision: A, Antonio, 2026-11-21 (question card, fit speed-up research thread: every recommended option).
Row: none (ADR 0384 stands).

## Tolerance and determinism contract

- f64 throughout; the contract of RFC 0361 holds unchanged for every scaling.
- The stop of the scaling, `NU_TOL`, `STATE_TOL`, `STATE_SHARE_TOL`, `NU_MAX`, `MAX_HALVINGS`,
  `MAX_ROUNDS` and `STATE_ROUNDS` are unchanged. The new constants (the slope solve's relative
  residual 1e-6, the step-halving rule, `NU_TOL / 10`, the 2% seed neighbourhood) are fixed,
  never derived from host or thread count.
- Groups iterate in index order; a round's shared evaluation is one scaling as today. The slope
  solves run per good on the current pool into index order, with no float reduction across
  goods or threads.
- Every start is a function of the call's arguments. `Fit`, `Trial` and `EpsilonFit` are
  bit-identical across runs and thread counts; tested.

## How outputs move, and how that is checked

Both methods bracket the same root of the same function to the same width, so nu moves by at
most `NU_TOL`. Pre-registered bounds, new fit against main's on the same inputs:

- `fit_at_spending`: nu within 2e-9 per group; shares within 1e-9; `clamped`, held groups and
  typed errors identical in kind, naming the same group.
- `fit_at_epsilon` (CI fixture): nu within 1e-8, s_c within 1e-5, d_0 within 1e-8 relative, the
  pooled coefficient within 1e-6.
- Slope: the implicit slope within 1e-4 relative of a central difference on the fixture's
  interior groups.
- Any golden or committed `epsilon.json` that moves is re-blessed in the PR with a
  `### BEHAVIOUR:` CHANGELOG line, after the bounds pass.

## Spike (one day, cloud, not merged)

p2 fixture, `fit_at_epsilon` at epsilon 0 and 0.4, release, the timers of the #653 spike branch.
Record per `fit_at_spending`: evaluations, slope solves and their Jacobian products, bisection
steps, seeded rounds that fell back; and trial wall.

Go if all hold:

1. Scalings per trial fall by at least 2x at both epsilons.
2. Trial wall falls by at least 1.5x at both epsilons, slope solves included.
3. Every bound of the previous section holds.
4. Bisection steps are under 20% of iteration steps.

Otherwise record the negative result on #653 and in this RFC, keep whichever of D4 and D5 pass
on their own, and stop.

### Spike result (2026-11-21): no-go

Run on an unmerged branch from main at 54fc991 (patch and log kept outside the repository), p2
fixture, release, 4 cores, the trial set-up of the #653 spike (epsilon 0 with s_c 1; epsilon 0.4
with class 1 at s_c 0.5). D2 to D5 implemented as written, with the slope of the #664 comment:
dF/d nu_k from one residual evaluation at nu + 1e-6 at the solved ln w, one GMRES solve per good
(relative residual 1e-6), the share at the moved point through `cross_border_parts`. Walls are
the median of three runs; counts are deterministic.

| | epsilon 0: main | D2 to D5 | D2 to D4 | epsilon 0.4: main | D2 to D5 | D2 to D4 |
|---|---|---|---|---|---|---|
| scalings per trial (all callers) | 179 | 164 | 171 | 799 | 693 | 698 |
| nu fit evaluations (`Fit::evaluations`) | 76 | 61 | 68 | 79 | 44 | 46 |
| trial wall, s | 0.53 | 0.96 | 1.47 | 3.73 | 2.58 | 3.31 |
| Newton / bisection steps | | 44 / 18 | 54 / 24 | | 160 / 0 | 224 / 0 |
| slope solves (products) | | 82 (875) | 87 (975) | | 223 (2,173) | 245 (2,453) |
| seeded groups / fell back | | 9 / 2 | | | 32 / 9 | |

Go numbers:

1. Scalings per trial fall 1.09x and 1.15x (need 2x): not met.
2. Trial wall: 0.56x (slower) and 1.44x (need 1.5x): not met.
3. Bounds. `fit_at_spending`: nu within 1.6e-13 and 1.1e-10 (2e-9), shares within 1.2e-13 and
   4.8e-11 (1e-9), `clamped` identical. `fit_at_epsilon`: nu within 9e-15 and 5e-11 (1e-8), s_c
   within 1.3e-14 (1e-5), d_0 identical, pooled within 4e-16 and 1.4e-11 (1e-6). Slope against a
   central difference (step 1e-3) on interior groups: 2e-6 to 7e-5 relative (1e-4). One miss:
   at epsilon 0 two groups whose target equals their share at nu 0 to rounding end at nu
   1.7e-16 and 1.4e-16, interior, where main clamps them at 0, so `clamped` differs in the
   trial. A tie at the corner, but the bound asks for identical flags: not met.
4. Bisection steps: 29% at epsilon 0 (the two tie groups, root at the bracket's end), 0% at
   0.4 (need under 20%): not met at epsilon 0.

Why: the RFC's premise is gone. It took the #653 measurement (94 to 96% of a trial in nu fit
scalings), made before the per-good Newton scaling (#656). With #656 on main a trial is 0.5 s
and 3.7 s on the fixture, not 13.5 s and 36.7 s, and the nu fit is 76 of 179 scalings at
epsilon 0: the data state's scalings now outnumber it. Brent already ends in 4 to 6 rounds per
`fit_at_spending`; Newton saves 2 to 10 evaluations there, and each slope costs an
`at_border_nu` (a `reprice`), two `Model::new` and a GMRES solve, which at epsilon 0 costs more
than the scalings it saves. D5 alone (D2 to D5 against D2 to D4) saves 7 and 5 evaluations per
trial; D4 saves at most one scaling per fitted group per `fit_at_spending`. Neither alone can
reach a 2x fall in scalings when all four together give 1.15x, so none is kept.

Outcome: per the rule above, nothing is merged; `border.rs` keeps Brent, per-group upper ends
and cold rounds. ADRs 0398 to 0402 stay accepted but unimplemented; their supersession, if
wanted, is a new RFC. The question for the per-trial time is now the full-pack split (#653,
Mac), which decides whether the data state (D1 B) is worth an RFC.

## Acceptance (the PR)

1. Switch-on behaviour test through `fit_border_nu`: on the fixture, `Fit::evaluations` is at
   most half of main's and the nu are within the bound. Fails on main.
2. Unit tests of the bracketed iteration on synthetic f: monotone, root at an end, flat f (the
   coarse resolution `NU_TOL` documents), a wrong-signed and a non-finite slope (bisection),
   convergence never outside the bracket.
3. Slope against central differences (bound above).
4. Corner cases unchanged: clamped at nu 0, held at the upper end (`TooLittleCost`), short at
   nu 0 (`Infeasible`), upper-end scaling failure with halving; existing tests green unedited.
5. Bit-identity of `Fit` and `Trial` in pools of 1, 2 and 8 threads and across two runs.
6. `fit_border_nu_seeded` with an outside seed stays non-authoritative; its tests unchanged.
7. No tolerance loosened; epsilon-off goldens unchanged.

## Rollback

Revert the PR: `border.rs` returns to Brent, one scaling per upper end and cold rounds. No
format, id or hash domain changes.

## Expected gain (estimates, nothing measured)

Per `fit_at_spending`: 13 to 17 scalings down to about 6 to 8 (D2 to D4), each with one slope
solve. Per `fit_border_nu` of 4 rounds: about 60 down to about 16 with D5. The nu fit is 94 to
96% of a trial on the fixture, so 2x to 3.5x per trial there. The full-pack split is still
unmeasured (#653); if `solve_scale` or the data state dominate there, the gain is smaller and
D1 B is the next question.

## Spec changes

Written in this PR (ADRs 0398 to 0402):

- 17: register entry P183 (the nu fit's bracketed Newton iteration, slope solve, shared upper
  ends and seeded rounds).
- econ SPEC, Calibration, border friction fit: the method, marked not yet in code until the PR
  of D6 lands (#664). ADR 0092's Brent and ADR 0373's scope are amended by these rows; their
  record files are not rewritten.

## References

- Press, W. H. et al. (2007). Numerical Recipes, 3rd ed., section 9.4 (Newton with bracketing).
- Brent, R. P. (1973). Algorithms for Minimization without Derivatives.
- Oliveira, I. F. D. and Takahashi, R. H. C. (2020). An enhancement of the bisection method
  average performance preserving minmax optimality. ACM Trans. Math. Softw. 47.
- De Luca, T., Facchinei, F. and Kanzow, C. (1996). A semismooth equation approach to the
  solution of nonlinear complementarity problems. Math. Program. 75.
- Su, C.-L. and Judd, K. L. (2012). Constrained optimization approaches to estimation of
  structural models. Econometrica 80.
- Blondel, M. et al. (2022). Efficient and modular implicit differentiation. NeurIPS.
