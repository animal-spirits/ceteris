# Retrospective, 12 November 2026

Scope: a look back over the project to date, written during the repo migration pause. It
synthesises the earlier reports rather than redoing them: progress 4 and 10 November, scope 7
November, development laws 8 November, roadblock recon 8 November, polish plan 9 November, CI
timings 9 November, infrastructure gaps 10 November, econ numbers vs literature 4 November, and
the 11 November modelling, toy-zoo, literature-baseline and public-release reports. Also read:
queue #179 and its comments, the open issue list (83 open), `git log` on main since the rewrite
(50 commits, 10 to 11 November), RFC 0257, 0313, 0342, `crates/ceteris-econ/src/`.

Nothing was re-run. Numbers carried from earlier reports keep their date. Claims I could not
check against code or GitHub are marked **(unverified)**. Effort is owner-weeks (ow) on RFC 0313's
basis, or S (under a day), M (days), L (a week or more).

## 1. Verdict

**Throughput is very high; validated progress is not keeping pace with it.** In the two days on
the rewritten main (10 and 11 November) 50 PRs merged: 10 P5 kernels, 9 P4 kernels or wirings,
6 data sources, the E3 price package, toy-zoo and literature-baseline harnesses, and a dozen
docs/ADR PRs. Over the same period, none of the economic misses recorded on 4 November was
re-measured, the P3 gate (#171) is still open, and the pack-based checks are blocked on a Mac that
was just migrated to a fresh checkout without a built pack.

The core law is "working, right, fast, in that order, per feature". The project is following it
for performance (spikes are recorded as no-go and dropped, see section 3.4) but not for phase
order: P4 and P5 code is being built while P3's gate and the C7 free-moment misses are open.

## 2. What changed since 4 November

| Area | 4 Nov | 12 Nov | Source |
| --- | --- | --- | --- |
| Phase | Pre-P3 closing, P3 about 40% (7 Nov) | P3 gate open (#171); P4 A1-A3, B1-B2, C1-C2, D1, D3, E merged; P5 A1, B1-B3, C1-C4, D1-D2, H merged as kernels | #179, git log |
| 1.0 estimate | 77-124 ow scope | 74-118 ow scope, 48-78 pace (RFC 0313, 6 Nov); not re-rated since | RFC 0313 D1 |
| Decision records | 312 rows, 290 RFCs, 214 proposed | 331 rows, 320 RFC files; by first status line about 95 accepted, about 214 proposed | `rfcs/`, `spec/rows.toml` (grep, approximate) |
| 10-goods tick | 41.2 ms vs 35 ms budget | about 34 ms (7 Nov) | scope-progress |
| 45-goods tick | not measured | 230 ms vs 160 ms ceiling (8 Nov; 329 ms on a loaded host earlier) | roadblock recon |
| Price rule (P5 E) | storage-on V1 and x10 collapse open | E1, E2, E3 merged, all off by default; x10 still fails on the fixture | RFC 0342, literature-baseline report |
| Licences | open | AGPL-3.0 + commercial for the simulator (#514); game source-available; CLA | ADR 0346-0348 |
| Data | 6 licence drafts pending | FAOSTAT, PRIMAP-hist, Ember, ICP, GLC_FCS30D added (#508-#512); FAOSTAT/PRIMAP pinned (#515); EIA replaces IEA (#499, ADR amendment pending) | git log |
| Repo | private atbraz/ceteris, Mac + droplet runners | animal-spirits/ceteris, rewritten main, CI move to hosted runners in #517 (open) | memory, open PRs |

## 3. Technology, data, modelling

### 3.1 Economic validation: the misses are unchanged and partly unmeasurable right now

The 4 November table still describes the model against the literature. No PR since then
re-measured any free moment:

| Moment | Model (date) | Literature | State |
| --- | --- | --- | --- |
| H2 distance elasticity | -0.454 (4 Nov) | about -1.1 | miss; each fix moved it further away |
| Hard / soft Brexit, UK welfare | -0.63% / -0.24% (4 Nov) | -2.66% / -1.34% (Dhingra et al. 2017) | miss, 4-5x small |
| ACR-IO correlation | 0.862 (4 Nov) | bar 0.9 | miss |
| US-China 2018 pass-through and welfare | agreed 28 Oct | near-complete, about -0.04% | untested since ADR 0200 (#245 open) |
| BACI PPML theta by class | 10.9-14.1, no spread (30 Oct) | class spread 2.9-16.9 | miss (#241) |
| Short-run trade elasticity (rho) | BACI + Teti fit was noise twice | about 0.76 at 1 year (BLP-N 2023) | unidentified (#200) |
| x10 border shock | fails: main dies at 1130 (default) and 601 (frictionless) | ACR autarky loss | fails (11 Nov fixture) |

Three things make this worse than the table suggests:

1. **"Agrees by construction" is a large share of the passes.** Openness, per-class theta, PIGL
   parameters, education returns and mode-cost ratios all pass because they are borrowed or
   fitted to the target (econ-numbers report, legend). They test the code, not the model.
2. **The C7 gate passed under redefined criteria.** RFC 0313 records "C7 has passed (#271)";
   RFC 0257 lets a tier-2 miss pass when an ADR traces it to an unbuilt mechanism, and the
   distance and Brexit misses were registered as tier 2 (#184, closed). That is within the rules,
   but it means the pre-P3 gate did not test the free moments. The 10 November report says the
   same: "the economics gate did not move".
3. **The literature baseline points at one parameter.** On the fixture, reverting the fitted
   border nu (ADR 0092) is the only switch that changes the x10 outcome (default mode survives;
   frictionless delays failure from tick 601 to 702). The distance-elasticity drift also began when
   nu was fitted. This is a single run per cell on one fixture, so it is a lead, not a finding.

**Risk:** P4 and P5 calibrate on top of the same trade core. Each new mechanism adds parameters
that can absorb the miss, which makes the trade-core error harder to locate later.

### 3.2 Built but not on: off-by-default features and unwired kernels

A grep of `crates/ceteris-econ/src` for references from outside each module:

| Module | Lines | Referenced from the tick or CLI | Status |
| --- | --- | --- | --- |
| `fiscal.rs` (P5 C1) | 223 | no | kernel only |
| `monetary.rs` (P5 B1) | 199 | no | kernel only |
| `hlw.rs` (P5 B3) | 345 | no | kernel only |
| `regime_hazard.rs` (P5 B2) | 150 | no | kernel only |
| `energy_policy.rs` (P5 C3) | 64 | no | kernel only |
| `state_cells.rs` (P5 C4) | 90 | no | kernel only |
| `investment.rs` (P5 D1) | 235 | no | kernel only |
| `entry_exit.rs` (P5 D2) | 115 | no | kernel only |
| `structural.rs` (P4 E) | 192 | no | kernel only |
| `sfc.rs` (P5 A1) | 224 | yes (4 refs) | kernel, partly used |
| `wagegrid.rs`, `disease.rs`, `settlement.rs` | 234, 282, 269 | yes, systems 225, demography, 229 | wired, off by default |
| E1/E2/E3 price package | n/a | yes, `PriceAnchor::e3` | wired, off by default |

About 1,600 lines of P4/P5 kernel code have unit tests but no path from a run. Under law 2
("runs end to end through the real code path, with a test that fails without it") these are not
yet "working"; they are closer to tested libraries. That is a reasonable way to parallelise, but
only if each kernel has a tracked wiring issue and a date. I found no wiring issue per kernel
(the P5 issues #213, #214 are umbrella issues) **(unverified beyond the issue list titles)**.

Off-by-default features carry a second risk: the default path is the one the goldens, benches
and literature checks exercise, so an off feature can rot. E3 is the clearest case: it is the fix
for the P5 price collapse, yet the 11 November baseline ran with the anchor "already off by
default", so E3 is not in the matrix.

### 3.3 Calibration: fitted versus structural parameters

- The project's own reports keep finding that fitting one target moves free moments the wrong
  way (border nu vs distance elasticity; buyer price vs welfare; anchor vs non-trading cells). The
  modelling research note (11 Nov, section 1.1) lists ten such collisions. The common pattern is a
  fitted wedge absorbing a missing mechanism.
- Tagging parameters fitted vs free (#185) was closed, but there is no check that a free moment is
  never also a calibration target except the C7 hold-out guard (RFC 0314). The P9 held-out
  scorer (#246) is the general version and is still open.
- Price, wage and employment speeds (kappa1 0.05, kappa_w 0.1, eta 0.3) are set by stability
  limits, not by micro moments (#242 open). Every P5 dynamic result depends on them.
- The rho prior (12-48 month half-life) is consistent with BLP-N's 1-to-10-year ratio (about 19
  months, roadblock recon section 2). That is a useful check that costs nothing and is not yet a
  test.

### 3.4 Determinism and goldens

- Strong. Bit-identical hashes across thread counts are checked; platform transcendentals are
  banned (#368, #430); a Linux golden check exists (#393). Failed speed-ups were bit-identical and
  were dropped for speed reasons, not determinism (batched dispatch 1.4-2.4x slower; produce
  attempt 1 slower), which is the laws working as written.
- Gap: no cross-architecture job yet (#203); Linux aarch64 deferred to the release candidate
  (11-12 card). The 4 November note that main's goldens and benchmark tests stay red until the
  2000 pack is recalibrated was closed by #183 **(unverified on the new checkout; the pack is not
  built there)**.
- 23 ignored tests in the workspace. Most are measurement harnesses or nightly pack runs; one is
  a real open finding (`zoo_pop.rs` Z36 momentum, 6.4% below analytic) and one is a deferred
  feature (route choice, ADR 0017).

### 3.5 Performance

10 goods meet the budget (about 34 ms vs 35). 45 goods do not (230 ms vs 160 ms, 8 Nov). The
recon's ranked plan (snapshot encode off the tick, then a goods-innermost spike, then a budget ADR
on the remainder) is sound and correctly sits behind the right gate. Nothing has moved since 8
November, which is correct under law 5 because the 50-goods pack (#206) and the economics are not
right yet. G1 flatness 1.083 against 1.25 is fine.

### 3.6 Data and licences

- Good: licence metadata is complete (`sources.toml`), the paid-build profile gate exists, no
  licensed raw data is in history (public-release audit 2.1), and five open sources landed in a
  day. AGPL + commercial removes the earlier MIT/Apache ambiguity for a paid build.
- Risk: the research build depends on non-commercial sources for its best identification
  (TRAINS, Teti, MAcMap for rho; EXIOBASE for IO). The "published estimate" source class (recon
  section 3) is the route for the paid build and is not yet an issue I can find **(unverified)**.
- Ember was added (#510) and then ruled out by Antonio on 12 November; its manifest entry needs
  removing or marking.

### 3.7 UI and release

Workbench data layers W1-W4, the session (fork, seek, run-to), halt panel and problem report are
merged; widgets are scheduled after C3/C4 (UI schedule, 10 Nov). Release polish P1, P2, P7, P8 are
issues #408-#411; nothing signs or notarises yet (infrastructure gaps, item 1). The release path
is not on the critical path today, but every item in it needs Antonio's accounts.

## 4. Process

### 4.1 Thread and queue workflow

- **Works:** one task per thread, spikes with go/no-go numbers, negative results written down
  (`docs/research/p5-*` spikes, #179 comments), decision cards answered quickly (seven on
  11-12).
- **Queue drift.** #179 is the queue of record but its checkboxes are stale: #165 (C6 gate),
  #174, #176, #184-#188 are closed yet unchecked; the list does not show the P4/P5 kernel work at
  all. The coordinator works from memory, so the queue and memory can disagree.
- **Decided issues left open.** #261-#267 carry the `decided` label and are still open. They are
  noise in the open count (83) and make the real backlog harder to read.
- **Batch shape.** The 11 November burst produced many small kernel PRs in parallel. That is
  efficient for throughput but each kernel is reviewed alone, so cross-kernel consistency
  (stock-flow, units, timing) is checked only when wiring happens.

### 4.2 Rules and records

- **Rule conflict.** `CLAUDE.md` says "Antonio alone decides and merges spec, RFC and research
  PRs"; the standing rule since 10 November (project memory) is that threads merge everything,
  including RFC and spec PRs, once CI is green. A new agent reading only the repo will follow the
  stricter rule. One of the two should change.
- **ADR status.** About two thirds of RFC files still say `proposed`; a sweep was done (#186) but
  new built work lands behind switches and stays proposed (RFC 0342 is "accepted, built behind
  `PriceAnchor::e3` (off by default)", which is the right form). The count is noise unless status
  is updated at wiring time.
- **Issue home.** CLAUDE.md and the development laws still name `atbraz/ceteris` as the issue
  home; the repo is now `animal-spirits/ceteris`.

### 4.3 CI

- 9 November sample: PR jobs on the single droplet runner had a median queue of 7 m and p90 of
  29 m (max 41 m), median duration 12 m; the `rows` check (6 s of work) queued 19 m median. Two of
  ten push jobs and one of fourteen PR jobs failed, at least two in nextest; whether these were
  flaky or real is not recorded.
- #517 moves PR checks to GitHub-hosted runners. Hosted minutes are free only once the repo is
  public (memory, Antonio 11-12); while private, cost depends on the plan **(unverified)**.
- Main CI skips `*.md` and `rfcs/**`, so docs PRs only run `rows`. That is cheap and right.

### 4.4 Single-Mac dependency

The Mac is still the push runner, the pack-gate runner, the only built pack, the quiet benchmark
host and the only signing host (infrastructure gaps, item 2). Antonio has no physical access. The
migration made this concrete: the fresh checkout has no built pack, so every pack-gated check
(C7 moments, x10 on the pack, W6 baseline, RFC 0286 screen) is blocked until the pack is rebuilt
or copied from the old checkout. The B2 backup bucket (11-12 card) and the recovery runbook (#460)
address loss; they do not address the queue of pack work waiting on one machine.

### 4.5 Cost

The project instructions ask each thread to log approximate tokens and model. Recent memory files
log tool-call counts (20-40 per Opus thread on the P5 E3 work) but not tokens. I cannot rank
threads by cost from the records. The P5 E3 diagnosis took at least five Opus threads (root
cause, spike A/B, island sink, D1/D2, credit limit), four of them no-go; each was justified, but a
cheaper fixture-level toy (the Hume two-island toy, Z13) would likely have isolated the money
sink first.

## 5. Ranked improvements

Payoff: H high, M medium, L low. Ranked by payoff over effort.

| # | Improvement | Effort | Payoff | Why |
| --- | --- | --- | --- | --- |
| 1 | **Rebuild or copy the 2000 pack on the new Mac checkout, then rerun the free moments** (H2 distance, Brexit, ACR-IO, US-China #245, x10) on current main, with and without E3 and with border nu reverted. One report, fixed pass numbers from RFC 0257. | M (Mac) | H | Every economic statement since 4 Nov rests on numbers that are a week old and pre-date E1-E3. |
| 2 | **Wire-or-park rule for kernels**: no new P4/P5 kernel until the existing unwired ones (section 3.2) each have a wiring issue, and at most three are unwired at a time. | S | H | Keeps "working" honest; makes cross-kernel stock-flow errors show up early. |
| 3 | **Fix the CLAUDE.md merge rule and repo name** to match the 10 November standing rule and `animal-spirits/ceteris`. | S | M | New threads read the repo, not memory. |
| 4 | **Border-nu investigation** as a research issue: refit with distance and Brexit as joint targets, or replace the fitted wedge with the unbuilt mechanism (NTBs, log-distance friction ADR 0272). | M | H | The single best lead for three misses at once (3.1 item 3). |
| 5 | **Off-by-default review at each phase gate**: every switch either becomes default with a golden move, gets a dated issue, or is deleted. Add E3 to the literature-baseline matrix now. | S | M | Prevents switch rot; tests E3 where it matters. |
| 6 | **Queue hygiene**: tick #179 against closed issues, close #261-#267, add the P4/P5 kernel and wiring items. | S | M | One source of truth for the coordinator. |
| 7 | **Hume two-island toy (Z13) before any further P5 money work.** | S-M | M-H | Cheapest test of whether E3 gives price-specie flow; would have shortened the E3 diagnosis. |
| 8 | **Micro-moment speeds (#242)**: calibrate kappa1 to price-change frequencies and kappa_w to a wage Phillips slope before P5 dynamics are reported. | M | M-H | Every dynamic result depends on these. |
| 9 | **Pack mirror off the Mac** (B2, decided) plus a second pack-capable host for pack-gated tests only (a rented Linux box, since pack tests are not Mac-specific **(unverified: GEOS/tippecanoe builds on Linux)**). | M | M | Removes the queue of pack work behind one machine. |
| 10 | **Cost log**: one line per thread with model and token estimate, as the project instructions already require. | S | L-M | Lets the coordinator see which thread types are expensive. |
| 11 | **Re-rate 1.0 on measured pace** at the P3 gate, separating "kernels merged" from "validated". | S | M | RFC 0313's 0.1-0.4 measured ratio was one phase; kernel throughput overstates progress. |
| 12 | **Published-estimate source class** for the paid build (recon section 3) as an RFC. | M | M | The paid build otherwise has no identified rho or IO validation. |

Not recommended: more performance work before item 1 and #206 (law 5); new data sources before
the pack is rebuilt; GPU (no f64 on Apple GPUs, recon 1g).

## 6. What is on track, slipping, at risk

- **On track:** determinism, the laws' handling of speed-ups, licences and data provenance, P4
  population pipeline (wired systems with toy checks), decision turnaround.
- **Slipping:** P3 gate (#171) and its branches E, H, J; the 45-goods budget; the economics misses;
  issue hygiene.
- **At risk:** validation evidence (no fresh numbers, pack unavailable), unwired P5 kernels,
  off-by-default switches outside test matrices, the single Mac, and calendar (RFC 0313: 1.0
  between September 2029 and January 2034 at 12 hours a week, depending on basis).
