# RFC 0362. Newton scaling: step control as measured, and two criteria the spike could not meet

Status: accepted
Date: 2026-11-20
Design sections: 15 (calibration), 17.1; econ SPEC, Calibration, stage 2
Release: 1.0 (P4)
Research: spike on branch `claude/project-thread-ga207j` and its note on #653; `reports/fit-speedup-research-2026-11-20.md`
(#650, #651)

## Summary

RFC 0361 accepted a per-good Newton phase for the stage 2 scaling and fixed its step control in
D3 before any code ran. The one-day spike (#653) ran D1 to D4 as written and the slow scalings
did not converge: the first step from the sweep point is 1e5 to 1e11 in ln w, `exp` overflows,
the line search fails, and every slow scaling of good 1 falls back. Three changes fix it, and a
fourth check closes a hole the spike found in the Krylov solve. This RFC replaces D3's constants
and merit with the measured ones (D1), and brings two pre-registered criteria of RFC 0361 into
line with what the spike measured instead of passing them by silence (D2, D3). RFC 0361 stays as
accepted; where the two differ, this RFC governs. D1, D2 and D4 of RFC 0361 and its D5 to D7 are
untouched.

## Spike numbers (fixture `tests/golden/p2-fixture`, 4 cores, release; #653)

One `fit_at_epsilon`, wall-clock, sweep-only against Newton:

| NEWTON_AFTER | eps 0 | eps 0.4 |
|---|---|---|
| sweep only | 13.5 s | 38.0 s |
| 20 | 0.49 s (27x) | 3.58 s (10.6x) |
| 50 | 1.43 s | 4.14 s |
| 100 | 14.0 s | 38.1 s |

D3 B's rule (least total wall, ties to the larger) selects 20 (7 % better than 50 at eps 0.4, 3x
at eps 0). With RFC 0361's D3 A as written (max-norm merit, no step cap, 20 steps, NEWTON_AFTER
50) eps 0 fell 13.0 to 1.7 s and eps 0.4 only 36.3 to 21.4 s (1.7x): good 1's slow scalings all
fell back.

## Decisions

### D1. What is Newton's step control?

Replaces RFC 0361 D3 A's constants and merit; D3 B's selection rule stands and picked 20.

Options:

- A. As measured: merit = 2-norm of F (the backtracking test is on ||F||_2; the stop rule still
  reads max |F|); the Newton step is scaled to at most 10 in ln w before the line search;
  `NEWTON_MAX` 60 steps per good; `NEWTON_AFTER` 20; at most 30 halvings of the step; and a GMRES
  step is accepted only if its true residual ||J d + F||_2 is at most 2 eta ||F||_2 (the Krylov
  residual can pass on a near-singular J).
- B. RFC 0361 D3 A as written. Does not work (above).
- C. Park the Newton phase.

Recommendation: A, because it is the only variant measured to work and its constants are fixed
before the code is merged; the changes are a norm, a cap and a bound, not a new method.

Decision: A, Antonio, 2026-11-20 (question card, Newton-spike thread: "Amend D3, build PR 2").
Row: ADR 0394

### D2. What bound does the Newton scaling's ln w meet against the sweep?

RFC 0361 pre-registered max |Delta ln w| <= 1e-9 per good against the sweep-only oracle. The
spike saw 1.7e-9 at eps 0 on 141 scalings (2.1e-10 at eps 0.4, 586 scalings). Both results meet
the stop rule (max |F| <= 1e-13), so both are valid solutions; the RFC's own estimate, two
solutions within tol of the root differ by up to 2 kappa tol, put kappa near 1.1e3 for Brazil's
coal block, and 1.7e-9 implies an effective ||J^-1|| near 8.5e3, eight times that estimate. The
estimate was low: along a near-singular direction a tol-small residual leaves ln w loose. Downstream quantities (nu, s_c, d_0) differ by 3e-16, 7e-15 and 0 in the spike, and by
6e-16, 3e-15 and 0 on the fixture in the merged tests.

Options:

- A. Keep 1e-9 and treat the spike's 1.7e-9 as a failed bound. The code would pass only if the
  oracle itself were rerun from the Newton point, which hides the difference.
- B. Raise the ln w bound to 1e-8 (about six times the largest seen), keep every other bound
  (`|Delta r| / r` 1e-12, nu 1e-8, s_c 1e-5, d_0 1e-8), and tighten the tested nu, s_c and d_0
  agreement on the fixture to 1e-13, which is what the code reaches. The reason is recorded:
  ln w is determined only to 2 ||J^-1|| tol along near-singular directions, while every quantity
  the model reads is determined far tighter.
- C. Pass ln w only through downstream quantities, no bound on ln w.

Recommendation: B, because it states the measured fact instead of hiding it, keeps a bound that
fails if the Newton point is wrong, and moves no downstream bound.

Decision: B (the recommended option), applied on Antonio's go to merge this PR (2026-11-20), relayed by the coordinator; he did not pick it on a card.
Row: ADR 0395

### D3. Is "no fallback" still a go criterion?

RFC 0361's go criteria included no fallback on the spike's scalings apart from the unbalanceable
and plateau cases. The spike had 12 fallbacks in 598 scalings at eps 0.4 and 0 in 141 at eps 0.
Eleven are good 4 at a sweep iterate with ln w between -116 and -347, where the reduced Jacobian
is near singular and GMRES's true residual stays above its forcing term; the note on #653 does
not say why the twelfth fell back, and PR 2 reports it. A fallback returns main's own result (D4 of RFC 0361), so correctness
holds, and its cost is the Newton attempt (at most 60 steps of at most 300 products per good)
on top of the full sweep.

Options:

- A. The criterion becomes: every fallback is diagnosed (`Scaling::newton` records the first
  failure, good and reason), is of the near-singular kind above, and the total wall-clock with
  fallbacks counted is the number that must meet the speedup gate (it does: 10.6x at eps 0.4).
- B. Keep zero fallbacks and park Newton.
- C. Start the Newton phase only from a sweep point with bounded ln w, to avoid the 12.

Recommendation: A, because the fallback is the designed safety net and its measured cost is
inside the gate; C is a new rule tuned to 12 cases and is not warranted by them.

Decision: A (the recommended option), applied on Antonio's go to merge this PR (2026-11-20), relayed by the coordinator; he did not pick it on a card.
Row: ADR 0396

## Acceptance of PR 2

RFC 0361's acceptance items 5 to 10 apply with these changes: the 20-step bound is replaced by
60 steps per good and the measured per-good maximum, reported by `Scaling::newton` (fixture: 5
to 7); item 7's first half (Jacobian product against a dense Jacobian by forward differences) is
not a direct test, because the product is a closure inside the scaling; it is covered by Newton
converging in at most 7 steps per good to 1e-13, which a wrong J cannot do, and the GMRES half
(against dense LU) is a unit test. The fallback test (item 8) injects the unbalanceable good
(all of its legs closed) and checks that Newton failed on it and that the error is the sweep's
bit for bit; the rounding plateau is not injected.

## Spec changes

Written in this PR (ADRs 0394 to 0396): 17, P181 constants (`NEWTON_AFTER` 20, `NEWTON_MAX` 60, 2-norm
merit, step cap 10, 30 halvings, true-residual check) and "in code"; econ SPEC, Calibration, stage 2,
the Newton phase text updated and "not yet in code" removed. D2 and D3 (ADRs 0395, 0396) are in the same PR.
