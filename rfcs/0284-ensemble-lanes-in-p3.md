# RFC 0284. Ensemble lanes move from pre-P3 into P3 (amends ADR 0207)

Status: accepted
Date: 2026-11-01
Design sections: 21
Research: P3 readiness review, 2026-11-01 (project files, `research/p3-readiness-2026-11-01.md`)

## Summary

ADR 0207 puts ensemble lanes (ADR 0024, pre-P3 Branch E) in pre-P3, and the pre-P3 plan closes
only once E has merged. The P3 readiness review of 1 November 2026 found that E has neither a
branch nor a plan of its own yet, and that it has to wait for Branch D. In P3, only task C1
(ensembles with member draws) needs lanes. Tasks 0, A, B, D, G and H do not, and the P3 plan
already lets C1 run members as separate processes, with the same hashes, if lanes are late.
Keeping E in pre-P3 therefore holds the whole of P3 back for 1.5 to 2.5 owner-weeks of work
that only one P3 task uses. This RFC moves E into P3 as a branch that runs beside A and B and
lands before C1. Lanes stay in 1.0 with ADR 0024's requirements unchanged.

## Decisions

### D1. Do ensemble lanes stay a pre-P3 gate item, or move into P3?

Options:

- A. Move E into P3 beside C1: pre-P3 closes on C and D; the lane retrofit is a P3 branch that runs alongside A and B and must merge before C1.
- B. Keep E as a pre-P3 gate item: P3 starts only after lanes merge, which puts about 1.5 to 2.5 more owner-weeks on the pre-P3 path.

Recommendation: A, because only P3 C1 needs lanes, and P3 0, A, B, D, G and H run without them.

Decision: A, Antonio, 2026-11-01.
Row: ADR 0288

## Spec changes

- 21 (pre-P3 and P3 phase lines): ensemble lanes leave pre-P3's scope and join P3's, which now
  names ensembles on lanes (ADR 0288).
- ADR 0207: its status line notes the amendment. Its triage table keeps 0024 on the 1.0 path,
  now in P3.
- `specs/pre-p3/PLAN.md`: Branch E moves out; pre-P3 closes when C7's criteria pass and D has
  merged. The same file records two owner decisions from the same review: D runs in parallel
  with C and rebases after it, and the D5 workflow is drafted by an agent and committed by the
  owner.
- `specs/p3/PLAN.md`: a new Branch L (lanes) before C1. "Starts from" no longer waits for lanes,
  and the estimate grows by L's 1.5 to 2.5 owner-weeks, which come out of pre-P3's estimate.
