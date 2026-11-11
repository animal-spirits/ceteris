# 0160. Trade-core seats: corridor planner and commodity trader

Status: proposed; amended by ADR 0239 (seats and scoring to 2.0-alpha)
Date: 2026-10-28
Design sections: 5.3, 5.5, 8.2, 17, 23.4; ADRs 0017, 0091, 0130
Phase: 2.0-alpha or later (commodity trader; corridor planner, with endogenous infrastructure), ADR 0239

## Context

Economy-only 1.0 has no politics, so its first seats should need only the trade core. Transport
Tycoon and OpenTTD show how one built line reroutes cargo across a map, and Victoria 3 makes
construction wait on capacity. Mount & Blade and Offworld Trading Company build play on price
gaps between markets and the trader's own price impact. Section 5.5 lets states propose
infrastructure projects but has no model of delay or cost overrun, though overruns are large and
fat-tailed (Flyvbjerg, Holm and Buhl 2002: rail about 45% on average, roads about 20%). The seats
in 0130 and the research notes (supply-chain manager, macro fund) do not cover either loop.
Survey of other games' mechanics (2026-10-28)

## Options

1. Wait for 2.0 seats.
2. Two trade-core seats, plus a delay and overrun model for 5.5 projects.

## Decision

Option 2. Both seats act only through edits and are scored against their AI baseline under CRN
(23.4).

- Corridor planner: builds one corridor (canal, rail line, port, pipeline, road) as an edit to the
  transport graph. Completion delay and cost overrun are drawn per project from reference-class
  distributions by project type, keyed by the run seed. Score: market-access gains (5.5; Donaldson
  and Hornbeck 2016) against realized cost.
- Delay and overrun join 5.5 for AI-proposed projects too, as a lab mechanism subject to ablation
  (15.7). Distribution parameters are new section 17 entries, fitted to published reference-class
  data (licence to check under 0005).
- Commodity trader: a trading house with capital that buys, stores and ships goods between province
  markets. Below a size threshold it is a price taker; above it its orders enter the storage rule
  of 0091 like any stockholder, so its own price impact shows. Scenarios: rice 2007-08, wheat
  2010, gas 2022.

## Tests

- A corridor with zero overrun and zero delay reproduces a plain infrastructure edit.
- A trader of size zero leaves the branch identical to its parent.

## Consequences

Two playable seats exist before politics, and 5.5 gains a realistic cost side. Cost about 8-12
part-time weeks for both seats; the overrun model about 2-3 weeks including data.
