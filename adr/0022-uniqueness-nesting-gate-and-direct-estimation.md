# 0022. Uniqueness check, static nesting gate and direct estimation

Status: accepted (owner, 2026-10-24, PR 1); implemented on p2-econ (MIGRATION Task 9; the estimation part lands with PLAN Task 18)
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
3. theta_g (0018), border frictions (P05) and mode cost ratios (P04) are estimated by PPML
   structural gravity on 2000 BACI with the route-choice cost structure, in
   python/ceteris/calibrate, outside simulation-based inference.

## Migration

- Add the spectral check to python/ceteris/reference and call it from the V1 test.
- Add the PPML estimation; its outputs enter the pack with provenance "data", not "calibrated".
- Update the gate text in the P2 SPEC.md.

## Tests

The gate itself; the PPML code reproduces published estimates on a published sample before it
runs on the pack.

## Mapping to P2 as built

- The uniqueness check is `python/ceteris/reference/uniqueness.py` (MIGRATION Task 9), Theorem 1
  of the paper (transcribed in `specs/econ/SPEC.md`, Equilibrium nesting). No example in the paper
  is a multi-sector model with an input-output table, so the check uses the special case of its
  Online Appendix A.3 (Eaton-Kortum with intermediates and quasi-symmetric costs) good by good, at
  theta_g and the good's smallest value-added share, and takes the largest radius; the SPEC lists
  what this leaves out. P2 has no agglomeration (A5 off) and no migration; the function raises for
  either until a phase that adds one derives its matrix. At the P2 defaults the bound is 0.932, so
  V1 runs in tolerance mode. It runs before V1 in `python/tests/test_p2_gate.py` (PLAN Task 20);
  its tests are in `python/tests/test_reference.py`.
- V1 already compares steady states with the custom static GE (the reference model).
- PPML on 2000 BACI runs on least-cost (S05) costs between countries, not route-choice costs:
  route choice is deferred past P2 (ADR 0017, Deferral). It reuses
  `python/ceteris/reference/ppml.py` (Task 12, which recovers a known elasticity on synthetic
  data) and first reproduces the published estimates of Yotov, Piermartini, Monteiro and Larch
  (2016) on their sample. Estimation is the first step of PLAN Task 18, since it needs the BACI
  cache and the pack's costs (Task 17); outputs carry provenance "data".
- The P2 gate text in `specs/econ/SPEC.md` is rewritten (MIGRATION Task 9).

## Consequences

The P2 gate is well posed, and fewer parameters enter joint inference in P9.
