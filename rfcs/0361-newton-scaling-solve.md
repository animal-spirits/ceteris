# RFC 0361. Per-good Newton solve for the stage 2 scaling, and nest hygiene in the epsilon fit

Status: proposed
Date: 2026-11-20
Design sections: 15 (calibration), 17.1, 19.4; econ SPEC, Calibration, stage 2; ADRs 0089,
0097, 0272, 0376, 0384 to 0387
Release: 1.0 (P4)
Research: `reports/fit-speedup-research-2026-11-20.md` (#650, #651), candidates C, D and E;
parallel sweep #652

## Summary

The epsilon fit spends its time in nested equilibrium solves, not in its root finders. Per trial
it runs about 55 to 75 data-state fixed points and 175 to 200 further scalings, and a scaling
takes 7 to 4,027 sweeps: the stage 2 scaling (`steady::sinkhorn_with`) is a linearly convergent
iteration on a problem with a contraction factor near 1 (Brazil's 48 coal origins, 0.99908 a
sweep, econ SPEC stage 2), run to `SINKHORN_TOL` = 1e-13 hundreds of times. #652 runs the goods
of each sweep in parallel, bit-identical; it does not change the number of sweeps.

This RFC proposes two changes, each behind the gates below:

1. Newton (D1 to D4). When a scaling has not met its stop rule after a fixed number of sweeps,
   each good that is still out solves the scaling's own residual by Newton's method, with a
   matrix-free Krylov linear solve and the existing sweep as fallback. The stop rule, the
   residual and the returned level are unchanged; only the way the iterate reaches them is new.
   Scalings that converge within the sweep phase are bit-identical to main; the rest move within
   the stop rule's tolerance.
2. Nest hygiene (D5, D6). `solve_scale` starts every evaluation's scalings from one reference,
   its scale-0 evaluation, instead of cold; `data_state_spending` stops its non-final scalings
   at a tolerance tied to the pass change, as `calibrate_from` already does, and returns only
   from a pass whose scaling ran at `SINKHORN_TOL`.

Every speed figure here is an estimate; nothing has been run. The kernel freeze (2026-11-13)
holds: this RFC is the gate, and no solver or fit code is written before Antonio decides D7.

## The scaling as a root problem

Per traded good g, over its active origins o (n_g of them), with x = ln w the log weights, the
sweep of `sinkhorn_with` computes, at the shifted weights w_o = exp(x_o - max x):

- Phi_d = sum_o G_od w_o (`origin_sums_good`, gross factors), b_d = E_d / Phi_d;
- R_o = sum_d N_od b_d (`destination_sums_good`, net factors), got_o = w_o R_o;
- r = sum_o got_o / sum_o V_o;
- the gap F_o(x) = ln got_o - ln(r V_o), and the map T(x)_o = ln(r V_o) - ln R_o.

The scaling is solved when max_o |F_o| <= tol, or every good is within its rounding
(`summation_depth`) and the mixing has stalled. F is the residual the stop rule already reads
(`GoodSweep::gap`), and F(x) = (x - max x) - T(x): the sweep is the fixed-point iteration on F,
and Newton is the root solve of the same F.

F is invariant to x + c 1 (one level per good), and its entries satisfy
sum_o exp(F_o) r V_o = r sum_o V_o, so one equation is implied by the others. Pinning one
origin's x and dropping its equation leaves a square system of n_g - 1 unknowns, nonsingular
when the good's origin-destination support is connected (Sinkhorn uniqueness up to scale,
`sinkhorn_is_unique_up_to_scale`).

The Jacobian is applied without forming it. For a direction v (v at the pinned origin 0):

- u_d = sum_o G_od w_o v_o / Phi_d (one `origin_sums_good` on w * v, divided by Phi);
- dR_o = -sum_d N_od b_d u_d (one `destination_sums_good` on b * u);
- dgot_o = got_o (v_o + dR_o / R_o), dln r = sum_o dgot_o / sum_o got_o;
- (J v)_o = v_o + dR_o / R_o - dln r.

One product costs one sweep of one good. The derivatives are analytic, never finite
differences, so the product is exact to rounding.

## Decisions

### D1. Does a Newton phase enter the scaling, and in what form?

Options:

