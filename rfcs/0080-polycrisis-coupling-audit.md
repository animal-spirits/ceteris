# 0080. Polycrisis coupling audit

Status: proposed
Date: 2026-10-24
Design sections: 3, 4.4, 14.7, 21.2
Phase: every phase gate from P5

## Context

With this many modules, the risk is not only missing mechanisms but missing couplings between
them. The Cascade Institute's analysis of global polycrisis (Lawrence et al. 2024, Global
Sustainability) describes three causal pathways by which crises entangle: common stresses
acting on several systems, domino effects where one system's failure triggers another's, and
inter-systemic feedbacks. A world simulator is credible on polycrisis only if those couplings
exist in code and are tested.

## Options

1. Rely on module specs.
2. A coupling register: for every pair of modules, the pathways that connect them, where each
   is implemented, and a test that exercises it; reviewed at each phase gate.

## Decision

Option 2.

- `specs/COUPLINGS.md` lists module pairs (economy, finance, demography, health, politics,
  conflict, climate, water, food, energy, infrastructure, information), the pathway type, the
  state variables that carry it, and the test.
- Each phase gate from P5 checks the register: a listed coupling without a test fails the gate;
  an unlisted coupling found by channel-shutdown analysis is added.
- Propagation code is shared across layers where possible (with 0075), so attribution works
  across them.

## Migration

- P5 onward; the register starts with the couplings already designed.

## Tests

- The register is complete for implemented modules: every cross-module read in the code appears
  in it (checked by a script over declared read and write sets, 19.4).

## Consequences

Compound crises (pandemic plus debt plus food, war plus energy plus inflation) are built
deliberately, and gaps are found by audit rather than by surprise.
