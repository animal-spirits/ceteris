# 0156. Influence web from derivatives

Status: proposed
Date: 2026-10-28
Design sections: 12.6, 14.7, 20.2; ADRs 0049, 0096, 0117
Phase: P5 (economic levers), P6 (political edges)

## Context

Democracy 4's policy web makes a complex system learnable by drawing which policies move which
statistics and voter groups. Its links are authored and fixed. Ceteris can compute them: ADR 0096
gives forward-mode derivatives of any output with respect to parameters and edits. The leverage
map of 0117 is spatial and per outcome; nothing shows the per-lever view. Survey of other games' mechanics (2026-10-28)

## Options

1. An authored influence graph, as in Democracy 4.
2. A graph generated from model derivatives at the current state.
3. No graph; the explainer only.

## Decision

Option 2.

- Nodes: the seat's levers (tariffs, tax rates, transfers, policy rate), a fixed set of
  intermediate variables (prices, wages, employment, inflation, the exchange rate) and outcome
  groups (pop skill groups, sectors).
- Edges: the derivative of each node with respect to its parents over a one-year horizon, from
  0096 tangents on the current path, recomputed quarterly in the background. Edges below a
  threshold are hidden. The 16-tangent budget is shared with 0117; random tangent directions are
  used above it, with their sampling error shown.
- Moving a lever animates along its edges. Every edge opens the explainer (0049) for that pair.
- The view is labelled local and path-conditional: it holds near the current state and until the
  event path changes.
- Political edges (interest groups, approval) are added in P6.

## Tests

- The web never changes a state hash.
- For a lever with a closed-form effect in the P2 reference model, the edge weight matches it
  within tolerance.

## Consequences

The influence web is generated from the model, so it stays correct as the economy changes, which
an authored graph cannot do. Cost about 6-8 part-time weeks after 0096 lands.
