# Geo SPEC

Status: draft (P1 needs no human approval)
Phase: P1
Design sections: 5.1, 5.2, 5.3, 5.4, 5.6, 16.1, 19.1, 19.4, 20.1, 21.1 (P1); ADRs 0006 to 0011; route choice (P2): ADR 0017

## Purpose

The world pack and `ceteris-geo`. The pack pipeline (`ceteris-pack`) turns pinned
public-domain sources into provinces at 1 January 2000 borders, an H3 base raster and a
multimodal transport graph, and nests a county display layer in the provinces (ADR 0009).
`ceteris-geo` loads the pack, prices graph edges per tonne from
mode parameters, and answers least-cost path queries through a cache that is dropped when
any edge cost changes. For P2 it also computes route-choice trade costs per good (ADR 0017,
Route choice below). It owns the physical part of the generalized trade cost (5.4); the
inventory, risk, tariff and friction terms, and the iceberg mapping, arrive with trade in P2.

## Pack (built by `ceteris pack build` with the `geos` feature, about 2.5 minutes; needs GEOS 3.15 and tippecanoe)

Inputs are pinned by URL, sha256 and size in `fetch.toml` and `counties.lock.toml`; same inputs
give byte-identical tables (`tests/golden/p1-pack.txt`; for the tile archive, its layer inputs).
Outputs go to `data/packs/2000/` (not in git).

| Table | Rows | Content |
|---|---|---|
| `provinces.arrow` | 4,796 | id, name, 2000 country and sovereign, current country, anchor lon/lat, area, elevation mean and spread, TRI, terrain class, coastal, dev-slice flag, cell, port and airport counts, rail flag, mineral site counts by group |
| `countries.arrow` | 242 | code, name, sovereign, map colour slot |
| `cells.arrow` | 78,483 | H3 cell (resolution 4, finer for provinces too small to own a resolution-4 centre), province, area, elevation mean and spread, TRI |
| `nodes.arrow` | 40,534 | kind, province (u32::MAX for sea), lon/lat, label, sea zone |
| `edges.arrow` | 134,810 | endpoints (a < b), mode, length in km, mean TRI, chokepoint id |
| `counties.arrow` | 53,084 | name, current country, geoBoundaries level, province, area, elevation mean and spread, TRI (resolution-6 sampling) |
| `sea_zones.arrow` | 725 | H3 resolution-1 parent per zone |
| `chokepoints.arrow` | 10 | name, canal flag; edge chokepoint id k is row k - 1 |
| `web/map.pmtiles` | | vector tiles, layers provinces, countries, counties (from zoom 4); feature ID = index + 1 |
| `web/*.fill` | | GPU fill buffers per layer: Web Mercator triangles with a feature index per vertex, long edges split for the globe (ADR 0010) |
| `web/*.json` | | columnar province and county attributes for the map |
| `manifest.json` | | config, source checksums, table sha256, build statistics |

Every Arrow table carries schema metadata `pack_format`; the core refuses a mismatch.

## State components

| Component | Entity | Type | Units | Initialized from |
|---|---|---|---|---|
| Node kind, province, position | transport node | SoA columns | degrees | `nodes.arrow` |
| Edge endpoints, mode, length, TRI, chokepoint | transport edge | SoA columns | km, m | `edges.arrow` |
| Adjacency | node | CSR offsets over both directions of each edge | | built at load |
| Edge cost and hours | transport edge | `f64` | 2000 USD per tonne, hours | priced at load and on every change |
| Closed chokepoints | graph | list of ids | | empty |
| Graph version | graph | `u64` | | bumped on every repricing |

Node `i < P` is province `i`. Edges are undirected and stored once.

## Equations

Edge price for mode m, length l (km) and mean ruggedness r (TRI, m):

```
line(m)   = f_m * k_m * l * phi(r)            (phi = 1 for sea and air)
hours(m)  = k_m * l / s_m
phi(r)    = min(1 + beta * r / 100, phi_max)   terrain factor, road and rail only
access(j) = line(road) + h_j,  hours(road) + t_j   for rail terminal, listed port, minor port, airport
canal     = line(sea) + h_canal
```

