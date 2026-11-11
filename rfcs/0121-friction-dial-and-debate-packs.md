# 0121. Friction dial and debate packs

Status: proposed
Date: 2026-10-28
Design sections: 2.5 (principle 4), 3.4, 14.2, 23.3
Phase: P3 (friction dial, trade debates); 3.x (long-run thesis packs)

## Context

Equilibrium nesting (3.4) means every friction has a limit where the model collapses to a known
equilibrium. That is the best teaching property the model has, and it is visible only in tests.
Famous policy disputes (the China shock, NAFTA, Brexit, the 2018 tariffs) and long-run theses
(the Great Divergence, institutions versus geography) are natural experiments users want to
run. Brainstorm (ideas 9, 10 and 50).

## Options

1. Status quo: users build these as ordinary branches.
2. A friction dial in the app, and curated experiment packs for debates, each with predictions
   stated before the run.

## Decision

Option 2.

- Friction dial: one control per friction (iceberg trade costs, Armington elasticity, labour
  switching costs, price stickiness, inventory gain), running from the calibrated value to its
  nesting limit. Presets name the limits: Ricardo, Heckscher-Ohlin, Krugman, Eaton-Kortum,
  frictionless Walras. Each setting is a parameter-edit branch under CRN; the map animates
  between settings.
- Debate packs: a scenario, the branches each side would run, their stated predictions (with
  sources), and the preregistered metrics of ADR 0119. Packs settle nothing on their own; they
  show which assumptions decide the answer.
- Trade debates ship from P3. Long-run thesis packs (1500 or 1800 to 2000) wait for pre-1995
  packs (ADR 0131).

## Migration

- P3: dial and trade debate packs as scenario files. 3.x: thesis packs.

## Tests

- Each dial preset at its limit reproduces the V0 nesting result for that equilibrium.
- A debate pack runs end to end from its manifest.

## Consequences

The nesting property becomes a user-facing feature. Debate packs need care in stating each side
fairly, with citations.
