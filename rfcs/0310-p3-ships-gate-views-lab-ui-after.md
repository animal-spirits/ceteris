# RFC 0310. P3 ships the gate's workbench views; the rest of the lab UI follows the gate (amends ADR 0207)

Status: accepted
Date: 2026-11-06
Design sections: 21
Release: 1.0, P3
Research: P3 readiness review, 2026-11-01 (project files, `research/p3-readiness-2026-11-01.md`); `specs/p3/PLAN.md` question Q2

## Summary

ADR 0207 keeps ADRs 0049, 0137, 0143, 0155, 0188 and 0197 lean in P3 at 4 to 6 owner-weeks, but
their own ADRs cost 15 to 25 part-time weeks. The P3 plan put the question as Q2 and the owner
ruled on 2026-10-30; no record was written. This RFC records it and amends the ADR 0207 table.

## Decisions

### D1. How much UI is in P3?

Options:

- A. P3 ships the workbench views the gate needs (W1 to W4). The rest of ADRs 0155, 0188 and
  0197 (W5, W6, F4) becomes a parallel lab-UI track that starts after the gate and lands before R.
- B. P3 ships all of them, at the full cost of their ADRs (15 to 25 part-time weeks).

Recommendation: A, because the gate needs only W1 to W4, and the track keeps the work in 1.0
without holding the gate.

Decision: A, Antonio, 2026-10-30 (recorded here 2026-11-06).
Row: ADR 0323

## Spec changes

- 21 (P3 line): the views the gate names; the lab-UI track follows the gate (ADR 0323).
- ADR 0207: its status line notes the amendment. Its P3 row keeps 0155, 0188 and 0197 on the 1.0
  path, with their remainder in the lab-UI track after the gate.
- `specs/p3/PLAN.md`: Q2 points at ADR 0323.

## Consequences

- The P3 estimate counts W1 to W4 only; the track is estimated when it starts (issue #208).
- ADRs 0155, 0188 and 0197 keep their status; only the phase of their remainder changes.
