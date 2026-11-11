# R Implementation Plan: release 1.0

**Goal:** Ship 1.0: a calibrated, validated economic world (P0 to P5, P8, P9) that a user can fetch, build and run on a clean machine and get the published hashes (spec/21 section 21.1, R Release; ADR 0207). This plan is a skeleton at the level of tasks and named tests; each branch's detailed plan is written when it starts.

**Gate (spec/21 R Release):** a clean machine builds the pack and reproduces the published 1.0 hashes. It needs the user-side fetch and build of the pack including manual sources, a signed macOS build, hashes checked across architectures, and a minimum pedia.

**Starts from:** main after the P9 gate for the gate itself. Branches A, E, F and G need nothing from P9 and can start earlier.

**Spec:** new `specs/release/SPEC.md` (pack profiles, fetch and build, packaging, pedia, hash publication); `crates/ceteris-pack/config/sources.toml` (licence verdicts); spec/16, 19, 21; ADRs 0005, 0050, 0113, 0127, 0128, 0158, 0207 and the release decisions on #261 to #266.

**Out of R:** everything ADR 0207 moved to 1.x or 2.0; the in-game download menu; music; the paid build; trademark registration.

## Decisions in force (from project memory, 2026-11-06)

| Issue | Decision |
|---|---|
| #178 | Licence AGPL-3.0-or-later with commercial licences from the owner (ADR 0346); CLA for outside contributions (ADR 0348). |
| #230 | `[project] distribution` in `sources.toml` flips to open in the release PR, after the NC audit below. |
| #261 | Trademark registration deferred past 1.0. A fork-rename policy is therefore not a 1.0 item. |
| #262, #263 | 1.0 is free on Steam; the paid build is 2.0 at 14.99 USD; no Early Access. |
| #264 | No NC data in the paid build; NC-only sources are free-track opt-in. |
| #265 | Fetch script now; no in-game download menu in 1.0. |
| #266 | No music in 1.0. |

Note: a second set of comments on #261, #262, #265 and #266 (6 Nov, 15:31, "All recommended") reads differently (register the name, paid Steam 1.0, script only, CC0/CC BY music fetch). The table above follows project memory and the first comments; the owner has an open question card on it, and Branches F and H change only if he picks the other set.

## Open points for the owner

| # | Question | Default if not answered |
|---|---|---|
| O1 | ADRs 0050, 0128 and 0158 are still `proposed`. Accept them as written for the 1.0 path in one status RFC? | Branch 0 writes that RFC with one decision per ADR; nothing is built before it is accepted |
| O2 | ADR 0158 also describes onboarding lessons; the R line of spec/21 asks for a minimum pedia only. Lessons in 1.0? | Pedia only; lessons join the lab-UI track after the P3 gate (ADR 0323) |
| O3 | A signed macOS build needs an Apple Developer ID and notarization credentials, which only the owner can hold. When are they provided? | The signing step is built against a test certificate and stays unrun until they exist; the workflow file is a `.github` change for the owner |

## Global constraints

Earlier phases' constraints carry over (determinism, exact money, goldens only re-blessed in a commit that says why, decision ids and register codes from the rows tooling in the PR that writes them). Also:

- **Releasable only.** The release pack reads no source with `commercial_use = false` or `closed_source_only = true` (ADR 0050); `pack check-sources --profile release` is the check.
- **No `.github` edits from `claude/*` branches.** Workflow changes are drafted under `docs/ci/` and applied by the owner.
- **Reproducible.** Published hashes carry the engine commit and toolchain (`rust-toolchain.toml`, `Cargo.lock`).

---

## Branch 0: `r/records` (before code)

- [ ] Write `specs/release/SPEC.md` from `specs/TEMPLATE.md`.
- [ ] RFC for O1 with one decision per ADR; record the decisions table above and O2 and O3.
- [ ] Commit `docs(release): r spec and accepted release adrs`.

## Branch A: `r/pack-profiles` (ADR 0050; #256, #231)

- [ ] **A1** Release profile: any source with `commercial_use = false` or `closed_source_only = true` fails the build if the pack reads it; the research profile allows it with `use` in validation, complement or fallback. The CLI flag exists; the tests make it the gate. Failing tests: `fixture_manifest_with_an_nc_source_fails_release_and_passes_research`; `release_pack_provenance_lists_no_non_commercial_source`.
- [ ] **A2** Pack manifest records its profile; the scorecard records which profile produced it. Test: `manifest_profile_is_recorded_and_scorecard_labels_research_only_results`.
- [ ] **A3** NC audit (#231): a per-source verdict in `sources.toml` for every source the 1.0 pack reads (38 of 123 entries carry `commercial_use = false` today); sources with unverified licences (IEA, Gennaioli and LIS are the known ones) get a verdict or a replacement before `distribution` flips. Test: `every_source_the_release_pack_reads_has_a_verdict_and_a_check_date`.

## Branch B: `r/fetch-and-build` (#228)

- [ ] **B1** User-side `ceteris pack fetch` for every releasable source, with hash pinning per file, and manual sources (those that need a registration or a click-through) listed with exact instructions and the expected file hash. Failing tests: `fetch_verifies_every_file_hash`; `missing_manual_source_stops_with_the_instruction_and_the_expected_hash`.
- [ ] **B2** `ceteris pack build` on a clean machine from the fetched files (GEOS and tippecanoe requirements documented or vendored). Test: `clean_machine_build_reproduces_the_published_pack_hashes` (Mac or a clean runner).
- [ ] **B3** Fetch script instead of an in-game menu (decision #265). Test: `fetch_runs_without_the_app`.

## Branch C: `r/cross-arch-hashes` (ADR 0128)

- [ ] Golden hashes of the 1.0 run equal on macOS arm64, Linux x86_64 and Windows. Failing test first: the cross-platform golden job (a `.github` change, drafted under `docs/ci/` for the owner). Test: `golden_hash_equal_on_all_three_platforms`.

## Branch D: `r/signed-macos` (O3)

- [ ] Release build, code signing and notarization of the macOS app; the signing step takes credentials from the environment and fails closed without them. Test: `unsigned_build_is_refused_by_the_release_script`. The workflow file is drafted under `docs/ci/`.

## Branch E: `r/pedia` (ADR 0158; #229)

- [ ] Terms in tooltips link to concept pages generated from `spec/` and `rfcs/` at build time: the equation, current parameter values with sources and register ids, and the deciding ADRs (the parameter pages of ADR 0127 are the same pages). Failing tests: `every_pedia_page_names_a_source_file_and_commit`; `build_fails_on_a_dangling_term_link`. Acceptance (#229): the pedia ships with the 1.0 app.

## Branch F: `r/open-distribution` (#230)

- [ ] In the release PR: `[project] distribution = "open"` in `sources.toml` after Branch A3 passes, the licence files and README licence section checked, the changelog entry. Test: `check_sources_passes_with_distribution_open` (fails while any closed-source-only source is read).

## Branch G: `r/steam-listing` (decisions #262, #263, #266)

- [ ] Free Steam listing for 1.0, no Early Access, no music, no paid build. The store page, build upload and depot layout are owner actions; this branch produces the artifact list and the hashes to publish. Test: `release_artifact_list_matches_the_published_hashes`.

## Branch H: gate

- [ ] A clean machine fetches, builds and reproduces the published 1.0 hashes on all three platforms; the signed macOS app opens; the pedia ships. Recorded in `tests/perf/r-gate.md` and the changelog.

## Dependencies

0 before any code; A before F (the flip needs the audit); B before the gate; C and D need the owner's CI changes and credentials; E independent; G after B and C; H last.
