# RFC 0317. Canonical summation order, province slices of the ledger and goods-innermost dispatch blocks

Status: accepted
Date: 2026-11-08
Design sections: 4.6, 8.3, 19.2, 19.4
Release: 1.0, P3
Research: owner-side reports `roadblock-recon-2026-11-08.md` (items 1b, 1c, 4c) and
`tick-gap-45-goods-2026-11-08.md`; issues #197, #200, #202; Lee, Vuduc, Demmel, Yelick 2003
(SpMM over k right-hand sides); Demmel, Nguyen 2015 (ReproBLAS)

## Summary

The 45-goods tick measures 230 ms at 14 threads against the 160 ms ceiling (ADR 0235): dispatch
69, produce 50, deliver 34, end_tick 32 ms. The levers left need layout work, and one of them
changes the reference summation order. The owner decided that the order change and the layout
work go in one RFC, so that golden hashes move once. This RFC specifies four things and the
plan that lands them.

1. Produce (E2) runs one task per province. Accounts are opened by kind (pops, then
   enterprises, then markets, `EconWorld::new`), and pops and enterprises are sorted by
   province (19.4), so the accounts a province touches in E2 are three contiguous ranges that
   no other province touches: its pops, its enterprises and its market. The ledger hands out
   those ranges as disjoint mutable slices (money, goods, tally) and each task keeps its own
   journal block. Every account sees the same operations in the same order as the serial
   loop, so balances, holdings, the journal and the state hash are bit-identical (class A).
   The only float sums that cross provinces are the per-good `created` and `destroyed`
   tallies; they are in no snapshot and no hash (`impl Encode for Ledger`), and they feed only
   the goods identity check at a relative tolerance of 1e-9 (`GOODS_TOLERANCE`).
2. The goods ledger stays account-major (an account's goods side by side, as 19.4 asks), so
   province slices are contiguous. Per-good systems keep `take_columns` and `put_columns`.
3. Batched dispatch reads each kernel row once for a block of goods stored side by side
   (`[f64; B]` per row or province), not for goods kept in separate buffers. The spike that
   reused `dispatch_lanes` with goods as lanes was bit-identical but 1.4 to 2.4x slower: each
   lane is its own slice, so one row read became B strided gathers, and NEON has no gather
   (19.4). SpMM beats repeated SpMV only when the k values of a row are contiguous (Lee et al.
   2003). With goods innermost, each good's sums keep their scalar order, so blocked dispatch
   is class A on goods that share a kernel.
4. A canonical order with a fixed number of interleaved accumulators (k = 4) becomes the
   reference for the long single-good reductions that cannot be blocked across goods. This is
   the one golden move. Today's loops are scalar and dependent (44% of dispatch stalled on
   dependent adds at 10 goods, econ Task 19); auto-vectorisation never reassociates floats,
   and hand SIMD changes the order. Fixing k, not "the machine's width", keeps one result on
   every target: scalar code emulates it exactly.

Laws: working, then right, then fast. No stage reports a speed until its correctness gate
passes, and no class A stage moves a golden.

## Decisions

### D1. What is the reference summation order for long reductions?

Options:

