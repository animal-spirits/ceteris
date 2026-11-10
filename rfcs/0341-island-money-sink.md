# RFC 0341. Stop the island money sink at x10 (P5 Branch E3)

Status: accepted, spike no-go 2026-11-10; not built
Date: 2026-11-10
Design sections: 8.2, 12.2, 17 (P03b, P178); trade SPEC Money-anchored rule; econ SPEC E2, E4
Release: 1.0, P5a
Research: [P5 E3: island money sink at x10, 2026-11-10](../docs/research/p5-e3-island-money-sink-2026-11-10.md)

## Summary

ADR 0341 put the x10 criterion on task E3. On the fixture at ntb 10 the island p24 leaks about
4e7 numeraire a tick to the rest of the world, which triples positive balances elsewhere and
drives the excess demand that kills output by t500-600. The chain: the 10x wedge lifts the
island's imported input prices 9-fold and its value added goes negative, so it idles; the
anchored gap (ADRs 0338, 0340) does not read the origin stock, so on idle ticks (Q = 0, D small)
it reads +2 and prices rise against months of unsold stock; once value added turns positive the
province produces for a tick (the eta 1 flip); E2 pays the producer `p Q` for output nobody buys;
the market's loss goes to the pops through the signed dividend (ADR 0112), unbounded; and the
SALE money funds unbudgeted import orders that leave the island. Two links are defects: the
stock-blind gap and an unbounded market loss charged to households. The wedge pricing and the
flip are consequences. The level term and budget-limited orders of the E3 spike are rejected.

## Decisions

### D1. How does the anchored price rule read the origin stock?

Options:

- A. A stock term in the anchored gap with its own gain: `gap -= kappa_s (S - s_g D) / max(s_g D,
  floor_g)` on every traded cell with coverage s_g > 0, anchor mode only, gain set by a spike
  inside the trade SPEC stability region. The default mode keeps kappa2 = 0.
- B. Turn on P03b globally (kappa2 0.05, the probe): the island stops producing to stock and the
  fixture survives 700 ticks (Y min 0.90), but the index drifts to 1.35 and D/Q to 1.87, and the
  default mode's stability results move.
- C. No stock term: idle ticks keep ratcheting prices against unsold stock.

Recommendation: A, because the probe shows the term is needed, and scoping it to the anchor keeps
the default mode and its goldens untouched.

Decision: A, Antonio, 2026-11-10 (with D2 A; the spike sets kappa_s).
Row: none yet, taken in the spec PR once the spike passes

### D2. What bounds the loss a province market charges its households?

Options:

- A. Pay producers on sales: the origin market pays `p` per unit it sells (this tick's output
  first), and unsold output stays the producer's stock. A loss then lands on the enterprise,
  under its loss cap and write-off (ADR 0112), which charges only positive balances, so it is
  bounded and the producer sees it. Changes E2's sale rule and the market account; needs a
  one-day spike with a go/no-go number (x10 fixture survives 700 ticks, index within 1.10).
- B. The market keeps its losses as retained losses instead of a negative dividend: tested
  (E3MKT), the market carries -1.2e10 and the same collapse follows. No-go.
- C. Keep the signed dividend and rely on D1: the probe leaves the p24 pops at -2.2e9 by t300.

Recommendation: A, because it is the only option that stops paying for unsold output, which is
where the money leaves.

Decision: A, Antonio, 2026-11-10 (with D1 A; go if x10 survives 700 ticks with the index within 1.10).
Row: none yet, taken in the spec PR once the spike passes

## Rejected in the E3 spike (2026-11-10)

- Level term `kappa_l lbar` in the anchored rule (amends ADR 0338): holds output only by raising
  the index to 1.28-2.81 and still dies by t700.
- Budget-limited intermediate orders (cash plus `kappa_o` of revenue at capacity): barely binds;
  D/Q rises the same way.

## Spec changes

None until D1 and D2 are decided and the spike passes. Then: trade SPEC Money-anchored rule (D1),
econ SPEC E2 and E4 (D2), Section 17 (a new P code for kappa_s), rows for each accepted item.

## Spike result (2026-11-10): no-go

[Study](../docs/research/p5-e3-d1d2-spike-2026-11-10.md). No kappa_s in 0 to 0.2, with or without
pay on sales, keeps x10 alive for 700 ticks with the index within 1.10. Pay on sales moves the
island's loss from its pops to its enterprises, but the loss cap and write-off do not bound an
enterprise's overdraft, so the same money leaves and the world dies at the same ticks. The ntb 3
residual is the same mechanism: an unbounded external deficit financed by whichever account may
go negative. Nothing here is built; the next step is a new decision on bounding that deficit.
