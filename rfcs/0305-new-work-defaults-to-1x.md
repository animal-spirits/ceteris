# RFC 0305. New work defaults to 1.x, not 1.0 (amends ADR 0207)

Status: accepted
Date: 2026-11-04
Design sections: 21
Research: progress report and plans of 2026-11-04 (project files, `reports/progress-report-2026-11-04.md`, `reports/plans-from-review-2026-11-04.md`)

## Summary

ADR 0207 (28 October 2026) put 1.0 at 52 to 82 owner-weeks with its lean rule: each phase
builds only what its gate and the 2011 to 2024 scorecard need. ADR 0248 (30 October) re-estimated
it at 80 to 129 after ADRs 0208 to 0247, and the roadmap timeline of the same day, with the
later accepted ADRs folded in, at 83 to 135: about 60% more in a week. Most of the growth came
from proposals accepted into a 1.0 phase one at a time, each defensible alone. The lean rule
says what a phase needs but not where a new proposal lands when nobody says. This RFC sets that
default: new work goes to 1.x unless the owner puts it in 1.0.

## Decisions

### D1. Add a scope guard so new work defaults to 1.x, not 1.0?

Options:

- A. Yes: a proposal accepted after 2026-11-04 that adds work to a phase before R lands as a 1.x
  minor behind a flag, unless the owner's answer on its card names a 1.0 phase. Work needed to
  pass a gate already in Section 21.1, or to fix a miss against one, is not new scope and stays
  in its phase. Every RFC states the release it targets.
- B. No: scope is set per ADR as now.

Recommendation: A, because 1.0 grew about 60% in a week, and a default stops the range widening
with every study.

Decision: A, Antonio, 2026-11-04.
Row: ADR 0314

## Spec changes

- 21 (opening paragraph): the default for new work after 2026-11-04 (ADR 0314).
- ADR 0207: its status line notes the amendment.
- `rfcs/0000-template.md`: a `Release:` header line (1.0 with its phase, or 1.x).

## Consequences

- An RFC or ADR that adds 1.0 work has to say so, and the owner sees that on its card.
- Fixes for gate misses, such as the misses C7 registers for P4, are not affected.
- The pre-P3 re-rate compares spent hours with ADR 0248's table plus only the work the owner put
  into 1.0 after this date.
