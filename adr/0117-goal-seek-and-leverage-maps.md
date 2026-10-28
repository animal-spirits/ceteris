# 0117. Goal-seek interventions and leverage maps

Status: proposed
Date: 2026-10-28
Design sections: 14, 15.2, 20.1; ADR 0096
Phase: P9 (needs the calibration build of ADR 0096)

## Context

The workbench answers "what if X?". Users often want the inverse: "what would it take to avoid
Y?". ADR 0096 already makes the P2 kernels generic over a dual-number type for calibration, so
the same build gives derivatives of outcomes with respect to edits. Brainstorm:
https://claude.ai/artifact/N9zi5kznSaqvjg9ztw4WMT (ideas 2 and 3).

## Options

1. Status quo: users search the intervention space by hand.
2. Goal-seek: an optimizer over a declared lever set, using forward-mode tangents on the smooth
part and a discrete search over event paths.
3. Option 2 plus a leverage map mode showing the sensitivity of one outcome to local shocks.

## Decision

Option 3.

- Goal-seek: the user states a target (an outcome predicate or a scalar to reach), a lever set
  (edit types from 14.2 with bounds) and a cost on edits (count, size, earliness). The search
  minimizes edit cost subject to the target, in the ensemble mean. Gradients come from ADR 0096
  tangents, conditional on the event path; when the event path changes, the search restarts
  from the new path. Every candidate is confirmed by a full ensemble before it is reported.
- Leverage map: for a chosen outcome and date window, the derivative with respect to a unit
  shock in each province's productivity, trade cost or inventory. With more inputs than the
  16-tangent budget, use forward gradients with random tangent directions (Baydin et al. 2022),
  averaged over draws, and show their sampling error.
- Both run headless through the CLI; the map mode reads their output.

## Migration

- P9: on the ADR 0096 calibration build; leverage map as a map mode in the app.

## Tests

- On the P2 golden scenario, goal-seek with one lever recovers a planted tariff that produces a
  target trade flow, within the solver tolerance.
- Leverage-map entries match central finite differences under CRN to 1e-4 relative for sampled
  provinces.

## Consequences

Counterfactual questions can be posed as targets, and the map can show where the world was
fragile, not only what it looked like. Gradients stay conditional on discrete events, which the
UI states.
