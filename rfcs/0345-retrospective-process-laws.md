# RFC 0345. Process laws from the 12 November retrospective

Status: accepted
Date: 2026-11-12
Design sections: 21.2
Release: 1.0, all phases
Research: `reports/retrospective-2026-11-12.md`

## Summary

The 12 November retrospective found three process gaps: about 1,600 lines of P4 and P5 kernels
with no path from a run, off-by-default switches outside the goldens and literature checks, and a
merge rule in `CLAUDE.md` and law 9 of `docs/development-laws.md` that disagreed with the standing
rule Antonio set on 10 November. A change to a development law is an RFC; this one records his
three answers.

## Decisions

### D1. Pause new P4/P5 kernels until the unwired ones are wired?

Options:

- A. Wire-or-park: no new kernel until each unwired kernel has a wiring issue; at most three
  kernels unwired at a time.
- B. Keep building kernels in parallel; wire at phase end.
- C. Stop P4/P5 until the P3 gate passes.

Recommendation: A, because it keeps law 2 (working through the real code path) honest without
stalling P4 and P5.

Decision: A, Antonio, 2026-11-12.
Row: ADR 0353

### D2. How are off-by-default switches handled at phase gates?

Options:

- A. Gate review: at each phase gate every off-by-default switch becomes the default (a named
  golden move), gets a dated issue, or is deleted. Switches that fix a gate criterion join the
  literature-baseline matrix when they land.
- B. Leave switches off until someone needs them.

Recommendation: A, because an off path is outside the goldens and literature checks and can rot.

Decision: A, Antonio, 2026-11-12.
Row: ADR 0354

### D3. Who merges spec, RFC and research PRs?

Options:

- A. Threads merge on green once CI passes and the branch is up to date; Antonio alone decides
  (question cards). An agent or coordinator message is still never his decision.
- B. Antonio alone merges.

Recommendation: A, because it matches the standing rule of 10 November and new threads read the
repository, not memory.

Decision: A, Antonio, 2026-11-12.
Row: ADR 0355

## Spec changes

- `docs/development-laws.md`: law 2 gains the wire-or-park cap; new law on off-by-default
  switches; law 9 reads "Antonio alone decides; threads merge on green".
- `CLAUDE.md`: the two merge-rule lines and the stage-gate list follow the laws.
- `.claude/rules/rfcs.md` step 5: threads merge on green.
