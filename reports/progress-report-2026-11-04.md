# Ceteris Paribus: progress report, 4 November 2026

Sources: `origin/main` (131 commits, last 3103d07 #124), `origin/pre-p3/audit` (50 commits ahead
of main), `spec/` (design, migrated from DESIGN.md), `spec/rows.toml` (312 decision rows),
`rfcs/README.md` (290 RFCs), `specs/pre-p3/PLAN.md`, ADRs 0207 and 0248, project memory topic
files. Counts below come from greps over those files; where a figure is a recollection from
memory and not re-measured today, it says so.

## 1. Verdict

**Ahead on code, behind on economics, and the plan keeps growing.**

- Code throughput beats the plan. In 12 days since the design snapshot (23 Oct) P0, P1, P2,
  the full Python-to-Rust migration, the native app shell and most of pre-P3 have landed.
  ADR 0248 measured code at 5 to 10 times the pace ADR 0207 assumed.
- The economics are not yet at the pre-P3 gate. C6 (recalibrate and re-run the P2 gate) is
  running on the Mac; C7 (the 28 October benchmark rerun, the actual gate) has not started.
  Several fit criteria have been met only after being redefined (Section 5).
- Scope has grown faster than work has been retired: 1.0 went from 52-82 owner-weeks
  (ADR 0207, 28 Oct) to 80-129 (ADR 0248, 30 Oct) to 83-135 in the timeline artifact with
  later accepted ADRs folded in. That is +60% in a week. Remaining work: 77-124 owner-weeks on
  the scope basis, 51-84 on the pace basis; at 12 h/week from Feb 2027 that is roughly
  Oct 2029 (pace, best) to Jan 2035 (scope, worst). The owner chose the scope basis and a
  re-rate at the pre-P3 gate.
- Main CI's rust job is red until the 2000 pack is recalibrated for main's code (stale static
  digest since f1ffeb2; memory, 3 Nov). The nightly full-pack gate is live but expected to
  fail on goldens until the pack re-bless in C6.

On track for 1.0? Not measurably yet. The honest status is "on plan for the order of work,
off plan for the size of it". The pre-P3 gate re-rate (owner-weeks actually spent on B and C
against the 0248 table) is the first real test; until then any date is the 0248 range.

## 2. What is implemented, by phase

Phase order (spec 21, ADR 0207): P0, P1, P2, pre-P3, app shell, P3, P4, P5a, P5b, P8, P9, R
(= 1.0); P6, P7, P10 are 2.0+.

