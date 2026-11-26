# P5a external-balance price spike, 2026-11-26 (RFC 0365 D2)

Status: no-go for the proxy tested; the ADR 0228 form itself is not tested. Nothing built.
Harness: `spikes/p5-bop/p5_bop_spike-2026-11-26.rs` (ignored tests `baseline`, `proxy`; drop into
`crates/ceteris-econ/tests/`). Gate: `small_world`, E3 package (pay on sales, credit line of one
loss cap), `EconParams::frictionless()`, ntb 10 on every pair, 1,500 ticks, world price index at
most 1.07 of base throughout.

## Baseline (E3 package, no closure)

| tick | index | output |
|---|---|---|
| 1 | 1.0356 | 1.000 |
| 100 | 1.0433 | 0.990 |
| 500 | 1.0467 | 0.990 |
| 1000 | 1.0567 | 0.989 |
| 1500 | 1.0692 (max 1.0712) | 0.989 |

The 700-tick go number of RFC 0342 holds (max 1.052 at 700). The 1,500-tick gate is missed by
0.0012: the index drifts up about 0.0003 a tick from tick 600. Country money shows why. The
island's enterprises sit pinned at the credit cap (-2.93e8, flat), so the credit limit does bound
the island. The mainland drifts instead: country 0 pops lose 957.9e6 to 913.5e6 and country 1 gains
748.9e6 to 814.2e6 between ticks 300 and 1,500, decelerating. That is an unpriced external
imbalance between mainland countries, the family-1 mechanism at small size.

## Proxy tested

The model has no currency layer: one numeraire, no financier account, no country exchange rate.
ADR 0228 in full (q_c from the month's net currency flow, e_c, financier ledger account, wages
and prices in local currency) is a kernel and ledger change and was not built for a spike. The
proxy: q_c accumulates the change in country c's money (pops, enterprises, markets), and each
tick every import into c carries an ad valorem wedge `gamma * max(0, -q_c / M)`, via
`set_wedges` (a one-sided Lerner half of a depreciation).

| attempt | normaliser M | gamma | max index | output at 1,500 |
|---|---|---|---|---|
| 1 | country's own initial money | 0.5 | 168.9 | 0 |
| 1 | same | 5 | 450.9 | 0 |
| 2 | mean initial money of the four countries | 1 | 13,546 | 0 |

**No-go.** The island's q reaches -13 M0 at its credit cap (the cap is the intended bound, so q
there is large by construction) and the wedge on its imports collapses the world; the mainland
drift is too small to price before that. Two attempts, as the process rule allows. No gamma
sweep was run: the failure is in the form, not the gain.

## What is and is not established

- The E3 package alone misses the 1,500-tick gate by 0.0012 (1.0712 vs 1.07), a slow mainland
  drift, not a runaway.
- A wedge driven by cumulative money change prices the credit-capped island and collapses the
  world at every tested scale. Not tested: the full ADR 0228 rule (symmetric, with surplus
  countries appreciating, financiers absorbing the gap, local-currency wages), or a q that
  excludes balances sitting at the credit limit.
- No band, tolerance or go number moved.
