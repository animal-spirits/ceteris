# UI app schedule to 1.0 (2026-11-10)

Sources: roadmap-timeline and 1.0-roadmap (30 Oct), pre-1.0-polish-plan (9 Nov), specs/p3/PLAN.md Branch W, specs/app/PLAN.md, issues #204 #208 #408-#411. Owner-week = ow (ADR 0207 unit, full attention, agents coding). Estimates are the plans' own; nothing here is re-rated.

Calendar: 0.8 of full time to Jan 2027, then 12 h/week (about 0.3 ow/week) once the full-time job starts (~Feb 2027). Plan-of-record dates are the scope basis; the pace basis is the expected case. Dates below are the plans' straight-line windows, not commitments.

## State

App shell (ADR 0088, `app/`) is done: steps 1-7, globe only (0250), Tauri removed, 30 Oct 2026 (PRs 81-95). Slint+wgpu map, inspector, time controls with run-ahead ring, P2 modes, province panel exist. Not in the shell: difference maps, fan charts, outliner, edit log, branch manager, multi-window, sound.

## Ordered work items

| # | Item | Phase / issue | Depends on | ow | Window (pace / scope) | Start now? |
|---|---|---|---|---|---|---|
| 1 | W1 Branch manager and edit log (fork at cursor, seek, run to tick, edit log with god flag, all via command registry 0188). Test: app and headless give same hash | P3 Branch W, #204 | pre-P3 B2 (W1 may start after B2) | in W1-W4 total 1.5-2.5 | Feb-Jul 2027 / Aug 2027-May 2028 | Partly: UI scaffolding against the existing fork API; real data after B2 |
| 2 | W2 Difference map (branch minus baseline, diverging ramp passing 0221 check, sign-robustness hatching, paired interval on pick) | #204 | C3 effect tables, shell | (in 1.5-2.5) | same | Blocked on C3 |
| 3 | W3 Fan charts + paired-effect panel, 0137 chart tokens | #204 | C3 effect tables, CRN ensembles | (in 1.5-2.5) | same | Blocked on C3 |
| 4 | W4 Why panel (0049 channel shares, background job) | #204 | C4 | (in 1.5-2.5) | same | Blocked on C4 |
| 5 | P3 gate vertical slice: J2 tariff experiment shown in W1-W3 (trailer v0, "coming soon" page) | P3 gate J, #205 | items 1-3, J2 result (Mac) | 0.5-1 (J) | end of P3 | Blocked |
| 6 | P7 Stability pack: halt panel + repro bundle (0214), 100-year soak, fuzz, 16 GB run, "report a problem" | #410 | J1; Mac + 16 GB machine | 1-1.5 | after P3 gate | Halt panel design/spec can start now; soak needs Mac |
| 7 | P4(polish) Accessibility: CVD palettes, reduce-motion, UI scale to 200%, keyboard-complete, contrast audit, CI checks | polish plan P4 (1.x unless re-raised) | W6 | 1-1.5 | after W views | Contrast/CVD ramp checks can start now |
| 8 | W5 Lenses, W6 Keyboard/exports (0155, 0188, 0197 remainder) | #208 (post-1.0 label), W5/W6 in PLAN | after gate | in lab-UI track, unscoped | after P3 gate | Blocked; scope plan due after J closes |
| 9 | P1 First-run setup screen (0215): disk check, per-source rows, resume, manual-file drop zone, schema-change rebuild; prebuilt pack | #408 (exempt from scope guard) | #228 fetch/build, licence audit #231 #174, RFC amending 0215 (0333, merged) | 0.5-1 (+1-2 pack) | in R window; sooner if licence audit clears | UI screen code can start now against fixtures; clean-Mac test needs Mac; pack waits on audit |
| 10 | P2 Showcase scenario set (scenario files, nightly CI) | #409 | P3 gate; P5a/P8/P9 for later scenarios | 2-3 | after P3 gate, bulk in R window | After gate |
| 11 | P13 Performance feel (UI never blocks, cancel, ETA, speed readout) | polish P13 | G gates, Mac timings | 0.5-1 | 1.x | Non-timing parts yes |
| 12 | P5 Reproduce/share UX (hash badge, reproduce button, ceteris:// links) | polish P5 | F3, H | 0.5-1 | 1.x | After F3 |
| 13 | P6 Run report export | polish P6 | F2, W2, W3 | 1-1.5 | 1.x | Blocked on W2/W3 |
| 14 | P11 UI polish (ADR 0262 cut, 27 of 34 touches) | polish P11 | W views | 3-4.5 | 1.x | Blocked |
| 15 | P3(polish) Five lessons + lesson frame (0158) | polish P3 | W1-W3, C | 1-1.5 | 1.x | Blocked |
| 16 | Relief globe stage 1 (0255) in P3, stage 2 in R | #233 (post-1.0) | globe, P3 | 4-8 total (unscheduled for 1.0) | not 1.0 unless re-raised | No |
| 17 | P8 Steam path, macOS + Linux (Linux app amends 0216 by RFC, ~+1 ow) | #411 | signed build (#228), Antonio account | 1-1.5 (+1) | R window | Steamworks account and Linux build spike can start now |
| 18 | P9 Store assets, `ceteris capture`, trailer | polish P9 (needs scope ruling) | P2, P11 | 1.5-2.5 | before store page | Capture tool design only |
| 19 | P10 Community hooks | polish P10 | repo opens; `.github/` is Antonio-only | 0.5-1 | R window | No |
| 20 | Remaining shell-adjacent: Linux app port (x86_64, aarch64) | #411 | winit/Slint/wgpu on Linux | in #411 | with #17 | Yes (CI build on droplet runners) |

P5a, P5b, P8 (growth/hazards) and P9 (calibration) are engine phases, not UI phases. UI touches inside them are small: P5a/P5b add map modes and panels for money, banks and flows; P8 adds hazard and chokepoint views on the freight globe; P9 adds scorecard and hindcast views. These are not separately estimated in any plan; they ride inside each phase's estimate. The polish plan notes most showcase scenarios (item 10) need P5a, P8 and P9, so showcase UI work lands in the R window (Oct 2029-Oct 2031 pace, May 2031-May 2034 scope).

## Critical path and dependencies

1. Pre-P3 (B, C, D, benchmark rerun) is the gate on everything in W. Nothing in W2-W4 is buildable before C3/C4 effect tables.
2. The first UI deliverable of real size is W1-W4, 1.5-2.5 ow, inside the P3 window (Feb-Jul 2027 pace; Aug 2027-May 2028 scope).
3. First-run, Steam, signing (#408, #411, #228) all need the Mac and the licence audit (#231, #174, EXIOBASE reply pending). Antonio has no physical Mac access, so Mac steps go via Mac runners/RC threads.
4. Polish (P2-P18 other than exempt P1/P2/P7/P8) is 1.x by the 9 Nov scope decision; P9 trailer/store assets needs a separate ruling before a store page.

## Can start now (no Mac, no spec, no licence block)

- W1 scaffolding against the existing fork API (full test after B2).
- P1 setup-screen UI against fixture source lists.
- P7 halt panel design and repro-bundle format.
- Linux app CI build and Slint/wgpu check on Linux (#411).
- Contrast/CVD ramp checks and UI scale 200% (accessibility groundwork).
- `ceteris capture` design note (no code before P2/P11).

## Blocked

- W2/W3 on C3; W4 on C4; P5 on F3; P6 on W2/W3; P2 scenarios on P3 gate.
- P1 prebuilt pack, clean-Mac run, Steam depots on licence audit (#231, #174), #228, Mac.
- Soak and 16 GB runs on Mac and 16 GB machine.
- P8 Steamworks setup needs Antonio (account, fee, tax forms).
- Spec/RFC merges (0216 Linux amendment, 0215 amendment status) need Antonio alone.

## Totals (UI-bearing, ow)

Exempt Must set: P1 0.5-1 (+1-2 pack), P7 1-1.5, P8 1-1.5 (+1 Linux), P2 2-3 = 4.5-7 plus the pack and Linux additions, about 6.5-10 (polish plan). P3 views W1-W4: 1.5-2.5. 1.x polish (P3 lessons, a11y, P5, P6, P11, P13): about 7-11. Part-time at 12 h/week, 1 ow is about 3.3 calendar weeks.

## Open questions for Antonio

- Is P9 trailer/store assets in or out of the 1.0 exemption? It sits outside the 4.5-7 but the store page needs it.
- W5/W6 (lenses, keyboard, exports) is labelled post-1.0 (#208) yet P4(a11y) depends on W6 keyboard; confirm a11y keyboard-complete is in the 1.x set.
- The roadmap HTML (30 Oct) still lists shell steps 5-7 as Nov 2026; the timeline HTML says shell done. This doc follows the timeline.
