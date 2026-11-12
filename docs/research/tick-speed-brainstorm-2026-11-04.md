# Tick speed brainstorm (2026-11-04)

Goal: the full-pack tick from about 42 ms to under the 35 ms budget (spec 21, 19.2), and a plan
that still holds when the goods list grows from 10 (7 traded) to about 50. Measured on `main` at
3103d07 (#124). f64 stays (settled); every idea keeps runs bit-identical to each other.

Each idea carries a determinism class:

- **A**: same bits as today. No golden moves; checked by the 24-tick state-hash and journal
  comparison #124 used.
- **B**: deterministic but reassociates float sums, so goldens move once (CHANGELOG
  `### BEHAVIOUR`). Still identical across runs, thread counts and architectures.
- **C**: changes the model. Needs an RFC.

## 1. Profile summary

**Setup.** M4 Pro, 10 performance and 4 efficiency cores. The installed pack
(`data/packs/2000`, 1 Nov) no longer matches `main`'s static digest, so I recalibrated a
private copy in the scratchpad (19 min, `ceteris calibrate`; the installed pack is untouched).
**Noise:** the load average was 11 to 46 throughout because another thread was calibrating, so
wall times are ±10% at best. Instruction counts are the stable numbers.

**Whole tick**

| measure | value |
|---|---|
| criterion `econ_pack/tick` | 42.3 ms [41.8, 43.2] |
| sim loop, `ceteris run`, 60 and 108 ticks (difference against a 1-tick run) | 43.5 and 43.4 ms per tick |
| instructions per tick (all threads) | 3.36 G at 60 ticks and at 108 ticks; flat, no growth with run length |
| single thread (`RAYON_NUM_THREADS=1`) | 148 ms per tick, 3.25 G instructions, 0.67 G cycles, IPC about 4.9 |
| CPU per tick at 14 threads | user 246 ms + sys 55 ms, against 148 ms of serial work |

The parallel speed-up is 148 / 43 = 3.4x on 14 threads. The code does not run too many
instructions (IPC 4.9 is high for this core). The time goes to **serial sections and load
imbalance**: about 150 ms of CPU per tick is spent spinning, in syscalls and in slower cores.

**Per system** (criterion, under load; the per-system benches start from cold clones, so they
add up to more than the tick):

| system | ms |
|---|---|
| deliver | 2.2 |
| labour | 2.8 to 4.8 |
| produce | 5.2 |
| consume (E3) | noisy; about 6 to 7 in quieter runs (#124: 6.0) |
| of which price_refresh | 4.4 |
| source (T2) | 1.7 to 3.6 |
| clear | 1.9 |
| **dispatch (T2b)** | **24.7** |
| reprice | 0.24 |
| distribute | 0.95 |
| end_tick | 3.5 |
| local_chain (inside produce) | 1.5 |

**Where the CPU goes** (Time Profiler, 25 s of `econ_pack/tick`, about 140k samples once pack
loading is removed, grouped by the innermost ceteris frame):

| share of CPU | what |
|---|---|
| 17.8% | rayon spinning and idle (`swtch_pri`; most of the 55 ms of sys time) |
| 13.9% | ledger: `post_all` chunk deltas, journal copy, `transfer`, `good_totals` |
| 12.5% | `ceteris_trade::source::dispatch` (kernel dispatch per good) |
| 12.1% | sourcing: `source::source` closures, `origin_access_into` (T2, and E3's unit source) |
| 9.1% | end_tick invariants; mostly the income check, which scans the whole journal |
| 6.7% | malloc, free, Vec growth, page zeroing |
| 6.2% | T2b `trade_legs`, `buyer_orders`, `split` |
| 4.4% | E3 demand (PIGL, `category_prices`; `libm::pow` is about 1.7% of the tick) |
| 4.3% | labour, produce, `cap_output` (also `pow`) |
| 3.4% | T2b `flows.record` |
| 2.5% | E3 price refresh (`arrivals`, `delivered_prices`) |
| 1.8% | local chain |

**Serial sections on the main thread.** I estimate 15 to 18 ms of each 42 ms tick runs on the
main thread with no other work in parallel. The estimate is rough, because tick edges were
inferred from the bench's clone calls. The shares of that serial time are:

| share | where |
|---|---|
| 16% | T2b serial tail in `systems::dispatch` (goods moves, pipeline pushes) |
| 16% | `Pipeline::in_flight_all`, called 3 times per tick (deliver, dispatch, end_tick) |
| 11% | `clear` |
| 11% | `produce` (serial ledger goods moves) |
| 9% | `deliver` |
| 7% | `Ledger::transfer`, one entry at a time (clear, distribute, deliver) |
| 5% | `end_tick` serial part |
| 3.5% | Vec growth in the ledger |
| 3% | `good_totals` |

Earlier profile of the T2b per-good region (trade SPEC, 2026-11-03): its wall time is the
slowest good, 11.9 ms, against 71 ms of task time over 7 goods on 14 threads, followed by a
6.8 ms serial tail.

**Reading.** About 42 ms ≈ 15 to 18 ms serial + about 12 ms for the T2b per-good critical path
+ about 12 to 15 ms in other parallel regions and barriers. To reach 35 ms, the serial sections
and the T2b critical path are the levers. A 10% cut in instructions buys little on its own.

**Bench artifacts to know.** `iter_batched` clones the world each iteration. A clone does not
keep the spare capacity of its Vecs, so the timed tick pays to regrow them (`finish_grow`) and
to fault in fresh pages. The sim loop measures the same 43 ms, so the artifact is small today.
It will hide or exaggerate allocation fixes, so judge those in the sim loop.

## 2. Ranked ideas

Gains are wall time on today's 7-goods pack unless marked. Cost: S under a day, M a few days,
L a week or more.

| # | idea | class | gain | cost | risk | measure first |
|---|---|---|---|---|---|---|
| 1 | Run T2b's serial goods moves in parallel by good, and overlap each good's moves with the dispatch of the other goods | A | 2 to 4 ms | S-M | low | per-good tail time in the sim loop |
| 2 | Journal as owned blocks (move the Vecs in, no `par_extend` copy), and an income check that reads only the WAGE, DIVIDEND and REBATE blocks, or tallies at posting | A | 2 to 3.5 ms | S-M | low | end_tick 3.5 ms; journal length per tick |
| 3 | `in_flight_all`: compute it once where the pipeline has not changed in between, and run it in parallel by good (each good's column sum keeps its order) | A | 1 to 2.5 ms | S | low | its 3 call sites' main-thread time |
| 4 | Batch the remaining serial ledger traffic (clear, deliver, produce, distribute) into entry blocks for `post_all`, with goods moves in parallel by good, the way #124 did for T2b | A | 3 to 5 ms | M | medium: per-good order of `created` and `destroyed` must hold | serial main-thread time per system |
| 5 | `post_all`: replace the dense per-chunk `i128` delta arrays (accounts × chunks) with bucketed sparse sums; integer sums are exact in any order | A | 0.5 to 1.5 ms, plus CPU | M | low | chunks per tick × accounts |
| 6 | Largest goods first in the per-good region (LPT order; outputs are per-good slots) | A | 0 to 2 ms | S | none | per-good task times and which core runs them |
| 7 | Keep rayon workers on the performance cores: QoS user-interactive for worker threads, or 10 threads, or a dedicated pool for the per-good region | A | 0 to 3 ms | S | none | the bimodal 4.8/6.1 ms per good in the trade SPEC |
| 8 | Cache each crossing's weight `k × carry × y_b`: it is computed in the z pass and again in the share pass | A | 0.5 to 1.5 ms on the critical path | S | none | crossing pass time per good |
| 9 | Run the z pass in parallel over origin gateways (each row is independent) | A | 0.3 to 1 ms | S | none | same |
| 10 | Interleave 2 to 4 independent CSR rows per loop to hide add latency (each row keeps its own order) | A | 10 to 25% of the crossing pass | M | low | the CPU counters in econ SPEC Task 19: 44% of cycles stalled on dependent adds |
| 11 | Remove bounds checks from the hot loops (`index`, `as_slice`, `eq` leaves in dispatch), safely with iterators and pre-sliced rows, or with `get_unchecked` under an exception to `unsafe_code = forbid` | A | 5 to 10% of dispatch | S-M | none for safe code; the `unsafe` route changes a rust.md rule | count of panicking branches in the dispatch disassembly |
| 12 | PGO (`cargo pgo`) and `-C target-cpu`. Rust never contracts FMA without an explicit `mul_add`, so float results stay the same; confirm on x86 and arm | A | 3 to 10% overall | S | low: CI must rebuild the profile | sim loop with and without PGO; hash comparison |
| 13 | Allocation churn: persistent scratch for per-tick Vecs (`end_tick` slots, `post_all` lists, `blocks`), and mimalloc as allocator | A | 1 to 2 ms; much of the sys time | S-M | none | Instruments Allocations: bytes and calls per tick |
| 14 | Fewer, coarser parallel regions; fuse adjacent `par_iter`s; batch tiny ones serially. Each barrier costs 20 to 100 µs plus its imbalance | A | 1 to 3 ms | M | low | count of parallel regions per tick |
| 15 | Split each good's dispatch into its two demand classes as parallel tasks, with per-class accumulators combined at the end | B (A only with #17) | 3 to 5 ms (about 40% of the critical path) | M | low | class pass times |
| 16 | Deduplicate `pow` in demand, labour and `cap_output` on (base, exponent) per province or market rather than per pop or enterprise | A | 0.5 to 1 ms | S | none | `pow` call count per tick |
| 17 | Order-free exact summation for scatter sums (Neal's xsum, a Kulisch-style superaccumulator, or ReproBLAS binned sums): any order gives the same correctly rounded sum, so dispatch can gather by destination, split classes and chunk freely | B, one golden move | enabler: unlocks #15 and intra-good chunking; large at 50 goods | L | medium: about 2 to 4x the cost of an add on those accumulators | microbench of `xsum` against `+=` on the at_b and arrive scatters |
| 18 | A task graph across systems: every good's T2, T2b and T3 as one pipeline per good, with money joined at the end, in place of a barrier between each pair of systems | A if dependencies hold | 3 to 8 ms | L | medium: ordering proofs | data-dependency map of the systems |
| 19 | Warm-started solvers (PIGL inversion, clearing wage, labour, Leontief) with the start point carried in the snapshot (ADR 0097 allows it only then) | B (state and schema change) | depends on iteration counts | M | medium | rounds per call per tick; the Anderson probe in `fixed-point/` is the harness |
| 20 | Model shortcuts: skip a good's kernel dispatch when its prices and fills did not move; prune or low-rank the sea legs | C | large | L | high | not first |
| 21 | GPU (Metal or wgpu), SME on the M4, or the AMX unit | rejected | n/a | n/a | WGSL and Metal have no f64; SME and AMX accumulate with fused multiply-add and are Apple-only, so hashes would differ across architectures | n/a |
| 22 | f32 state or kernel | rejected (settled) | 6% on one pass | n/a | n/a | n/a |
| 23 | Hand-written assembly or NEON intrinsics for division and gathers | low value | the 2-lane f64 SIMD division was 0.4 ms slower (econ SPEC Task 19); the code is limited by gathers and scatters | M | low | only after #10 |

### Notes on the top items

- **#1, #3 and #4 are one theme.** Ledger goods slots are indexed by account × good, so moves
  of different goods never touch the same slot. Only the per-good `created` and `destroyed`
  tallies and the pipeline pushes need their order kept, and that order is within each good.
  Cutting the serial loops by good, in the same order within each good, gives the same bits.
  Together these attack most of the 15 to 18 ms of serial time.
- **#2.** The income check is an O(journal) scan every tick, 9% of all CPU. The same total is
  available where distribute pays the flows, or by keeping the journal as blocks tagged by
  flow. The invariant stays as strong; it just reads less.
- **#6 and #7 go first because they are free.** The trade SPEC already records one good's
  dispatch as bimodal by the core it lands on (4.8 against 6.1 ms). With 7 tasks on 14 threads,
  one task on an efficiency core sets the wall time of the whole region.
- **#17 is the strategic one.** Today, with 7 tasks and 14 threads, the per-good region has idle
  cores and wants intra-good parallelism. At 30 to 40 traded goods it becomes
  throughput-bound, about 180 to 240 ms of CPU by the 28 Oct reality check, and then only
  per-crossing cost (#8, #10, #11, #12) and fewer serial sections help. Exact accumulation is
  what lets the scatter loops be decomposed in any way without fixing an order. Pay for it once,
  with a golden move, before the 50-goods pack lands.

### What #1 to #13 add up to

All of these are class A, so they need no RFC and move no golden. Their mid estimates add up to
about 12 to 20 ms of wall time, partly overlapping, which reaches 35 ms with margin. The safe
plan is to bank #1, #2, #3, #6 and #7 first (about 5 to 10 ms, each at most S-M), then measure.

## 3. What to measure first

1. **A per-system wall-time harness in the sim loop.** `Instant` spans around each system and
   the T2b sub-phases, with per-good task start and end times and the core type each ran on
   (`pthread_get_qos_class_np`, or the core id where available). Run 60 ticks of `ceteris run`
   on a quiet machine. This replaces the cold-clone per-system benches.
2. **Instruction counts per system at 1 thread** (`/usr/bin/time -l` reports retired
   instructions and cycles on Apple silicon), so that noise from other threads does not move the
   numbers.
3. **Journal length per tick, `post_all` chunks, accounts, and parallel regions per tick:**
   the inputs to #2, #5 and #14.
4. **Allocation bytes and calls per tick** (Instruments Allocations, or a counting global
   allocator in a probe).
5. **Thread placement:** `RAYON_NUM_THREADS` 10 against 14, and QoS user-interactive against
   default, on a quiet machine. My two rounds (44 and 43.7 ms) were drowned by load.

## 4. Reproduce

```
cargo build --release --bin ceteris
ceteris calibrate --pack <copy of data/packs/2000> --out <dir>   # copy the 6 tables into the copy
CETERIS_PACK=<copy> cargo bench -p ceteris-econ --bench econ -- 'econ_pack/'
/usr/bin/time -l target/release/ceteris run --pack <copy> --ticks 61 --out <dir>   # minus a 1-tick run
xctrace record --template 'Time Profiler' --launch -- <econ bench binary> --bench 'econ_pack/tick$' --profile-time 25
```

The profile was aggregated from `xctrace export` (time-profile table) by the innermost frame
whose symbol names a ceteris crate. The scripts are in the thread's scratchpad and are not
committed.
