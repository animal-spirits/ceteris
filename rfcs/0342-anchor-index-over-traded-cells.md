# RFC 0342. Anchor the price level on cells that trade (P5 Branch E3)

Status: accepted, built behind `PriceAnchor::e3` (off by default)
Date: 2026-11-10
Design sections: 8.2, 12.2, 17 (P177); trade SPEC Money-anchored rule; econ SPEC E2, E3, T4
Release: 1.0, P5a
Research: [P5 E3: why the mainland dies at x10 under the credit limit, 2026-11-10](../docs/research/p5-e3-mainland-deflation-2026-11-10.md)

## Summary

With pay on sales (RFC 0341 D2) and the credit limit Antonio chose on 2026-11-10, the island's
money leak at x10 closes, but the three mainland countries die by t140. The cause is the index
the money anchor (ADR 0338) reads. The island's enterprises sit at their credit limit and make
nothing; its local markets read D > 0, Q = 0 and their prices climb without bound (good 9 at
316x base by t80) though nobody buys. The anchor's index is a fixed-weight arithmetic mean of
buyer prices, so that one cell lifts I from 1.04 to 1.37 with money unchanged, and the nominal
term pushes every other price down to 3% of base by t130. Real demand then outruns output, stocks
run out and the Leontief inputs stop production. Dropping the island from the anchor's index
alone removes the failure. A chained index over cells that trade, with pay on sales and a credit
line of 1 loss cap, keeps x10 alive for 1500 ticks (Y min 0.989, index within 1.07, balances
flat) and leaves ntb 1 exact. Each of the three alone fails.

## Decisions

### D1. Which price index does the money anchor read?

Options:

- A. A chained index over cells that trade: `ln I_t = ln I_{t-1} + sum_c s_c ln(q_c,t /
  q_c,t-1)` over cells bought at both ticks (traded buyer prices finite, local cells with
  sales > 0), `s_c` the base weight times last tick's buyer price, normalised over those cells.
  The chain state goes in the snapshot. The reported index, V1 and I0 stay as they are.
  Spike: x10 lives 1500 ticks with D2 and D3.
- B. Keep the fixed-weight index and stop prices rising at cells with Q = 0 and no stock:
  tested, CL 1 dies t100-200 and the island's overdraft grows to -1.8e10. No-go.
- C. A fixed-weight geometric index: damps the jump but a cell that rises without bound still
  moves I without bound. Not spiked.

Recommendation: A, because a price nobody pays then cannot move the price level, and it is the
only option the spike shows working.

Decision: A, Antonio, 2026-11-10.
Row: ADR 0343

### D2. How large is the credit line on traded input orders?

Options:

- A. 1 loss cap (ADR 0112): x10 Y min 0.989, island enterprise debt held at -2.9e8.
- B. 3 loss caps: Y min 0.991, debt held at -8.8e8.
- C. 0 (cash only): deadlocks without a shock, since inputs must be paid before the sales that
  fund them. No-go.

Recommendation: A, because it survives as well as B with a third of the debt.

Decision: A, Antonio, 2026-11-10. The credit line is the loss cap T_e = M S_e itself (P148), so orders stop where the write-off starts and no new parameter is added.
Row: ADR 0344

### D3. How is the E3 fix built?

Options:

- A. One E3 change with all three parts (pay on sales, credit line, chained anchor index), in
  anchor mode only; RFC 0341 D1's stock term keeps kappa_s = 0 (the spike needs none). The
  default mode and its goldens do not move.
- B. Build the chained index first and the other two after: the index alone dies at t700 from
  the money leak, so the x10 gate stays red in between.

Recommendation: A, because the parts only pass together.

Decision: A, Antonio, 2026-11-10.
Row: ADR 0345

## Spec changes

Trade SPEC Money-anchored rule (E3 package: index of the nominal term, chain state); econ SPEC (pay on sales, credit line). No P code: the credit line is P148.

## Before build

- Rerun the ADR 0338 data-state and nominal-shock checks with the chained index.
- Record, not fix: world D/Q stays at 1.04-1.12 under the package, a uniform excess demand the
  anchored rule does not price.
