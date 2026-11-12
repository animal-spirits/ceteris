# Pre-1.0 polish plan, 9 November 2026

Goal: a free Steam 1.0 that is compelling. 1.0 is a pure simulator (game mechanics are 2.0+). Native Slint/wgpu app.
Sources: RFC 0313 (74 to 118 owner-weeks, scope basis; R is 5 to 9), `reports/scope-progress-2026-11-07.md`, `specs/p3/PLAN.md`, queue #179, ADRs 0158, 0188, 0197, 0214, 0215, 0216, 0217, 0219, 0252, 0262.
All estimates are mine, in owner-weeks on RFC 0313's scope basis (about 34 hours of owner attention per week, agents writing code). They are not measurements. Steam rules and asset sizes are from memory: verify against Steamworks docs before relying on them.

## Thesis

The product is the counterfactual: ask "what if", fork, run, compare, trust the answer because the run reproduces. The polish plan serves one loop: install, first fork comparison within minutes, a result the user can verify and share. Everything below is ranked by how much it protects that loop.

Biggest threat to "compelling": first run. ADR 0215 has each user download about 6 GB over 50 files, 13 of them manual downloads from three providers, then build the pack. A Steam user expects click and play. Decision card 2 addresses this.

## Order by the working / right / fast philosophy

1. Working (nothing else counts until this holds): clean machine to first fork comparison through the real code path; one flagship scenario end to end in the app; a halt shows a panel and a repro bundle instead of a crash; a signed build opens.
2. Right: showcase scenarios pinned by hash and reproduced across platforms (ADR 0128, Branch H); every pedia and lesson claim sourced; exports licence-aware (ADR 0213); accessibility checks in CI; a visible "what this model is not" statement. Calibration targets for showcase findings are written down before the runs (development laws).
3. Fast and feel: tick budgets (10 goods 35 ms met; 45 goods 160 ms, measured about 198 ms at 14 threads, needs a layout RFC), UI never blocks on background jobs, 0262 and 0252 touches. Only after Right is green.

Timing: polish does not start before the P3 gate (J1 to J3). Exception, a vertical slice at the P3 gate: the J2 tariff experiment shown in W1 to W3 (branch manager, difference map, fan chart) is the first showcase and the source for trailer v0 and a "coming soon" store page. Most showcase scenarios need P5a, P8 and P9 to be credible, so the bulk lands in the R window.

Decision 2026-11-09 (Antonio, card): Steam 1.0 app ships on macOS and Linux (Apple silicon; Linux x86_64 and aarch64). Amends ADR 0216 by RFC; adds about 1 owner-week to P8. Windows waits for demand.

Decision 2026-11-09 (Antonio, card): first run ships a prebuilt redistributable pack plus user fetch for the rest. P14 moves from Should to Must (1 to 2 owner-weeks); amends ADR 0215 by RFC.

Decision 2026-11-09 (Antonio, card): only P1, P2, P7, P8 are exempted from the 1.x scope guard (4.5 to 7 owner-weeks). P3 to P6, P9 to P18 go to 1.x unless re-raised. Consequence to settle: the two earlier choices (Linux app, about 1 owner-week; prebuilt pack P14, 1 to 2) are not in that 4.5 to 7, so 1.0 additions total about 6.5 to 10 owner-weeks if both stand. P14 is first-run work and is treated as part of P1 unless Antonio says otherwise. P9 (trailer, store assets) is a Steam launch need and sits outside the exemption; it needs a ruling before the store page.

## Workstreams

Tier: Must = without it 1.0 is not compelling or not shippable. Should = clear gain. Could = after the rest.

