# Peripheral and quality-of-life features, 11 November 2026

Scope: auxiliary, nice-to-have and QoL features not yet in the plan. Sources read: `reports/second-pass-brainstorm-2026-11-08.md`, `pre-1.0-polish-plan-2026-11-09.md`, `infrastructure-gaps-2026-11-10.md`, `ui-schedule-2026-11-10.md`, `research/ceteris-brainstorm-additions.md`; RFC titles grepped for 0116, 0158, 0188, 0196, 0197, 0219, 0220, 0221, 0258, 0262. Effort S under 2 days, M under 2 weeks, L longer; value H/M/L; all my estimates, not measurements. RFC statuses are as found in `rfcs/` (many still say proposed). No issues or queue rows created. 1.0 = pure simulator; game mechanics are 2.0+.

Legend: PLANNED = already covered (anchor given, not re-estimated). EXTEND = planned item with a gap worth adding. NEW = not found in plans or RFCs (grep, not exhaustive).

## Top 5 recommended

1. Run bundle (`ceteris bundle`) plus `CITATION.cff` and seed/hash/commit stamp on every export. NEW, S-M, H, 1.0.
2. Autosave and crash recovery of the edit log (resume after kill). NEW, S, H, 1.0.
3. Run notes and bookmarks on the timeline (user text pinned to tick x branch, stored as non-state metadata). NEW, S, M-H, 1.0.
4. `ceteris sweep`: batch/parameter-sweep runner over a scenario file, headless, Arrow output. NEW, M, H, 1.0 (CLI only).
5. Pack inspector (`ceteris pack inspect` and in-app view: coverage, provenance, fitted vs free tags). NEW, S-M, M-H, 1.0.

## 1. Run and scenario management

| Feature | Status | Effort | Value | Fit | Depends on |
| --- | --- | --- | --- | --- | --- |
| Named runs and branches, fork, seek, edit log | PLANNED (W1, 0116, 0188) | - | - | 1.0 | - |
| Undo/redo of edits as branch-version steps | PLANNED in spirit (0188 undo tree, 0262 undo toast) | - | - | 1.0 | - |
| Autosave of edit log plus "resume last session" after crash or quit | NEW | S | H | 1.0 | scenario file, 0211 run files |
| Bookmarks: named (tick, branch, view) positions, jump list | NEW | S | M | 1.0 | W1 cursor |
| Run notes and annotations (free text on tick/branch/entity; stored outside the run hash) | NEW | S | M-H | 1.0 | W1, run files |
| Fork diff as text/table: edit-log diff and first-divergence tick between two branches | EXTEND (W2 is the map diff only) | S | H | 1.0 | W1, CRN |
| Saved workspaces (panel layout, lens, link groups) | EXTEND (0196 link groups, 0258 saved views) | S | M | 1.0 | panel host |
| Divergence bisect: "find first tick where two runs differ" in CLI | NEW | S | M | 1.0 (also a debug tool) | journal hashes |
| Recent scenarios list, scenario templates | NEW | S | L-M | 1.0 | scenario files |
| Scenario gallery in app | PLANNED (P18) | - | - | 1.x | - |

## 2. Export and reproducibility

| Feature | Status | Effort | Value | Fit | Depends on |
| --- | --- | --- | --- | --- | --- |
| Run report (HTML/PDF), `ceteris://` links, run-hash badge, reproduce button | PLANNED (P5, P6, 0197) | - | - | 1.x | - |
| Repro bundle on halt | PLANNED (0214, P7) | - | - | 1.0 | - |
| Run bundle on demand: scenario + pack hash + edit log + commit + results digest in one zip, `ceteris replay bundle.zip` | NEW (generalises the halt bundle) | S-M | H | 1.0 | 0214 bundle |
| `CITATION.cff`, Zenodo DOI per release, "how to cite this run" copy button | EXTEND (0215 has Zenodo for sources; 0262 N5 copies citations of values) | S | M-H | 1.0 | public repo |
| Seed, pack hash, commit, thread count stamped into every export and chart footer | NEW | S | H | 1.0 | exports |
| Chart/map image export with embedded provenance (PNG/SVG metadata, licence-aware per 0213) | EXTEND (0197 SVG export) | S | M | 1.0 | 0213 |
| CSV/Arrow/xlsx/GeoPackage export | PLANNED (0197) | - | - | 1.0 | - |
| Frame-exact capture / timelapse from a hash (`ceteris capture`) | PLANNED (P9) | - | - | 1.x | - |
| Determinism self-test button ("verify this install"): replays a bundled 24-tick fixture, shows hash match | NEW | S | M | 1.0 | goldens, P5 |

## 3. Scripting and API

