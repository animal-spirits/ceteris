# 0176. Endogenous neutral rate in the policy rule

Status: proposed
Date: 2026-10-28
Design sections: 9.3, 15.4; ADR 0065
Phase: P5

## Context

The central bank's Taylor rule (9.3) uses a fixed intercept (P35). Estimates of the neutral
real rate fell by about two percentage points in the US, the euro area and the UK between 2000
and 2015 (Holston, Laubach and Williams 2017). With a fixed intercept the simulated policy rates
in 2009-2021 sit too high and the effective lower bound binds too rarely, which misses a 15.4
target. ADR 0065 already lets agents read published statistics rather than true state.
Gap audit (2026-10-28): https://claude.ai/artifact/HRDQVbjJz8MrcDuPphPyYs

## Options

1. Status quo: fixed intercept.
2. An exogenous tape of published r* estimates.
3. The central bank runs a Holston-Laubach-Williams filter on the statistics it observes and
   uses the estimate as its intercept.

## Decision

Option 3.

- Once a year each central bank updates its estimate of r* with a one-sided HLW Kalman filter
  on published output, inflation and the real policy rate (0065). The filter's variances are
  fixed at the published HLW values, so no free parameter enters unless smoothing of the
  estimate needs one (at most 1 register entry).
- The Taylor rule uses the estimate as its intercept. P35 becomes the 2000 initial value.
- Because r* is estimated from simulated data, a lower growth path lowers the intercept
  endogenously.

## Migration

- P5: a filter state per central bank, updated in the annual statistics step.
- Pack: initial r* for 2000 from published HLW estimates (ADR 0182).

## Tests

- With the filter frozen at the initial value, golden hashes are unchanged.
- Fed the historical US, euro-area and UK series, the filter reproduces published HLW
  estimates within their bands.
- In the holdout, the share of quarters at the lower bound 2009-2015 rises toward history.

## Consequences

Low-rate dynamics after 2008 become reachable without a tape. Register: 0-1 entries.