- A. Left to right, one accumulator, everywhere (today). Blocked dispatch (D4) still gets
  independent accumulators across goods, but a good with its own kernel (on a real pack,
  every good whose theta or kappa differs, ADR 0279; and every good while rho is on, since the
  lagged crossing kernel is per good, #200) stays stalled on dependent adds. No golden move.
- B. Four interleaved accumulators, fixed: element j goes to accumulator j mod 4, each taken
  in index order, combined as (a0 + a1) + (a2 + a3), with a tail of fewer than 4 elements
  added to its accumulators in the same rule. k = 4 is two NEON f64 registers and one AVX2
  register; scalar, NEON and AVX2 code give the same bits. Applies only to the reductions
  listed in the spec change (dispatch and arrivals row sums of unblocked goods, the kernel
  row normalisers, and any other loop a stage shows at 2% of the tick or more), each named in
  19.4. FMA stays explicit (`mul_add`), as now. One golden move.
- C. The machine's width (2 on NEON, 4 on AVX2, 8 on AVX-512). Fastest per machine; results
  differ across architectures. Fails determinism.
- D. Binned reproducible sums (ReproBLAS) for every reduction: order-free but about 7 adds per
  element, about 4x a dot product; slower than today. ADR 0298's exact accumulator stays the
  tool for scatter accumulators whose order blocks a decomposition.

Recommendation: B, because it is the only option that both removes the dependent-add stall
for goods that cannot be blocked and keeps one result on every target; it is the order the
real 45 to 50 goods pack needs, where few goods share a kernel.

Decision: B, Antonio, 2026-11-08.
Row: ADR 0327

### D2. How does the ledger lay out goods holdings?

Options:

- A. Account-major, as today: `goods[account * n_goods + g]`, goods innermost. Province slices
  are contiguous. Per-good systems keep their transposes (`take_columns` and `put_columns`,
  three pairs a tick at E2 traded, E2 local and E4), about 66 MB each way at 45 goods.
- B. Good-major: `goods[g * n_accounts + a]`. Removes the transposes; a province's holdings
  become 45 strided runs, against 19.4 ("goods are the innermost dimension"), and the
  snapshot byte order changes (a golden move with no model gain).
- C. Account blocks: accounts in tiles of 64, good-major inside a tile. Serves both access
  patterns at some cost to each; a new indexing scheme in every reader.

Recommendation: A, because it is the written design, makes D3 possible without renumbering
accounts, and leaves the snapshot bytes as they are. The transposes are measured in stage S0;
if they exceed 5 ms a tick, block columns (`take_columns` filling B goods side by side,
for D4) are the next step, not a layout change.

Decision: A, Antonio, 2026-11-08.
Row: ADR 0328

### D3. How does province-parallel produce sum the per-good created and destroyed tallies?

Options:

- A. Exact replay: each province task logs its quantities in order; a serial pass adds them in
  province order. Tallies bit-identical; the replay is about one add per enterprise output
  and per input use (several million dependent adds a tick at 45 goods), which keeps part of
  the serial cost.
- B. Per-province partials: each task sums its own `created[g]` and `destroyed[g]`; the
  partials are added in province order after the tasks join. Deterministic and independent of
  thread count; the tallies differ from today's in the last bits. Nothing hashed changes,
  because the tallies are in no snapshot and only feed the 1e-9 identity check.
- C. ADR 0298's exact accumulator for the tallies. Order-free and exact; a new dependency of
  the core ledger on an accumulator that is parked.

Recommendation: B, because the tallies are a check, not state; the fixed province order makes
them reproducible, and the identity check's tolerance is many orders above the difference.
Stage S1 asserts both claims (no hash change, gap well inside tolerance) in a test.

Decision: B, Antonio, 2026-11-08.
Row: ADR 0329

### D4. How does batched dispatch lay out goods?

Options:

- A. Goods-innermost blocks: goods that share a kernel (same theta class and kappa group, in
  good order) form blocks of B = 4 or 8 (stage S3 picks one, recorded in 19.4); price, fill
  and the `Sourced` outputs of a block are stored `[row][B]`, padded to B and aligned to 128
  bytes. The kernel row (`col u32`, k, `inv_duty`, `inv_loss`) is read once and the inner
  loop is a dense B-wide op. Each good's sums keep the scalar order: class A. A good left
  without a block partner runs alone in the D1 order. Ensemble lanes (ADR 0024) sit outside
  the block: `[row][lane][B]`, interactive play runs one lane.
- B. Goods as lanes of `dispatch_lanes`, each in its own buffer (the failed spike). Measured
  1.4 to 2.4x slower.
- C. Transpose into blocks inside dispatch each tick and back out. Keeps other systems
  unchanged; the transposes cost about what the blocking saves at 45 goods.

Recommendation: A, because it is the layout 19.4 already prescribes, it is the condition under
which SpMM wins, and it is bit-identical where goods share a kernel.

Decision: A, Antonio, 2026-11-08.
Row: ADR 0330

## Province slices of the ledger (D3 detail)

- `Ledger::province_slices(ranges)` takes, per province, its pop range, its enterprise range
  and its market account, and returns one `ProvinceLedger` per province: mutable money,
  wide flags, goods and tally slices for those accounts, a journal block taken from the
  scratch pool, and `created` and `destroyed` partials of length `n_goods`. The split is safe
  (`split_at_mut` over the three kind ranges, in province order); no `unsafe` (ADR 0295).
- A `ProvinceLedger` has the ledger's `transfer`, `ship`, `create`, `destroy` and `goods`
  with the same arithmetic. A call that names an account outside the province panics in debug
  builds and is a test failure.
- After the tasks join, `put_province_slices` appends the journal blocks in province order
  (moved, not copied, as RFC 0316 step 1 does for posted blocks) and adds the partials in
  province order. The concatenation equals the serial journal entry for entry.
- Overflow is checked per account in `moved`, as now; each account's sequence is unchanged, so
  the same overflow is caught at any thread count.
- Province tasks are many small fixed chunks with work stealing (19.4 Parallelism); results go
  to fixed slots.

## Golden-hash migration (one change)

1. Stages S1 to S4 are class A. Each passes the ADR 0294 gate as it stands and moves no
   golden. A stage that turns out not to be bit-identical does not land on its own: it is
   folded into S5.
2. S5 is the only PR that regenerates `tests/golden/` and each crate's `tests/golden.rs`. It
   changes only the reductions D1 lists, in one commit, and its body states the old and new
   hashes per golden.
3. S5's evidence replaces "identical" with "explained": per reduction, an oracle test that the
   scalar emulation and the SIMD kernel agree bit for bit on random inputs (proptest), and
   that each agrees with the old order to a relative 1e-12 per call; over 24 full-pack ticks,
   the state hash identical at 1 thread and all threads, on the Mac and on Linux x86, and the
   largest relative difference of prices, quantities and balances against the old order
   reported (expected near 1e-12, bounded at 1e-9); `p0_acceptance` and `p2_acceptance` pass
   unchanged; the ledger checks hold every tick.
4. After S5 the ADR 0294 gate applies again with the new goldens. A later order change needs
   its own RFC.

## Correctness gates before any speed claim

- Working: `cargo test --workspace`, clippy with `-D warnings`, `scripts/check_clippy_bans.sh`,
  the ledger checks every tick.
- Right (class A stages): ADR 0294 (state hash and journal identical over 24 full-pack ticks,
  1 thread and all threads, Mac and Linux x86), plus an oracle test per stage on the fixture:
  province-parallel E2 against the serial E2, blocked dispatch against scalar dispatch per
  good, bit for bit.
- Right (S5): the migration evidence above.
- Fast: only after both. Criterion tick and per-system sim-loop timing on a quiet host
  (`scripts/bench_quiet.sh`), with the host load reported, and single-thread CPU per year as
  the flatness gate measures it.

## Staged implementation plan

| Stage | Work | Size | Class | Expected (est.) |
|---|---|---|---|---|
| S0 | Measure: E2 time by part (traded inputs, local inputs, wages), transpose cost, distinct (theta, kappa) pairs among the 45 traded goods; confirm nothing outside the ledger reads `created` or `destroyed` | S | none | decides D2's block-column trigger and S3's B |
| S1 | `province_slices`, E2 one task per province (D3 B), oracle test | M | A | produce 42 to 51 ms to about 5 to 8 ms at 14 threads |
| S2 | Profile deliver and end_tick superlinearity (recon 1e) | S | none | sizes later work; out of this RFC's scope |
| S3 | One-day spike of D4 A on a real block, 45 goods, 14 threads. Go: a block of 8 goods in under 2x one good's time | S | A | go or no-go for S4 |
| S4 | Blocked layout for source, dispatch, arrivals and deliver of blocked goods | L | A | 30 to 50 ms of dispatch, plus part of source and deliver |
| S5 | D1 B on the listed reductions; the golden move | M | B | 10 to 25% of the remaining single-good loops |
| S6 | Re-measure; restate the 19.2 table; ADR 0235's budget lever only on the measured remainder | S | none | |

S1 and S3 are independent and can run in parallel. S4 waits on S3's go. S5 waits on S4 (or on
S3's no-go), so that every reduction it changes is final.

## Risks

- A province's E2 touches only its own accounts today. A later E2 change that pays across
  provinces breaks the slice API by construction (a debug panic and a failing oracle test),
  not silently.
- Blocks form only where goods share a kernel. On a real pack with per-good kappa that may be
  few goods; D1 B is what keeps those goods fast, and the kappa grid stays the owner call
  ADR 0279 reserved.
- Ensemble lanes outside the block multiply the tile to `lanes x B`; with 32 lanes and B = 8
  a row tile is 2 KB. S3 measures one lane; the ensemble path is measured before 1.0 lanes
  ship.
- The S5 tolerance claims are estimates until S5 measures them; if the 24-tick drift against
  the old order exceeds 1e-9 relative, S5 stops and reports rather than widening the bound.

## Spec changes

- 19.4 Data layout: goods-innermost blocks of B goods for blocked kernels, lanes outside the
  block (D4).
- 19.4 Tick execution: the reference order for the listed reductions is four interleaved
  accumulators combined (a0 + a1) + (a2 + a3) (D1), with the list.
- 19.4 Parallelism: per-province systems receive disjoint account slices; cross-province
  float tallies are summed by province partials in province order (D3).
- 4.6 / `specs/core/SPEC.md` Ledger: `province_slices` and `put_province_slices`; goods stay
  account-major (D2).
- `specs/econ/SPEC.md` E2: one task per province.
- `specs/trade/SPEC.md` dispatch and arrivals: blocked goods (D4).
- 19.4 / ADR 0294: the one-time golden move of S5 and its evidence.
- 19.2: restate the 45-good budget table after S6.
