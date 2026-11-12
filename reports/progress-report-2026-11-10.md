# Ceteris Paribus: progress report, 10 November 2026

Sources: merged PRs on `atbraz/ceteris` from 7 to 10 November (GitHub PR list, 100 most recent
closed PRs, #331 to #434), and the 4 November progress report. Nothing here is re-measured
unless stated. Figures carried from the 4 November report are marked as such.

## 1. Verdict

**Code moved on the pre-P3 gate items, P4 population and the app. The economics gate did not
move.** No merged PR in this window is titled as C6 or C7 work. The 4 November open problems
(pack recalibration, fit criteria, goldens on main) are carried forward unchanged until a PR
closes them.

## 2. Done since 4 November (merged PRs)

- **Performance (RFC 0316, 0317, 0319):** #358 zero-copy journal, #359 and #363 netted and
  in-flight totals, #364 end_tick reuses dispatch scan, #355 and #361 goods-major passes,
  #362 parallel push_all, #367 parallel snapshot hash, #376 one task per province (E2),
  #403 to #405 good-major slots, fused totals and parallel ledger checks (S1 to S3).
- **Lanes and gates:** #331 to #334 lane-generic dispatch, source kernel, EconLanes;
  #343 shared buffers; #345 and #352 ensemble lanes; #330 and #346 G1 flatness on CPU time,
  #347 G1 wired into pack-gate; #368 banned platform transcendentals; #430 libm for trig,
  exp, pow in the app.
- **Calibration and econ (C1 to C4):** #339 ensemble member draws; #351 `ceteris calibrate rho`
  (E2a); #354 rho panel from BACI; #371 GTD trade-war reader; #379 CRN interval test (C3);
  #380 channel shutdown; #381 to #383 explain, blame, explain --reference (C4);
  #387 trade-share channel (C4); #420 pass history fix; #423 border and NTB edit targets
  (ADRs 0336, 0337); #415 and #416 Trends input and moving-reference path solver (F1, ADR 0243).
- **Run, serve and CLI:** #384 fetch, #385 and #389 serve and tick stream (F3, F4),
  #386 commitments table (C2), #390 chunk-store mirror pull, #418 showcase manifest and first
  scenario (#409), #422 halt repro bundle and replay, #424 W1 session with fork, seek and
  run-to.
- **App (W1 to W4, #204):** #394 difference map data, #395 fan chart geometry, #396 branch
  manager data, #397 why panel data, #431 halt panel and report-a-problem (#410).
- **Population, P4:** #406 and #413 spec and P4 ADR acceptance (RFC 0335), #414 moving
  reference spec, #425 cohort cells and ageing (A1), #429 Brass mortality and tempo fertility
  (A2), #433 education progression and headship (A3), #434 demography wired into the tick
  (#209).
- **Packaging and art:** #417 first-run pack setup with mirror fallback, resume and disk check;
  #432 cloud-doable P8 Steam path (packaging, depot scripts, privacy draft); #426 art inventory
  and placeholders; #428 158 SVG mocks; #401 Statistics Finland source; #412 drafts for a
  prebuilt pack and Linux app.
- **Records and process:** #365 development laws in CLAUDE.md; #391 and #392 accepted ADRs
  0212, 0211, 0128, 0331; #393 Linux golden check for Branch H (#203); #399 Arrow IPC examples;
  #400 and #402 ADR status and RFC 0319 ADRs; #333 Arrow IPC only for 1.0; #398 runner disk cleanup.

Closed unmerged, superseded by a merged PR or recorded as a negative result: #360, #372, #374
(E2 one task per province, replaced by #376), #373 (S3 block dispatch spike, no-go),
#344 (flatness bench on calibrated params), #375 (sub-stage timing, folded into #377).

## 3. In progress

- P4 population: A1 to A3 and demography wiring are merged. Next is the F2 branch (stage 5 and
  tracking tests), not yet in a merged PR.
- Pre-P3 C6 (recalibrate and rerun the P2 gate) and C7 (the 28 October benchmark rerun). No merged
  PR in this window; C7 has not started as of 4 November.
- Tick budget: 41.2 ms against a 35 ms budget (4 November figure, not re-measured). Known unbuilt
  wins are about 4.7 ms (journal as owned blocks, parallel goods moves).
- App UI: W1 to W4 data and the session are merged. Widget work that needs C3 and C4 is in the
  UI schedule (`reports/ui-schedule-2026-11-10.md`).

## 4. Blocked on Antonio

Source: project memory, not checked against GitHub issues or PRs in this report.

- Review and send six licence drafts (OECD/ICIO, IEA, CEPII, Gennaioli, LIS, PWT). EXIOBASE
  reply sent 9 November, awaiting answer; non-commercial, out of the paid build.
- Art decisions: app mark, final-art maker, Noto fonts, emblem coverage.
- Calls on issues #169, #171, #172, #175, #201, #207.
- Spec merges E1c and G3. ADRs 0050, 0128, 0158 decisions. Apple signing.
- Nightly pack-gate check.

## 5. Mac batch

Waits on J2 (#205). Commands for S0 and S4 are in `docs/research/spikes/rfc0319/mac-s0-s4-commands.md`.
Also owed: ADR 0294 hash check, pack trend columns, `pack fetch --record` for SEA and AAMNE, and
the #409 pack-hash pin, border checks and pre-registered checks. Mac timings are not comparable to
the cloud runners.

## 6. Risks

- The economic gate is not passed. Fit criteria were met only after being redefined (4 November,
  section 5 of that report). The pre-P3 re-rate is the first real test.
- Scope keeps growing. The 4 November estimate is 83 to 135 owner-weeks.
- Main's goldens and benchmark tests stay red until the 2000 pack is recalibrated (C6).
- Pack trend columns and the G1 gate need the Mac; cloud runs cannot confirm them.
- Licence exposure: EXIOBASE is non-commercial and must stay out of the paid build.

## 7. 1.0 estimate

Carried forward from 4 November: 83 to 135 owner-weeks (ADR 0248 basis, scope). No new
estimate is made here. Re-rate at the pre-P3 gate.

## 8. Stale project memory to prune

Listed for the PR description, not edited here.

- "P4 A2 mortality/fertility not yet wired into tick (A3)": superseded by #434 (wired) and #433 (A3).
- Queue item "Fork-from-last-snapshot in halt panel (needs Session in shell)": W1 #424 and halt
  panel #431 are merged.
- Queue item "Next unblocked P4 branch after A2 (running)": A3 and wiring are merged.
- "Merged 11-10" line listing #426, #428, #430 to #432: duplicates this report and goes stale.
- Done counter "7" in the queue: no longer tracks anything once the queue is replaced.
- Perf line "45-goods tick about 200 ms vs 160 ms": stale until the Mac batch re-measures it.
