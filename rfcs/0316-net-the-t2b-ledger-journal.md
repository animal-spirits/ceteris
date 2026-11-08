# RFC 0316. T2b posts netted legs; posting stops copying and zeroing per-chunk vectors

Status: draft
Date: 2026-11-08
Design sections: 4.6, 19.2, 19.4
Release: 1.0, P3
Research: issue #202 (G3), PR #355 profile

## Summary

At 45 goods `Ledger::post_all` posts 22.2M entries a tick (PR #355 profile): about 8M buyer
payments (at most 178,666 buyers x 45 goods) and about 14M trade legs, all but the buyer
payments between market, gateway and rebate accounts. It costs 62 ms of the 168 ms dispatch at
14 threads (161 ms on one), against a 160 ms tick budget. The cost is memory traffic, not
arithmetic: a 530 MB journal copy, and 170 dense `i128` delta vectors over 184k accounts
(about 500 MB zeroed, scattered into and read back). Nothing in production reads the journal:
it is cleared every tick, is in no snapshot or state hash, and the only consumers are tests,
the `identity` example, and the per-account `tally` for WAGE, DIVIDEND and REBATE (RFC 0287
item 3). An entry carries a flow, two accounts and an amount, never a good, so a journal
holding the sum of each good's entries per (flow, from, to) tells everything the Flow tag can.

Two changes, in order. Neither changes a balance, a state hash or a golden.

1. Bit-identical (ADR 0294 gate as it stands). The journal keeps the per-good blocks it was
   posted from instead of copying them (the buffers move into the ledger and return to the
   scratch pool at `begin_tick`), and `post_all` sums into a fixed number of lane vectors
   (32; chunk i goes to lane i mod 32, a lane takes its chunks in order) instead of one per
   chunk. Memory traffic falls from about 3 GB to about 1 GB a tick. Lanes are fixed, not
   per-thread, so an overflow is caught at the same lane addition at any thread count. The one
   observable difference is which transient overflow is caught, which ADR 0294 already
   leaves open ("only an overflow check needs an order").
2. Netted legs (this RFC's decision). Every good shares the network's legs and their order
   (`Kernel`: "all goods share the network's legs"), so trade legs have a fixed edge index. T2b
   writes each good's rounded leg amounts into a dense per-edge column instead of entries, sums
   the columns over goods (exact: integers), and posts one entry per edge with a non-zero
   total. Buyer payments likewise: one entry per buyer with a non-zero total, to its market.
   Rounding is unchanged (half to even per buyer and good, per leg per good, `split` per
   good), so the sums of rounded amounts, hence every balance, equal today's. Entries fall from
   about 22M to the number of network edges plus buyers, an estimate of 0.5 to 1.5M to be
   confirmed with the G3 row counts.

## Decisions

### D1. What does the T2b journal record?

Options:

- A. Per-leg journal as now; ship step 1 only. Expected post 62 to about 20 ms at 14 threads;
  no spec change beyond the overflow note; the 24-tick journal identity gate stays as is.
- B. Netted journal for PURCHASE, TRADE and TARIFF: one entry per (flow, from, to) per tick,
  in a fixed order (flow, then edge or buyer order). A per-leg mode (`journal_detail`, off in
  production, on in the `identity` example and in tests that count legs) posts today's entries.
  Expected post 62 to about 3 to 5 ms; the per-good tasks also stop writing 530 MB of
  entries. The gate becomes: state hash identical, and the netted journal of a detailed run
  equal to the netted run's journal (an oracle test, as `post_all_is_post_of_each_block_in_turn`
  does for posting).

Recommendation: B, after A lands. A alone leaves post near 20 ms and the journal at 530 MB
resident; B removes the entries instead of moving them faster, and P5's flow matrices
(`specs/p5/PLAN.md`) need only net flows per account pair, which they can accumulate during
posting.

Decision: pending.
Row: pending

## Risks

- The `p2_properties` tests that count or scan legs (journal length deltas, per-good gateway
  nets, lines 858 to 1081, 3222 to 3435, 4171) run in detail mode; a mismatch between modes is
  a test failure by construction. About a dozen tests change their setup, none their claim.
- Two posting paths exist until the detail mode is dropped. The oracle test and the `identity`
  digest keep them equal; drop the detail path only if a later RFC retires leg-level audit.
- A netted amount sums up to 45 rounded legs; it fits `i64` (world GDP in minor units is far
  below 9.2e18) and is asserted, not split.
- Opposite-direction legs on one pair (a to b and b to a) stay separate entries; netting them
  would lose the gross flow a matrix may want.
- Future per-good flow accounting (not in any plan) would need the detail mode or a good tag
  on the Flow; the journal could not give it today either.
- Step 1 changes the lane partition of the overflow check; goldens cannot be affected unless a
  run overflows, which halts it today.

## Spec changes

- 4.6 / `specs/core/SPEC.md` Ledger: the journal of a tick is the sequence of posted blocks;
  T2b's PURCHASE, TRADE and TARIFF entries are per-(flow, from, to) totals (D1 B), with
  `journal_detail` for per-leg entries.
- `specs/trade/SPEC.md` T2b: legs written to per-edge columns, summed over goods, posted once.
- 19.2: restate the 45-good budget table after the Mac measurement of both steps.
- 19.4 / ADR 0294: gate wording for journal identity under D1 B; overflow checked per fixed
  lane rather than per chunk.
