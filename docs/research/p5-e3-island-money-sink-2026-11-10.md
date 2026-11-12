# P5 E3: the island money sink at x10, 2026-11-10

Fixture (`small_world`), frictionless, anchor on (ADRs 0338, 0340), ntb 10 on every border, from
the blessed reference state. Single-run fixture numbers; estimates, not calibration. Trace patch
and harness: `docs/research/spikes/p5-e3/island-trace-2026-11-10.patch` (E3P/E3L/E3R hooks in
`labour`, the schedule and `reprice_anchored`) and `p5_e3_spike_k2.rs` (env K2 sets P03b kappa2).

## Question

Why does the province market of the island p24 (its own country) lose about 4e7 numeraire per
tick at ntb 10, none at ntb 1? Suspects: the eta 1 labour flip, the market paying for output its
households cannot buy, and import prices at the wedge.

## Findings

All three suspects are links of one chain.

1. **Import prices at the wedge make the island's value added negative.** At ntb 10 the delivered
   index of the goods p24 does not make jumps from 1.5-1.9 to 14.4-17.5 (goods 1, 2, 5, 6) in one
   tick. Every p24 enterprise uses them, so the value-added price falls from 0.31-1.05 (tick 1) to
   -0.32 to -0.90 (tick 2). No enterprise hires, frictionless labour finds no root and the wage
   halves per tick (51.8, 25.9, 12.9, 1.15, 0.57). This is the economics of a 10x barrier on an
   import-dependent island and is not the defect.
2. **The anchored gap is blind to the origin stock, so idle ticks push prices up.** With Q = 0 and
   D small but positive, the midpoint gap reads l = +2 (ADR 0340), so the step is +0.06 to +0.13
   while the origin stock S sits at 2.9e7 to 5.6e7 units against D of 1.7e4 (goods 0, 3, 4).
   P03b (kappa2) is 0 by default and storage is off frictionless, so no term reads S. Prices
   ratchet up on idle ticks until some enterprise's value added turns positive; it hires the whole
   province for a tick (eta 1), produces, and the price falls; then idle again. That is the
   0-to-0.8 output flip.
3. **The market pays for output nobody buys, and the signed dividend charges it to the pops.**
   E2 pays `p Q` to the producer for every unit (SALE). At ntb 10 the island's buyers are gone:
   pops hold negative money and buy nothing (household purchases 0 from tick 2), and abroad the
   10x loss prices them out. Unsold value per producing tick is 1e7 to 6e7 and the origin stock is
   worth 1.05e8 (t1) to 7.1e8 (t40). The market ends each producing tick at -2e7 to -9e7 and pays
   that signed cash to the country pool (ADR 0112), so the p24 pops reach -1.2e9 by t40.
4. **Unbudgeted import orders carry the money out.** The SALE money goes to p24 enterprises, which
   spend 3e7 to 1.5e8 a tick on intermediate imports at the 10x prices (stock-target orders, no
   budget) against 1e7 to 2e7 of exports. The rest of the world gains what the island loses, so
   net money stays fixed while positive balances elsewhere triple (the spike note's finding).

So the leak is: stock-blind price rise on idle ticks, then output the market must buy, then a
signed market loss charged to the households, financing import bills that leave the island.

## Probe: the P03b stock term under the anchor (K2)

| kappa2 | index range | Y min | result at t700 |
|---|---|---|---|
| 0 | 1.00-1.10 | 0 | dies t500-600 (baseline) |
| 0.05 | 0.93-1.35 | 0.90 | survives; D/Q 1.87 and rising |
| 0.1 | 1.00-1.68 | 0 | dies by t300 |
| 0.2 | 0.96-2.42 | 0 | dies by t300 |

At 0.05 the island stops producing to stock (Q 0 from about t50, S sells down from 2.8e8) and its
pops lose about 2.2e9 by t300, a fifth of the baseline's 1.2e10. The term is needed, but it is not
enough on its own, and it is global (every coverage good everywhere), so it moves the stability
region of the trade SPEC. Larger gains clip on cells where D is near 0.

## Residual not traced

At ntb 3, and at ntb 10 with kappa2 0.05, p24 pops keep losing (to -2.2e9 by t300) while the p24
market's pre-dividend cash is near 0 (-1e6 to -3e6). The pool is per country, so another province
of the island's country, or the enterprise loss cap with the write-off (which charges only
positive balances), may carry it. Open; it is what D2 of the RFC has to bound.

## Rejected (spike of the same day)

- A, level term kappa_l lbar: holds output only by raising the index (1.28-2.81), dies by t700.
- B, budget-limited intermediate orders: barely binds; D/Q rises the same way.

## Outcome

RFC 0341 (draft): a stock-aware anchored gap and a bound on what a province market can charge its
households.
