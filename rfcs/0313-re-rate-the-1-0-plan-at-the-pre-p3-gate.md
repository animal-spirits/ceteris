# RFC 0313. Re-rate the 1.0 plan at the pre-P3 gate (amends ADR 0248)

Status: accepted
Date: 2026-11-06
Design sections: 19.2, 21
Release: 1.0, pre-P3
Research: `reports/plans-from-review-2026-11-04.md` item 5c; `reports/progress-report-2026-11-04.md` sections 1 and 4; ADR 0248 (the tables replaced here)

## Summary

ADR 0248 set the plan of record at 80 to 129 owner-weeks (scope basis) and required that, at the
pre-P3 gate, the owner-weeks spent on pre-P3 Branches B and C be compared with its table and the
economics share re-rated. B and C have merged (#127, #163) and C7 has passed (#271).

**The spent figures below are the agent's estimates, made at the owner's request on 2026-11-06,
because the B history was squashed and no hours were logged. They are not measurements. Treat
them as a ruling to overwrite, not a fact.**

## Method and assumptions

1. **What B and C were allotted.** ADR 0248 gives pre-P3 as a whole 9 to 15 owner-weeks (scope
   basis). `specs/pre-p3/PLAN.md` has 29 tasks: A 6, B 10, C 7, D 5, F 1. B and C are 17 of 29
   (59%), so their share is 5 to 9 owner-weeks.
2. **What they cost.** Owner attention, with agents writing the code, is estimated from what the
   repository shows of it, between 30 October (B starts after A) and 6 November (C7 merged):
   - owner decisions recorded in RFCs dated 30 October to 6 November: 53 (counted from `Decision:`
     lines), at 15 minutes each: 13 hours;
   - merged pre-P3 PRs in the window, about 20, at 45 minutes of review and merge each: 15 hours;
   - steering and thread prompts, 2 hours on each of 7 active days: 14 hours.
   Total about 42 hours. The same method with 10 and 25 minutes per decision, 30 and 60 minutes
   per PR, and 1 and 3 hours a day gives 25 to 65 hours.
3. **Unit.** ADR 0248's calendar table implies about 34 hours of attention per owner-week
   (221 weeks at 12 hours for 77 owner-weeks). 25 to 65 hours is then 0.7 to 1.9 owner-weeks.
4. **Not counted.** The owner's own hours outside the repository (reading, thinking, the
   roadmap and research threads) are not visible and are not in the 42 hours. If they were large,
   the true figure is higher; the range is meant to cover that, but may not.

Result: B and C cost an estimated 0.7 to 1.9 owner-weeks against an allotment of 5 to 9, about
0.1 to 0.4 of the scope basis. ADR 0248's pace basis for pre-P3 was 0.67 of the scope basis.
Nothing is concluded for later phases from this one ratio: P4 on is calibration-heavier, and the
decisions that took attention here (53 in a week) came from the audit's size, which later phases
may not repeat.

## Scope changes since ADR 0248, for the table

- Ensemble lanes move from pre-P3 into P3 (RFC 0284): P3 gains 1.5 to 2.5.
- The 50-goods pack costs 5 to 7 (progress report of 2026-11-04) against the 2 to 4 ADR 0271 guessed:
  P3 gains 3.
- Relief 0255, conflict onset 0254 and Paradox cues 0252 are post-1.0 (issue #233), so the
  +8 to 14 listed in the timeline artifact stays out of 1.0.
- Pre-P3 remaining after B, C and C7: D mostly merged (#134, nightly gate), E moved, F superseded:
  0.5 to 1. App shell: ADR 0248's remaining 2 to 3 is kept; this RFC did not re-check it.

## Decisions

### D1. What is the plan of record for 1.0 after the pre-P3 gate?

Remaining at 6 November 2026, owner-weeks:

| Phase | Scope basis (before) | Scope basis (after) | Pace basis (after) |
| --- | --- | --- | --- |
| Pre-P3 | 8 to 13 | 0.5 to 1 | 0.4 to 0.8 |
| App shell | 2 to 3 | 2 to 3 | 0.6 to 1 |
| P3 | 9 to 14 | 13.5 to 19.5 | 6 to 10 |
| P4 | 7 to 11 | 7 to 11 | 5 to 8 |
| P5a | 6 to 10 | 6 to 10 | 4 to 7 |
| P5b | 17 to 27 | 17 to 27 | 12 to 19 |
| P8 | 7 to 12 | 7 to 12 | 5 to 8 |
| P9 | 16 to 25 | 16 to 25 | 12 to 19 |
| R | 5 to 9 | 5 to 9 | 3 to 5 |
| Total | 77 to 124 | 74 to 118 | 48 to 78 |

The pace basis applies ADR 0248's per-phase pace-to-scope ratios to the new scope numbers; it is
not re-rated from B and C. At 12 hours a week from February 2027, with ADR 0248's months per
owner-week (0.66 low, 0.70 high on scope; 0.63 and 0.67 on pace): scope basis March 2031 to
January 2034, pace basis September 2029 to June 2031.

Reference only, not a basis: if the measured ratio of 0.1 to 0.4 held for all remaining work the
scope total would be 7 to 47 owner-weeks. One phase of audit work does not support planning on it.

Options:

- A. Adopt the "after" columns: scope basis 74 to 118 is the plan of record, the pace basis
  48 to 78 is reported beside it, and the measured ratio is tracked at the P3 gate.
- B. Keep ADR 0248's numbers (77 to 124 and 51 to 84) and add only the P3 changes.
- C. Re-rate again at the P3 gate and change nothing now.

Recommendation: A, because the scope changes are known and the only soft part, the pace basis,
is already labelled a report. The B and C hours stay an estimate until the owner overwrites them.

Decision: A, Antonio, 2026-11-06 ("Merge 276", recommended option taken).
Row: ADR 0324

### D2. Does the tick budget need re-rating?

Findings: the criterion tick at the pre-P3 gate was 41.2 ms against the 35 ms P2 budget. The
speed stack (PRs #148 to #156; ADRs 0294 to 0298) brought the sim-loop tick to about 34 ms on a
loaded Mac (load average 4 to 14), 33.8 to 34.0 ms in PR #155 and about 1.3 ms lower with
mimalloc in PR #156. The 160 ms ceiling for 45 goods is unchanged; its per-good lines are fitted
in P3 (ADR 0235).

Options:

- A. Keep the 35 ms budget for the 10-goods pack: it is met. Keep 160 ms. Re-measure on an idle
  Mac when #169 runs.
- B. Tighten the 10-goods budget to the new measured 34 ms.
- C. Raise it back to 41 ms.

Recommendation: A, because the budget is met and the timings come from a loaded machine, so
tightening on them would fit noise.

Decision: A, Antonio, 2026-11-06 ("Merge 276", recommended option taken).
Row: ADR 0325

## Spec changes

- 21 (opening paragraph): the plan of record cites ADR 0324.
- 19.2: a sentence with the measured tick and the budget status (ADR 0325).
- ADR 0248: its status line notes the re-rate. Its tables are replaced by D1's.

## Consequences

- The plan of record falls by 3 to 6 owner-weeks on the scope basis; the P3 additions nearly
  cancel what pre-P3 spent.
- The next re-rate is at the P3 gate. Logging weekly hours from now would make that figure a
  measurement; this RFC does not require it.
