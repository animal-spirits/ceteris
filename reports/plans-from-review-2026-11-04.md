# Plans from the 4 November review

Follows `progress-report-2026-11-04.md`, `architecture-overview-2026-11-04.md` and
`econ-numbers-vs-literature-2026-11-04.md`. Nothing here takes a decision id; any item that becomes
an RFC takes its numbers in its own PR.

Correction to the earlier replies: ADR 0272 (log-distance crossing friction) is accepted but
**built in P4** with the 50-goods recalibration (owner ruling 2026-10-31), so C7 does not test it.
ADR 0027 (short-run trade adjustment) is **already planned** as P3 Branch E, with rho fitted from
BACI local projections and on by default only if its falsification test passes (P3 plan Q3).

## 1. Distance elasticity and Brexit magnitudes

**Interpretation.** With iceberg legs, each class's distance coefficient is about -0.09 theta_c, so
the model cannot produce -1.1 from geography alone. The border term nu is fitted to ICIO openness
and absorbs what distance should carry; when shocks hit the border (Brexit), the response is
measured against a border that is too "cheap" in elasticity terms and too large in level, so
welfare moves 4 to 5 times too little. The two misses have one cause, and 0272 targets it.

**Plan (recommended): keep 0272 in P4, register the misses now.**
- C7 records distance (-0.45 vs -1.1) and Brexit (-0.63/-0.24 vs -2.66/-1.34) as ADR 0257 tier-2
  misses, each naming 0272 and P4 as the removal phase. No code.
- P3 runs its tariff experiments knowing magnitudes are low; P3's gate (CRN tightens intervals,
  null intervention, year relabel) does not depend on them.
- Cost: none beyond C7's records.

**Alternative: bring 0272 into P3.** The workbench would show credible magnitudes from the start,
but it costs a recalibration on the 4-class pack that P4's 50-goods recalibration then repeats
(about 1 to 2 owner-weeks extra, the reason you chose P4).

## 2. Passes that test nothing

**Interpretation.** About half the "agreements" are targets the model is fitted to. The only free
checks of the trade block are the ACR-IO median and correlation, welfare signs and the old US-China
run. A model can hit every fitted target and still give wrong counterfactuals, which is what the
Brexit numbers show.

**Plan (recommended): add report-only literature rows to C7.** Within your 4 Nov ruling (held-out
checks report-only until P9):
- Print beside each benchmark case its literature value and source: Brexit (Dhingra et al. 2017),
  US-China 2018 pass-through and welfare (Amiti-Redding-Weinstein; Fajgelbaum et al.), distance
  (Head-Mayer), ACR-IO (Costinot-Rodriguez-Clare).
- Rerun the US-China shock on the current code (last run 28 Oct, before 0200).
- Tag each row in the econ SPEC as "fitted" or "free" so future reports separate them.
- Cost: about half an owner-day, inside C7. No RFC: no criterion changes.

## 3. Long-run elasticity applied monthly

**Interpretation.** Theta is a long-run number (Boehm et al.: under 1 in year one, about 2 long
run). Applying it every month makes sourcing too elastic in the short run. Two observed symptoms
fit this: the x10 shock diverging (sourcing swings faster than prices settle) and storage-on
cycles. That link is a hypothesis, not a measurement.

**Plan (recommended): run P3 Branch E first in P3, and widen its falsification.**
- Order P3 so Branch E (0027) lands before the workbench views.
- Add to its tests, reported only: x10 convergence with rho fitted vs rho = 1, and the storage-on
  frictionless starts of RFC 0286. If either converges, it is evidence for pulling those fixes out
  of P5.
- Cost: no new scope; reordering inside P3, plus two report-only runs.

## 4. Gates redefined after the numbers

**Interpretation.** Five criteria moved in nine days (V1 start, 0247 criterion 3, H2 distance,
x10, tick budget). Each move had a reason, but each came after the result was seen. That weakens
what "gate passed" means, especially for P9, where pre-registration is the whole point.

**Plan (recommended): freeze C7 now and apply the rule ADR 0257 already has.**
- The C7 thread brief restates the criteria exactly as `specs/pre-p3/PLAN.md` C7 and ADR 0268 set
  them (openness in [1/1.5, 1.5] of ICIO per class; graded ACR: signs, median in [0.8, 1.25],
  correlation >= 0.9; x10 reported) and the PLAN's own stop rule: a miss stops work and goes to
  you.
- Any change to a criterion after a run goes to you as an RFC with before and after numbers
  (0257: changed tests need an ADR). No new decision, only enforcement.
- Cost: none.

## 5. Records and scope

**Interpretation.** At least 17 built ADRs still say "proposed", so the index understates what is
settled, and scope has grown about 60% in a week. Without a cap and real hours, the 1.0 range
widens every time a study lands.

**Plans:**
- **5a Status sweep (recommended).** One Sonnet thread drafts one RFC that lists each built but
  proposed ADR (0092, 0093, 0094, 0098-0101, 0103, 0104, 0106, 0247, 0256, 0266, 0267, 0276,
  0280, 0281) with its as-built line, for you to accept in one card. Run it after C6/C7 so the
  as-built lines are final. Cost: about 1 hour of your review.
- **5b Scope guard for 1.0 (your call).** New work proposed after today defaults to 1.x unless you
  pull it into 1.0 on its card; amends ADR 0207's scope rule, so it needs an RFC.
- **5c Re-rate at the gate.** Already decided. It needs your own estimate of hours spent on B and
  C, because the B history was squashed.

## Order

1. Now: 4 (C7 brief) and 2 (C7 report rows) go into the C6/C7 threads.
2. At the pre-P3 gate: 1 (misses registered), 5c (re-rate), 5a (status sweep).
3. P3 start: 3 (Branch E first).
4. Any time: 5b if you want it.
