# 0235. Budget lines from measured per-good cost

Status: proposed
Date: 2026-10-29
Design sections: 19.2; ADRs 0016, 0092, 0097, 0234

## Context

DESIGN 19.2 budgets 80 to 160 ms per monthly tick for 45 goods. P2 measures 34 ms on the full
pack with 10 goods, 7 traded (econ SPEC, Task 19), and the trade line already uses its 30 ms there.
Dispatch costs about 5.9 ms of CPU per traded good (41.3 ms for 7 in series) and runs in parallel
across goods, so trade cost grows with the traded-good count. Extrapolated, not measured: today's
systems at 45 goods take about 70 to 120 ms, and all 1.0 systems about 90 to 150 ms.
Assessment: https://claude.ai/artifact/HnTopU8Pz7RiuPLTNGWR1E

## Options

1. Keep 19.2 until the 45-goods pack exists.
2. Raise the ceiling to 250 ms (4 to 6 ticks per second).
3. Keep the 160 ms ceiling; restate the per-system lines from measured per-good cost now, and
   measure scaling on a synthetic 45-goods pack.

## Decision

Option 3 (owner, 2026-10-29).

- **Ceiling.** A whole tick stays at most 160 ms on 19.2 hardware (6 ticks per second, 0.5
  simulated years per second).
- **Per-good lines.** Each goods-indexed system states its budget as fixed cost plus cost per good
  (per traded good for trade), fitted to measurements, and the 19.2 table shows both the formula
  and its value at 45 goods.
- **Synthetic pack.** A generated pack with 45 goods (the 10 real goods' parameters resampled
  across theta classes, IO coefficients scaled to keep value added) on the full province set,
  used only for benchmarks. Its tick time is reported next to the real pack's.
- **Re-budget order.** If the synthetic 45-goods tick exceeds the ceiling, the levers are, in
  order: gateway count and crossings (0092), the dispatch speed plan (0097), and only then the
  ceiling, by a new ADR.

## Tests

- The synthetic pack runs 100 years without an invariant violation.
- Benchmarks at 10, 20 and 45 synthetic goods give the per-good slope; each 19.2 line is checked
  against its formula with the 20% regression gate.

## Consequences

Budget lines track how cost actually scales before the real 45-goods data exists. The synthetic
pack is a benchmark fixture, never a calibration input.

## Migration

- P3: the synthetic pack generator and the scaling benchmark; 19.2 restated from the fit.