| Feature | Status | Effort | Value | Fit | Depends on |
| --- | --- | --- | --- | --- | --- |
| `ceteris serve` read-only, `ceteris://` links | PLANNED (0197) | - | - | 1.0 | - |
| Pure-Python socket client (no PyO3) | PLANNED (0197, 1.x) | - | - | 1.x | serve |
| Writes and lockstep controllers | PLANNED (0197, 1.x) | - | - | 1.x | serve |
| Headless CLI parity with app commands | PLANNED (0113, 0188 registry) | - | - | 1.0 | - |
| `ceteris sweep` / ensemble-over-parameters runner: grid or Latin-hypercube over scenario edits, CRN, results in Arrow | NEW (CRN ensembles exist; a sweep driver does not) | M | H | 1.0 CLI | CRN, headless |
| Example notebooks (Jupyter/marimo) using the Python client: fork, compare, plot | NEW | S-M | M | 1.x | Python client |
| JSON Schema for scenario and edit-log files, editor autocomplete | NEW | S | M | 1.0 | run files |
| Shell completions and `--help` examples, man pages | NEW | S | L-M | 1.0 | CLI |
| MCP adapter | PLANNED (0197, 1.x) | - | - | 1.x | - |
| Policy-bot tournaments | brainstorm only | S-M | M | 2.0+ | writes API |

## 4. Data tooling

| Feature | Status | Effort | Value | Fit | Depends on |
| --- | --- | --- | --- | --- | --- |
| Pack inspector: sources, vintages, licences, coverage by country/year, imputed vs measured | NEW (data-quality map mode in brainstorm, view only) | S-M | M-H | 1.0 | 0217 profiles |
| Per-series provenance pages | PLANNED (P15, 0219) | - | - | 1.x | - |
| Validation dashboard: fitted-vs-free moment scoreboard, hindcast errors, per-pack-change deltas | EXTEND (second-pass C4; no UI) | M | H | 1.0 (dev) / 1.x (in app) | #185 tags |
| `perf/history.csv` and trend page | PLANNED (second-pass A4) | - | - | 1.0 | - |
| Pack diff: what changed between two pack builds (series, values, hash) | NEW | S-M | M | 1.0 | pack manifest |
| Stale-pack detector with clear error | PLANNED (second-pass C2) | - | - | 1.0 | - |
| Data-quality map mode | brainstorm only | S | M | 1.x | inspector |
| User data overlay: import a CSV series and plot against sim output (observed vs simulated) | NEW | M | H | 1.x | indicator registry 0258, units |

## 5. Accessibility

| Feature | Status | Effort | Value | Fit | Depends on |
| --- | --- | --- | --- | --- | --- |
| CVD palettes, UI scale 200%, reduce-motion, keyboard-complete, contrast audit | PLANNED (0221, P4) | - | - | 1.x | W6 |
| Screen-reader spike | PLANNED (P17) | - | - | 1.x | Slint a11y |
| High-contrast theme and light/dark theme switch | NEW (no dark/light RFC found) | S-M | M | 1.x | tokens 0137 |
| Text-first alternative for charts: sortable data table beside every chart | NEW | S | M-H | 1.x | chart panels |
| Sonification of a series | brainstorm-adjacent only | M | L | 2.0+ | audio |
| Font-size and dyslexia-friendly font option | NEW | S | L-M | 1.x | UI scale |

## 6. Localisation

| Feature | Status | Effort | Value | Fit | Depends on |
| --- | --- | --- | --- | --- | --- |
| Fluent pipeline, English-only 1.0 | PLANNED (0220) | - | - | 1.0 | - |
| Number/date/currency formatting by locale, units chips | PLANNED (0258, 0262 N3-N4) | - | - | 1.0 | - |
| Community translation workflow (Weblate or Fluent files via PR) | NEW | S | M | 1.x | public repo, 0220 |
| Pseudo-locale test (catches truncation and hard-coded strings in CI) | NEW | S | M | 1.0 | 0220 |

## 7. Onboarding and learning

| Feature | Status | Effort | Value | Fit | Depends on |
| --- | --- | --- | --- | --- | --- |
| Five lessons plus lesson frame, in-app pedia | PLANNED (0158, P3, P16) | - | - | 1.x | - |
| Glossary tooltips on every term, linked to pedia | EXTEND (0158 pedia; 0262 T1 tooltips) | S | M | 1.x | pedia |
| "What this model is not" page | PLANNED (P2/P15) | - | - | 1.0 | - |
| Sample scenario opens on first launch with a guided first fork | EXTEND (P1 setup, P3 lessons) | S | H | 1.0 | P2 flagship |
| Offline copy of docs inside the app bundle | NEW | S | M | 1.0 | mdBook P15 |
| "Explain this number" (F12 explains, 0188) with the source and equation id | PLANNED (0188, W4 why panel) | - | - | 1.x | - |
| Classroom mode (instructor scenario + student logs + replay grade) | brainstorm only | M | M-H | 2.0+ | bundles |

## 8. Performance tooling

