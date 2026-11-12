# P3 readiness review, 1 November 2026

Checked against origin/main f3bd6a3 (after #127, #120, #125, #126, #128), branch
`pre-p3/audit` e4786a6, open PRs (#124 draft, #15 stale), and the open threads. No code changed.

Sources: ADR 0207 (rfcs/0207), ADR 0248, `specs/pre-p3/PLAN.md` (close condition: "Pre-P3
closes when C7's three criteria pass (or the owner accepts a recorded miss) and D and E have
merged"), `specs/p3/PLAN.md` ("Starts from: main after pre-P3 ends: B8 and C6 recalibrated, D1
and D2 snapshots across interventions and parameters as state, E lanes with `const L`, the
benchmark rerun passed"), spec/21 (pre-P3 accept line).

## Summary

Pre-P3 is about half done. Branch B is merged and its numbers are good, but C is one task in
(C1, unmerged), and D and E have not started; neither has a branch. P3 cannot start from main
until C6, D1, D2 and E land. The two benchmark criteria still to grade both wait on a pack
rebuild: openness passed on the B8 pack, and ACR has not yet been measured in the form that
grades it. Remaining pre-P3 work is about 5 to 9 owner-weeks (my judgement, item by item below).
ADR 0248's re-rate is due at the gate, not now.

## Gate criteria

| # | Criterion | Status | What blocks it |
|---|---|---|---|
| 1 | Branch B (B1 to B8) merged, P2 gate on the recalibrated pack | **Met**, #127 (2fe2a0d). B6 split price rule deferred to P5 (ADR 0090). | Nothing |
| 2 | Openness within 1.5x ICIO shares | **Met on B8, to recheck.** Ratios 1.151 / 1.043 / 1.144 / 0.973 by class on the B8 pack. The asserting test `openness_within_1_5_of_icio` does not exist yet. | C6 recalibration moves it; C7 must write the test and rerun |
| 3 | ACR formula holding (median in [0.8, 1.25], corr >= 0.9, signs) | **Open, at risk.** The country form on B8 has median 1.077 (passes), correlation 0.862 (fails). The province-graded form (ADR 0268) is built in #128 but has not been measured on a pack with the 0269 mode costs. | Pack rebuild with 0269, then C6, then C7 |
| 4 | x10 border shock | **Met by ruling.** Reported only, P5 criterion (ADR 0090, owner "A", 2026-10-31) | Nothing |
| 5 | C1 to C5 audit ADRs (0098, 0100, 0099, 0104, 0103) | **Open.** C1 built on `pre-p3/audit` (RFC 0282, rows 0282 to 0286), no PR, based on f1ffeb2 so behind #128. ADR 0098's 5% real-GDP check misses 47 of 79 (D7, under study in the PWT vs WDI thread). C2 to C5 not started. | C1: D7 outcome and a PR. C2 to C5: work |
| 6 | C6 recalibrate and rerun P2 gate | **Open** | C1 to C5 |
| 7 | C7 benchmark asserted on the final pack | **Open** | C6; criteria 2 and 3 |
| 8 | D1 to D4 (snapshots across network and parameter edits, warm starts, minors) | **Open, not started.** No branch. P3 B1, B2 and C1 need D1 and D2. | Sequencing only (the plan puts D after C) |
| 9 | D5 nightly full-pack gate | **Blocked.** Main CI runs on the Mac runner (#113) but has no pack job, and no `pack-gate.yml` exists. The `rows` guard fails any `claude/*` branch that touches `.github/`. | Antonio has to commit the workflow, or the task gets dropped |
| 10 | E ensemble lanes (`const L`, ADR 0024) | **Open, not started.** ADR 0024 is still proposed. The plan says E gets its own plan after D. | D; a plan |
| 11 | Benchmark rerun on an idle Mac (budgets) | **Open.** The quiet-loop tick is 39 ms on the Mac at econ-review, against a 35 ms budget. #124's output-neutral speedups are still a draft, and the idle rerun is queued. | Idle Mac; #124 |
| 12 | Goldens on main | **Open.** #128 moves the p1-geo and econ pack goldens, which are re-blessed at the next pack build, so pack-gated tests stay red until then. | Pack rebuild (owner-side, GEOS) |
| 13 | F design snapshot | **Superseded** by #126 (DESIGN split into spec/). Should be closed in the plan. | Nothing |

Not gate items, but open alongside:

- **Storage-on frictionless misses.** Storage is off by default and the default mode passes, so these do not block the gate. Six rounds of fixes failed; the storage thread has a card asking Antonio to choose between deferring to P5 (recommended), a redesign of the shortage price step, or a storage-on-only rule.
- **Repository settings.** "Require branches up to date" and `rows` as a required check on main are still with Antonio.

## Numbering collision

`pre-p3/audit` uses RFC 0282 and rows 0282 to 0286. Main stops at 0281. The storage thread's
card proposes recording its result "in RFC 0282", but that number is already taken on the audit
branch. The next free numbers are **RFC 0283 and row 0287**. Parameters are free from P160. CLAUDE.md's collision rule
(merge main and renumber) would catch this at merge, but whichever thread merges second loses a
renumber.

## P3 plan against what exists

`specs/p3/PLAN.md` (#89) predates the spec split and several merges. What is stale:

- **Paths and procedure.** It names `specs/DESIGN.md` sections and "ADR numbers from main". Since #126, decisions take rows from `ceteris-rows next`, design lives in `spec/NN-*.md`, and proposals are RFCs. Task 0.1 ("mark 0234 to 0236 accepted") now goes through the RFC and rows procedure.
- **Register.** It says the next free number is P150. It is now P160.
- **Done already.** Task G2's goods count landed with #120, and the plan ticks it.
- **Records not yet written.** The Q1 decision (member draws from register priors) and the Q2 decision (gate views only, with a lab-UI track amending 0207's table) each need an RFC and a row. Neither exists. ADRs 0234, 0235 and 0236 are still `proposed`, and so are 0024 (lanes), 0091 and 0280 (storage).
- **Random numbers.** The finding of #89 still holds: the econ tick draws no random numbers, so gate 3 (tighter intervals under CRN) needs C1's member draws. Nothing on main has changed that.
- **App shell.** Complete (ADR 0250, steps 1 to 7), so Branch W is not blocked by the shell.
- **Budgets.** P3 gate 5 checks budgets against 19.2 as 0235 restates it. The 35 ms whole-tick budget is currently missed (39 ms), and 0235's per-good lines need G2's synthetic 45-goods pack, which is P3 work.

## Remaining pre-P3 work (judgement)

Owner-weeks in ADR 0207's unit, anchored on how B went.

| Item | Owner-weeks |
|---|---|
| C1 close (D7 outcome, PR) | 0.3 to 0.5 |
| C2 to C5 | 1.5 to 3 |
| C6 recalibrate, re-bless, gate | 0.5 to 1 |
| C7 assert tests and rerun | 0.3 to 0.5 |
| D1 to D4 | 1 to 1.5 |
| D5 (owner commit) | 0.1 to 0.2 |
| E lanes (plan and build) | 1.5 to 2.5 |
| Idle benchmark, #124 | 0.2 |
| Total | 5.4 to 9.4 |

At 30 October, ADR 0248 put all of pre-P3 at 8 to 13 (scope basis) or 6 to 10 (pace basis). Since
then B, the C7 groundwork with 0268 and 0269, and C1 have landed. The remainder looks like the
pace basis, but the economics share is still unmeasured: the hours that went into storage and
D7 went into dead ends rather than gate items.

## Roadmap re-rate (ADR 0248)

0248 says: "At the pre-P3 gate the owner-weeks spent on B and C are compared with this table".
The gate has not been reached and C is one task in, so **no re-rate is due**. Two notes for when
it is:

1. B ran from about 28 October (decision 9) to its merge on 1 November. That is about five
   calendar days with several threads in parallel. B's history was squashed, so per-task time
   cannot be read from git; the re-rate needs the owner's own count of hours.
2. Moving E into P3 (decision 2 below) would move about 2 owner-weeks off pre-P3's critical path
   without changing the total.

## Decisions put to Antonio (cards in this thread)

1. Start D now in parallel with C, or keep the plan's order (C, then D, then E)?
2. Do ensemble lanes (E) stay a pre-P3 gate item, or move into P3 next to C1?
3. How does the D5 nightly full-pack gate land, given agents cannot touch `.github/`?

Not posed here, because another thread already owns them: the storage-on close-out (storage
thread card) and D7 (the PWT vs WDI study).
