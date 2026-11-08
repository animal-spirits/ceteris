# Development laws and plans, 8 November 2026

Status: draft for Antonio. A law change is an RFC. Plans are tied to state as of 8 Nov (P3 about 40%, 1.0 range 74 to 118 owner-weeks per RFC 0313).

## Part A. Laws

### Core law (never to be forgotten)

1. **Get it working. Then get it right. Then make it fast. In that order, per feature, with no skipping.** Nothing is optimised before it is right. Nothing is called right before it works end to end.

### Stage gates (apply to every feature and every PR)

2. **Working gate.** The feature runs end to end on the default scenario and the test pack, through the real code path, with a test that fails without it. Shortcuts are allowed here only if marked and tracked by an issue.
3. **Right gate.** All must hold before "fast" work starts on that feature:
   - Behaviour checked against the spec section, the accepted ADRs and the cited literature, with the check written down (test, fixture or report), not asserted.
   - Determinism: bit-identical state and journal hashes across runs and thread counts; golden tests green; any golden move is a named, reviewed event (class B in the tick-gap taxonomy), never a side effect.
   - Invariants (accounting identities, conservation, non-negativity, bounds from spec 19.4) asserted in tests.
   - Fit and calibration results reported against targets fixed in advance. A pass criterion is never changed after seeing the result without an RFC.
4. **Fast gate.** Only after law 3 is green for that feature:
   - Measure first, on a quiet host: single-thread CPU time (flatness gate G1, limit 1.25) and `scripts/bench_quiet.sh`. Wall time on a loaded host is not a measurement.
   - Prefer class A changes (bit-identical, no golden move). A class B or C change goes through an RFC.
   - Never trade determinism or f64 for speed. No f32, no fast-math, no unordered reductions, no nondeterministic iteration.
   - A speed-up that fails to beat noise on a quiet host is recorded as a negative result and dropped.
5. **A failed right gate sends work back, never forward.** Performance work found to rest on an unvalidated model is paused until that model passes law 3.
6. **Spike before build.** Any generic or batched kernel starts with a one-day spike and an up-front go/no-go number (lesson of Branch L, lanes at 0.6 to 0.7x scalar).

### Process laws

