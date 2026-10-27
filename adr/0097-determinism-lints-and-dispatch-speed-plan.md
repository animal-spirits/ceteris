# 0097. Determinism lints and the post-gate dispatch speed plan

Status: proposed
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
  build, as does a config path clippy cannot resolve.
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
scratch. Step 4's fused multi-good pass is the remaining lever on the critical path.

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

## Tests

- A clippy fixture calling each banned method and naming each banned type fails
  `cargo clippy -D warnings`.
- State hashes match at 1 thread, 3 threads and all threads, before and after step 2.
- Property test: gathered per-target columns equal a scatter reference bit for bit, before the
  reciprocals change.
- Null-fork test across a route-choice edge change: a child that loads the snapshot taken just
  after the change reproduces its parent's hashes.
- Steps 1, 3 and 4 leave the goldens unchanged; step 2 re-blesses them once, in its own commit.

## Consequences

The determinism conventions become build failures instead of review items. Most P2 dispatch
time moves off the critical path, at the cost of one re-bless and 8 bytes per `Crossing`.
History memory falls without touching state. Any later warm-started solver has to declare its
warm-start vector as snapshot state or pay for a cold start.
