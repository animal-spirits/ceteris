# 0097. Determinism lints and the post-gate dispatch speed plan

Status: accepted (owner, 2026-10-26; as built on pre-p3/speed)
Date: 2026-10-26
Design sections: 4.5, 14.3, 19.2, 19.4; trade SPEC Dispatch; econ SPEC Task 19

Timing: the lints and the warm-start rule apply now. The speed changes land after the P2 gate,
as one branch off `p2-econ`, so they do not collide with the calibration work in flight.

## Context

DESIGN 4.5 requires identical state hashes across runs and thread counts, and 14.3 requires a
null fork to reproduce its parent bit for bit. Both rest on conventions that nothing enforces:
`fixed_sum` instead of rayon's `sum`, fixed chunks, no hash-ordered iteration. Three accidental
breaks are easy to write:

- rayon's `sum`, `reduce` and `fold` split work by thread count and stealing, so the
  association order of a float reduction changes with the pool.
- `std::collections::HashMap` and `HashSet` iterate in a per-process random order.
- `f64::algebraic_*` and `f32::algebraic_*` (unstable `float_algebraic`, likely to stabilize)
  let the compiler reassociate and contract, so results depend on the optimizer.

ADR 0017 warm-starts route-choice solves from the last solve. A solve that stops at tolerance
returns different low bits depending on where it started, so a child that loads a snapshot and
cold-starts diverges from a parent that warm-started. ADR 0016 removed Sinkhorn scalings from
snapshots for the same reason.

The Task 19 benchmark (M4 Pro, full pack) puts a tick at 34 ms, of which dispatch is 20.6 ms.
One good's dispatch is 5.3 ms run serially, and sets the critical path of the 7.9 ms parallel
source-and-dispatch stage. CPU counters show 44% of dispatch cycles stalled on dependent sums
and divisions; memory reads take at most 1.2 of 5.2 ms. Parallelism across goods is spent:
41.3 ms of serial work over seven goods cannot beat the slowest good. Peak memory over 120
ticks with history recording is 1.88 GB, and `history.rs` buffers every column, keys included,
as `Vec<f64>`.

## Options

Determinism: (1) keep the conventions and rely on the golden and thread-count tests;
(2) enforce them with clippy `disallowed-methods` and `disallowed-types`.

Speed: (a) bandwidth work from the earlier survey (hot/cold kernel split, compact indices);
(b) parallelism and latency inside one good's dispatch; (c) GPU or matrix-unit offload.

## Decision

Option 2, and option (b).

### Determinism rules, effective now

- A workspace `clippy.toml` disallows, with a reason naming this ADR:
  - methods `rayon::iter::ParallelIterator::{sum, product, reduce, reduce_with, fold,
    fold_with, try_fold, try_fold_with, try_reduce, try_reduce_with}`;
  - types `std::collections::HashMap` and `std::collections::HashSet`;
  - methods `f64::algebraic_{add, sub, mul, div, rem}` and the `f32` equivalents.
- CI runs `cargo clippy --workspace --all-targets -- -D warnings`, so a banned call fails the
  build. Clippy ignores a config path it cannot resolve without a diagnostic, so a fixture
  counts the diagnostics of every banned path and fails on any other count (As built).
- Parallel float reductions go through `ceteris_core::reduce::fixed_sum`, or write to fixed
  slots and combine the slots in index order. Ordered maps are `BTreeMap`, or a `Vec` indexed
  by id.
- A use whose order provably never reaches state takes
  `#[expect(clippy::disallowed_types, reason = "...")]` at the use site. Known cases on
  `p2-econ`: the snapshot blob store (`snapshot.rs`, keyed lookup only, never iterated). The
  Arrow metadata maps in `export.rs` and `calibration.rs` become `BTreeMap`, which costs
  nothing and also fixes the key order of written metadata.
- Warm starts: any vector that warm-starts an iterative solve whose result enters state is
  snapshot state, or that solve always cold-starts. This amends ADR 0017 and covers Anderson
  acceleration histories (19.4), which are snapshotted or reset at every load.