- A. Hybrid, per good: the existing sweep (Joint or PerGood Anderson, unchanged) runs for
  `NEWTON_AFTER` sweeps; if the scaling has not stopped, each good still out of its stop rule
  runs Newton from the sweep's last point (Tang et al. 2024's warm-up pattern), goods in
  parallel into fixed slots and combined in good order (#652). Goods already within their stop
  rule keep their sweep-phase weights.
- B. Newton from the first sweep for every good. Simpler control flow, but every scaling moves
  within tolerance, including the many that converge in 7 to 50 sweeps today, and the cheap
  scalings pay a Krylov setup.
- C. Per-good stop and per-good Anderson only (report candidate D; `Accel::PerGood` exists).
  Small change; removes the "every good sweeps until the slowest converges" waste but not the
  slow mode, whose rate stays 0.99908 a sweep.

Recommendation: A, because it removes the slow mode at its cause (Newton's rate does not depend
on the second eigenvalue) while leaving every scaling that already converges fast bit-identical,
and the sweep phase keeps globalising the start.

Decision: pending.
Row: none yet.

### D2. How is the Newton step solved?

Options:

- A. Matrix-free GMRES on the reduced system, restart 30, at most 10 restarts per step
  (300 products), modified Gram-Schmidt, Givens rotations, serial, operations in index order.
  Inexact Newton forcing term eta_k = clamp(||F_k||_inf, 1e-8, 1e-1) relative (Eisenstat and
  Walker 1996). Each product is one good-sweep; a Krylov method resolves one isolated slow
  eigenvalue in a few iterations, which is the near-closed block's case.
- B. Dense LU with partial pivoting (ties to the lowest row) on the reduced Jacobian, assembled
  column by column: n_g - 1 products per Newton step, so about as many good-sweeps as the slow
  scaling needs today, plus an O(n_g^3) factorisation.
- C. Knight and Ruiz (2013) conjugate gradients. Needs a symmetric formulation; the tariff makes
  G and N differ (gross against net), so J is not symmetric here.

Recommendation: A, because its cost is products the sweep already knows how to compute, it
needs no n_g by n_g storage, and it is deterministic as written. B stays as the test oracle on
fixtures small enough to factor.

Decision: pending.
Row: none yet.

### D3. Step control, stop and constants

Options:

- A. Newton step delta from D2; backtracking alpha in 1, 1/2, ..., 2^-12, accepting the first
  with a finite F and ||F(x + alpha delta)||_inf <= (1 - 1e-4 alpha) ||F(x)||_inf. A good
  stops when its gap is at most tol, or it is within its rounding and a step gave no decrease.
  `NEWTON_AFTER` = 50 sweeps, `NEWTON_MAX` = 20 steps per good. After all goods stop, one full
  sweep evaluation (the existing `step`) gives the returned residual and r and applies the
  existing stop rule unchanged; the existing level shift (largest ln w of each good at 0) is
  applied. The pinned origin is the lowest-index origin at the good's largest x when Newton
  starts.
- B. As A with `NEWTON_AFTER` chosen by the spike from the pre-registered set {20, 50, 100}: the
  value with the least total wall-clock on the spike's scalings, ties to the larger (more
  scalings stay bit-identical). Fixed before PR 2 and recorded here; not changed after seeing
  fit results without an RFC.

Recommendation: B, because 50 is a guess and the switch point trades bit-identical cheap
scalings against Newton's reach; fixing the rule in advance keeps it inside the calibration
rule.

Decision: pending.
Row: none yet.

### D4. What does a Newton failure do?

A good's Newton fails on: a GMRES solve that does not reach eta_k in 300 products (a singular or
near-singular reduced Jacobian), a line search without an accepted step while the good is not
within its rounding, a non-finite value, or `NEWTON_MAX` steps without stopping. The final
evaluation of D3 can also fail the stop rule.

Options:

- A. Any failure discards every good's Newton result and resumes the sweep loop exactly where
  the sweep phase left it (iterate, Anderson histories, stall counters, sweep count). The call
  then returns what main returns, bit for bit, including its error. `Scaling` records
  `newton_steps`, `products` and the first failure (good, reason, gap at that point) for the
  log; the stop rule and error kinds are unchanged.
- B. Keep the goods whose Newton succeeded and resume the sweep only for the rest. Less wasted
  work, but the result is a mix no single method produces, and the fallback is no longer the
  oracle.

