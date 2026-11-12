# Second development pass (optimization and process): brainstorm, 8 November 2026

Scope: what to do after queue #179 is done. Sources: #197, #202, #179, #180, #181, #346, #351, #353; RFCs 0024, 0092, 0097, 0313; spec 19.2/19.4; `reports/` (progress, architecture, approximate-solves, econ-numbers, scope-progress, plans-from-review); `tick-speed-brainstorm-2026-11-04.md`; project memory. Payoff and effort are my judgement, not measurements. No issues or PRs opened.

## 1. Lessons

### Technical
1. **Benchmark before building a generic kernel.** Branch L ran a full RFC, ADR 0024/0288, L1-L4 (#329-#349) and ended with lanes at 0.6-0.7x scalar throughput, 350 MiB per lane, and a stage profile showing a perfect lane kernel gains at most ~20% (#197). Owner ruled scalar members. A one-day spike (L=1 vs L=8 on the real pack, memory per lane) would have shown it. Same shape earlier: parallel per-good dispatch (reverted), class split (bit-identical, not faster), f32 and GPU probes.
2. **The tick is serial-tail and critical-path bound, not instruction bound.** IPC 4.9, 3.4x speed-up on 14 threads, 7 traded goods cap per-good parallelism. At 45 goods this reverses: throughput-bound.
3. **45 goods is the real performance problem.** Quiet-host run (#202, 8 Nov): 329 ms vs 160 ms ceiling. dispatch 168 ms, deliver 67, produce 52, consume 11, clear 10, source 9, labour 7. Fixed-plus-linear form fits badly (deliver, produce, clear, dispatch superlinear, so cache/memory pressure). Peak RSS 10.5 GB on the synthetic 100-year run vs the 6 GB interactive target. The 34 ms headline is a 10-good number.
4. **Wall time on a shared host is not a measurement.** G1 "failed" at ratio 2.42 and was pure load (#202); gating on single-thread CPU time gave 1.083 (#346). The G2 fit and the 160 ms check were invalid twice (load 6.6 to 77, then 40-111) before SIGSTOP of CI worked. Mac load 55-141 voided the L4 first run.
5. **Superlinear scaling in goods** (deliver, produce, clear) was not in any spec model; found only because G2 measured it. Per-system budgets should be written from measured scaling, not per-good constants.
6. **Calibrated pack = hidden global state.** Every pack or code change forces recalibrate plus golden re-bless; stale installed pack turned main CI red repeatedly (#183, 3 Nov). Pack-dependent tests are mostly Mac-only.
7. **Fit problems found late, by audit, not by gate:** Ethiopia negative PIGL weight (#180), RAS pass 1 not converging, factors 1.27-3.17 (#181), H2 distance regressed with each fix (-0.753 to -0.454), PPML thetas 10.9-14.1 for every class (#241), ACR correlation 0.862 vs 0.9. Per-member recalibration (needed for structural ensemble draws) is too slow and fragile, so C1 draws dynamic parameters only (decision 7 Nov, #180/#181).
8. **Pass criteria drifted.** 41 alterations in nine days: 11 principled, 20 patch, 2 goalpost (V1 recast, 0247 c3); later H2 to diagnostic, x10 to reported, ACR graded, tick budget re-rate. About half of the "agreements" are fitted targets (econ-numbers report).
9. **Determinism infrastructure paid off.** Hash comparison (24-tick state/journal) let class-A speed work ship with no golden moves; clippy bans (0097) caught the rayon/HashMap traps. Keep and extend.
10. **Small, honest negative results are cheap and worth recording.** approximate-solves (adopt none), f32 recheck, Anderson probe (income loop 28 to 8 iters but runs once per start), PIGL (1 round).

### Process
1. **Merges are manual and serial; branches go stale.** Required checks: rows plus up-to-date. Antonio merges spec and `.github` PRs; agents cannot touch `.github/` (CI wiring of G1 needed a non-claude branch). PRs queue behind one human.
2. **Compute is the constrained resource.** Hosted CI minutes exhausted 29-30 Oct; PR CI and pack-gate share one Mac with Remote Control sessions and benchmarks (load average 16-277 observed). "Ask before cloud threads" is standing.
3. **Records drift from code.** 215 of 304 files in `rfcs/` still say `Status: proposed`; 17+ built ADRs were unflagged on 4 Nov (#186). ADR 0024 (lanes) remains "proposed" with a departure from its intent; spec 19.4 still describes the exact accumulator (ADR 0298) while #238 is parked/dropped; `specs/p3/PLAN.md` needed a refresh (#249).
4. **Effort accounting is estimated after the fact.** RFC 0313 re-rated B+C from 53 decisions, 20 PRs and "7 days at 2 h", not logged hours. 1.0 range 74-118 owner-weeks is therefore soft.
5. **Owner-attention is the bottleneck, not agent throughput.** 53 owner decisions in 8 days. Question cards help; batching is not yet systematic.
6. **Threads repeat setup cost.** Each Mac thread re-derives load gating, pack path, caffeinate, log locations. Recorded in memory but not as a script.

## 2. Gaps

| # | Gap | Evidence |
|---|---|---|
| G1 | No quiet, dedicated benchmark host; no CPU-time/instruction-count harness for all benches | #202 comments; L4 first run; tick-speed brainstorm noise note |
| G2 | 45-goods tick at 2x the ceiling; memory 10.5 GB | #202 04:10Z |
| G3 | Dispatch plan items unbuilt: journal as owned blocks, parallel goods moves (about -4.7 ms), `in_flight_all`, scratch reuse, P-core QoS (#239) | progress report 3.3; brainstorm items 1-4, 6, 7, 13, 14 |
| G4 | Per-lane / per-member memory (350 MiB/lane, 1.4 GB scalar) bounds ensemble size: 32 members to 2050 = 13 GB disk, memory untested for scalar x32 | #197, #202 |
| G5 | Calibration is slow (~19 min), Mac-only, and not incremental; structural uncertainty cannot enter ensembles | #180, #181, brainstorm sec 1 |
| G6 | Free (out-of-sample) checks are few; fitted vs free untagged (#185) ; distance, Brexit, ACR-IO correlation miss | econ-numbers sec 5 |
| G7 | Long-run theta applied monthly (E: rho not yet fitted; `calibrate rho` PR #351 lacks panel extraction and tariff source) | #351 body |
| G8 | Cross-platform determinism untested (H not started); `mul_add` and libm `pow` on x86 vs arm unverified | specs/p3 Branch H, spec 19.4 |
| G9 | CI: rust job not required; Mac-only gates (pack, flatness, golden) run nightly, so a regression is found a day later | memory runners/required checks; #347 |
| G10 | No perf regression history; numbers live in issue comments | #202 |
| G11 | Pack tests need owner-built pack; cloud threads cannot run them | CLAUDE.md, #202 "fixture-checked" |
| G12 | No fuzz/property testing of the ledger and edit-log invariants beyond the golden set | by inspection of specs/p3 test lists (named unit tests only) |

## 3. Candidate work items

Payoff / effort: H, M, L. Effort S under a day, M a few days, L a week or more.

### A. Measurement foundation (do first)
| Item | Payoff | Effort | Evidence / note |
|---|---|---|---|
| A1 Dedicated bench host or reserved window: CI runners paused by script (the SIGSTOP method worked on 8 Nov), or a second small machine / cloud box for benches only | H | S-M | Every Mac benchmark in #202/#197 was contaminated. Cloud box costs minutes; the SIGSTOP script is free |
| A2 One `scripts/bench_quiet.sh`: pause runners, check load < 2, caffeinate, log load per sample, resume | H | S | Replaces per-thread rediscovery |
| A3 CPU-time and retired-instruction metrics as the default for gates; wall only reported (extend #346 beyond G1) | H | S-M | #346 precedent |
| A4 Bench results appended to a checked-in `tests/perf/history.csv` (commit, host, load, per-system ms), plus a threshold check | M | S | G10; nightly pack-gate already exists |
| A5 Per-system span profile in the sim loop (`FLATNESS_SPANS`, #349 stage timing) made permanent and exposed in `ceteris run --profile` | M | S | Partly built |

### B. Performance (after A)
| Item | Payoff | Effort | Evidence / note |
|---|---|---|---|
| B1 Profile the 45-goods tick per good: why deliver and produce are superlinear (cache, allocation, journal growth) | H | M | deliver 9.6 ms/good fit but 67 ms measured; unexplained |
| B2 Dispatch at 45 goods: ADR 0235 order is gateway count/crossings (0092), then 0097 plan, then raise ceiling by ADR | H | L | dispatch 168 of 329 ms |
| B3 Class-A bank: journal blocks, parallel goods moves, `in_flight_all`, LPT order, P-core QoS, scratch reuse (items 1-4, 6, 7, 13 of tick brainstorm). Bit-identical, no RFC | M | M | -4.7 ms known at 10 goods; at 45 goods serial tail share may shrink, so re-rank after B1 |
| B4 PGO plus `target-cpu` (#236) | M | S | 3-10% claimed, never measured; CI must rebuild profile |
| B5 Memory diet: per-world memory at 45 goods (10.5 GB) and scalar-member footprint for 32-member ensembles | H | M | Interactive target 6 GB; G4 |
| B6 Skip unchanged goods' dispatch (#240); measure exact-match hit rate first | M-H if hit rate high | S to measure, L to build | approximate-solves report; class C needs RFC |
| B7 Ensemble throughput: shared read-only data across scalar members (kernel, pack tables), copy-on-write state, fork-at-tick reuse | M | M | Lane memory was 350 MiB/lane; shared immutables are the likely win |
| B8 Decide on exact accumulation (ADR 0298) once B1/B2 show whether intra-good parallelism matters at 45 goods | M | decision | Parked (#238); spec 19.4 still states it |

### C. Calibration and fit
| Item | Payoff | Effort | Evidence / note |
|---|---|---|---|
| C1 Fast calibration: warm start from previous pack, incremental solve of changed block, cache stage 1-2 | H | L | Enables structural draws (#180/#181 reasoning) and shortens every pack-change loop |
| C2 Make `calibrate` a deterministic, cached, content-addressed build step; stale-pack detection in test harness with a clear error | M | M | CI red from stale pack, 3 Nov |
| C3 Diagnose #180/#181 root cause (data, code, transfers) before P4 recalibration | H | M | Open, pre-P3 labels |
| C4 Free-moment scoreboard: tag fitted vs free (#185), run each on every pack change, report deltas | M | S-M | econ-numbers sec 5 |
| C5 ADR 0272 log-distance friction (#212) is the known fix for distance and Brexit; schedule explicitly with 50-goods pack | H | L | Already planned P4 |

### D. Testing
| Item | Payoff | Effort | Evidence / note |
|---|---|---|---|
| D1 Property tests on ledger conservation, edit-log replay, fork equivalence over random edits (proptest with fixed seeds, thread counts 1/3/14) | M-H | M | Today only named examples; 1/3/14-thread determinism test was recommended 30 Oct and not confirmed built |
| D2 Cross-arch determinism job (H) early rather than late: x86 Linux hash vs Mac hash on the toy and fixture worlds | H | M | G8; cheap in cloud on the fixture |
| D3 Mutation or coverage pass on calibrate/trade crates | L | M | Only if bugs keep escaping |
| D4 Golden policy: every re-bless states why in one line; CHANGELOG `### BEHAVIOUR` entry required (CI check) | M | S | Rule exists for class B; extend to pack re-bless |

### E. CI and agent workflow
| Item | Payoff | Effort | Evidence / note |
|---|---|---|---|
| E1 Merge queue or auto-rebase bot (GitHub merge queue on main) for non-spec PRs; keep manual merge for spec/RFC/research | H | S-M (settings + `.github`, Antonio) | Repeated out-of-date PRs; "Antonio alone merges spec" rule keeps its scope |
| E2 Batch spec PRs into a weekly bundle with one review sheet | M | S | 53 decisions in 8 days |
| E3 Required check for rust on fixture-only tests in cloud (fast, hosted-free or cheap), Mac for pack tests only | H | M | Minutes exhausted; G9, G11 |
| E4 Thread prompt templates / scripts for Mac tasks (load gate, caffeinate, log dir, result comment format) | M | S | Process lesson 6 |
| E5 Time logging per thread (token and hour lines in memory, as project instructions already ask) so RFC 0313-style re-rates use data | M | S | RFC 0313 caveat |
| E6 Status sweep automation: `ceteris-rows check` warns when code cites an ADR whose status is `proposed`; fix remaining 215 | M | S-M | #186; drift list above |
| E7 Pre-implementation spike rule for any generic/abstraction branch: one-day measurement on real pack, go/no-go card before RFC | H | S (policy) | Lesson T1 |

## 4. Suggested ordering

1. **Foundation (1 week):** A2, A3, A4, E7, E5 (policy and small scripts), then A1 decision.
2. **Diagnose (1 week):** B1 (45-goods profile), B5 (memory), C3 (root causes). These decide what B2-B8 are worth.
3. **Cheap wins:** B4 PGO, B3 class-A items re-ranked by B1, D1 property tests, D2 cross-arch on fixture, D4, E6.
4. **Structural:** B2 (gateway count/crossings, 0092; then 0097 plan), B7 (shared immutables), C1/C2 (fast calibration).
5. **Conditional:** B6 (only if hit rate high), B8 (only if intra-good parallelism needed), C5 follows the P4 schedule.
6. **CI/process in parallel from the start:** E1, E3 (needs Antonio for `.github`).

Rough sizing: foundation + diagnose about 2 owner-weeks; the performance pass 3-5; calibration speed 2-3. Estimates are mine and unvalidated.

## 5. Open questions for Antonio

1. Is a dedicated benchmark machine (or a paid cloud box used only for benches) acceptable, or should the SIGSTOP window on the Mac be the standard?
2. Is the 160 ms ceiling at 45 goods fixed? Measured 329 ms. Options: optimise to 160, raise by ADR (ADR 0235 allows this last), or ship 1.0 with fewer traded goods.
3. Interactive memory target 6 GB vs 10.5 GB measured at 45 goods: which constraint moves?
4. Merge queue (E1) for non-spec PRs: allowed, given agents cannot edit `.github/`? You would apply the setting.
5. Exact accumulation (ADR 0298, spec 19.4) stays in the spec while #238 is parked. Keep as plan of record, or amend the spec?
6. ADR 0024 stays "proposed" after the scalar-members ruling: supersede or amend (RFC)?
7. Does the second pass include fast calibration (C1), or is calibration speed accepted until P9?
8. Should cloud threads be allowed for fixture-only work if minutes are replenished, or stay Mac-only?
