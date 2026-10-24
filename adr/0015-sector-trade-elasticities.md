# 0015. Sector-specific trade elasticities

Status: proposed
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
scales with classes, not sectors. Values are estimated outside the simulation (0019).

## Migration

- P01 becomes a per-sector vector with a class map. Until the estimation runs, use
  Caliendo-Parro values mapped to ICIO sectors for the 10 P2 goods.
- Key the kernel cache by (theta class, weight class).
- Class boundaries are an agent default under 22.2, recorded in their own ADR.

## Tests

H2 per class: PPML on simulated flows recovers each class's theta within its prior band.

## Consequences

More kernels in memory (classes x weight classes); the 19.2 trade line is budgeted for 5 theta
classes.
