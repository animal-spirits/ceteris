# RFC 0286. Storage-on frictionless V1 misses: cause recorded, fix deferred to P5

Status: deferred
Date: 2026-11-01
Design sections: 8.2, 17 (P151); trade SPEC Competitive storage
Research: storage-on frictionless V1 diagnosis, 2026-11-01 (project files,
`storage-misses/storage-on-frictionless-v1-2026-11-01.md`)

## Summary

ADR 0280 reports two storage-on V1 misses on the B8 pack, both frictionless and not diagnosed
there: seed 20 (a good-1 cycle in Switzerland, 0.254 at 6,000 ticks) and the data state (France
province 1094, 6.99 at 6,000). Storage is off by default and the default mode passes every start.
The diagnosis found one cause for both. At a stored good's origin with no output (frictionless
eta 1 takes output to 0 in one tick), ADR 0280's gap divides by Q_floor, so each step hits the
P151 clip of 0.3. At theta 16.9 a clipped step moves orders between near-identical origins by
e^5, and the stock overshoots (28 times its target in Switzerland), so that releasing it sets off
the next collapse. The Swiss cycle coexists with a stable fixed point: the tangent map's spectral
radius is 0.642 for prices and 0.994 with stocks and expectations, and shocks of 1e-4 to 0.1 to
a converged state decay. France 1094 runs the same cycle on its good-1 exports; its frictionless
wage is the unique root from tick 9.

Six rounds of fixes did not produce a rule that holds. Each one slowed a shortage price step to
remove the cycle, and each moved the failure somewhere else:

- Bounding the stored gap (over max(Q, D + b), over max(Q, D), or on the shortage side only), or
  clipping stored goods at 0.1 or 0.15, removes the Swiss cycle. But the frictionless data state
  then overflows the ledger at ticks 123 to 136, through a local-goods trap. Stored inputs stay
  rationed for about 18 ticks; local good 9's price runs up and sets the frictionless wage alone;
  goods 7 and 8, which buy good 9, stop hiring; good 9's Leontief chain needs good 7; and all
  three prices and the wage then climb at the 0.1 clip with relative prices frozen. A break test
  (holding goods 7 and 8 at base labour for 13 ticks) confirmed the loop in France 1102, and
  France 1106 and Switzerland 526 enter it too.
- Bounding only the excess-supply side keeps the cycle and starts new ones (Slovenia, Senegal,
  Portugal).
- Adding a bounded gap for local goods converges all eight storage-on V1 starts, but it breaks
  storage off: the frictionless data state overflows before tick 100, and the default data state
  goes from 9.3e-3 to 0.70. On the fixture it reopens ADR 0266's frictionless labour spiral and the
  price-sweep overflow. Capping the frictionless wage root by local inputs overflows alone and
  breaks the natural-rate and fixed-point tests.

A rule that holds needs the price step at Q = 0 redesigned for every good. That belongs with the
storage-on gate (ADR 0091), not with pre-P3.

## Decisions

### D1. How do the storage-on frictionless misses close?

Options:

- A. Record and defer: this RFC records the cause, the screen and the tests any fix must pass;
  storage stays off; the storage-on gate of ADR 0091 carries the fix to P5.
- B. Redesign the shortage price step now: open-ended work on the step at Q = 0 for all goods,
  judged on all 16 V1 starts and the fixture tests.
- C. Ship the bounded stored gap and the bounded local gap for storage-on runs only: the rule then
  forks by mode, and the fixture spiral with storage on is untested.

Recommendation: A, because storage is off by default and the default mode passes, and every fix
so far moved the failure rather than removing it.

Decision: A (deferred), Antonio, 2026-11-01.
Row: none (deferred).

Earlier picks in the same diagnosis, superseded by D1: the bounded stored gap (Antonio,
2026-11-01, withdrawn after its prototype overflowed the frictionless data state), and fixing the
local-goods trap first (Antonio, 2026-11-01, done as the fix screen above).

## What a fix must pass at the gate

- All eight storage-on V1 starts (default and frictionless; seeds 20, 21, 22 and the data state)
  at 6,000 ticks, and the eight storage-off starts no worse than the B8e baseline.
- The fixture's `frictionless_labour_spiral_is_bounded`, `frictionless_price_sweep_stays_bounded`,
  `frictionless_unemployment_is_the_natural_rate` and fixed-point tests.
- ADR 0280's storage tests. A rule with a kink at the steady state needs one-sided differences in
  `partial_adjustment_keeps_the_storage_loop`: central differences across `max(Q, D + b)` missed
  the map by 1e-5 at H 1e-6.

## Spec changes

None.