Path cost: c_od = min over paths of the sum of edge prices (Dijkstra; heap keys are the bit
patterns of non-negative `f64`, ties pop the lower node first). Hours accumulate along the
chosen least-cost path. A closed chokepoint prices its edges at infinity.

### Route choice (ADR 0017; `route_choice.rs`): built but off

Built and tested in P2 but not used: route choice is deferred past P2 (owner, 2026-10-24; ADR
0017, Deferral) and trade keeps least-cost legs (S05). No dispersion in the P44 prior brings the
spectral radius below 1 on the P1 graph.

Per traded good g, with weight per value w_g, time cost lambda_g (trade P43) and route
dispersion theta_r (trade P44), each edge gets an iceberg factor and a route weight

```
t_kl = 1 + c_kl * w_g + lambda_g * hours_kl / 24        (c, hours from the edge price above)
a_kl = t_kl ^ -theta_r                                   (0 on a closed edge)
z_s. = row s of (I - A)^-1 = sum over walks from s of the product of a     (Allen and Arkolakis 2022)
tau_sd = z_sd ^ (-1/theta_r)
T_sd  = [(I - A)^-1 (A o H) z_s.]_d / z_sd                 expected route hours
```

The series converges only when the spectral radius of A is below 1; `EdgeFactors::spectral_radius`
(power iteration, norm ratio from the all-ones vector, at most 10,000 iterations, relative change
1e-10) is checked before every solve and a radius of 1 or more panics with its value, never
clipped. As theta_r grows, tau tends to the product of edge factors along the least-cost path.
The international factor is one solve per gateway on the whole graph; the domestic factor one
solve per province on its country's subgraph (`solve_from(.., Some(nodes), ..)`, results
indexed like `nodes`).

z spans hundreds of decades over the globe, so a residual-norm method (conjugate gradient, as
the migration plan first specified) leaves far entries at 0 or wrong: on the 6-node test graph CG
at a relative residual of 1e-12 gives tau = infinity at theta_r 1000. Each solve instead scales
z by the least-cost walk, z_k = exp(-D_k) u_k with D the Dijkstra distance under the weights
theta_r ln t, and runs symmetric Gauss-Seidel on u in Dijkstra order from u = 1 (the least-cost
walk alone, a lower bound), until no entry changes by more than 1e-12 relative. The scaled
weights exp(-(theta_r ln t_kl + D_l - D_k)) lie in [0, 1], and tau = exp((D - ln u) / theta_r)
is computed from logs, so it stays finite where z underflows. A warm start is a previous z.
Deterministic: each solve is sequential; solves and the radius's rows run in parallel into fixed
slots.

## System order

No tick systems in P1: the graph is static except for shocks. `PathCache` compares the graph
version on every query and drops all trees when it moved (19.1: recompute only when edges
change). `warm` computes missing trees in parallel; results are independent of thread count.

## Invariants

| Invariant | Statement | Tolerance | Checked where |
|---|---|---|---|
| Reachability | Every province reachable from every probe province | exact | `tests/pack_properties.rs` |
| Triangle inequality | c(s,b) <= c(s,a) + w(a,b) on every edge | 1e-9 | `tests/pack_properties.rs` |
| Symmetry | c(a,b) = c(b,a) | 1e-9 relative | `tests/pack_properties.rs` |
| Shock reversibility | Close then reopen a chokepoint: identical bits | exact | `tests/pack_properties.rs` |
| Thread independence | Cache warmed on 1 and 8 threads: identical bits | exact | `tests/pack_properties.rs` |
| Pack reproducibility | Rebuild gives the stored table hashes | exact | `ceteris-pack/tests/build.rs` (`pack_tables_match_golden`, `two_builds_write_the_same_bytes`) |
| County nesting | Every county lies in one province; of counties at most half the area limit, under 5% of area falls outside it | 5% | `ceteris-pack/tests/build.rs` (`the_pack_tables_hold_their_invariants`) |
| Mode ordering | Default per tonne-km: sea < rail < road < air | exact | `graph.rs` unit test |
| Route-choice radius | Spectral radius of A below 1 for every traded good at P44 | exact (asserted) | `solve_from`; `tests/pack_properties.rs` (ignored while route choice is deferred) |

## Parameters

