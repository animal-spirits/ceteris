# RFC 0360. Epsilon search by a fixed parallel grid, with typed infeasibility

Status: proposed
Date: 2026-11-18
Design sections: 15 (calibration), 17.1; ADRs 0092, 0272, 0353, 0373, 0376
Release: 1.0 (P4)
Research: none in the repository; follows the H2 diagnosis of 2026-11-18 (`epsilon.rs` bracket
bug) and the parallel refit scoping of 2026-11-18 (slice 1 merged as #639)

## Summary

ADR 0272 Decision 3 fits epsilon by Brent's method on f(epsilon) = model pooled distance
coefficient less the data's, over [0, `EPSILON_MAX` = 2] to `EPSILON_TOL` = 1e-3
(`crates/ceteris-calibrate/src/epsilon.rs`, `fit_epsilon_inner`). After #639 the two bracket
ends run concurrently, but Brent's 8 to 12 further trials are sequential, and each trial is
single-threaded, so a 16 vCPU host sits mostly idle through a refit of 9 to 13 trial-times.

This RFC replaces Brent on epsilon with a k-section grid: each round evaluates a fixed set of
epsilons that depends only on the current bracket, in parallel, and keeps the sub-interval
where the sign changes. Three rounds reach the tolerance. Brent stays where it is for nu
(ADR 0092) and s_c (ADR 0376).

A grid reads the sign of f at many points at once, so a wrongly signed point now picks the
wrong bracket outright. Today one is: the bracket maps every error containing "outside the
model's" to `None`, scored as `+|data_pooled|` ("too little distance cost, raise epsilon").
Two different failures carry that phrase:

- the group's share at nu's upper end is still above its target (`border.rs`, held group with
  `f_hi > 0`): too little cost, a true positive sign;
- the target is above the share at zero friction: a border group short at nu 0 (`border.rs`,
  held group with `f_lo < 0`), or a class short even at scale 0 (`solve_scale`, settled). No
  epsilon fixes it; it says nothing about the sign of f.

The second case is scored as the first. The 5-class fit on 11-17 reported "still above the
data's at epsilon 2" for this reason, although the same trial reached -0.917 on 11-14. The
fix types the two apart and makes the second a hard, named error. It is a correctness fix and
lands before the grid (D5).

## Decisions

### D1. Does a parallel grid replace Brent for the epsilon search?

Options:

- A. Grid (k-section) replaces Brent on epsilon. Results differ from Brent's within
  `EPSILON_TOL`; `epsilon.json` and any epsilon-on golden move once, as a reviewed event.
  Deterministic and independent of the thread count.
- B. Keep Brent; #639 (concurrent ends) is the whole speed-up. Bit-identical, about 2x on the
  ends only.
- C. One coarse grid round, then Brent in the narrowed bracket. Fewer trials than A, but
  about 5 to 7 sequential trials remain; little gain over B on 16 vCPU.

Recommendation: A, because it is the only option that turns idle cores into wall-clock, and a
grid also exposes non-monotone f (D4), which Brent hides.

Decision: pending.
Row: none yet.

### D2. How many points per round?

The grid size is a fixed constant, never derived from the thread count or the host, so a run
on 4 or 64 threads computes the same trials and returns the same bits.

Options:

- A. 15 interior points per round (16 sub-intervals). Round 1 is 16 points, epsilon = 2i/15,
  i = 0..15, ends included: width 0.133. Rounds 2 and 3: width 0.0083, then 0.00052 <
  `EPSILON_TOL`. Three waves, at most 46 trials. One wave fits a 16 vCPU host.
- B. 7 interior points (8 sub-intervals): four waves, at most 32 trials; fits an 8-core Mac.
- C. 31 interior points: two rounds would not reach 1e-3 (0.065, then 0.002), so still three
  waves at twice the CPU. Rejected on cost.

Recommendation: A, because the target host is 16+ vCPU and three waves is the minimum for
1e-3 at any practical size; on a smaller host A is still correct, just two batches per wave.

Decision: pending.
Row: none yet.

### D3. Which epsilon is reported?

Options:

- A. Of the two grid points bracketing the sign change in the last round, the one with the
  smaller |f|; ties take the lower epsilon. Its trial is already computed, so no extra
  sequential trial, and it is within 5.2e-4 of the root by construction.
- B. Linear interpolation in the final bracket, then one more trial there: one extra
  sequential trial-time for an accuracy the tolerance does not ask for.

Recommendation: A, because the reported epsilon is always a computed trial and the tolerance
is met without a fourth wave.

Decision: pending.
Row: none yet.

### D4. What do sign changes and infeasible points do?

Options:

- A. Classify each point: `Ok(trial)` (sign of f), `TooLittleCost` (sign +, as today),
  `Infeasible { class or group, epsilon, residual }` (no sign). Any `Infeasible` point in a
  round fails the fit with that typed error, naming the class or group and the epsilon. More
  than one sign change among a round's points fails the fit as non-monotone, listing the
  points. f(0) <= 0 keeps today's `at_zero` result; f(2) > 0 keeps today's error.
- B. As A, but `Infeasible` points are dropped and the bracket is taken from the remaining
  points if they still show exactly one sign change.

Recommendation: A, because a structural infeasibility (good 2's class share, H2 diagnosis)
is a model-structure question for RFC 0355, and B would hide it behind a fitted number: a
silent clamp under the calibration rule (2026-11-16). B also brackets across a dropped point
whose f is unknown.

Decision: pending.
Row: none yet.

### D5. How is the work sequenced?

Options:

- A. Two PRs. PR 1 (Right): typed infeasibility in `border.rs` and `solve_scale`, the bracket
  stops reading a zero-friction shortfall as too little cost; Brent unchanged; with a test that
  fails on main. PR 2 (Fast): a one-day spike on the 16 vCPU box with a go/no-go number, then
  the grid if the spike passes.
- B. One PR with both.

Recommendation: A, because the development laws put Right before Fast, the bug fix changes
the search outcome today with Brent, and the grid is not bit-identical so it needs its own
measurement.

Decision: pending.
Row: none yet.

## Determinism

- Grid points are a pure function of the bracket and the constant of D2: lo + (hi - lo) i /
  16, computed in f64 in that order. Never of thread count or completion order.
- A trial is a pure function of (inputs, targets, epsilon, limits), starts cold (ADR 0097) and
  shares no state, so trials run with `par_iter().map().collect()` into index order. No float
  reduction across trials; rayon `sum`/`reduce`/`fold` stay banned.
- Progress lines, `TrialCache` stores and `RunLog` checkpoints are committed after each
  wave's join in ascending epsilon, through #639's `compute`/`commit` split, so `RunLog.seq`
  and file names do not depend on scheduling. `TrialCache::store` is already write-then-rename
  and keyed by epsilon bits.
- The bracket choice, the D3 tie-break and the D4 errors depend only on the committed values.
  The first error in ascending epsilon is the one reported.
- `fit_border_nu_seeded` (ADR 0373, non-authoritative) is not used.
- Brent on nu and s_c inside a trial is unchanged.

## Expected gain (estimates, nothing measured)

Per-trial time on the Mac: about 4.5 h on the failing 5-class case (9 h 11 min for two
sequential trials, 11-17); RFC 0272 estimated 8 to 12 Brent evaluations.

| | Waves of trial-time | Trials (CPU) |
| --- | --- | --- |
| Main before #639 | 10 to 14 | 10 to 14 |
| Main after #639 | 9 to 13 | 10 to 14 |
| Grid, D2 A, 16 vCPU | 3, each slowed 1.2x to 1.6x by shared memory bandwidth | up to 46 |

Wall-clock gain on a 16+ vCPU host: about 2x to 3.5x on the search, at 3x to 4x the CPU.
Resident memory per trial is unknown; 16 at once must fit 32 GB with margin. Per-core speed of
the rented host against the Mac is unknown. Speeding up a single trial (parallel Sinkhorn,
scoping item F) is separate and multiplies on top.

## Acceptance

PR 1 (bug fix):

1. A `CalibrationError` variant (or equivalent typed result) separates "too little cost" from
   "short at zero friction"; no string matching on error text in the bracket.
2. Test, fails on main: a fixture where one class is short at scale 0 at a settled pass; the
   fit returns the typed infeasibility naming the class and epsilon, not "still above the
   data's at epsilon 2".
3. Test: a group whose share at nu's upper end is above target still scores as too little
   cost (sign +), and the fit still recovers a known epsilon (`fit_recovers_a_known_epsilon`).
4. Existing epsilon tests unchanged and green; no tolerance loosened.

Spike (before PR 2), on the 16+ vCPU host, logged in `~/ceteris-logs/`:

5. One trial alone, and 16 concurrent trials at 16 grid epsilons: wall-clock and peak RSS. Go
   if 16 concurrent take at most 1.6x one trial and peak RSS is under 24 GB. Otherwise record
   the negative result in this RFC and stop at #639.

PR 2 (grid):

6. Switch-on behaviour test through the real path (`fit_epsilon_logged`, as `ceteris
   calibrate` calls it) on the CI fixture: the run log's trial epsilons are the D2 grid points
   in order, and the result is within `EPSILON_TOL` of the known epsilon. Fails with Brent.
7. Determinism: `EpsilonFit` JSON and the run log bit-identical in pools of 1, 2 and 8 threads
   (`ThreadPoolBuilder::install`), and across two runs.
8. The k-section search is a function over a closure f, unit-tested on synthetic f: monotone
   (finds the root within tolerance), root at a grid point, f(0) <= 0, f(2) > 0, an
   `Infeasible` point (typed error), two sign changes (non-monotone error).
9. `cached_fit_is_bit_identical` and `concurrent_end_trials_match_sequential` adapted to the
   grid and green.
10. Measured on the 16+ vCPU host on the same inputs: grid wall-clock at most half of Brent's
    after #639. Otherwise record the negative result and revert to Brent.
11. epsilon-off goldens unchanged. Any epsilon-on golden or committed `epsilon.json` that
    moves is re-blessed in the same commit with a `### BEHAVIOUR:` CHANGELOG line.

## Spec changes

Once accepted, in the PR that takes the rows:

- ADR 0272 Decision 3: "epsilon by Brent" becomes "epsilon by a fixed k-section grid (RFC 0360)".
- Section 15 / econ SPEC calibration: the epsilon search, grid size, reported point, and the
  typed infeasibility and non-monotone errors.
- Section 17: the grid size and `EPSILON_TOL` as calibration constants, with codes from
  `next-code P`.

## Effects

- Goldens: none with epsilon off. With epsilon on, one reviewed move within `EPSILON_TOL`.
- Fits that today report a wrong-direction "still above the data's at epsilon 2" will instead
  fail with a named infeasibility; on the 5-class branch (#600) that is expected until RFC 0355
  settles good 2's class.
- Old `TrialCache` entries stay valid but rarely hit, since grid epsilons differ from Brent's.

## Open

- Whether the per-class epsilon_c of the H2 diagnosis (fix 4) supersedes this search; a vector
  epsilon would need a different method, and this RFC would cover only the scalar case.