### Speed plan, after the P2 gate

One branch off `p2-econ`, in this order:

1. **Lints** (no hash change): `clippy.toml`, the `BTreeMap` swaps, the `#[expect]`s.
2. **Gather-parallel dispatch and stored reciprocals, one commit, one golden re-bless.**
   - Gather: each target gateway or destination sums its incoming legs over
     `crossings_by_target` and `domestic_by_destination`, which the kernel already builds, in
     fixed chunks of targets. `ByColumn` lists a column's entries in row order, so every
     per-target sum keeps the old scatter's order and bits. Applies to the crossing pass (by
     target gateway), the origin pass into `gross_a` and `units_a` (by gateway, over
     `access_by_gateway`), and the domestic and arrival passes (by destination). No scatter,
     so no atomics.
   - The run-wide `lost` diagnostic is one dependent add chain over every leg. Each chunk keeps
     a partial and the partials combine through `fixed_sum`. Per-province totals such as
     `paid[d]` and `received[o]` keep their bits if the gather walks origins in ascending
     order with the home term at `o = d`; where that is awkward, the same re-bless covers it.
   - Reciprocals: the crossing loop divides four times per crossing. `Crossing` stores
     `inv_duty = 1 / (1 + t)` and `inv_loss = 1 / (tau * border_tau)` at kernel build, and
     `inv_z = 1 / z[a]` is hoisted per row; the same for `e.tau` and `s.y[b]` in the access
     loops. `x / d` and `x * (1 / d)` round differently, which is why this shares the re-bless
     with the gather. The trade SPEC records the new rounding.
   - Target: the one-good critical path from 5.3 ms toward 41.3 ms / 12 cores, about 3.5 ms.
     Rerun `source_dispatch_parallel` against the 8 ms budget.
3. **Typed, compressed history** (bit-identical; history only reads state): implicit tick,
   province and good keys, native column widths (a market row falls from 80 to 49 bytes), then
   ALP (Afroozeh, Kuffó and Boncz, SIGMOD 2024; the `alp` crate) or byte-stream-split plus
   zstd for prices and quantities. Lossless.
4. **Fused multi-good pass** (bit-identical), only if dispatch is still on the critical path
   after step 2: store each crossing's per-good `k` contiguously, goods innermost, and run the
   inner arithmetic as a fixed-width loop over goods. Each good keeps its own operation order,
   so its bits are unchanged. Lanes (ADR 0024) later take the same slot.

Out of scope here, each with its own phase: ensemble variance reduction and chunked Merkle
state with copy-on-write (P3), mixed-precision route-choice refinement (P8, on measurement).
Kernel row scaling for f32 underflow is ADR 0093's.

Option (a) is deferred: memory is under a quarter of dispatch time, so it pays only when lanes
widen the data. Option (c) stays rejected: the hot loops are sparse, latency-bound gathers that
need f64 `powf` and `exp`, Apple GPUs have no f64, and the M4 matrix unit needs nightly
intrinsics or unsafe code, which the workspace forbids.

### As built

Clippy 0.1.93 neither errors nor warns on a `clippy.toml` path it cannot resolve; the ban
silently never fires. The algebraic paths do resolve. The fixture
(`scripts/check_clippy_bans.sh`, crate in `scripts/clippy-bans/`, outside the workspace)
therefore counts the diagnostics: one per banned method (20) and one per use of each banned
type (6), and fails on any other count, so a path that stops resolving fails CI.

Step 2's gather was built bit-identical to the scatter (a proptest held every written field
equal by `to_bits()`, `lost` included), and the stored reciprocals landed on top of it. It
halved one good's dispatch run alone (5.0 to 2.7 ms), but lost inside the tick, which already
runs the seven goods in parallel. Alternating criterion rounds on the full pack (M4 Pro, AC,
98 W, no thermal warnings, 2026-10-26), gather with coarse task sizes against the serial
scatter promoted to `dispatch`:

| bench | gather r1 | gather r2 | scatter r1 | scatter r2 |
|---|---|---|---|---|
| `source_dispatch_parallel` | 10.34 | 10.39 | 8.24 | 7.80 |
| `unit_source_dispatch_parallel` | 7.63 | 7.65 | 6.52 | 6.23 |
| `dispatch_one_good` | 2.69 | 2.72 | 5.03 | 6.05 |
| econ `tick` | 44.75 | 44.43 | 35.19 | 38.44 |
| `source_dispatch_parallel`, 1 thread | 51.15 | | 47.21 | |

Two more scatter runs gave 7.79 and 7.73 ms and ticks of 35.13 and 35.17 ms. Tuning did not
close the gap: coarse task sizes (at least 50 gateways and 512 provinces per task) left the
gather at 10.34 ms; serial targets inside a good, 10.84 ms; recomputing each crossing at its
target instead of storing it, 10.87 ms; dropping the serial `lost` chain altogether (not
bit-identical, a bound only), 9.78 ms. Under the tick each gather pass waits on its barrier
while every core serves other goods, and the gather moves about twice the scatter's memory per
good (the per-leg terms are written and read back), so per-good wall time rose to about 9 ms.
By the decision rule (keep the gather only below both the scatter and 8 ms), `dispatch` is the
serial scatter again, bit-identical, so no re-bless; the gather and its oracle tests are
removed and the reciprocals stay. The scatter's per-call buffers moved into `Sourced`'s
scratch. Step 4's fused multi-good pass was then built and measured; see below.