Recommendation: A, because the fallback is then main's own result, which makes it the rollback
inside every call and keeps the failure surface exactly main's.

Decision: pending.
Row: none yet.

### D5. Does `solve_scale` start its scalings from a reference?

Today every evaluation f(s) of `solve_scale` (ADR 0376) runs `data_state_parts`, whose data
state starts its first scaling cold and whose final scaling (`solve_at`) starts cold
(`start: None`).

Options:

- A. The evaluation at s = 0 runs cold, as today, and keeps two log-weight matrices: its data
  state's last scaling and its final scaling. Every later evaluation of the same call (s = 1 and
  Brent's points) starts its data state's first scaling and its final scaling from those. An
  evaluation is then a function of s and the call's arguments alone, not of the search path:
  the pattern `fit_at_spending` already uses from nu 0 (ADR 0097 holds; nothing carries across
  calls or trials). `data_state_spending` takes an optional start and returns its last log
  weights.
- B. Start each evaluation from the previous one. Fewer sweeps, but the result depends on Brent's
  path. Rejected under ADR 0097 and the 2026-11-16 warm-start no-go.
- C. Keep cold starts.

Recommendation: A, because it is the documented path-independent pattern and costs one cold
evaluation per call instead of 10 to 14.

Decision: pending.
Row: none yet.

### D6. How tightly does the data state run its scalings?

Today `data_state_spending` runs every pass's scaling to `SINKHORN_TOL`, while `calibrate_from`
uses tol_k = clamp(`INNER_PER_OUTER` * change_{k-1}, `SINKHORN_TOL`, `INNER_TOL_MAX`).

Options:

- A. Pass 1 at `INNER_TOL_MAX` (1e-6), later passes by `calibrate_from`'s rule through one shared
  function. The data state returns only from a pass whose change is at most `PASS_TOL` and whose
  scaling ran at `SINKHORN_TOL`; otherwise it runs one more pass. The returned spending is
  therefore settled to the same tolerances as today (Eisenstat and Walker 1996; Dube, Fox and Su
  2012 on why the last solve must be tight).
- B. As A, pass 1 also at `SINKHORN_TOL`, exactly `calibrate_from`'s rule.
- C. Keep every scaling at `SINKHORN_TOL`.

Recommendation: A, because pass 1 starts farthest from the fixed point and is the costliest pass
to run tight, and the termination condition already guarantees a tight last pass. Anderson on
the passes is on PR #600 and not part of this RFC.

Decision: pending.
Row: none yet.

### D7. How is the work sequenced under the freeze?

Options:

- A. Spike first (below, not merged); then PR 1 nest hygiene (D5, D6); then PR 2 Newton (D1 to
  D4), each with its gates. PRs 1 and 2 start only on Antonio's go under the freeze, written on
  #653.
- B. Spike, then one PR with everything.
- C. Defer all of it until the flagship passes its bands; the spike may still run.

Recommendation: A, because the hygiene items are small and independent of Newton, each PR moves
fit outputs once and needs its own reviewed golden move, and a failing spike stops the Newton
work without blocking the hygiene.

Decision: pending.
Row: none yet.

## Tolerance and determinism contract

- f64 throughout. No f32, no fast-math, no implicit FMA; `exp` and `ln` as the sweep uses them
  (offline crate, rust.md).
- The stop rule is unchanged: max |F| <= tol (`SINKHORN_TOL` or the inner tolerance passed in),
  or every good within `SINKHORN_TOL + eps (big + summation_depth + 4.5)`. Newton changes how the
  iterate gets there, never what counts as solved.
- Each good's Newton, GMRES and line search run serially in a fixed operation order. Goods run on
  the current rayon pool and are collected into index order; no float reduction crosses goods or
  threads (rust.md; rayon `sum`/`reduce`/`fold` stay banned).
- `NEWTON_AFTER`, `NEWTON_MAX`, the restart, the restart cap, the forcing bounds and the line
  search constants are fixed constants, never derived from the host or thread count.
- Every start is a function of the call's arguments (D5): no state from an earlier call, trial or
  run.
- Hence: `Scaling`, the data state, `Trial` and `EpsilonFit` are bit-identical across thread
  counts and across runs. That is tested, not assumed (acceptance 9).

## How outputs move, and how that is checked

