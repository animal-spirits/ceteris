## 5. World Representation: Geography, Provinces and Transport

Geography enters the economy through land and resource endowments and through trade costs, so the map is a raster of physical and economic fundamentals aggregated into modern administrative provinces and linked by a multimodal transport graph.

### 5.1 Base raster

A global hexagonal grid (H3 resolution 4, cells of roughly 1,800 sq km; finer cells for provinces too small to hold one; resolution 6 for county statistics) stores: elevation, slope and ruggedness (ETOPO 2022), land cover, climate normals, crop suitability (FAO GAEZ v4, current input levels), gridded population and built-up area, night lights (for downscaling economic activity), energy and mineral resources, and coastlines and ports. The raster feeds province aggregates and transport costs; it is not simulated directly, except by the climate damage module.

### 5.2 Provinces

Provinces are first-level administrative regions from Natural Earth (public domain; 4,594 units at mixed levels), remapped to 1 January 2000: South Sudan in Sudan, Serbia and Montenegro in Yugoslavia, Crimea in Ukraine. Regions are kept whole, whatever their area (ADR 0251; ADRs 0006 and 0009 split those over 250,000 km²). A county layer from geoBoundaries of 53,084 units is drawn on the map and carries raster statistics; it is not simulated. The post-2000 sovereignty changes (Timor-Leste, Montenegro, Kosovo, South Sudan) remain validation events, not scripted ones.

| Attribute | Source | Dynamic? |
| --- | --- | --- |
| Area, terrain, ruggedness | Raster | No |
| Population by age, sex, urban share | Gridded population, census tables | Yes |
| Output by broad sector | Subnational output datasets (DOSE), gridded GDP, night lights; reconciled to national ICIO totals | Yes |
| Arable land and yields | GAEZ v4, land cover | Yields yes; area by land conversion |
| Energy and mineral resources | Open energy-asset trackers, geological deposit data | Discovery, depletion |
| Infrastructure (roads, rail, ports, airports, pipelines, power) | OpenStreetMap (ODbL) and open infrastructure datasets | Built by investment |
| Ethnic settlement areas | GeoEPR | Shares evolve |
| Owner, controller, claims | Borders as of 2000 | Yes |
| Local institutions (autonomy, property security) | National institutions plus subnational indicators where available | Slowly |

### 5.3 Transport graph

Nodes are province centroids, ports, airports and sea-lane waypoints. Edges carry one or more modes, each with a per-ton-km cost, speed, capacity and risk that depend on investment and fuel prices.

The 2000 graph has 4,594 province anchors, 2,662 rail nodes, 2,234 ports (1,001 listed, 1,233 minor), 1,154 airports and 29,543 open-water H3 resolution-3 cells, grouped into 725 sea zones. Ten chokepoints tag their sea cells; Suez, Panama, the Turkish Straits, Gibraltar and Bab-el-Mandeb are explicit links. Edge costs are priced in the core from P04 parameters, not stored in the pack. Chokepoints carry closure hazards and coastal provinces a piracy hazard (ADR 0242).

| Mode | Edge existence | Cost driver | Notes |
| --- | --- | --- | --- |
| Road | Network between adjacent provinces | Distance, road class, congestion, fuel | Dominant within continents |
| Rail freight | Built lines | Distance, gauge breaks, electrification | Bulk goods |
| Inland waterway | Navigable rivers, canals | Distance, seasonal depth | Bulk goods; not yet built |
| Maritime (container, bulk, tanker) | Sea lanes via chokepoints (Suez, Panama, Hormuz, Malacca, Bosporus) | Distance, bunker fuel, port capacity, freight rates | Chokepoint closures are shocks; a bulk and tanker rate per theta class, P154 and P155, on for class 3 (ADR 0247) |
| Air freight | Airport pairs | Distance, jet fuel | High value-to-weight goods |
| Pipeline | Built oil and gas pipelines | Throughput, transit fees | Geopolitically exposed; not yet built (P8, with energy assets) |

Relative mode costs are calibrated priors: the ordering (sea cheapest per ton-km, then rail and waterway, then road, air far above) is imposed; magnitudes are estimated against international transport-cost data and price gaps (Section 15).

### 5.4 Generalized trade cost

For good g from province o to province d, the cost per unit is the least-cost path over the graph:

```latex
c_{od,g} = \min_{\text{paths}} \sum_{e} \left( f_{e,m} \, w_g \, \ell_e + h_{e} \right) + p_{o,g} \, r \, T_{od} + \rho_{od} \, p_{o,g} + t_{od,g} + \nu_{od}
```

where f is the per-ton-km mode cost, w the good's weight per unit of value, l edge length, h transshipment and port cost, r the interest rate times transit time T (inventory carrying cost), rho the loss and disruption risk, t tariffs and other policy wedges (applied tariffs initialized from WTO and World Bank tariff data), and nu non-policy frictions (language, legal system, currency, sanctions compliance). For comparison with the GE reference model the cost maps to an iceberg factor tau = 1 + c / p. Per-unit components matter: Hummels and Skiba (2004) show per-unit shipping costs change the composition of trade, which pure iceberg models miss.

**Route choice (ADR 0017, deferred past P2).** P2 keeps the least-cost path above (S05) with the gateway kernel of 8.3. Route choice, built in ceteris-geo and ceteris-trade, tested and off, replaces the hard minimum with a soft minimum; it is revisited with the P8 corridor network. Following Allen and Arkolakis (2022), each shipment draws Frechet route-specific shocks with the trade elasticity as shape parameter, so the cost averaged over all routes has a closed form:

```latex
\tau_{od}^{-\theta} = \left[ (I - A)^{-1} \right]_{od}, \qquad A_{kl} = t_{kl}^{-\theta}
```

where t(k,l) = 1 + c(k,l) / p is the iceberg equivalent of edge (k,l)'s per-unit, time and risk costs from the formula above, evaluated at the good's reference price (updated annually) so that A changes only when edges or reference prices change. Tariffs and non-policy frictions apply once per border crossing, on the gateway link. The inverse is never formed: kernels come from sparse solves (I - A)x = b from each gateway and province, run only when A changes, and the spectral radius of A is checked to be below 1 at pack build. A is built per (theta class, weight class). Expected transit time for the shipment pipeline (8.4) is the route-probability-weighted time and needs one more pair of sparse solves.

Route choice makes costs differentiable in edge parameters (gradient-based calibration needs this; minima are not differentiable), makes the gateway factorization of 8.3 exact, reroutes trade smoothly when a chokepoint closes, and gives the welfare effect of an infrastructure change in closed form for 5.5. As theta grows the soft minimum converges to the least-cost path, which is switch S05, the P2 default, and still drives the freight-cost map mode.

### 5.5 Endogenous infrastructure

States and enterprises propose projects (highways, rail lines, ports, pipelines) scored by expected market-access gains, following Donaldson and Hornbeck (2016), against cost. Network design is informed by Fajgelbaum and Schaal (2020) optimal transport networks: an AI planner ranks candidate links by the marginal welfare gain implied by current flows and congestion. Built infrastructure lowers edge costs, which raises market access, which changes location choices: the main spatial feedback loop of the model.

### 5.6 Licensing note

OpenStreetMap is licensed under ODbL, whose share-alike terms attach to derived databases. OSM-derived tables are built by each user's pipeline or, where shipped, ship as a separate artifact under ODbL, attributed. Every source's licence is recorded in the pack manifest (Section 16).