Step 4 was built and measured, and not kept. The fused pass ran the seven goods' dispatch as one
serial pass over the legs, goods innermost: per-leg factors stored as `[f64; 7]` per entry
(`k`, the reciprocals and the lag), per-gateway and per-province accumulators goods innermost,
outputs transposed back into each good's columns at the end. A test held every written field of
every good, `lost` included, equal to `dispatch` by `to_bits()`. Fusing cut the arithmetic of the
seven goods by about a quarter (one pass about 27.0 ms against 37.6 ms for seven `dispatch` calls in
sequence; a first build that read each good's own `Csr` entries took 103.8 ms), but as one task it
replaces seven tasks that already run in parallel, so the stage became 28.5 ms against the
scatter's about 7.7 ms. Shared index loads and interleaved dependent chains cannot recover a factor of
seven: each good's sums, products and stores remain its own. A single-good pass on the same
layout (`[f64; 1]`, run once per good in parallel) was within noise of the scatter (8.20 against
8.15 ms in one run). Alternating criterion rounds (full pack, M4 Pro, AC, 98 W, 2026-10-27;
the fused build against the scatter at a31f129, each with its own target directory). `source_dispatch_fused` and
`dispatch_fused` were prototype benches and are removed with the code:

| bench | fused r1 | fused r2 | scatter r1 | scatter r2 |
|---|---|---|---|---|
| `source_dispatch_parallel`, scatter in the same binary | 7.70 | 7.75 | 7.76 | 7.78 |
| `source_dispatch_fused` (parallel sourcing, then one fused pass) | 28.30 | 28.65 | | |
| `dispatch_fused` (seven goods, one pass) | 26.95 | 27.03 | | |
| `dispatch` (seven goods in sequence) | 37.65 | 37.57 | 37.75 | 37.32 |
| `dispatch_one_good` | 4.98 | 4.96 | 4.96 | 4.93 |

The fused pass was not wired into the tick: its stage alone is about 20 ms over the scatter's.
`dispatch` stays the serial scatter per good. A serial fusion cannot win against the stage, which
runs the goods in parallel. A fused pass that is also parallel inside, with its barriers paid
once for all seven goods rather than once per good, was not tried and its outcome is unknown.

Gateway count (measured with this step; a proposal only, nothing changed). ADR 0092's gateway
masses are deferred (border friction fit), so access still sums over gateway pairs, and the
moves in Phi_d below are consistent with a smaller count removing access instead of only
redistributing it (the masses were not run). With the pack's own rule at a total of
300 (k-medoids per country, the 600 rule reproduces the pack's `gateways.arrow` byte for byte),
on a scratch pack copy: parallel source and dispatch of the seven goods at the 600 pack's
calibrated prices, unit spending, full fills and a cold sourcing each run, takes 4.70 ms against
9.70 ms at 600 (median of 60, two rounds each). These figures come from a cold harness outside
criterion, so their absolute values differ from the criterion rows above; the ratio is the useful
number.
But |d ln Phi_d| at the same prices has medians of 0.03 to 2.0 by good (maxima 2.9 to 11.3), far
above the 1e-3 bound, and the calibration of the 300 pack does not settle (100 passes, last change
3.2e-2, 842 s), so H2 has no steady state to run on at 300. At 600 H2 distance is -0.7531
(s.e. 0.0161), unchanged. No count is proposed: the count can be set for speed only once the
masses make access independent of it, and then both measurements are to be repeated. Whether
the 300 pack's calibration failure comes from the gateway change or from a limit of the
calibrator was not investigated.

Step 3 (typed, compressed history) is bit-identical: the history only reads state, and the
state hashes and run files are unchanged. In memory the history keeps values only, at native
width (f64, u32, bool); the tick, province and good keys follow from the sampled ticks and the
row order and are built when written, so a market row is 49 bytes instead of 80. Files are
Arrow IPC with zstd buffer compression (the `zstd` feature of `arrow-ipc`, already a
dependency of the workspace through `ceteris-core`), with the columns unchanged;
`ceteris run --history-format csv` writes the same tables as CSV, f64 as the shortest text
that parses back to the same bits. The `alp` crate was not taken: its licence (Apache-2.0)
fits the workspace, but it is version 0.0.4 from a single vendor, and its encoding would need
a custom column encoding that Arrow readers (`read_table`, pyarrow) cannot open, which breaks
the unchanged-columns rule. zstd over the repetitive key columns and the smooth price series
needs no new dependency. A byte-stream-split transform has the same reader problem and was
left out. Full pack, 120 ticks with history: peak 1.58 GB (Arrow) and 1.51 GB (CSV) against
1.88 GB before, and 1.15 GB without history; the markets file is 65 MB (610 MB as CSV).

Re-measured on the branch head after the records were written (M4 Pro, AC, 98 W, no other
job, 2026-10-27; two separate criterion runs, medians): `source_dispatch_parallel` 8.54 and
8.41 ms, a whole tick 37.3 and 35.5 ms, `unit_source_dispatch_parallel` 6.55 and 6.56 ms. The
stage is about 5% over its 8 ms budget and the tick is at or over its 35 ms budget; the runs
earlier in the branch gave 7.7 to 7.8 ms and 35.1 to 35.2 ms on the same code, so the spread
between sessions is as large as the gap to the budgets.

## Tests

- A clippy fixture calling each banned method and naming each banned type fails
  `cargo clippy -D warnings`.
- State hashes match at 1 thread, 3 threads and all threads, before and after step 2.
- Property test: while the gather existed, every field it wrote equalled the scatter's bit for
  bit. The gather is removed; `parallel_splits_do_not_depend_on_the_thread_count` covers the
  parallel loops and the scatter.
- Null-fork test across a route-choice edge change: a child that loads the snapshot taken just
  after the change reproduces its parent's hashes.
- Steps 1 and 3 leave the goldens unchanged; the reciprocals of step 2 re-bless them once,
  together with ADR 0093's weights, in their own commit.

## Consequences

The determinism conventions become build failures instead of review items. Most P2 dispatch
time moves off the critical path, at the cost of one re-bless and 8 bytes per `Crossing`.
History memory falls without touching state. Any later warm-started solver has to declare its
warm-start vector as snapshot state or pay for a cold start.