Two scalings that both meet the stop rule differ by at most about 2 kappa tol in ln w, where
kappa is the reduced Jacobian's condition number; for the Brazil block 1 / (1 - 0.99908) gives
kappa near 1.1e3, so about 2e-10 (estimate). Downstream quantities are themselves solved to
tolerances well above that (nu 1e-9, s_c `SCALE_TOL` 1e-6, epsilon `EPSILON_TOL` 1e-3 on the
grid of ADR 0385).

Pre-registered comparison bounds, Newton against the sweep-only oracle on the same inputs:

- per good, after the level shift: max |Delta ln w| <= 1e-9, |Delta r| / r <= 1e-12, both
  residuals within the stop rule;
- data state (D5, D6 against main): spending and sales max relative difference <= 1e-10;
- fit at one epsilon (`fit_at_epsilon`, CI fixture): nu within 1e-8, s_c within 1e-5, d_0 within
  1e-8 relative, typed errors identical in kind and naming the same class or group;
- epsilon fit: the same reported grid point; if the point differs, the two candidates are
  adjacent grid points and the RFC records both |f| values before the golden is re-blessed.

Any golden or committed `epsilon.json` that moves is re-blessed in the PR that moves it, with a
`### BEHAVIOUR:` CHANGELOG line, only after the comparison above passes. Goldens that do not run
the calibration do not move.

## Flat or unbalanceable scalings

Two cases fail the stop rule today, and Newton must not hide either.

1. Rounding plateau. The gap stalls above `tol` but above the `within` bound: the 11-19 refit
   stopped on good 2 at about 4e-13 against 1e-13. F is evaluated by the same sums, so Newton has
   the same floor: steps stop decreasing ||F||, the line search exhausts while the good is not
   within, and D4 resumes the sweep, which ends exactly as main does. Newton adds only a
   diagnosis (the good and the gap where descent stopped). What the stop rule should do on that
   plateau is the open decision on that refit; this RFC does not touch it, and a refit on
   da75013 may change what is needed.
2. No balancing exists. If a good's support splits into blocks whose supply and spending cannot
   both match with one r per good, the reduced Jacobian is singular. GMRES does not reach its
   forcing term within the product cap, D4 falls back, and main's error results. A structural
   infeasibility stays an error for the model-structure decisions (RFC 0355, ADR 0387), never a
   clamp.

Cost of a fallback: at most `NEWTON_MAX` steps of at most 300 products per good on top of main's
cost. The spike reports how often fallback happens.

## Spike (one day, cloud, not merged)

On a branch from main, results written into this RFC:

1. Instrument (report candidate A): wall-clock stamps on progress lines; counters for scalings,
   sweeps per scaling per good, data-state passes, and time in the sweep, `Setup::settle`,
   `demand_weights` and `Model::new`.
2. Run `fit_at_epsilon` at epsilon 0 on the CI small pack. Dump the inputs of its slowest
   scaling. If the small pack has no near-closed block (sweep under 500), one scaling dumped
   from the full pack on the Mac is needed (about a minute; a Mac launch needs Antonio's message
   in that thread).
3. Implement D1 to D4 on the branch; compare against the sweep and the dense LU oracle; apply
   D3 B's selection rule.
4. Implement D5 and D6 on the branch; rerun step 2.

Go for Newton (PR 2), all of:

- on a scaling where the sweep needs more than 500 sweeps: Newton meets the stop rule in at
  most 20 steps and at most 2,000 products in all, and its wall-clock is at most one fifth of
  the sweep's;
- the comparison bounds above hold on every scaling of step 2;
- `fit_at_epsilon` at epsilon 0 is not slower than main on the small pack (at most 1.0x);
- no fallback on the step 2 scalings other than the cases of the section above.

Go for the hygiene (PR 1): the data-state and `solve_scale` part of step 2's wall-clock falls by
at least 1.5x, and the comparison bounds hold.

No-go on either: record the measured numbers here as a negative result and drop that part.

## Acceptance

PR 1 (D5, D6):

1. Switch-on behaviour test, fails on main: through `fit_at_epsilon` on the CI fixture, the
   `solve_scale` evaluations after the first report a start from the reference (a counter on
   the scaling, or sweeps strictly fewer than a cold start on the same input), and the data
   state's non-final passes report tolerances above `SINKHORN_TOL`.
