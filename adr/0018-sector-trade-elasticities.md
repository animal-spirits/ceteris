# 0018. Sector-specific trade elasticities

Status: accepted (owner, 2026-10-24, PR 1); implemented on p2-econ (MIGRATION Task 5)
Date: 2026-10-24
Design sections: 8.3, 17.1 (P01), 19.4, 18 (H2)

## Context

One theta for every sector is below current practice. Sector estimates differ by an order of
magnitude (Caliendo and Parro 2015), and that dispersion drives the welfare and reallocation
effects of tariff counterfactuals, the product's core experiments.

## Options

(1) A single theta. (2) Theta per sector, each with its own kernel. (3) Theta per sector,
grouped into 3 to 5 classes that share kernels.

## Decision

Option 3. Kernels and route-choice matrices are built per (theta class, weight class), so cost
scales with classes, not sectors. Values are estimated outside the simulation (0022).

## Migration

- P01 becomes a per-sector vector with a class map. Until the estimation runs, use
  Caliendo-Parro values mapped to ICIO sectors for the 10 P2 goods.
- Key the kernel cache by (theta class, weight class).
- Class boundaries are an agent default under 22.2, recorded in their own ADR.

## Tests

H2 per class: PPML on simulated flows recovers each class's theta within its prior band.

## Mapping to P2 as built

- P01 becomes a vector over the 7 traded goods (`TradeParams.theta`, one f64 today). Starting
  values are Caliendo and Parro (2015) estimates mapped to the P2 goods with 2000 BACI trade
  weights; class boundaries are an agent default recorded in their own ADR (0023).
- P2 already builds one kernel per traded good, and each good has its own weight w_g, so weight
  classes are goods and theta classes add no kernels while goods number 7. The cache key becomes
  (theta class, weight class) all the same, so classes can merge later.
- Several Caliendo-Parro values lie outside the current P01 prior (4 to 8); the prior widens with
  the register update.
- H2 per class extends `python/tests/test_p2_gate.py`; the owner's distance-elasticity gate
  (-1.1 to -0.7) stays.

## Consequences

More kernels in memory (classes x weight classes); the 19.2 trade line is budgeted for 5 theta
classes.
