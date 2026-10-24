# 0019. Uniqueness check, static nesting gate and direct estimation

Status: proposed
Date: 2026-10-24
Design sections: 3.4, 15.2, 15.3 (V1), 17.1, 21.1 (P2); roadmap P2 gate

## Context

With agglomeration (A5) and trade, and with migration from P4, the spatial equilibrium can be
non-unique; then "within 1% of the GE reference" is ill-posed, since both models can be right
and sit in different equilibria. Reference models with perfect foresight
(Caliendo-Dvorkin-Parro, Kleinman-Liu-Redding) cannot be matched on transition paths by agents
with heuristic expectations. And theta_g, border frictions and mode costs are identified by
gravity equations the model nests, so estimating them by simulation wastes the run budget.

## Decision

1. Before V1, evaluate the uniqueness condition of Allen, Arkolakis and Li (2024), a
   spectral-radius bound on the matrix of elasticities built from theta, agglomeration and
   (from P4) migration dispersion, at the tested parameters. Where it fails, V1 starts both
   models from the same point and reports which equilibrium each reaches instead of a 1% gap.
2. V1 compares steady states with the custom static GE. Comparisons with CDP and KLR are sign
   and rank only, as the roadmap already states.
3. theta_g (0015), border frictions (P05) and mode cost ratios (P04) are estimated by PPML
   structural gravity on 2000 BACI with the route-choice cost structure, in
   python/ceteris/calibrate, outside simulation-based inference.

## Migration

- Add the spectral check to python/ceteris/reference and call it from the V1 test.
- Add the PPML estimation; its outputs enter the pack with provenance "data", not "calibrated".
- Update the gate text in the P2 SPEC.md.

## Tests

The gate itself; the PPML code reproduces published estimates on a published sample before it
runs on the pack.

## Consequences

The P2 gate is well posed, and fewer parameters enter joint inference in P9.
