# 0014. Route-choice trade costs

Status: proposed
Date: 2026-10-24
Design sections: 5.3, 5.4, 5.5, 8.3, 8.4, 17.2 (S05), 19.4

## Context

P1 computes least-cost paths by multi-source Dijkstra from gateways and caches them
(ceteris-geo, paths.rs). A minimum over paths is not differentiable in edge costs, makes the
gateway factorization of the trade kernel an approximation, and reroutes trade discontinuously
when a chokepoint closes.

## Options

(1) Keep least-cost paths. (2) k shortest paths with logit weights. (3) Route choice with
Frechet route shocks (Allen and Arkolakis 2022): tau^(-theta) = (I - A)^(-1), with A =
t^(-theta) edge by edge.

## Decision

Option 3. It sums over all routes in closed form, makes the gateway factorization exact, is
differentiable, and gives closed-form welfare effects for infrastructure (5.5). Option 2 is
heuristic and still jumps when the set of k paths changes. Least-cost paths remain as S05 and
drive the freight-cost map mode.

## Migration

- ceteris-geo: add a route-choice module beside paths.rs. Edge iceberg factor t(k,l) = 1 +
  c(k,l) / p_ref(g) from the existing per-edge cost components (P04), at each good's reference
  price, updated annually.
- Build A per (theta class, weight class). Assert spectral radius below 1 at pack build and on
  every edge change, and fail loudly: the fix is coarser edges or a different class, never
  clipping.
- Solve (I - A)x = b with a sparse iterative method warm-started from the last solve, from each
  gateway (international factor) and within each country (domestic factor). Never form the
  inverse.
- Expected transit time per (o,d) is the route-probability-weighted time, from one extra pair
  of solves with the time-weighted matrix; it feeds the shipment timing wheel.
- Tariffs and non-policy frictions apply on gateway-to-gateway links only.
- Keep paths.rs and its cache for S05 and the map mode.

## Tests

- On a toy graph, route-choice costs converge to least-cost path costs as theta grows.
- Finite-difference check of the derivative of tau with respect to an edge cost.
- Scaling one edge's cost up to closure changes tau continuously.
- Spectral-radius assertion in the pack property tests.

## Consequences

ceteris-geo golden hashes change. The trade kernel (0013) uses route-choice factors directly,
and 5.5's infrastructure planner can score links with the Allen-Arkolakis welfare formula.
