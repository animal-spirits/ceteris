# 0013. Trade routing as a posted-price logit on a two-level kernel

Status: accepted (owner, 2026-10-23, with `specs/trade/SPEC.md`)
Date: 2026-10-23
Design sections: 3.4, 5.4, 8.2, 8.3, 17 (P01, P05, P41, S02, S05), 18 (H1, H2), 19.4

## Context

Section 8.3 moves goods by entropic optimal transport with KL penalties on export supply and
import demand, solved by warm-started unbalanced Sinkhorn each tick over a two-level
(gateway and domestic) structure. Section 5.4 combines per-unit freight, carrying cost, risk,
tariffs and frictions, mapped to an iceberg factor for the GE reference. P2 must pass V1
(convergence to the GE reference within 1%) and H2 (distance elasticity -0.7 to -1.1).

## Options

1. Per-tick unbalanced Sinkhorn with both marginals, as designed.
2. Origin-side merchant logit: each origin routes its supply over active destinations by
   expected log netback, p_d^theta tau_od^-theta, the home route included. This is the same
   entropic family with the demand-side penalty at zero; it is closed form.
3. Closed-form gravity with fixed shares (no price response).

Costs: (a) per-unit plus iceberg; (b) iceberg from freight only; (c) iceberg from freight plus
the time cost of transit (Hummels and Schaur 2013).

## Decision

Option 2 with costs (c).

- Flows have the structural gravity form X_od = a_o b_d tau_od^-theta, and the steady state of
  the stock-flow markets is the spatial equilibrium the GE reference computes on the same
  kernel. No per-tick iteration: Sinkhorn and Anderson acceleration stay in the reference and
  calibration, and a test checks that balanced Sinkhorn at equilibrium prices gives the same
  flows on small worlds.
- International pairs go only through gateways; the kernel is the sum over gateway routes
  (entropic route choice, Allen and Arkolakis 2022). Shares at each gateway do not depend on
  the origin, so aggregating flows and payments at gateways is exact and K_od is never
  materialized.
- tau = 1 + c w_g + lambda_g T per leg. Freight alone gives distance elasticities around
  -0.1 to -0.4 for manufactures; the literature time cost (new register entry P41) is added on
  its own evidence, not fitted to H2, and H2 blocks the P2 gate (owner decision).
- Border friction nu (P05) is an iceberg loss; tariffs are a routing wedge whose revenue is
  paid on FOB value to the importer.

## Consequences

- Trade costs per tick fall to a few sparse products per good, well inside the 30 ms budget.
- Within a tick, buyers cannot re-source away from an origin that runs out; shortages show up
  as inventory draws and price moves, re-routing follows next tick.
- Per-unit cost composition effects (Hummels and Skiba 2004) and endogenous freight rates wait
  for a transport sector (P8); S05's per-unit option is not available until then.
- If H2 still misses, work stops for an owner decision before any parameter moves.