| Phase | Status | Evidence |
| --- | --- | --- |
| P0 Foundations | Done | `ceteris-core`: entity ids, scheduler, Philox RNG, double-entry ledger, event log, snapshots; golden `p0-toy.txt`, `p0_acceptance` test |
| P1 World and map | Done | `ceteris-geo` (graph, least-cost paths, route choice built and off), `ceteris-pack` geo/raster/web; goldens `p1-geo`, `p1-pack`; map moved from web to native (ADRs 0011, 0088) |
| P2 Economic core | Gate passed 26 Oct (with V1 recast, see 5) | `ceteris-trade`, `ceteris-econ`, `ceteris-reference`, `ceteris-calibrate`; goldens `p2-econ`, `p2-pack`, `p2-fixture`; `p2_acceptance`, `p2_gate` |
| Rust migration (ADR 0113) | Done | No `python/` left; GE reference, PPML, Sinkhorn, calibration, pack builders in Rust |
| App shell (ADR 0088) | Done 30 Oct (#78 to #95) | Slint 1.18.1 + wgpu 30 in `app/`; 120 fps p50 on the Mac, 53k-county recolour 0.09 ms; Tauri deleted |
| Pre-P3 A (speed, ADR 0097) | Done (#28) | Determinism lints, gather dispatch, `Crossing` rewrite, compressed history |
| Pre-P3 B (econ review) | Done (#127) except B6 | B1-B5, B7, B8 built; B6 split price rule deferred to P5 (ADR 0090); storage-on frictionless V1 misses deferred (RFC 0286) |
| Pre-P3 C (audit) | C1-C5 done on `pre-p3/audit`; C6 running; C7 not started | ADRs 0282-0286, 0289-0290, 0294-0312 (price levels, transfers, three final demanders, labour shares, dividends, wealth shares, drift) |
| Pre-P3 D (P3 prerequisites) | Done (#134, D5 5f19110) | Interventions replayed on seek, network by content digest, params as snapshot state (ADRs 0291-0293); nightly pack gate on the self-hosted Mac runner |
| Pre-P3 E (ensemble lanes) | Moved to P3 Branch L (ADR 0288) | |
| Pre-P3 F (design snapshot) | Superseded by #126 (spec split into `spec/` + `rows.toml`) | |
| P3 early items | G2 done (#120); 0277, 0279 (#125); 0268, 0269 (#128) | Goods count from the pack; least-iceberg paths per good; air share; province-graded ACR |
| P3 to R | Not started | |

Size: about 85k lines of Rust in 9 crates plus 4.5k in the app; 993 `#[test]` functions,
58 integration test files, 6 golden-hash files.

### Merged PRs since 29 Oct on main (selection)

#100 whole first-level provinces (0251); #101 fidelity tiers (0257); #113 self-hosted CI;
#114-#118 goods at ICIO level, log-distance friction, seeds by RAS (0271-0275); #119 licences;
#120 goods count from pack; #121 air freight (0277); #122 energy scope (0278); #124 tick speed;
#125 least-iceberg paths (0279); #126 decision tracking and spec split; #127 econ review (B);
#128 graded ACR and P04 borrowed costs (0268, 0269); #129 seat placement (0239); #130 PWT vs WDI
study; #131 nearest-year seeds (ADR 0287, RFC 0283); #132 lanes into P3 (0288); #133 storage misses
deferred (RFC 0286); #134 Branch D (0291-0293); #135; #136 CLAUDE.md cut. On the audit branch:
#137 to #143 (C1 to C6 code).

## 3. Open problems right now

1. **C6 pack issues (from C2, on the Mac pack):** Ethiopia gets a negative PIGL weight on
   rebuild; the installed pack with transfers does not converge on RAS pass 1; pass-1 RAS
   factors 1.27 to 3.17 (coal highest). Diagnose before C6 completes; Antonio to see the
   fixed-point factors first (audit hand-off).
2. **Mac fetch owed:** `pack fetch --record` for SEA and AAMNE.
3. **Tick budget:** 41.2 ms (criterion) vs 35 ms budget; owner stopped tick work on 4 Nov and
   re-rates the budget at the P3 gate. Known unbuilt wins about -4.7 ms (journal as owned
   blocks, parallel goods moves).
4. **Main's goldens and benchmark tests red** until the 2000 pack is recalibrated.
5. **Proposed but built:** at least 17 ADRs that the code implements are still `proposed` in
   the index (0092, 0093, 0094, 0098-0101, 0103, 0104, 0106, 0247, 0256, 0266, 0267, 0276,
   0280, 0281). Their "as built" and status updates are part of C6/B records.
6. **Fit, from the 30 Oct audit and 1 Nov readiness review (memory, not re-measured):** H2
   distance elasticity -0.454 (B8 pack) against a literature range near -1, now a reported
   diagnostic (ADR 0264); PPML thetas 10.9 to 14.1 for every class; ACR country correlation
   0.862 misses its bar; P04 mode-cost fit never converged, borrowed priors in use (ADR 0269).
   Openness on the B8 pack is within 1.5x ICIO (1.151, 1.043, 1.144, 0.973), but no asserting
   test exists until C7.

## 4. Full technical roadmap to 1.0

Estimates in owner-weeks, ADR 0248 (scope basis / pace basis), remaining at 30 Oct.

| Phase | Content | Gate (spec 21.1) | Scope | Pace |
| --- | --- | --- | --- | --- |
| Pre-P3 (rest) | C6 recalibrate, re-bless, B8 gate rerun; C7 benchmark (openness within 1.5x ICIO, ACR holds, x10 reported) | P2 gate on recalibrated pack + benchmark | (8-13 total) | (6-10) |
| P3 Workbench | Branches, all 14.2 intervention types, CRN, ensembles on lanes `const L` (Branch L, ADRs 0024, 0288), member draws from register priors, difference maps, fan charts, why panel, CLI + scenario files + Arrow exports, replay loader (0241), 0234-0236 gates; 50-goods pack (0271-0275, 5-7 wk est.) | Null intervention = parent; year relabel keeps hash; CRN tightens tariff intervals | 9-14 | 4-7 |
| P4 Population | Brass mortality and fertility, education, occupational mobility, DDC migration, balanced-growth calibration with country trends and moving GE reference (0243), i128 money | UN 2000-2010 within tolerance; population conserved | 7-11 | 5-8 |
| P5a Money and fiscal | Currencies, regimes, central bank rules, taxes, budgets, SFC matrices, quarterly investment step (7.4, 0202, 0204), split price rule (0090), storage-on fix (RFC 0286), x10 collapse fix | SFC closes every tick; policy shock signs and lags | 6-10 | 4-7 |
| P5b Banks, debt, flows | Banks, housing, equity, sovereign bonds and default, IMF, portfolio flows, sequence-space HANK reference in Rust (0025), full finance (0227, 0229, 0230), hazards (0242) | Responses within tolerance of HANK; spreads respond | 17-27 | 12-19 |
| P8 Growth and energy | Frontier growth, technology jumps, catch-up (0108), learning curves, structural change, emissions, pandemics (SEIR), chokepoints (0242); energy scope (0278) | 2000-2010 growth, shares, emissions; stable to 2150 | 7-12 | 5-8 |
| P9 Calibration and hindcast | Pre-registration, joint block check, SBI, emulators, Sobol (all Rust), hazard fits, 2011-2024 out-of-sample with replay on/off, 2005/2010 origin packs, held-out sources (report-only until P9), `Real`/`Dual<N>` retrofit (0096) | Scorecard (15.4) and natural-experiment table (15.5) with CRPS and coverage | 16-25 | 12-19 |
| R Release | User-side pack fetch and build, signed macOS build, cross-architecture hashes, minimal pedia | Clean machine reproduces 1.0 hashes | 5-9 | 3-6 |
| **Total remaining** | | | **77-124** | **51-84** |

After 1.0: climate emulator and damages as 1.1 (ADR 0207); P6 politics 2.0-alpha, P7 war 2.0-beta,
P10 Steward mode 2.0-alpha/2.x; seats and all game mechanics 2.0+ (ADRs 0239, 0240).

Additions accepted after 0248 and not in its table (timeline artifact, 30 Oct): relief 0255
(+4-8), conflict 0254 (+2-3), Paradox cues 0252 (+2-3). Since then: 50-goods pack (5-7 vs 0271's
2-4 guess), audit RFCs 0282-0303 (larger than C's original five tasks), held-out validation
sources for P9.

## 5. Difficulties, changes, pivots, cancellations, setbacks

### Platform and process pivots
- **Closed source to open source, non-commercial** (ADR 0005 fallback superseded 28 Oct): users
  fetch their own data, so NC-licensed sources are usable.
- **Python to Rust, all of it** (ADR 0113): the GE reference, calibration, PPML, pack builders,
  and later HANK, SBI and Sobol (owner chose in-tree Rust over out-of-tree Python, +8-12
  owner-weeks across P5b and P9).
- **Web map to native**: ADR 0011 (Rust map), then ADR 0088 (fully native Slint + wgpu, no
  Tauri/React/webview). Tauri app deleted (#95).
- **European slice dropped**: full globe from P2 (ADR 0012).
- **Decision tracking reworked** (#126): DESIGN.md split into `spec/NN-*.md`, decisions as rows
  in `rows.toml`, ADR files moved to `rfcs/`, from 0282 RFC and ADR counters separate.
- **CI moved to a self-hosted Mac runner** (#113) when hosted minutes ran out; nightly
  full-pack gate (D5).

### Scope cancellations and deferrals
- **1.0 is a pure simulator** (ADR 0207 scope rule, 0238, 0239, 0240): seats, scoring, fog of
  data, narration, flavour, eras, codex all moved to 2.0+. ADR 0207 kept 53 ADRs on the 1.0
  path and deferred 63.
- **Climate to 1.1** (ADR 0207; deviates from design 22.1 "climate first-class").
- **No railroading** (ADRs 0241, 0242): only 1 Jan 2000 data; every later event emergent;
  pandemics, regimes and technology jumps become endogenous hazards (adds P8/P9 work).
- **Rejected:** ADR 0114 (merging sub-first-level units, 4,796 to about 3,650 provinces).
- **Deferred:** route choice past P2 (0017; built, off); split price rule to P5 (0090);
  storage-on frictionless fix to P5 (RFC 0286); RE storage switch to P9 (0111); `Real`
  generic and dual-number gradients to P9 (0096); ensemble lanes into P3 (0288); Walrasian
  clearing to P3; Python distribution dormant (0003).

### Setbacks in the economic core
- Productivity calibration (nominal and real VA) failed with no root (25 Oct); the owner pulled
  ADR 0089 (data state = steady state) into P2 before the gate.
- **V1 recast** (26 Oct): V1 became the run from the perturbed steady state; the run from the
  shocked data state became a strict-xfail stress test.
- **H2 distance fit regressed** with each fix: -0.753 (P2 gate), -0.536 (gateway masses),
  -0.509 (fitted nu), -0.463 (bulk rate), -0.454 (B8); then made a reported diagnostic
  (0264), with log-distance crossing friction (0272) replacing 0264's kernel control.
- **Theta classes**: 3 classes (P2) to 4 harmonic (0101) to up to 7 (0274) for the 50-goods
  list.
- **28 Oct benchmark**: openness about 3x data, median 0.66 of ACR with correlation 0.57,
  x10 border shock stopped at tick 45. Responses: selection on trade links (0200), ACR with
  input-output links (0265), graded per province (0268), x10 reported not asserted and its
  collapse deferred to P5 (0090, 0207).
- **Stability patches**: price-gain caps per market (0263) and per coupled block (0267),
  labour capped by inputs (0266), loss write-off (0112), rents-to-VA from extraction (0256,
  0270, 0281; Albania stays an open V1 finding).
- **Storage-on frictionless V1**: two misses (CHE cycle, FRA 1094) traced to one cause, no
  variant passed both hard starts in six screens; recorded and deferred (RFC 0286).
- **Tick speed**: two parallel-dispatch attempts reverted or not faster; stopped at 41.2 ms.
- **Ops**: hosted CI minutes exhausted (29-30 Oct); Mac Remote Control sessions stop on idle or
  spend limit; agents cannot edit `.github/` (D5 needed a Mac session); stale installed pack
  turns main CI red.

The 30 Oct fidelity audit classified the 41 alterations since 23 Oct as 11 principled,
5 simplify, 20 patch, 3 defer, 2 goalpost (V1 recast; 0247 criterion 3 ruled ill-posed).
Later goalpost-like moves: H2 distance to diagnostic (0264), x10 to reported (0207 amended),
ACR graded per province (0268), tick budget re-rate deferred to P3.

## 6. Deviation from the original spec, quantified

Baseline: the design document of 23 Oct 2026 (DESIGN.md, 1,366 lines, ADRs 0001-0087 as its
proposal lanes). Now: `spec/` 2,572 lines (+88%), 312 decision rows.

| Measure | Count |
| --- | --- |
| Decision rows (ADRs) | 312 (0001-0312) |
| RFC files | 290; index status: 73 accepted, 214 proposed, 1 superseded (0013), 1 rejected (0114), 1 deferred (RFC 0286) |
| RFCs written after the design snapshot (0088 on) | 203 |
| RFCs whose text amends, supersedes or replaces an earlier ADR | 105, touching 75 distinct ADRs |
| Of those, aimed at the 23 Oct design-era ADRs (0001-0087) | 35 RFCs touching 22 design-era ADRs (0003, 0006, 0008-0011, 0014, 0015, 0017, 0018, 0021-0024, 0027, 0028, 0033, 0034, 0046, 0048, 0050, 0068) |
| Explicit supersessions/replacements | 7: 0013 by 0015/0016; 0033 by 0063; 0034 by 0278; 0100 by 0287 rows; 0104 by 0201; 0264 by 0272; 0188 by 0113; plus 0005's closed-source fallback |
| Rejected | 1 (0114) |
| Deferred out of their phase | at least 9 (0003, 0017, 0024, 0036/0037 climate, 0090, 0096, 0111 RE switch, RFC 0286, Walrasian S01) |
| Moved out of 1.0 by scope ADRs (0207, 0239, 0240) | 63 ADRs deferred by 0207, plus the seat and presentation ADRs of 0239/0240 |
| Approved module deviations from DESIGN (SPEC tables) | 26: econ 15, trade 11 |
| Design-era claims changed in kind | platform (Python, web/Tauri, closed source), slice to globe, climate first-class to 1.1, 10 goods to ICIO-level goods (0271, proposed), 5 PIGL categories to 3, CES energy nest to Leontief (P8), Sinkhorn per tick to destination sourcing, per-unit + iceberg to iceberg only, kappa1 0.2 to 0.05 and kappa2 to 0, 3 to 7 theta classes |

Reading: about one in three post-design RFCs revises an earlier decision; about a quarter of
the 87 design-era ADRs have been amended or replaced. The architecture's skeleton (deterministic
ECS-style core, ledger, gravity trade, PIGL, steady-state calibration, branchable runs) has
not changed; the parts that moved most are the platform, the scope of 1.0, the trade cost
model and the gate criteria.

## 7. What Antonio needs to decide next

Nothing new from this report. Pending items stay where they are: C6 RAS factors review, the
pre-P3 re-rate with the owner's hour count at the gate (B history was squashed, so it needs his
own estimate), and the status sweep that moves built ADRs from proposed to accepted.