Priors for register entry P04 (mode cost ratios), sub-IDs in `TransportParams`. Magnitudes
are placeholders until P9 calibrates them against freight rates and price gaps; only the
ordering is imposed (5.3).

| ID | Name | Prior / value | Class |
|---|---|---|---|
| P04a-d | Cost per tonne-km: road, rail, sea, air | 0.10, 0.04, 0.008, 0.80 USD. Direct PPML estimate recorded as a diagnostic, ADR 0022 (2000 BACI along S05 paths, road as numeraire; values kept, owner 2026-10-24): rail/road 52.6 (s.e. 24.1), sea/road 115.3 (46.3), air/road -2.39 (34.4), not identified (the fit drifts to a log-distance form; air on about 70 of 378,392 least-cost rows); a linear km form gives 0.12, 0.09, 3.8 | Literature + calibrated |
| P04e-h | Circuity: road, rail, sea, air | 1.3, 1.3, 1.0, 1.05 | Literature |
| P04i-l | Effective speed: road, rail, sea, air | 40, 25, 25, 500 km/h | Literature |
| P04m | Handling fee: rail terminal, listed port, minor port, airport | 5, 8, 20, 60 USD/t | Literature + calibrated |
| P04n | Handling time, same order | 24, 48, 72, 12 h | Literature |
| P04o | Canal transit fee | 5 USD/t | Literature |
| P04p | Terrain slope beta | 0.5 per 100 m TRI | Free |
| P04q | Terrain cap phi_max | 3.0 | Free |

Sea circuity is 1.0 because paths already follow the hexagonal sea grid.

## Equilibrium nesting

n/a in P1: costs feed the trade solver in P2, where tau = 1 + c / p maps them to the GE
reference model's iceberg factors.

## Test plan

- Unit: Dijkstra against Floyd-Warshall on random graphs (proptest); cache invalidation on
  repricing; mode ordering; terrain factor bounds; access and canal fees.
- Route choice (unit, toy graphs): convergence to the least-cost path factor as theta_r goes
  10, 100, 1000 (monotone, gap below 1e-3); derivative of tau in one edge cost against a central
  difference (1e-6); scaling one edge up to closure moves tau monotonically, no step above 5%;
  two disjoint routes against the closed form z = q / (1 - 2q); expected hours against the walk
  sum on a path; far entries below the f64 range keep tau exact; subgraph and warm-started solves;
  a radius above 1 panics.
- Pack properties: the invariants table above, on the built pack; skipped without one.
- Measurement: `cargo run -p ceteris-geo --release --example route_choice_radius` prints, per
  traded good, the smallest theta_r with radius below 0.99, the radius at the ends of the P44
  prior and the time of all gateway and domestic solves (MIGRATION Task 4 report).
- Pipeline (`ceteris-pack`, proptest): spherical area, antimeridian unwrap and wrap, split counts,
  bisection balance (by count and by weight) and determinism, county level choice, county
  splits, neighbour colouring, terrain classes, 2000 remapping, pack table invariants.
- Golden: `tests/golden/p1-geo.txt` (cost and hour bits from eight probe provinces) and
  `tests/golden/p1-pack.txt` (table hashes). Bless with `CETERIS_BLESS=1`.
- Benchmark: `dijkstra_grid_40k` (criterion), a 40,000-node grid the size of the pack graph.
- App: IPC test of `cost_from` on Tauri's mock runtime.

## Open questions

- Terrain factor form (`terrain_factor`): linear and capped by default; slope-based costs as
  in Allen and Arkolakis (2014) are an alternative. Calibrated in P9.
- Inland waterways and pipelines are not in the graph yet (ADR 0007); Great Lakes ports are
  dropped for the same reason.
- Rail, ports and airports are current vintages, not 2000; gauge breaks, ferries, bridges and
  tunnels across straits are not modelled.
- MRDS is about 89% United States sites: the resource mode shows coverage, not endowment.
- Natural Earth mixes administrative levels (232 units in the United Kingdom, 193 in
  Slovenia, 16 in Germany); merging small units and population-based splitting wait for
  gridded population in P4. County granularity is uneven too (ADR 0009).
- Whether the production app's asset protocol serves the HTTP Range requests PMTiles needs.
- Trees cover all nodes (650 KB each); restrict to provinces and gateways when P2 warms 600.
