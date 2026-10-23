# Geo SPEC

Status: draft (P1 needs no human approval)
Phase: P1
Design sections: 5.1, 5.2, 5.3, 5.4, 5.6, 16.1, 19.1, 19.4, 20.1, 21.1 (P1)

## Purpose

The world pack and `ceteris-geo`. The pack pipeline (`python/ceteris/packs`) turns pinned
public-domain sources into provinces at 1 January 2000 borders, an H3 base raster and a
multimodal transport graph. `ceteris-geo` loads the pack, prices graph edges per tonne from
mode parameters, and answers least-cost path queries through a cache that is dropped when
any edge cost changes. It owns the physical part of the generalized trade cost (5.4); the
inventory, risk, tariff and friction terms, and the iceberg mapping, arrive with trade in P2.

## Pack (built by `uv run python -m ceteris.packs`, about 25 s)

Inputs are pinned by URL, sha256 and size in `fetch.toml`; same inputs give byte-identical
outputs (`tests/golden/p1-pack.txt`). Outputs go to `data/packs/2000/` (not in git).

| Table | Rows | Content |
|---|---|---|
| `provinces.arrow` | 4,796 | id, name, 2000 country and sovereign, current country, anchor lon/lat, area, elevation mean and spread, TRI, terrain class, coastal, dev-slice flag, cell, port and airport counts, rail flag, mineral site counts by group |
| `countries.arrow` | 242 | code, name, sovereign, map colour slot |
| `cells.arrow` | 78,483 | H3 cell (resolution 4, finer for provinces too small to own a resolution-4 centre), province, area, elevation mean and spread, TRI |
| `nodes.arrow` | 40,522 | kind, province (u32::MAX for sea), lon/lat, label, sea zone |
| `edges.arrow` | 134,753 | endpoints (a < b), mode, length in km, mean TRI, chokepoint id |
| `sea_zones.arrow` | 725 | H3 resolution-1 parent per zone |
| `chokepoints.arrow` | 10 | name, canal flag; edge chokepoint id k is row k - 1 |
| `web/` | | simplified province and country GeoJSON and a columnar attribute table for the map |
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
| Pack reproducibility | Rebuild gives the stored table hashes | exact | `python/tests/test_packs.py` |
| Mode ordering | Default per tonne-km: sea < rail < road < air | exact | `graph.rs` unit test |

## Parameters

Priors for register entry P04 (mode cost ratios), sub-IDs in `TransportParams`. Magnitudes
are placeholders until P9 calibrates them against freight rates and price gaps; only the
ordering is imposed (5.3).

| ID | Name | Prior / value | Class |
|---|---|---|---|
| P04a-d | Cost per tonne-km: road, rail, sea, air | 0.10, 0.04, 0.008, 0.80 USD | Literature + calibrated |
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
- Pack properties: the invariants table above, on the built pack; skipped without one.
- Pipeline (pytest, hypothesis): spherical area, antimeridian unwrap and wrap, split counts,
  bisection balance and determinism, neighbour colouring, terrain classes, 2000 remapping,
  pack table invariants.
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
  gridded population in P4.
- Trees cover all nodes (650 KB each); restrict to provinces and gateways when P2 warms 600.