| Feature | Status | Effort | Value | Fit | Depends on |
| --- | --- | --- | --- | --- | --- |
| `bench_quiet.sh`, CPU-time gates, history csv, span profile `--profile` | PLANNED (second-pass A2-A5) | - | - | 1.0 | - |
| Honest speed readout, cancellable jobs, progress and ETA | PLANNED (P13, 0262 F4) | - | - | 1.x | - |
| Memory readout and "projected disk for this run" before start (retention estimate) | EXTEND (0212 retention) | S | M | 1.0 | 0212 |
| In-app profiler overlay (per-system ms of last tick) | EXTEND (A5 CLI only) | S | L-M | 1.x | A5 |
| Hardware check at first run: RAM/GPU vs minimum, suggested goods count | NEW | S | M | 1.0 | P1, 16 GB floor |

## 9. Modding and pack sharing

| Feature | Status | Effort | Value | Fit | Depends on |
| --- | --- | --- | --- | --- | --- |
| Scenario files shared as file + hashes; recipient rebuilds | PLANNED (0213, 0211) | - | - | 1.0 | - |
| War-surface modding | PLANNED (0166) | - | - | 2.0+ | - |
| Scenario packaging: scenario + custom data overlays + readme as one shareable archive, licence-checked | EXTEND (P18) | M | M | 1.x | 0213 |
| Share codes (short string = seed + pack hash + log digest) | brainstorm only | S | M | 1.x | bundles |
| Custom indicator definitions (user-defined derived series from a small expression language) | EXTEND (0258 registry, ex-command `:where`) | M | M-H | 1.x | 0258 |
| Steam Workshop for scenarios | NEW | M | M | 2.0+ | gallery, P8 |

## 10. Docs site

| Feature | Status | Effort | Value | Fit | Depends on |
| --- | --- | --- | --- | --- | --- |
| mdBook guide, limits page, provenance pages | PLANNED (P15, 0219) | - | - | 1.x | - |
| Rust API docs (`cargo doc`) and CLI reference generated from clap, published with the book | NEW | S | M | 1.x | public repo |
| Searchable ADR/RFC index page generated from `spec/rows.toml` | NEW (rows tool exists, no site) | S | M | 1.x | public repo |
| Changelog in app ("what's new"), release notes from CHANGELOG | EXTEND (CHANGELOG exists) | S | L-M | 1.0 | - |
| Model card / ODD document | PLANNED (blind-spots 15.7 gap) | - | - | 1.0 | - |

## 11. Testing tools

| Feature | Status | Effort | Value | Fit | Depends on |
| --- | --- | --- | --- | --- | --- |
| Property tests on ledger, edit-log replay, fork equivalence | PLANNED (second-pass D1, #409 context) | - | - | 1.0 | - |
| Cross-arch hash job | PLANNED (H, infra gap 4) | - | - | 1.0 | - |
| Fuzz and 100-year soak | PLANNED (P7) | - | - | 1.0 | - |
| App screenshot regression tests (headless Slint render, per-view golden images, tolerance) | NEW | M | M | 1.x | app CI |
| Scenario-file linter (`ceteris scenario check`: schema, unknown ids, out-of-range, licence) | NEW | S | M | 1.0 | schema |
| Invariant monitor toggle: run with conservation asserts on in the app, surfaced in halt panel | EXTEND (0214) | S | M | 1.0 | halt panel |
| Replay-diff tool when a golden moves: which system first diverged | EXTEND (bisect above) | S-M | H | 1.0 (dev) | journal hashes |

## 12. Community and support

| Feature | Status | Effort | Value | Fit | Depends on |
| --- | --- | --- | --- | --- | --- |
| Discussions, issue templates, CONTRIBUTING | PLANNED (P10) | - | - | 1.0 | - |
| "Report a problem" with prefilled GitHub issue | PLANNED (#461, 0214) | - | - | 1.0 | - |
| Opt-in usage stats | dropped for 1.0 (0340/0342) | - | - | - | - |
| Public roadmap page generated from queue and milestones | NEW | S | L-M | 1.0 | public repo |
| Known-issues/limits list surfaced in app next to the version | NEW | S | L-M | 1.x | P15 |
| Citing and "papers using ceteris" list | NEW | S | L-M | 1.x | CITATION.cff |
| Contributor onboarding: devcontainer or `scripts/setup.sh`, good-first-issue labels | NEW | S | M | 1.0 | public repo |

## Notes

- Scope guard: only P1, P2, P7, P8 are exempt from 1.x. "1.0" above means cheap enough to argue for; each NEW row still needs an issue and, where it touches an accepted ADR, an RFC.
- Several top-5 items share one mechanism (bundle, schema, metadata that is outside the run hash). Spec the metadata rule once: notes, bookmarks and workspaces never enter hashes (same rule as keymaps in 0188).
- Rank basis: value to the install, fork, compare, verify, share loop in the polish plan, divided by effort, restricted to items with no Mac or licence blocker.
- Not covered: game-facing ideas in `research/ceteris-brainstorm-additions.md` (seats, forecast game, puzzles); they remain 2.0+.
