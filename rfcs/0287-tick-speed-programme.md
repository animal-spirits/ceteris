# RFC 0287. Tick speed programme: bit-identical speed-ups first, then narrow exceptions

Status: accepted
Date: 2026-11-04 (decided 2026-11-04)
Design sections: 19.4
Research: tick speed brainstorm and profile, 2026-11-04 (project files, `tick-speed-brainstorm-2026-11-04.md`)

## Summary

The full-pack tick measures 42.3 ms in criterion and 43.5 ms in the sim loop on `main` at
3103d07, against the 35 ms budget (spec 21, from 19.2). The 2026-11-04 profile shows that
instruction count is not the problem. One thread runs a tick in 148 ms at an IPC of about 4.9,
and 14 threads run it only 3.4 times faster. About 15 to 18 ms of each tick runs on the main
thread alone: the T2b goods moves, `Pipeline::in_flight_all` three times, clear, produce,
deliver, and one-at-a-time ledger transfers. The T2b per-good region runs 7 tasks on 14 threads,
so its wall time is that of the slowest good. The end_tick income check scans the whole journal
and takes 9% of all CPU. The 50-goods pack will make the per-good region throughput-bound
instead of imbalance-bound.

This RFC fixes the order of work and the rules for it. Most of the gain comes from changes that
leave every bit as it is, and several of them only build what 19.4 already asks for: largest
tasks first, QoS classes in place of pinning, and systems with disjoint read and write sets run
concurrently. It also opens four narrow exceptions, each gated on measurement: `unsafe` for
unchecked indexing in named kernels, profile-guided builds, a different global allocator, and
order-free exact sums for scatter accumulators (one golden move). Warm starts are deferred until
iteration counts are measured. Model shortcuts are deferred. GPU, SME, AMX and hand-written
assembly are rejected for now.

## Decisions

### D1. How should speed work proceed?

Options:

- A. Bit-identical first: speed-ups that leave the state hash and the ledger journal
  bit-identical go first, one PR each, in the order of the plan below. Each PR shows the
  identity over 24 full-pack ticks at 1 thread and at all threads, on the Mac and on Linux x86.
  It reports the criterion tick and a per-system sim-loop timing as the acceptance numbers. No
  golden moves and no spec change beyond the as-built notes.
- B. Mixed: take the largest gains first, whatever their determinism class, accepting golden
  moves as they come.
- C. Stop speed work until the P3 gate re-rates the budget.

Recommendation: A. The profile puts 12 to 20 ms of wall time within reach without moving a
single golden, which reaches the 35 ms budget. Golden moves are paid for once, under D5, where
they buy something that bit-identical changes cannot.

Decision: A, Antonio, 2026-11-04.
Row: ADR 0294

Plan, in order. Each item is one PR. The first item is the measurement harness. An item whose
measured gain is below its noise is reported and dropped, not merged.

| # | item | section it builds | gain (estimate) |
|---|---|---|---|
| 1 | Sim-loop timing harness: wall-time spans per system and per T2b phase, per-good task start, end and QoS class, and instruction counts at 1 thread | 19.4 Tick execution | measurement |
| 2 | T2b goods moves run in parallel by good, and each good's moves overlap the dispatch of the other goods | 19.4 Parallelism | 2 to 4 ms |
| 3 | Journal kept as owned entry blocks (moved, not copied); the end_tick income check reads only the WAGE, DIVIDEND and REBATE blocks | 19.4 Tick execution | 2 to 3.5 ms |
| 4 | `in_flight_all` computed once where the pipeline has not changed between calls, and run in parallel by good | 19.4 Tick execution | 1 to 2.5 ms |
| 5 | Largest goods first in the per-good region; rayon workers at QoS user-interactive, or a 10-thread pool on performance cores, whichever measures faster | 19.4 Parallelism | 0 to 3 ms |
| 6 | Serial ledger traffic in clear, deliver, produce and distribute batched into entry blocks for `post_all`, with goods moves in parallel by good | 19.4 Parallelism | 3 to 5 ms |
| 7 | `post_all` sums by account bucket instead of dense per-chunk `i128` arrays; integer sums are exact in any order | 19.4 Tick execution | 0.5 to 1.5 ms |
| 8 | Crossing weights cached between the z pass and the share pass; z pass parallel over origin gateways; 2 to 4 CSR rows interleaved per loop, each row in its own order | 19.4 Linear algebra | 1 to 3 ms |
| 9 | Per-tick allocations moved to persistent scratch | 19.4 Tick execution | 1 to 2 ms |
| 10 | Fewer, coarser parallel regions; `pow` computed once per (base, exponent) per province or market | 19.4 Parallelism | 1 to 3 ms |
| 11 | Systems with disjoint read and write sets run concurrently, from the static dependency graph 19.4 describes | 19.4 Parallelism | 3 to 8 ms |

### D2. May hot kernels use `unsafe` to drop bounds checks?

Options:

- A. No: `unsafe_code = "forbid"` stays; bounds checks go only by safe restructuring (pre-sliced
  rows, iterators, asserted lengths).
- B. Narrow exception: `unsafe` is allowed only for unchecked indexing (`get_unchecked`,
  `get_unchecked_mut`) inside a module named `hot` in a crate, and only where safe restructuring
  was tried first. The workspace lint becomes `deny` with a module-level `allow`. Every unsafe
  block carries a `SAFETY:` comment and a `debug_assert!` of the bound. The crate keeps a safe
  twin of each kernel behind a cargo feature, and CI runs the twin and compares bits. The PR
  shows a gain of at least 2% of the tick in the sim loop.
