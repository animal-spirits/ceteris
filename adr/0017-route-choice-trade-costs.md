# 0017. Route-choice trade costs

Status: accepted (owner, 2026-10-24); deferred past P2 (owner, 2026-10-24, MIGRATION Task 4)
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

## Mapping to P2 as built

The decision holds with one deviation (owner, 2026-10-24): **a separate route-choice dispersion
parameter theta_r**, not the trade elasticity. With A = t^(-theta) the spectral radius on the P1
graph (40,534 nodes, 134,810 edges, median edge 122 km; freight plus transit-time cost only, no
handling fees) is 6.5 for coal, 7.4 for chemicals and 27 for machinery at theta 5, and 6.3, 7.2
and 21 at theta 8. Edges are short enough that t is close to 1, so no coarsening that keeps
province resolution brings it below 1.

- Route costs aggregate as tau_od = (sum over routes of prod t_kl^(-theta_r))^(-1/theta_r), solved
  as (I - A_r) x = b with A_r = t^(-theta_r); the kernel is then K = tau^(-theta_g). As theta_r
  grows this tends to the least-cost path (the first test).
- theta_r is a new register entry with a literature prior. The spectral-radius assertion stays,
  per (theta class, weight class). The task measures the smallest theta_r that passes for each
  class; if that lies outside the prior, work stops and the numbers go to the owner.
- Domestic factor: one solve per province on its country's subgraph (4,796 sources); international
  factor: one per gateway (about 600). Built at load and on edge changes, priced at reference prices.
- Transit hours per pair are route-probability weighted; crossing lags keep the P2 rule
  (round(hours / 730), capped at 3).
- `ceteris-geo` paths.rs and its cache stay for S05 and the freight-cost map mode.

## Deferral (2026-10-24)

Owner decision after MIGRATION Task 4: route choice is deferred past P2. P2 keeps least-cost legs
(S05) as the default and `Network.route_taus` stays `None`. The module (`ceteris-geo`
route_choice.rs, `ceteris_trade::LegTaus`, the kernel's use of route taus) stays in the tree,
tested and off.

**Measurement** (`cargo run -p ceteris-geo --release --example route_choice_radius`, P1 pack,
40,534 nodes, 134,810 edges, 14 threads; w_g from BACI 2000, lambda_g = P43 starts). Minimum
theta_r is the smallest value with radius below 0.99 (bisection on log theta_r, 0.5%). Gateway
solves are the time of all gateway solves (z and hours) at that minimum:

| Good | Radius at theta_r 182 | Min theta_r (radius < 0.99) | Gateway solves |
|---|---|---|---|
| agriculture | 5.36 | 33,716 | 227 s |
| oil and gas | 4.88 | 18,249 | 209 s |
| coal and mining | 2.96 | 4,735 | 176 s |
| food | 5.62 | 56,297 | 171 s |
| light manufacturing | 5.99 | 334,138 | 163 s |
| chemicals, fuels, metals | 5.61 | 56,488 | 162 s |
| machinery | 6.04 | 879,703 | 175 s |

The largest row sums of A sit at sea-grid nodes with very short edges (ln t down to 3e-6 per
edge, 0.17 km), plus one zero-length air edge.

**P44 prior.** 41 to 182, the routing elasticity of Fan, Lu and Luo (2021, "Valuing Domestic
Transport Infrastructure: A View from the Route Choice of Exporters"): 111.5, s.e. 35.4 (Table 3
p. 21, text p. 23, Table 4 p. 28), taken as the estimate +- 2 s.e. It is the only separate
estimate found. Allen and Arkolakis (2022, p. 2936), Fuchs and Wong (2026, pp. 16, 36) and
Ganapati, Wong and Ziv (2024, p. 268) set route dispersion equal to the trade elasticity (4 to 8).
Every good's minimum lies 26 to 4,800 times above the prior's upper end.

**Compounding.** Route costs are products of edge factors t = 1 + c w + lambda h / 24, so they
grow like exp(sum x) where S05 has 1 + sum x. At each good's minimum theta_r, coal from
Rotterdam to Shanghai costs tau 13.1 against 3.6 under S05 (agriculture 1.52 against 1.42,
machinery 1.30 against 1.29). This holds at any theta_r, not only near the radius bound.

**Build time.** 160 to 227 s per good for the gateway solves at radius 0.99, against the 2 s
`kernel_build` budget (trade SPEC); domestic solves take about 0.1 s. Near radius 1, walks that
turn back dominate and tau collapses toward 1, so the radius bound is not a usable operating
point either.

**Caveats.** Domestic solves used each country's subgraph without sea nodes: 16,270 of 279,498
domestic province pairs (mostly islands) had no route. Gateways were a proxy: 573 nodes, every
6th port or airport, since `gateways.arrow` does not exist yet.

**Solver.** Conjugate gradient cannot resolve z, which spans hundreds of decades. Solves scale z
by the least-cost walk and run symmetric Gauss-Seidel in Dijkstra order, with tau in logs
(`RouteTau.tau`); accepted by the owner.

**Revisit trigger.** A coarse corridor network with freight in P8, where edge log-costs are set
so that route-choice tau reproduces S05 tau on least-cost routes.

## Consequences

ceteris-geo golden hashes change. The trade kernel (0016) uses route-choice factors directly,
and 5.5's infrastructure planner can score links with the Allen-Arkolakis welfare formula.