| Id | Item | Tier | Low | High | Depends on | Needs |
| --- | --- | --- | --- | --- | --- | --- |
| P1 | First-run clean-machine path: setup screen (ADR 0215), disk check, resume, manual-file drop zone, schema-change rebuild, tested on a clean Mac | Must | 0.5 | 1 | R fetch work (#228) | Mac, clean machine |
| P2 | Showcase scenario set, 6 to 8, as hash-pinned scenario files with a nightly CI run that doubles as regression suite | Must | 2 | 3 | P3 gate; P5a, P8, P9 for later ones | Antonio picks the set; Mac for full-pack runs |
| P3 | First five lessons and the lesson frame (ADR 0158); each lesson forks, edits once, runs a CRN ensemble, walks the diff | Must | 1 | 1.5 | P3 W1 to W3, C | none |
| P4 | Accessibility pass: CVD palettes (ADR 0221), OS reduce-motion, UI scale to 200%, keyboard-complete (ADR 0188), contrast audit, checks in CI | Must | 1 | 1.5 | W6 | none |
| P5 | Reproduce and share UX: run-hash badge, "reproduce on this machine" button, `ceteris://` links (0197), run seal from the hash | Must | 0.5 | 1 | F3, H | none |
| P6 | Run report export: one page (HTML and PDF) with scenario, edits, hash, sources, caveats, chart and map images | Must | 1 | 1.5 | F2, W2, W3 | none |
| P7 | Stability pack: halt panel and repro bundle (0214), 100-year soak, fuzz, 16 GB minimum-hardware run, "report a problem" attaching the bundle | Must | 1 | 1.5 | J1 | Mac, 16 GB machine |
| P8 | Steam release path: Steamworks account, app, depots, build upload, store-page review lead time, privacy statement | Must | 1 | 1.5 | signed build (#228) | Antonio (account, fee, tax forms) |
| P9 | Store assets, trailer and capture tooling: `ceteris capture <scenario>` renders frame-exact video from a hash; capsule art, screenshots, 60 to 90 s trailer | Must | 1.5 | 2.5 | P2, P11 | Antonio edits and approves |
| P10 | Community hooks: GitHub Discussions, issue templates, CONTRIBUTING, in-app feedback link, scenario-sharing how-to | Must | 0.5 | 1 | repo opens (ADR 0218) | Antonio: `.github/` is off limits to agents |
| P11 | UI polish, ADR 0262 recommended cut (27 of 34 touches; 18.8 focused days) | Should | 3 | 4.5 | W views | none |
| P12 | Cue subset pulled back from 0252 (post-1.0 per RFC 0313): semantic zoom, visible pause, breathing selection edge, event pins, coalesced notices | Should | 1 | 2 | W views | Antonio (scope exemption) |
| P13 | Performance feel: UI thread never blocks, cancellable jobs, progress and ETA, honest speed readout (F4), load stages | Should | 0.5 | 1 | G gates | Mac for timings |
| P14 | Prebuilt redistributable pack in the download (ADR 0217 profile), user fetch only for the rest | Should | 1 | 2 | licence audit (#231, #174) | Antonio (card 2) |
| P15 | mdBook user guide, "limits of this model" page, per-series provenance pages (ADR 0219) | Should | 1 | 1.5 | pedia (#229) | none |
| P16 | Remaining five lessons (ten in total) | Could | 1 | 1.5 | P3 | none |
| P17 | Screen-reader spike, then support if Slint's accessibility layer allows (unverified) | Could | 0.2 | 1 | none | none |
| P18 | Scenario gallery in the app: import a shared scenario file, licence-checked | Could | 0.5 | 1 | P5, ADR 0213 | none |

Totals (owner-weeks, scope basis): Must 10 to 16, Should 6.5 to 11, Could 1.7 to 3.5, all 18.2 to 30.5. Must alone is 14 to 22 percent of the 74 to 118 plan. These are additions to the R budget (5 to 9), not part of it, except where the table says the base work is in the R plan already (P1, P7, P8 build on #228 and ADR 0214). The scope guard (#187: new work defaults to 1.x) means the Must set needs an owner exemption: card 3.

Already in plan, no new estimate: signed notarized macOS build (0216), pack fetch (0215), minimal pedia (#229), licence flip (#230), NC audit (#231), opt-in usage stats endpoint and privacy notice (0214), Zenodo archive and monthly URL drift job (0215; the CI job touches `.github/`, Antonio).

## Showcase scenarios (P2)

Each is a scenario file with a question in one line, one edit, a horizon, a written expected finding fixed before the run, and a pinned hash. A scenario that fails its own pre-registered check is dropped, not retuned.

| Scenario | Shows | Available after |
| --- | --- | --- |
| US tariff of 25% on Chinese manufactures, CRN versus independent seeds | fork, paired effect, why panel | P3 gate (J2) |
| Hard Brexit versus soft (shock files exist in `research/benchmark-2026-10-28/`) | difference map, forks side by side | P3 gate |
| Border friction x10 ("deglobalisation") | trade collapse and rerouting (#245) | P3 gate |
| China WTO-entry counterfactual | ghost of real history, replay on and off | P9 hindcast |
| Chokepoint closure and oil supply shock | freight globe, energy | P8 |
| Peg versus float under a capital-flow reversal | money, fiscal | P5a, P5b |
| Pandemic with and without export bans | hazards, SEIR | P8 (#222) |
| Same economy from 2000, fork at 2008, three policy responses | branch tree, fan chart | P5b |

## Dependencies and gates

- Needs the Mac or the built pack: P1 clean-machine run, P2 full-pack runs, P7 soak and 16 GB run, P13 timings (idle Mac only, #169), P9 capture.
- Needs Antonio: set choice (P2), Steam account and fee (P8), `.github/` items (P10 templates, drift job), scope exemption (card 3), trailer approval (P9), platform decision (card 1), licence reading for NC data on a free Steam listing (#231 and #264 cover the paid build; confirm the free Steam 1.0 is covered), employer filing (#267, deferred, not reminding).
- Release order: P3 gate vertical slice, then "coming soon" page and trailer v0, then P4 to P7 alongside the phases, then P2 set as phases land, then P8 submission, then a closed beta (card not posted; default is GitHub pre-release, Steam Playtest is the alternative), then launch.

## Risks

- Calibration credibility: a showcase finding that contradicts known history sinks the launch. Mitigation: pre-registered checks, drop failures, show uncertainty (fan charts), the limits page (P15).
- macOS-only on Steam (ADR 0216) misses most of the audience; Steam's macOS share is a low single-digit percent (verify). Card 1.
- First-run friction (above). Card 2.
- 45-goods tick at about 198 ms against 160: a "feel" risk at default speed; needs the layout RFC, not polish.
- Polish scope creep: 0262 alone is 25.8 focused days for all 34. Hold to the 27-item cut and Must tier until the P3 gate is green.

## Proposed issues (not created; awaiting Antonio's go-ahead)

Label `release-polish`, parent #179 or a new tracking issue, each with the acceptance test shown.

1. P1 First-run clean-machine path. Test: scripted clean-Mac run from install to first fork comparison.
2. P2 Showcase scenario set. Test: each scenario's hash reproduces on macOS and Linux; pre-registered check passes.
3. P3 First five lessons. Test: dismissing a lesson leaves the user's branch hash unchanged (ADR 0158).
4. P4 Accessibility pass. Test: palette check and reduce-motion check in CI; 200% scale screenshot.
5. P5 Reproduce and share UX. Test: app and headless run give the same hash.
6. P6 Run report export. Test: report names scenario, hash, sources, commit.
7. P7 Stability pack. Test: injected halt writes a bundle that `ceteris replay` reproduces; 100-year soak passes.
8. P8 Steam release path. Test: build installs from Steam on a clean Mac.
9. P9 Store assets, trailer, capture tooling. Test: `ceteris capture` twice gives identical frames.
10. P10 Community hooks (Antonio opens the `.github/` part).
11. P11 UI polish cut (0262). 12. P12 Cue subset (needs exemption). 13. P13 Performance feel. 14. P14 Prebuilt redistributable pack. 15. P15 Guide, limits, provenance. 16 to 18. P16 to P18 as Could.
