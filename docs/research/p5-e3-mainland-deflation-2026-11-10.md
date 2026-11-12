# P5 E3: why the mainland dies at x10 under the credit limit (2026-11-10)

Status: root cause found; fix spiked, go. Proposal: [RFC 0342](../../rfcs/0342-anchor-index-over-traded-cells.md).
Follows: [credit-limit follow-up](p5-e3-d1d2-spike-2026-11-10.md#follow-up-credit-limit-on-traded-input-orders-same-day).

Setup: `small_world` fixture, frictionless, anchor on (ADR 0338, gap of ADR 0340), ntb 10,
pay on sales (RFC 0341 D2) and a credit line of CL loss caps on traded input orders.
Harness and spike code: `docs/research/spikes/p5-e3/anchor-index-spike-2026-11-10.patch`
(the credit-limit patch plus env `TG` per-good trace, `G9` per-province trace, `XI`, `ZQ`, `CH`).
Run: `CL=1 PS=1 TG=1 cargo test --release -p ceteris-econ --test p5_e3_d1d2 -- --ignored --nocapture`.

## Root cause

The rise of the index is not a mainland price rise. It is the island's local prices, and the
money anchor turns it into deflation everywhere else.

1. With pay on sales and the credit limit, 5 of the island's 6 enterprises sit at their limit
   from t30 and its output is 0 (as intended: the leak is closed).
2. The island's local markets then read D > 0, Q = 0, no stock: the gap of ADR 0340 is +2 every
   tick and the price climbs without bound. Good 9 at p24 is 2.9x its base at t20, 15x at t40,
   90x at t60, 316x at t80. Nobody buys at those prices (island pops hold 0).
3. The anchor's index I is the fixed-weight arithmetic (Laspeyres) world index over buyer prices
   (`world_price_index`, weights `price_index_weights`). One cell with a small weight at 300x
   lifts I from 1.04 at t40 to 1.37 at t90. The money stock M is unchanged (M/M0 = 1.0000).
4. The nominal term `kappa_m (ln(I / I0) - ln(M / M0))` (ADR 0338) then subtracts up to 0.06 a
   tick from every market's gap. Index-weighted buyer prices of the mainland's goods fall to
   0.88 (t60), 0.48 (t80), 0.25 (t90), 0.03 (t130) of base, every good alike, while the island's
   good-9 index climbs to 5.4x.
5. At falling prices and a fixed money stock, real demand rises: world D/Q goes 1.05 (t40),
   1.10 (t60), 1.36 (t80), 2.6 (t100), 180 (t130). Origin stocks run out (S/D from 1.6 at t40 to
   0 by t80 on every traded good), input orders go unfilled, and output falls to 0 by t140
   through the Leontief inputs (the chain of the E3 diagnosis).

Without pay on sales (CL 1, PS 0) the island keeps flipping between producing and idling, its
local cells keep trading, its prices do not ratchet and I stays at 1.04; that run dies only at
t500 from the money leak of RFC 0341.

## Tests of the cause

| run (ntb 10, PS on) | index range | Y min | outcome |
|---|---|---|---|
| CL 1, as before | 1.00-1.37 | 0 | dies t110-140 |
| CL 1, anchor index without p24 (XI) | 1.00-1.07 | 0.989 | lives 700 ticks |
| CL 3, anchor index without p24 (XI) | full index to 5e8, mainland flat | 0.924 | lives 700 ticks |
| CL 1, no price rise at Q = 0 and S = 0 (ZQ) | 1.00-1.08 | 0 | dies t100-200; island overdraft to -1.8e10 (wages keep flowing) |
| CL 3, ZQ | 1.00-1.04 | 0.975 | lives 700 ticks |

Dropping the island from the anchor's index alone removes the failure, which pins the cause on
the index. ZQ is not a fix: it keeps the island producing at a loss and the overdraft grows.

## Fix spiked: chained index over cells that trade (CH)

The anchor reads a chained index: `ln I_t = ln I_{t-1} + sum_c s_c ln(q_c,t / q_c,t-1)` over the
cells c bought at both ticks (traded buyer prices finite; local cells with sales > 0), with
`s_c` the base weight times last tick's buyer price, normalised over those cells. A cell that
sells nothing drops out; a price nobody pays does not move I. Reported index, V1 and I0 unchanged.

| run | index range | Y min | positive balances | outcome |
|---|---|---|---|---|
| CH, CL 1, PS on, ntb 10 | 1.00-1.07 | 0.989 | 2.7e9 flat | lives 1500 ticks |
| CH, CL 3, PS on, ntb 10 | 1.00-1.05 | 0.991 | flat | lives 700 ticks |
| CH, CL 1, PS on, ntb 1 | 1.000 | 1.000 | 2.4e9 | steady state exact |
| CH, CL 1, PS off, ntb 10 | 1.00-1.13 | 0 | island pops -2e10 | dies t700 |
| CH, no CL, PS off, ntb 10 | 1.00-1.13 | 0 | island pops -2e10 | dies t700 |

**Go** for the three together: chained anchor index, pay on sales, credit line. Each alone fails:
the index alone leaves the money leak; pay on sales and the credit line without it set off the
ratchet. RFC 0341's go number (x10 survives 700 ticks, index within 1.10) passes; island
enterprise debt stays at -2.9e8 (CL 1), -8.8e8 (CL 3).

## Open

- World D/Q stays at 1.04-1.12 throughout: a uniform excess demand the anchored rule does not
  price (the earlier E3 candidate). Not fatal in 1500 ticks; stocks hold. Record, do not fix here.
- The reported world index still drifts to 1.066 by t1500 from the island's local cells.
- Not run: the ADR 0338 data-state and nominal-shock checks under CH (the spike's chained state
  is a process-wide static, so it cannot run beside other tests). The build carries the chain
  state in the snapshot and reruns them.
- kappa_s stays 0: RFC 0341 D1 needs no stock term with this package.
