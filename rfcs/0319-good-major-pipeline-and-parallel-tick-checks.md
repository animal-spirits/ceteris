# RFC 0319. Good-major pipeline slots and parallel tick checks for the 45-goods budget

Status: proposed
Date: 2026-11-09
Design sections: 19.2, 19.4; `specs/trade/SPEC.md` (pipeline, T1 deliver, T2b dispatch)
Release: 1.0, P3
Research: owner-side report `tick-gap-45-goods-2026-11-08.md`; `tests/perf/history.csv` runs
of 2026-11-08 (mbp, commits 569c353 and 3a00e08); draft PR #373 (RFC 0317 stage S3 spike)

## Summary

After RFC 0317 stage S1 (province-parallel produce) the 45-goods tick measures 202.5 ms at 14
threads on the Mac (criterion, load 2.0, run of 2026-11-08 23:02): dispatch 71.3, deliver 37.2,
end_tick 29.0, produce 23.9, consume 10.8, begin_tick 10.6, clear 9.5, source 8.6, labour 6.8 ms.
The ceiling is 160 ms (ADR 0235, ADR 0325). The S3 spike of blocked dispatch is a no-go (#373:
a block of 8 goods costs 8.9 times one good, nothing amortised), so S4 does not close the gap.

The span timings (run of 20:26, same host) show that about 70 ms of the tick is not kernel work
but memory traffic and serial passes that a layout change and parallel checks remove, all
bit-identical:

| Span | ms | Cause |
|---|---|---|
| `in_flight_all` (inside deliver) | 24.1 | four pipeline slots, `buyers x goods` each, scanned in blocks of 8 goods: 6 tasks for 14 threads, every task streams all four slots |
| deliver, rest | about 7 to 13 | each of 45 good tasks walks the whole buyer-major `due` matrix twice at stride 45, so every task touches every cache line of it; plus a serial copy of `due` |
| `t2b.apply` | 11.2 | `push_all`: a bucket sort of every good's shipments into (slot, buyer range) keys, then 64 tasks that each walk all 45 lists |
| `t2b.transit` | 7.9 | the in-flight scan again, after `push_all` cleared the cache |
| `begin.totals`, `end.ledger.totals` | 7.1, 7.0 | `Ledger::good_totals` copies the whole goods ledger (about 66 MB) into a transposed buffer, serially, then sums 45 columns one after another |
| `end.leontief` | 12.6 | serial loop over enterprises x goods |
| `end.ledger.sign` | 2.2 | serial scan of the goods ledger for NaN or negative holdings |

The cause of the first four rows is one layout fact: the pipeline is the one goods-indexed
array whose every hot reader works one good at a time (deliver per good, push per good,
in-flight per good) while it is stored goods-innermost, as 19.4 prescribes for every
goods-indexed array. Storing each slot good-major makes each good's data one contiguous run.
Every per-good sum then adds the same terms in the same order as today, so it is class A. The
last three rows are serial passes over account-major data that parallelise without changing a
result; they need no decision, and are part of the plan so that one RFC closes the gap.

Estimated result (estimates from the spans, not measurements; stage S0 measures): 64 to 75 ms
off, a tick of about 130 to 145 ms at 14 threads. Dispatch's kernel (about 50 ms of task time)
and the blocked-dispatch masked variant stay outside this RFC.

Laws: working, then right, then fast. Every stage below is class A and passes the ADR 0294
gate before it reports a speed. No golden moves.

## Decisions

### D1. How are the pipeline slots laid out?

Options:

- A. Good-major slots: `slots[s][g * n_buyers + b]`. Deliver, push and the in-flight total of
  good g each read or write one contiguous run per slot. The in-flight sum of good g runs over
  storage slots 0 to 3, buyers in order, from -0.0, as today: bit-identical. `push_all`
  becomes one task per good, each adding its list in order to its own runs (disjoint `&mut`
  slices, no sort): every cell gets the same terms in the same order, bit-identical. Readers
  of a single cell (`on_order(buyer, g)`) are unchanged in cost. A stated exception to 19.4's
  goods-innermost rule, for this array only. Ensemble lanes stay innermost:
  `[s][g][b][lane]`.
- B. Keep buyer-major slots; transpose `due` once a tick and keep per-slot in-flight partials
  by buyer chunk. Keeps 19.4 as written; the transpose costs about one slot's traffic a tick,
  and buyer-chunk partials change the in-flight summation order (a golden move, ADR 0327
  rules), for less saving than A.
- C. Keep buyer-major slots, only widen the in-flight scan to more tasks (blocks of 4 goods,
  or buyer chunks summed in a fixed order). Removes part of the 24 ms; the strided deliver
  scans and the push sort stay. Buyer chunks again move the order.

Recommendation: A, because it removes all four pipeline costs, is bit-identical, and the
exception follows the rule's own reason (the layout serves the access pattern of the hot
readers); every hot reader of this array works per good.

Decision:
Row:

### D2. What byte order does the snapshot keep for the pipeline?

Options:

- A. Keep today's bytes: `encode` writes each slot buyer-major and `decode` transposes back,
  both in parallel by good. No file-format change, snapshot goldens unchanged; snapshots are
  monthly at most, and the transpose is about 4 x `buyers x goods x 8` bytes each way.
