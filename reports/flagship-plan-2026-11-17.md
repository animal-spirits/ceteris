# Plan for the first flagship scenario attempt, 17 November 2026

Issue #624. Main at 84f9abb (#620 and #623 merged). Nothing was run for this plan. Hours are
estimates unless marked measured. Sources: RFC 0350, `reports/delivery-risk-2026-11-13.md`,
`reports/work-hierarchy-2026-11-14.md`, `reports/e3-benchmark-2026-11-16.md`,
`docs/free-moment-rerun.md`, `docs/free-moment-rerun-results.md`, PR #600, issues #559 to #561.

## 1. Flagship

Proposed: **Brexit, hard and soft** (Dhingra et al. 2017). Reasons: it has two-sided welfare
bands in RFC 0350; #560 is its open gap (-0.63% and -0.24% against -2.66% and -1.34%); it shares
its suspected cause with H2 (#559), so one refit tests both; the benchmark already runs it.
US-China 2018 is the alternative: its welfare band is narrow ([-0.1%, -0.02%]) and its last
measured pass-through was on an unsettled baseline, so a miss there is harder to read. Antonio
decides (card in the thread).

## 2. Pre-registered check

Fixed by the merge of this file; a change after a result is an RFC (development laws, "Right").
One commit, one calibrated full 2000 pack, `CETERIS_PRICE_ANCHOR=off` (the default). All of:

| # | Moment | Pass | Printed by |
|---|---|---|---|
| F1 | Hard Brexit, UK welfare | [-4.0%, -1.33%] | `benchmark_cases_are_reported` |
| F2 | Soft Brexit, UK welfare | [-2.0%, -0.67%] | `eu_loses_from_soft_brexit` |
| F3 | EU27 and Japan welfare under costlier borders | negative, both | `eu_loses_from_soft_brexit`, `japan_loses_from_costlier_borders` |
| G1 | H2 distance elasticity | [-1.1, -0.7] | `p2_gate h2_distance_elasticity` |
| G2 | Openness per class against ICIO 2000 | within [1/1.5, 1.5], every class | `openness_within_1_5_of_icio` |
| G3 | ACR-IO, border x1.1 | no wrong sign, median [0.8, 1.25], correlation >= 0.9 | `acr_formula_holds_under_costlier_borders` |

F1 to F3 are the flagship; G1 to G3 are guards, because every fitted wedge so far hit its target
while moving another moment (#554). All six pass, or the attempt is a miss. Reported, not scored:
x10, US-China, PPML theta spread, the held-out class count of the epsilon fit.

Void, not a miss: a panic in the harness, a base run that has not settled, or a pack whose
calibration tables belong to other inputs. A void run is fixed and rerun once; the fix changes no
parameter, bracket, weight or criterion.

Not the check: the `brexit-hard-vs-soft` showcase finding in `scenarios/manifest.toml` (CRN over
8 seeds). It is pinned after a pass, on the same pack.

## 3. Ordered steps

1. **PR #600 green** (fifth-class thread, in progress). Fixture BALANCE re-derived on merged
   main including #623; the 7 fixture-precision tests root-caused, not loosened; the two gate
   changes it lists (H2 skip under 8 crossings, tolerance moves) need Antonio's answer or removal;
   RFC 0355 text and rows. Blocks step 2.
2. **Mac run R1: refit.** `ceteris calibrate --pack P --fit-epsilon` on the #600 branch merged
   with main: epsilon, theta_2 and nu together.
   Gate A: the fit settles and every fitted share is at its target. If not, section 5.
3. **Mac run R2: H2.** `ceteris export`, then `p2_gate h2_distance_elasticity` (G1). Closes #559 on pass.
4. **Mac run R3: benchmark, off.** `benchmark -- --include-ignored --nocapture` (F1 to F3, G2,
   G3). Closes #560 on pass. Gate B: section 2.
5. **Record.** Result table appended to `docs/free-moment-rerun-results.md`, all rows, pass or miss.
6. **On pass:** card to Antonio to lift the kernel freeze; pin the three scenario hashes; then
   #561 (step 7). **On miss:** section 5.
7. **E3 (#561), after the flagship, not part of it.** R4: the same benchmark with
   `CETERIS_PRICE_ANCHOR=e3` on the refitted pack. Its precondition is a settled e3 base run
   (16 November: max |dln p| 4.998, unsettled) and x10 survival (#216).

## 4. Mac runs

Each: one task per thread, Antonio's go written in that thread, detached `caffeinate -imsu`, log
in `~/ceteris-logs/flagship/`, commit and pack hash on the first log line.

| Run | Needs | Hours |
|---|---|---|
| Build | release build of the branch | 0.3 |
| R1 refit | built 2000 pack, ICIO shares, `--fit-cache` dir | 2.0 to 2.6 (ADR 0272: 1.5 to 2 h; rounds added by #623 cost about 30% on the small pack) |
| R2 H2 | R1's pack, export, pinned Gravity and Natural Earth zips | 0.5 |
| R3 benchmark off | R1's pack | 0.2 (573 s measured for e3, 16 November) |
| R4 benchmark e3 | R1's pack | 0.2 |

One clean attempt: about 3.5 to 4 h. Budget with one void rerun: 8 h. A second refit is not in
the budget (section 5).

## 5. Decision rule on a miss

One refit on merged code is scored. On a miss at gate A or B: stop. No second fit with other
targets, brackets, weights, tolerances or starting points; no band moves. Record the full table,
then one structure card to Antonio. What the pattern says, written before the result:

| Result | Reading |
|---|---|
| Fit does not settle or a share sits at a bracket end | The wedges cannot hit the data state together: structure, not tuning |
| G1 passes, F1 or F2 too small | Distance is not the cause of #560; look at the Brexit shock mapping and input-output amplification |
| G1 misses, fit settled | The epsilon form does not carry the distance gradient; per-class d_0 (#212, RFC 0354) |
| F and G1 pass, G2 or G3 regress | Border nu and epsilon conflict; nu fitted to ICIO shares is the suspect |

Candidate structure options for that card (mine, not decided): per-class distance scale (#212);
nu as a residual instead of a fitted share target; a tier 2 ADR naming the unbuilt mechanism
(ADR 0257); the research-preview 1.0 of the delivery-risk report.

## 6. Cloud prep, in parallel, inside the freeze

None adds a kernel or wiring.

1. Band assertions in test code. RFC 0350 says they "follow in a separate PR"; none has landed
   (`p2_gate.rs` holds the H2 band as a diagnostic; `benchmark.rs` has no Brexit band). One
   ignored test that asserts section 2 makes the check mechanical. Before R1.
2. `docs/free-moment-rerun.md` still names RFC 0257's pass numbers; point it at RFC 0350.
3. Dry run of steps 2 to 4 on the committed small pack (RFC 0353), to find harness panics before
   Mac hours. Its numbers are not scored.
4. One script for R1 to R3 with logs and the result table, so the Mac thread runs one command.
5. Issues and tests for the three unfixed findings of the calibration audit: s_c sticking at 0,
   the misleading infeasibility message, the untested trade-weighted d_0. The d_0 test before R1.
6. x10 panic at `world.rs:89` (#216) and the unsettled e3 base run, diagnosed on the fixture or
   small pack. Diagnosis and tests only; no default flips.
7. Rented Linux box (#571 scripts) so R1 does not depend on the Mac. Antonio's action.
