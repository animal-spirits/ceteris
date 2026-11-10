# RFC 0339. Bounded price gap at zero output under the money anchor (P5 Branch E2)

Status: accepted
Date: 2026-11-10
Design sections: 8.2, 12.2, 17 (P03a, P03c, P151, P178); trade SPEC Money-anchored rule
Release: 1.0, P5a
Research: [P5 E2 zero-output step spike, 2026-11-10](../docs/research/p5-e2-zero-output-step-spike-2026-11-10.md)

## Summary

RFC 0286 deferred the price step at Q = 0 to P5, and ADR 0339 put the x10 border criterion on it.
The spike finds why the anchored rule of ADR 0338 deflates to the price bound at x10: a cell with
D > 0 and Q near 0 has a gap of 1e4 to 1e11 over the Q floor, and since the ADR 0338 means weight
by p (Q + D) / 2 that cell sets lbar_g and lbar, which drive every other price down at the clip.
A bounded gap for every good, the midpoint form 2 (D + b - Q) / (D + b + Q), removes it: the index
holds within 1.09 at x10, the fixture converges at x3 and runs at x5, and the steady state, nominal
shock and data state are unchanged. It does not carry x10: output then dies province by province
through what looks like an absorbing zero state of the Leontief chain, a production fault, not the
step. This changes ADR 0338's gap definition, and asks where x10 goes.

## Decisions

### D1. Which gap does the anchored price rule use at an origin?

Options:

- A. Midpoint `l = 2 (D + b - Q) / (D + b + Q)`, 0 when both are 0: bounded in [-2, 2], smooth at
  D = Q, equal to as built to first order at the steady state, and with the ADR 0338 weights the
  good mean is the value of excess demand over traded value. No Q floor in the anchored rule.
- B. Max form `l = (D + b - Q) / max(Q, D + b, Q_floor)` (RFC 0286's bounded stored gap, for every
  good): same fixture results, a kink at D = Q that needs one-sided differences in the storage
  tangent-map test.
- C. Keep the Q-floor gap of ADR 0338: x10 deflates to the price bound by tick 100.

Recommendation: A, because it removes the failure with no kink and gives the weighted mean a plain
reading, and B matches it only with a kink.

Decision: A, Antonio, 2026-11-10.
Row: ADR 0340

### D2. Where does the x10 criterion go, now that the price step alone does not carry it?

Options:

- A. Keep x10 a P5a gate criterion and carry it on a new task E3, the zero-output production trap
  (input orders on constrained output, the local Leontief chain at zero, the eta 1 labour flip):
  diagnose on the fixture, spec and RFC, then the pack. E2 builds the bounded gap now, off by default
  with the anchor.
- B. Assert at x3 on the fixture and the pack at P5a (the range the bounded gap holds) and report
  x10, moving its assertion to the storage-on gate of ADR 0091.

Recommendation: A, because the criterion was fixed in advance (development laws), and the spike
points to a specific, diagnosable mechanism rather than an open-ended one.

Decision: A, Antonio, 2026-11-10.
Row: ADR 0341

## Spec changes

- trade SPEC, Money-anchored rule: the gap l_o of D1 replaces (D + b - Q) / Q_eff in the anchored
  rule; the as-built rule keeps Q_eff. ADR 0340.
- x10: carried by task E3 (ADR 0341, amends ADR 0339's carrier).
- P5 PLAN, Branch E: E2 builds the bounded gap; task E3 added (ADR 0341).

## Before the build is switched on

- Mac: the RFC 0286 screen (eight storage-on and eight storage-off V1 starts, 6,000 ticks) with the
  anchor and the D1 gap, and the pack data state and x10 runs. The anchor stays off by default until
  they pass.