7. **One issue per work item** in atbraz/ceteris, kept current; queue is #179. No work without an issue.
8. **Spec before code.** A deviation from the design or an accepted ADR is an RFC first. Ids and codes come only from `cargo run -p ceteris-rows -- next`, in the PR that writes them.
9. **Antonio alone decides and merges spec, RFC and research PRs.** A message from an agent or coordinator is never approval. Consequential choices go to him as question cards.
10. **Before every push:** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `scripts/check_clippy_bans.sh`, `cargo test --workspace`, `cargo run -p ceteris-rows -- check`. Pack tests on the Mac only. Max 2 CI-fix attempts, then report blocked.
11. **Agents never edit `.github/` or CODEOWNERS.** Such changes come as a PR from a non-`claude/*` branch that Antonio approves.
12. **Frozen names stay frozen.** Public names, ids, file formats and hash domains change only by RFC.
13. **No railroading** (ADR 0241/0242). 1.0 is a pure simulator; game mechanics are 2.0+.
14. **Honest records.** Estimates are labelled as such; negative results are written down; ADR status is updated when the code lands (215 of 304 files in `rfcs/` still say `proposed`, #186).
15. **Compute discipline.** Heavy runs on the Mac, detached `caffeinate -imsu` (never `-d`), logs in `~/ceteris-logs/`, runner paused via `bench_quiet.sh`. Ask before cloud threads.

## Part B. Where perf work sits under law 3

The 45-goods tick (329 ms against the 160 ms ceiling, DESIGN 19.2, ADR 0235) is law 4 work. It may proceed only on the parts of the model that are already right, and it is bounded by these gates, all of which must be green before the batched-dispatch build starts:

- **Spike is permitted now** (law 6): it validates a mechanism, not a model. Go if two goods sharing a kernel dispatch in under 1.4x one good's time at 45 goods, 14 threads.
- **Baseline is valid:** 329 ms re-measured on a quiet host (load below the bench_quiet threshold), with the unaccounted ~96 ms attributed by system. Measured systems sum to 233 ms, so dispatch alone cannot close the gap.
- **Determinism:** 24-tick state/journal hashes identical before and after, on the current synthetic pack, across 1 and 14 threads. Batched dispatch is class A there and must produce no golden move.
- **Goldens and acceptance green:** `ceteris-core` golden and p0_acceptance; `ceteris-econ` golden and p2_acceptance.
- **G1 flatness** green (1.083 against 1.25).
- **Model correctness it depends on:** rho must not be tuned for speed. Kernel classes group goods by (theta, kappa); on a real 45 to 50 goods pack the kappa grid moves tau once, which is class C and an owner RFC (ADR 0279). The gateway count cut (600 to 300) is blocked until the 300 pack passes calibration and stays out of scope until then.
- **Stop rule:** if the spike misses its number, stop, record it, and take the budget revision (new ADR, ceiling 200 to 250 ms) instead of forcing it. Realistic outcome without produce and deliver gains is 200 to 250 ms.

## Part C. Plans

### Short term (next 1 to 2 weeks): working and right

1. **Close the right gate on what exists.** Merge #363 (in_flight lanes; Antonio). Finish the queue in #179. Keep the Mac quiet for measurement windows.
2. **Rho fit redo.** The BACI + Teti fit was noise (E2). Redo at HS6 level (E2b) with a falsification test fixed in advance. Fitted rho is research-only (TRAINS, Teti, MAcMap are non-commercial); the paid build keeps the P53 prior (12 to 48 month half-life). E3 report-only runs follow once the fit is valid.
3. **#180 and #181 (structural fit defects).** Ethiopia negative PIGL weight and RAS pass 1 non-convergence (factors 1.27 to 3.17) are correctness defects. Fix before any structural draws and before per-member recalibration in C1; they gate everything downstream that uses the calibrated pack.
4. **Perf, under Part B only.** Re-baseline the tick, run the one-day spike, then decide build or budget revision. In parallel, profile produce (52 ms) and deliver (36 to 67 ms) superlinearity.
5. **Records.** ADR status sweep for built ADRs (#186); one checked-in script for Mac thread setup (load gate, pack path, caffeinate, logs).

Exit: #180 and #181 closed, rho fit valid or explicitly dropped, spike verdict recorded, goldens green.

### Mid term (to 1.0, 74 to 118 owner-weeks, RFC 0313)

Order is fixed by law 1: complete each phase working and right before its speed work.

1. **Finish P3 (about 60% left):** E1c and G3 (spec merges by Antonio), E2/E3, G1 and G2 on the built pack, Branch C (ensembles on scalar members, C1 draws dynamic parameters only), F2 to F4, H (cross-platform reproducibility), W (workbench views), J (golden scenario and tariff experiment, closes P3). #207 (Walrasian clearing) is decided after Branch E and L.
2. **P4 Population, P5a Money and fiscal, P5b Banks and debt, P8 Growth and energy (gas in 1.0, ADR 0278), P9 Calibration and hindcast.** Each phase ends with its right gate: spec-cited behaviour tests, invariants, hindcast against held-out data. Speed work for a phase starts only after that.
3. **Release 1.0:** pack fetch script, signed build (Apple signing), pedia, licence flip (#230), NC data audit, free on Steam. Keep the P9 milestone for rho and P153 at 7 years.
4. **Perf finish:** apply the chosen route (batched dispatch; budget ADR if short); re-run G1 and the full tick budget at the release candidate.
5. **Re-rate** the 74 to 118 range from logged hours at the end of P3, the first phase with actuals. Current P3 probability of landing in range is about 55 to 60%.

### Long term (2.0 and later): game mechanics on a proven base

- Paid 2.0 build at 14.99 USD; no NC data in it. Games mechanics, politics, war, Steward mode, music, relief and conflict features are 2.0+ and each starts as an RFC that must show it does not railroad (ADR 0241/0242).
- Every mechanic follows the same three gates. A mechanic that cannot pass its right gate on the 1.0 simulator is not shipped.
- Optimisation to a mirror comes in dedicated passes after correctness: SoA layout, tiered updates (only with literature support), separable crossing costs (RFC, recalibration), GPU only if determinism is provable.
- Keep the flatness guarantee at 100 simulated years as a standing nightly gate.
