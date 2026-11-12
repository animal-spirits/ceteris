# RFC 0351. Capital stock enters production; investment orders at the calibrated GFCF share

Status: accepted
Date: 2026-11-12
Design sections: 7.4, 7.2
Release: 1.0 (P5 D1)
Research: none; follows `reports/economy-assessment-2026-11-12.md` and issue #534

## Summary

The capital pipelines of ADR 0204 are in the tick as system 234, off by default, and nothing
reads the capital they accumulate. This RFC links them to production and fixes what places the
orders until the 7.4 logit exists. Both are behind the existing switch: with capital off no
state is added and goldens do not move (ADR 0354).

## Decisions

### D1. What places investment orders, and does production read the capital?

Options:

- A. Production link with a fixed GFCF share: each enterprise orders, every tick, its good's
  GFCF share of final demand in its country (`inv / omega`) times its value added, a stand-in
  for the 7.4 logit. Capital switched on starts at the steady state that order sustains, and
  A is scaled by `(K / K0)^(1 - s_L)` with K0 the stock at switch-on, so a world whose orders
  stay at the steady state keeps its 2000 output and a shock to orders moves output.
- B. Accumulate only: capital stays unread until the logit is built.

Recommendation: A, because it makes the switch behave, and the logit later replaces only the
order rule.

Decision: A, Antonio, 2026-11-12 (card "Production link, fixed GFCF share").
Row: ADR 0366

## Spec changes

- 7.4: states the stand-in order rule and the production link (ADR 0366).

Follow-ups, tracked in #534: the 7.4 logit over options, the goods draw of each order (its mix
is empty, so no input is paid), cash calls and write-offs (Branch F).