- B. Write the good-major bytes. Saves the transpose; changes the snapshot format and every
  snapshot golden with no model change, a public format change for a few ms a snapshot.

Recommendation: A, because the format is public (CLAUDE.md: formats change only by RFC and for
cause) and the cost is outside the tick.

Decision:
Row:

### D3. Where are the in-flight totals computed?

Options:

- A. Inside the per-good tasks that already touch the good: T1's deliver task computes good
  g's total right after its deliveries, T2b's per-good task right after its pushes, each over
  its contiguous runs in today's order. `in_flight_cached` returns the values those tasks
  wrote (a fused parallel region, no separate scan). Bit-identical.
- B. A separate parallel scan, one task per good, after each pop and push. Bit-identical and
  simpler; one extra pass over the slots (about 2 to 3 ms est.) twice a tick.
- C. Running totals kept by push and pop (add the pushed units, subtract the popped slot's
  total). Cheapest; changes the order and accumulates rounding across ticks, so the transit
  accounts drift from the slots (the 1e-9 pipeline invariant would need a re-sync rule).

Recommendation: A, because it is bit-identical, has no extra pass and keeps the invariant
exact by construction; B is the fallback if the fused tasks make the code unclear.

Decision:
Row:

## Parallel checks and totals (no decision; class A)

- `Ledger::good_totals`: no transposed copy. One task per (good, chunk of 1024 accounts) runs
  `pairwise` over the strided column view, then `pairwise` over each good's partials: the
  same tree as `fixed_sum` on the copied column, so the same bits. Serves `begin.totals` and
  `end.ledger.totals`.
- `end.ledger.sign`: a parallel search for the first bad holding by index (`position_first`
  semantics), so the violation reported is the one the serial scan reports.
- `end.leontief` and the other end_tick enterprise checks: parallel by enterprise chunks,
  reporting the first failing enterprise in index order. The checks read only; results are the
  same.

## Correctness gates before any speed claim

- Working: `cargo test --workspace`, clippy with `-D warnings`, `scripts/check_clippy_bans.sh`,
  the ledger checks every tick.
- Right: ADR 0294 for every stage (state hash identical over 24 full-pack ticks at 1 thread and
  all threads, Mac and Linux x86; journal as ADR 0331 states). Oracle tests on the fixture:
  `push_all` against `push` in turn (exists, kept); `in_flight` of every good against a serial
  sum of the buyer-major slots; deliver against the serial deliver; snapshot bytes equal to
  today's for the same state (D2 A); `good_totals` against `fixed_sum` of the copied column;
  each parallel check reports the same first violation as the serial one on seeded bad states.
- Fast: after Right, criterion tick and per-system sim-loop timing on a quiet host
  (`scripts/bench_quiet.sh`), load reported; single-thread CPU per year for the G1 flatness gate.

## Staged implementation plan

| Stage | Work | Size | Class | Expected (est.) |
|---|---|---|---|---|
| S0 | Quiet-host re-baseline of the 45-goods tick and its spans; buyers count and slot bytes | S | none | confirms the table above |
| S1 | Good-major pipeline (D1), snapshot bytes kept (D2), deliver per good on contiguous runs, `push_all` one task per good, oracle tests | M | A | deliver 37 to about 6 to 10 ms; `t2b.apply` 11 to about 2 ms |
| S2 | In-flight totals inside the per-good tasks (D3) | S | A | `in_flight_all` and `t2b.transit` 32 to about 1 to 2 ms |
| S3 | `good_totals` without the copy; parallel sign and Leontief checks | S | A | 29 to about 4 ms over begin_tick and end_tick |
| S4 | Re-measure; restate the 19.2 table (ADR 0235); if still above 160 ms, the remainder goes to the masked block-dispatch variant (#373) or ADR 0235's levers, by measurement | S | none | |

S1 then S2 (S2 needs S1's runs). S3 is independent and can land first. RFC 0317 stage S5 (the
interleaved-accumulator golden move) is unaffected: none of the sums here is on its list, and a
later reordering of them would be its own RFC.

## Risks

- The estimates assume the spans are additive within the tick; the criterion tick and the span
  tick differ by about 8 ms. S0 measures before any stage starts.
- A future reader that wants one buyer's goods side by side from the pipeline pays strided
  access. Today only `on_order` reads by buyer, one cell at a time.
- Dispatch's kernel stays the largest system (about 50 ms of task time). If S4 lands above
  160 ms, the next lever is a measured one, not this RFC's.

## Spec changes

- 19.4 Data layout: the pipeline slots are good-major `[slot][good][buyer]` (lanes innermost),
  an exception to the goods-innermost rule because every hot reader works per good (D1).
- `specs/trade/SPEC.md` pipeline table and T2b goods moves: `push_all` one task per good;
  in-flight totals computed in the per-good tasks of T1 and T2b (D1, D3).
- `specs/trade/SPEC.md` (or `specs/core/SPEC.md` snapshot section): the pipeline's snapshot
  bytes stay buyer-major (D2).
- 19.2: the 45-goods table restated after S4.