2. Path independence: an evaluation of f(s) gives the same bits whether it runs first or after
   other evaluations of the same call.
3. The returned data state's last pass ran at `SINKHORN_TOL` (asserted in a test).
4. Comparison bounds above; existing tests unchanged and green; no tolerance loosened.

PR 2 (D1 to D4):

5. Switch-on behaviour test, fails on main: a synthetic good with two blocks coupled at 1e-3,
   where the sweep needs more than 500 sweeps; the scaling returns with `newton_steps > 0`,
   sweeps at most `NEWTON_AFTER` + 1, and matches the sweep-only oracle within the bounds.
6. Bit-identity below the switch: every existing scaling test that converges within
   `NEWTON_AFTER` sweeps returns the same bits as main.
7. Jacobian product against the dense Jacobian of a small fixture (dense oracle by forward
   differences in the test only, tolerance 1e-7 relative) and GMRES against dense LU.
8. Fallback: an injected singular good (two disconnected blocks) and a rounding plateau each fall
   back and return main's result and error bit for bit.
9. Determinism: `Scaling`, `data_state_spending`, `Trial` and `EpsilonFit` bit-identical in pools
   of 1, 2 and 8 threads and across two runs.
10. Measured on the spike's inputs: the go numbers above hold in the merged code. Otherwise revert
    PR 2 and record the negative result.

Both PRs: fmt, clippy, `scripts/check_clippy_bans.sh`, `cargo test --workspace`, `ceteris-rows
check`; goldens per the section above; `spec-reviewer` on the econ SPEC.

## Rollback

- Newton: a field `Limits::newton` (default on after PR 2; off runs the sweep alone, which is
  main's code path). Rollback is setting the default off: every scaling is then bit-identical to
  main, and the re-blessed goldens and `epsilon.json` revert with the commit that re-blessed them.
  The off path is also the test oracle, so it is never dead code. It is reviewed at each phase
  gate under ADR 0354.
- Hygiene: a revert of PR 1 restores main's starts and tolerances bit for bit; no format or cache
  key changes (`TrialCache` keys on epsilon bits and its entries stay valid in both directions,
  differing only within tolerance).
- No file format, hash domain or public CLI changes. `Scaling` gains fields; nothing persists it.

## Expected gain (estimates, nothing measured)

- Slow scalings (300 to 4,000 sweeps): 5 to 20x fewer good-sweeps, from 5 to 10 Newton steps of
  20 to 60 products each.
- Scalings under `NEWTON_AFTER` sweeps: unchanged.
- D5: one cold evaluation per `solve_scale` call instead of 10 to 14. D6: early data-state passes
  stop at 1e-6 instead of 1e-13. Together 1.5 to 3x on levels 3b and 5 of the report.
- Stacked with #652 and the RFC 0360 grid, the report's 10 to 30x per trial; the unknown share of
  non-scaling time (`settle`, RAS, `Model::new`) caps it, which the spike measures.

## Spec changes (on acceptance)

- econ SPEC, Calibration, stage 2: the Newton phase, its switch point, failure and fallback; the
  stop rule text unchanged.
- econ SPEC, Calibration, data state: the inner tolerance rule and the tight last pass; the
  reference start of `solve_scale`.
- 17: register entries for `NEWTON_AFTER` and `NEWTON_MAX` (solver constants, free numerical);
  codes from `next-code P` in that PR.

## References

- Knight, P. A. and Ruiz, D. (2013). A fast algorithm for matrix balancing. IMA J. Numer. Anal. 33.
- Brauer, C., Clason, C., Lorenz, D. and Wirth, B. (2017). A Sinkhorn-Newton method for entropic
  optimal transport. arXiv:1710.06635.
- Tang, X. et al. (2024). Accelerating Sinkhorn algorithm with sparse Newton iterations. ICLR.
- Saad, Y. and Schultz, M. H. (1986). GMRES. SIAM J. Sci. Stat. Comput. 7.
- Eisenstat, S. C. and Walker, H. F. (1996). Choosing the forcing terms in an inexact Newton
  method. SIAM J. Sci. Comput. 17.
- Dube, J.-P., Fox, J. T. and Su, C.-L. (2012). Improving the numerical performance of static and
  dynamic aggregate discrete choice random coefficients demand estimation. Econometrica 80.
