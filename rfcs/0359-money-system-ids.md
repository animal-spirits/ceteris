# RFC 0359. Money system IDs follow the code: 233 energy policy, 234 investment

Status: accepted
Date: 2026-11-17
Design sections: 9 (specs/money/SPEC.md, system order)
Release: 1.0
Research: none; follows #632 (kernel wiring review)

## Summary

`specs/money/SPEC.md` listed 231 as the r* filter, 233 as investment financing and 234 as
demand-system clearing. The code schedules 233 as `energy_policy` and 234 as `investment`
(`crates/ceteris-econ/src/systems.rs`), and runs the r* filter inside 230. A system ID is part
of every RNG stream address and a hash domain, so the first kernel to draw under 233 or 234
would freeze whichever numbering was in the code. Antonio chose the code as the record
(question card, #632, "A", and "go with the most correct long term options", 2026-11-17).

## Decisions

### D1. Which numbering is the record?

Options:

- A. The code. The spec table lists 233 energy policy and 234 investment, the r* filter runs
  inside 230 (231 stays unassigned, never reused), and investment financing and demand-system
  clearing take fresh IDs 241 and 242 in money's block. Banks (235) run with 242; housing (236)
  and sovereign debt (237) follow 242. No code, hash or golden moves.
- B. The spec. Renumber 233 and 234 in the code. No golden moves today, but the schedule order
  and every later RNG address would depend on the renumbering.

Recommendation: A, because it moves nothing that already runs.

Decision: A, Antonio, 2026-11-17.
Row: ADR 0383

## Spec changes

- specs/money/SPEC.md, System order: table as in D1; IDs are hash domains and change only by RFC (ADR 0383).

## Work

Docs only. Schedule order is the order of the `schedule()` list, not ID order, so 241 and 242
need no code until their kernels land.
