# RFC 0309. Ensemble spread at P3 comes from register priors

Status: accepted
Date: 2026-11-06
Design sections: 14
Release: 1.0, P3
Research: P3 readiness review, 2026-11-01 (project files, `research/p3-readiness-2026-11-01.md`); `specs/p3/PLAN.md` question Q1

## Summary

The P2 tick draws no random numbers, so K seeds of the same branch give K identical runs and the
P3 gate's tighter-intervals-under-common-random-numbers test cannot pass. The P3 plan put the
question as Q1 and the owner ruled on 2026-10-30; no record was written. This RFC records it.

## Decisions

### D1. Where does ensemble spread come from at P3?

Options:

- A. Each ensemble member draws the uncertain parameters from their register priors (theta by
  class, rho, nu, the price-rule gains). Common random numbers means a branch and its baseline
  share the draw.
- B. No member draws, as now: K seeds of one branch give K identical runs and the gate's
  tighter-intervals test cannot pass.

Recommendation: A, because it needs no new mechanism in the tick and the register already holds
the priors.

Decision: A, Antonio, 2026-10-30 (recorded here 2026-11-06).
Row: ADR 0322

## Spec changes

- 14 (opening paragraph): ensemble members differ by their parameter draws from the register
  priors; a branch and its baseline share the draw (ADR 0322).
- `specs/p3/PLAN.md`: Q1 points at ADR 0322.

## Consequences

- Tasks C1, C2 and J2 of the P3 plan depend on this; Branch C1 implements the draws.
- Hazard and shock draws stay with P5 and later phases (ADR 0242).