- C. Lift the ban workspace-wide.

Recommendation: B. Unchecked indexing does not change float semantics, so determinism is
untouched. The safe twin and the debug assertions keep a memory error from going unnoticed. A
2% bar keeps `unsafe` from spreading for small wins.

Decision: B, Antonio, 2026-11-04.
Row: ADR 0295

### D3. Should release and bench builds be profile-guided?

Options:

- A. PGO (instrumented build, a 24-tick full-pack training run, then an optimized build) for
  release and bench builds, with `-C target-cpu` set to a fixed baseline per platform
  (`apple-m1` on macOS arm64, none elsewhere). The state hash is checked to be identical between
  PGO and plain builds and across architectures.
- B. PGO only, with no target-cpu change.
- C. Neither.

Recommendation: A. Rust contracts a multiply and an add into an FMA only through an explicit
`mul_add`, and libm is pure Rust, so neither PGO nor a CPU baseline changes any float result;
the hash check proves it on every release. A fixed baseline, unlike `native`, keeps the binary
portable across the M-series chips. A plain build stays the default for `cargo test`.

Decision: A, Antonio, 2026-11-04.
Row: ADR 0296

### D4. Should the core use a different global allocator?

Options:

- A. mimalloc (MIT) as the `ceteris` binary's and the benches' global allocator, if the sim loop
  shows at least 1 ms per tick after D1 item 9; it needs `cargo deny` to pass.
- B. jemalloc (`tikv-jemallocator`, MIT or Apache-2.0).
- C. The system allocator.

Recommendation: A, conditional on the measured gain. The profile shows 6.7% of CPU in malloc,
free and Vec growth, and about 55 ms of system CPU per tick, part of it page faults on fresh
allocations. mimalloc is the better fit on macOS. Allocation does not touch results.

Decision: A, Antonio, 2026-11-04.
Row: ADR 0297

### D5. May scatter sums become order-free, at the cost of one golden move?

Options:

- A. Exact accumulation: the T2b dispatch accumulators that pin the order of work (`at_b`,
  `arrive`, `shipped`, `paid`, `received`, `lost`, `selected`) sum through an in-house, pure-Rust
  exact accumulator, a fixed-width superaccumulator over the f64 exponent range in the manner
  of Neal's xsum, with no `unsafe` and no FMA. It rounds once at the end, so any order of adds
  gives the same correctly rounded sum. That allows each good's dispatch to be split by demand
  class and into chunks, and the scatters to become gathers. Goldens move once, recorded as
  `### BEHAVIOUR` in the CHANGELOG. Other accumulators adopt it only where a measured
  decomposition needs it.
- B. Keep fixed-order sums; decompose only where the order can be preserved.

Recommendation: A, after D1 items 2 to 8 have landed. At 30 to 40 traded goods the per-good
region is throughput-bound, and the scatter order is what blocks splitting work below one good.
An exact sum is also more accurate than the sequential one it replaces. Runs stay bit-identical
to each other and across architectures and thread counts.

Decision: A, Antonio, 2026-11-04.
Row: ADR 0298

### D6. Should within-tick solvers warm-start from the last tick?

Options:

- A. Yes, from state carried in the snapshot (ADR 0097 allows only that). It changes the
  snapshot format and moves goldens.
- B. Defer: measure rounds per call per tick first (the Anderson probe in `fixed-point/` is the
  harness), and decide on the numbers.

Recommendation: B, because the profile does not show the solvers on the critical path (E3
demand is 4.4% of CPU and labour with produce 4.3%), and a snapshot-format change is not worth
it without a measured gain.

Decision: B (deferred), Antonio, 2026-11-04.
Row: none

### D7. Should the model take shortcuts for speed?

Options:

- A. Skip a good's kernel dispatch when its prices and fills have not moved, or represent the
  sea legs in low rank.
- B. Defer: both change outputs as a model change, and pruning crossings was already measured to
  move Phi_d by 50 to 95% (econ SPEC Task 19b).

Recommendation: B.

Decision: B (deferred), Antonio, 2026-11-04.
Row: none

### D8. GPU, SME, AMX or hand-written assembly?

Options:

- A. Reject for now: Metal and WGSL have no f64; SME and AMX accumulate with fused
  multiply-adds and exist only on Apple chips, so state hashes would differ across
  architectures; two-lane NEON division was already measured 0.4 ms slower (econ SPEC Task 19),
  and the dispatch loops are limited by gathers and scatters.
- B. Prototype one.

Recommendation: A. f32 stays settled (f32 recheck, 2026-10-29) and is not reopened here.

Decision: A (rejected), Antonio, 2026-11-04.
Row: none

## Spec changes

- 19.4 Tick execution: bit-identical speed changes are gated by the 24-tick hash and journal
  identity (ADR 0294); `post_all` sums need no fixed order because they are integers (ADR 0294).
- 19.4 SIMD on Apple Silicon: the unchecked-indexing exception (ADR 0295).
- 19.4, new block "Build": PGO with a fixed CPU baseline (ADR 0296), mimalloc where measured
  (ADR 0297).
- 19.4 Parallelism: order-free exact sums for scatter accumulators (ADR 0298).
- `.claude/rules/rust.md`: the `unsafe` line follows ADR 0295; the reductions line names the
  exact accumulator (ADR 0298).
