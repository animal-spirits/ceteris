# Closing the 45-goods tick gap: structural options, 8 November 2026

Question: how to get the 45-goods tick (329 ms, 14 threads, calibrated synthetic pack) under the 160 ms ceiling (DESIGN 19.2, ADR 0235). Read: RFC 0316, `tests/perf/history.csv`, both tick-speed brainstorms, `specs/trade/SPEC.md` (gateways), `specs/econ/SPEC.md` Task 19, DESIGN 8.3 and 19.2, ADRs 0092, 0235, 0279, `ceteris_trade::source::dispatch` and `Kernel::build`. Savings are my estimates from the measured numbers, not new measurements. Nothing implemented.

## 1. Two facts that shape the answer

1. **Dispatch alone cannot close the gap.** Measured systems sum to 233 ms (dispatch 108.2, produce 52.1, deliver 36.1, consume 11.0, clear 10.4, source 8.6, labour 6.5). Against 329 ms, about 96 ms is unaccounted (post, apply, push_all, serial sections, barriers), unless 329 predates RFC 0316 steps 1-2 and #362; `history.csv` has one row, under load 13. With dispatch at zero the tick would still be about 221 ms. Any plan must also cut the 96 ms and produce/deliver.
2. **Dispatch is superlinear because every good has its own kernel.** One thread, synthetic packs: dispatch 9.7 ms per good at 10 goods, 18.3 ms per good at 45 (DESIGN 19.2, 7 Nov). `Kernel::build` makes one `GoodKernel` per traded good (tau depends on theta_g and kappa_g, ADR 0279). A good's legs are about 20 MB (356,080 crossings at 32 B padded, 265,558 domestic, 20,705 access), so 45 kernels are about 0.9 GB, and 14 concurrent goods stream 14 different 20 MB sets through a shared cache. At 10 goods the kernel was compute-bound (55% useful, 44% stalled on dependent adds, Task 19); at 45 it is also bandwidth-bound. DESIGN 8.3 and 19.2 already prescribe the fix and it is unbuilt: "Goods sharing a (theta class, weight class) pair share a kernel, so kernel products are matrix-matrix (provinces x goods)".

## 2. Options, ranked

Classes: A bit-identical, B deterministic with one golden move, C model change (RFC).

| # | Option | Dispatch saving (of 108 ms) | Determinism | Effort | Notes |
|---|---|---|---|---|---|
| 1 | **Kernel classes, batched dispatch** (DESIGN 8.3/19.2 as written): one kernel per (theta class, kappa group); dispatch reads each CSR row once and loops over the group's goods inside, each good's sums in today's order | 40 to 65 ms; also about 3 to 5 ms of `source`, and 0.5 to 0.8 GB of kernel memory against the 10.5 GB peak | A where goods already share theta and kappa. The synthetic pack resamples the 10 real goods, so most of its 45 goods already share kernels: exact there. On a real 45-50 goods pack, a kappa log-grid moves tau once: C, the owner call ADR 0279 reserved | M-L | Inner loop over goods gives independent accumulators, which attacks the 44% dependent-add stall as well as bandwidth. Risk: the ensemble lanes ran at 0.6-0.7x scalar (#197); a one-day spike (2 and 8 goods per kernel, 45-goods world, 14 threads) decides go/no-go |
| 2 | **Gateway count 600 to 300** (ADR 0235's first lever, 0092) | crossings fall to about 1/4; measured source+dispatch halved at 7 goods (9.7 to 4.7 ms). About 35 to 50 ms alone, about 20 to 25 ms on top of option 1 | C: ln Phi_d moves 0.03 to 2.0 by good; recalibrate, goldens move | L | Blocked: the 300 pack fails calibration (Sinkhorn pass 3, residual 4.2e-5). Needs the per-class nu and masses first |
| 3 | **Separable crossing cost**: tau_ab = tau_AB x gateway factors, so crossing passes cost O(G + C^2) instead of O(G^2) | 50 to 70 ms | C, RFC, recalibration, H2 risk | L | Country-pair costs are standard in the literature, but Task 19b found within-pair tau spreads of 1.3x, which at theta 16.9 is an 80x weight spread: a real loss of port choice. Hold unless 1 and 2 fall short |
| 4 | **Layout**: crossings as SoA (k, inv_duty, inv_loss, col u32, lag u8), both demand classes in one pass over each row (two accumulators, each in its order) | 10 to 20 ms at 45 goods; was at most 1.2 of 5.2 ms at 10 goods | A | S-M | Mostly subsumed by option 1 (the kernel bytes per good fall anyway). Overlaps the concurrent dispatch-cost thread; leave it there |
| 5 | **Tiered update rates**: re-dispatch slow goods every n ticks, or on price moves | about 45% at 2/3 of goods quarterly | C; carried shares must be in the snapshot (ADR 0097) | M | Stale routing changes dynamics and has no literature basis; fails the literature-fidelity tiebreak |
| 6 | **Smaller per-tick good set**: fewer traded goods, or exact skip of unchanged goods (#240) | about 2.4 ms per traded good dropped; exact-skip hit rate likely near 0 with float prices | C (scope) / A (skip) | S to measure | Cuts the 45-goods goal (ADR 0271) rather than its cost |
| 7 | **Budget revision**: ceiling to 200 or 250 ms by new ADR | closes any remainder | none | S | ADR 0235's last lever by its own order; decide from measurements after 1 and 2 |

## 3. Projection

329 ms minus option 1 (about 50) and its share of `source` (about 4) gives about 275 ms. Option 2 on top gives about 250 ms. The rest, about 90 ms, has to come from the unaccounted 96 ms and from produce (52, superlinear: 5.9 ms per good fit, likely the goods x goods IO block per province) and deliver (36, superlinear). Those are not dispatch and are out of this note's scope, but they decide whether 160 ms is reachable at all. A realistic outcome without them is 200 to 250 ms, which is option 7's range.

## 4. Recommendation

**Build option 1, kernel classes with batched dispatch, after a one-day spike.** It is the design as written (no deviation, no RFC for the code), bit-identical on today's synthetic pack, the largest single lever, and it cuts memory toward the 6 GB target. Sequence:

1. Measure (S, before anything): the unaccounted 96 ms by system on a quiet host, and distinct (theta, kappa) pairs among the synthetic pack's traded goods. If 329 ms is stale, re-baseline.
2. Spike option 1 (1 day, E7 rule): go if two goods sharing a kernel dispatch in under 1.4x one good's time at 45 goods, 14 threads.
3. Build option 1 (M-L). Owner call later, by RFC: the kappa grid for real 45-50 goods packs (ADR 0279).
4. In parallel, unblock option 2 through the nu/masses calibration work already planned (ADR 0092).
5. Profile produce and deliver superlinearity (brainstorm B1).
6. Re-measure; if above 160 ms, write the option 7 ADR with the measured remainder.

Reject 5 and 6. Hold 3 unless 1 and 2 leave dispatch above about 40 ms.
