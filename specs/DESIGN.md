# Ceteris Paribus: Design Document for a Paradox-Style Economic World Simulation

2026-10-23 · @Antonio

## 1. Executive Summary

Ceteris Paribus (code name `ceteris`) is a deterministic, forkable, agent-based world simulator that starts on 1 January 2000 and runs open-ended. Its economic core is a province-level production-and-trade model disciplined by quantitative trade theory, presented through a Paradox-style map in a native desktop app. Any state variable or parameter can be edited before a run (ex ante) or at any tick during it (ex post). It is open source and non-commercial, and each user downloads the source data.

Six design bets carry the project:

1. **Hybrid microstructure.** Population groups (pops) and enterprises are agents trading in out-of-equilibrium markets with inventories and lags. A structural dynamic spatial general-equilibrium model (Caliendo-Dvorkin-Parro / Kleinman-Liu-Redding class) runs alongside as a reference model: in the frictionless limit the agent model must reproduce it.
2. **Every province is a market.** Each of about 4,000 provinces prices every good. Shipments between provinces follow structural gravity at posted prices, and origin markets clear across ticks through inventories, so the price dynamics are entropic optimal transport spread over time and literature trade elasticities map one-to-one onto model parameters.
3. **Institutions are a continuous state vector**, initialized from V-Dem, with explicit political parties built from interest-group coalitions. Labels such as 'electoral autocracy' are derived views, never mechanics.
4. **Every run is reproducible and forkable.** Counter-based random numbers keyed by entity and tick, periodic snapshots and an event log let any branch be replayed exactly, and let interventions be compared against their parent under common random numbers. Ensembles report distributions, never single trajectories.
5. **Data-driven initial conditions, few free parameters.** Following the data-driven ABM program of Poledna et al. (2023) and Wiese et al. (2024), the world is initialized from 2000 data (OECD ICIO, BACI, gridded population, V-Dem). Behavioural parameters are estimated on 2000-2010 and tested out of sample on 2011-2024, a window containing the 2008 crisis, the euro crisis, COVID and two tariff wars.
6. **Accounting is law.** Stock-flow consistency, goods conservation and population conservation are asserted every tick; a violation halts the run.

Headline specification:

| Dimension | Default | Rationale |
| --- | --- | --- |
| Start / horizon | 1 Jan 2000, open-ended | 25 years of out-of-sample history; long runs need climate, demography and stable growth |
| Economic tick | 1 month | Captures harvests, prices, credit and policy cycles |
| Military sub-tick | 1 day | Movement, battles, sieges |
| Provinces | 4,594 first-level regions from Natural Earth at 2000 borders, kept whole (ADR 0251) | Modern admin units carry the subnational data |
| Markets | Every province prices every good | Finest price geography; costs about 10x more trade-solver work than hubs |
| Goods | 45 sectors aligned with OECD ICIO, with energy and food staples split out | Direct mapping to input-output data |
| States | About 195 | Current sovereign states plus a handful of de facto entities |
| Pops | About 100,000: province x ethnic group x education x labour status | Matches modern data (GeoEPR, census education, labour surveys) |
| Hardware | MacBook Pro, M-series Pro, 18-36 GB | Desktop only; sets all budgets |
| Performance target | 0.5 to 1 simulated year per second, single run | Interactive play; ensembles and calibration overnight |

Stack: a Rust simulation core embedded in a desktop app. The app is fully native Rust, Slint panels over a wgpu globe, with no React, webview or Tauri (ADR 0088). The data pipeline, the reference model and calibration are Rust crates driven by the `ceteris` CLI (ADR 0113). The project is open source (Apache-2.0 / MIT) and non-commercial; each user's pipeline downloads the source data, so non-commercial sources are acceptable inputs and restricted data is never bundled (owner, 28 Oct 2026). Build order puts the map, accounting, markets, trade and the counterfactual workbench first, because those define the product (Section 21).

## 2. Vision, Scope and Design Principles

The product is a laboratory first and a game second: a living world map where the user watches economies, states and populations co-evolve from 2000 onward, intervenes, and compares branches of history quantitatively.

### 2.1 Modes of use

| Mode | User role | Typical question |
| --- | --- | --- |
| Observer | Watches; no control | What does the world look like in 2060 under baseline dynamics? |
| God mode (ex ante) | Edits initial state or parameters, then runs an ensemble | What if China never joins the WTO, or the trade elasticity were 8 instead of 5? |
| God mode (ex post) | Pauses at tick t, edits anything, forks a branch | What if Lehman is rescued in September 2008, given everything up to then? |
| Steward (2.0, ADR 0239) | Controls one state's policy levers; AI runs the rest | Can Brazil raise its growth rate with a different tariff and infrastructure policy? |
| Batch / scripted | The `ceteris` CLI runs experiment grids headless from scenario files | Distribution of 2030 outcomes across 64 seeds under three tariff regimes |

### 2.2 Era choice

The start is **1 January 2000**. The engine stays era-agnostic (goods, institutions and maps live in versioned data packs), and the pipeline can build a pack for any start year from 1995 to 2022 (the span of the OECD ICIO 2025 edition); 2000 is the primary scenario. Reasons:

- **Data replaces judgment.** OECD ICIO input-output tables and CEPII BACI trade flows start in 1995; gridded population, subnational GDP, V-Dem, UCDP conflict events and UN demographic data all cover 2000. Few hand-made decisions are needed.
- **A long out-of-sample window.** 2000-2024 contains China's WTO accession, the commodity supercycle, the 2008 financial crisis, the euro crisis, the Arab Spring, COVID, the 2022 energy shock and sanctions, and the 2018 and 2025 tariff rounds: natural tests of whether the model responds like the world did.
- **Open-ended future.** The model runs without a script from the first tick (ADR 0241), so the problem after 2024 is the same as after 2000, and long-run stability, demography and climate feedback become first-class requirements (climate feedback from 1.1, ADR 0207).

### 2.3 In scope

Economy (production, consumption, input-output, trade, transport, money, banking, sovereign debt, fiscal and monetary policy), population (demography, education, labour status, ethnicity, migration), technology (sector productivity growth and diffusion), politics (institutions, interest groups, parties, elections, legitimacy, unrest, regime change, secession), diplomacy and conflict (treaties, alliances, sanctions, interstate and civil war), climate (emissions, temperature, damages) and exogenous shocks (pandemics, disasters, commodity discoveries).

### 2.4 Non-goals for version 1

- Named individuals and leaders' personalities; leaders exist only as stochastic attributes of states and parties.
- Named technologies; technology is sector productivity (decision of 23 Oct 2026).
- LLM-driven leaders (deferred), tactical battle maps, 3D graphics, multiplayer, cloud hosting.
- Scripted events and historical railroading: post-2000 history is a validation target, not a script.
- Endogenous politics, borders and war (P6, P7), which are 2.0 (ADR 0136). In 1.x, institutions are 2000 data, borders are fixed, and policy comes from estimated reaction functions; the historical policy tape is the policy family of optional replay (ADRs 0210, 0241).

### 2.5 Design principles

1. **Mechanism over script.** No event fires because of a date; it fires because state variables crossed a threshold or a hazard realized.
2. **Every number has provenance.** Each parameter is tagged data, literature prior, or calibrated, with its source and an ID in the degrees-of-freedom register (Section 17).
3. **Conservation laws.** Money, goods and people are never created or destroyed except by explicit, logged processes.
4. **Equilibrium nesting.** Behavioural rules collapse to a known optimizing equilibrium when frictions are removed, giving a built-in correctness test and a bridge to the literature.
5. **Modularity.** Each module is a set of systems over shared component arrays with a written spec; alternative modules can be swapped to test model uncertainty.
6. **Explainability.** Every price, flow and decision can be decomposed in the UI into its drivers.
7. **Reproducibility.** Same data pack, parameters, seed and intervention list give bit-identical output.
8. **Two tiers per module.** Minimal first (closes the accounting loop), rich later. The world is always runnable.
9. **Open by construction.** Code, specs and every bundled dataset are redistributable; restricted data is excluded rather than worked around.

## 3. Modeling Philosophy and Global Assumptions

The model is a hybrid: agent-based dynamics for everything that moves, anchored to a structural equilibrium model for everything that can be measured against the trade and spatial literature.

### 3.1 Why hybrid

| Approach | Strength | Fatal weakness for this project |
| --- | --- | --- |
| Quantitative spatial GE / dynamic hat algebra | Clean, data-matched counterfactuals for trade and migration | No disequilibrium, crises, path dependence, wars or revolutions; politics is exogenous |
| DSGE / HANK | Expectations, monetary policy | Representative or limited heterogeneity, linearized around a steady state, no geography |
| Classical ABM (stylized) | Emergence, crises, rich heterogeneity | Many free parameters, weak calibration, hard to trust |
| Data-driven ABM (Poledna 2023 lineage) | Calibrated from accounts, competitive forecasts | Built for single economies over short horizons, no politics or war |
| Paradox games (Victoria 3) | Legible, playable, multi-system | Ad hoc mechanics, not calibrated, not reproducible for research |

Ceteris Paribus takes the agent architecture and legibility of the last row, the data discipline of the fourth, and uses the first as its reference model and validation target.

### 3.2 Unit of agency

Individuals are too many (about 6.1 billion people in 2000) and add little. The behavioural unit is the **pop**: a representative group defined by (province, ethnic group, education level, labour status). Within-pop heterogeneity is carried as moments (lognormal income with tracked mean and dispersion, age structure), so inequality and poverty headcounts are computable without simulating individuals. Enterprises are aggregated per (province, sector, ownership type: domestic private, state, foreign). States, central banks, commercial banks, interest groups and parties are explicit agents.

### 3.3 Global assumptions

Each assumption has an ID referenced by module specs. Assumptions marked (T) are testable switches: the alternative is implemented and compared.

| ID | Assumption | Alternative / test |
| --- | --- | --- |
| A1 | Discrete monthly time with a fixed system order within a tick | (T) Randomized order sensitivity runs |
| A2 | Space discretized into provinces; trade within a province is costless | Finer grid for selected regions |
| A3 | Pops are representative households with lognormal within-group income | (T) Two-point income distribution |
| A4 | Preferences are non-homothetic CES across consumption categories, CES within | (T) Stone-Geary / AIDS |
| A5 | Constant returns at the enterprise; external agglomeration economies at the province | (T) No agglomeration |
| A6 | Expectations are heuristic and adaptive, not rational | (T) Perfect-foresight reference in GE module |
| A7 | Trade costs combine per-ton transport cost, time cost, risk and policy wedges | (T) Pure iceberg costs |
| A8 | Goods are homogeneous by (good, origin); product differentiation arises from dispersion in buyer and merchant choice | Explicit quality tiers in rich tier |
| A9 | Money is explicit fiat currency issued through central and commercial banks; every transaction settles in a currency | Numeraire-only mode for debugging |
| A10 | States are unitary in diplomacy; internally their objectives aggregate interest-group preferences by power | Factional foreign policy in rich tier |
| A11 | Military outcomes are stochastic functions of force, logistics, technology, terrain and morale | n/a |
| A12 | Geography, initial conditions, weather variability and disaster hazards are exogenous; climate trends are endogenous | n/a |
| A13 | Agents observe the world with lags and noise; no agent sees true global state | (T) Full information |
| A14 | The world is closed: every flow has a counterparty inside the simulation | n/a |
| A15 | Long-horizon outputs are distributions, not forecasts | n/a |

### 3.4 The equilibrium-nesting principle

Every behavioural rule is specified as a relaxation of an optimizing benchmark with an explicit friction parameter. Setting adjustment speeds high, information lags to zero and dispersion parameters to their structural values must reproduce the benchmark. Examples: merchant logit routing collapses to structural gravity; inventory-based pricing converges to Walrasian clearing; dynamic migration choice collapses to the Caliendo-Dvorkin-Parro migration block. This gives every module a closed-form correctness test and makes literature elasticities usable as priors. Household saving, money and fiscal policy have no closed-form benchmark, so their reference is a sequence-space HANK model (ADR 0025), matched on impulse responses given the same intertemporal MPCs.

## 4. System Architecture and Simulation Loop

The engine is a data-oriented entity-component system: entity tables stored as structure-of-arrays, advanced by an ordered pipeline of systems, with every discrete decision written to an event log and every flow written to a double-entry ledger.

&#91;embedded content: system architecture · core, data, clients, calibration loop\]

The map client sends edits straight into the systems pipeline and reads results back through the query API; calibration closes the loop by writing posterior parameters into the next data pack.

### 4.1 Layers

1. **Data packs**: versioned, built offline by the `ceteris-pack` pipeline (geography, initial populations, capital, technology tree, institutions, parameter priors).
2. **World state**: component arrays keyed by stable entity IDs.
3. **Systems pipeline**: pure functions of (state, parameters, RNG streams) that mutate state in a fixed order.
4. **Ledger and event log**: every monetary and goods flow as a balanced transaction; every discrete decision (law change, war declaration, firm entry) as an event.
5. **Snapshot and branch store**: periodic full snapshots plus logs; any tick of any branch is reconstructible.
6. **Query API**: Arrow-based reads for the map client and external analysis tools.
7. **Clients**: web map, CLI batch runner; analysis reads the Arrow and Parquet outputs.

### 4.2 Entity model

| Entity | Key components | Approx. count |
| --- | --- | --- |
| Province | owner, controller, terrain, raster aggregates, land by use, infrastructure, local institutions, market (price and inventory per good) | 4,000 |
| SeaZone | shipping lanes, chokepoints, control, piracy risk | 300 |
| MerchantPool | shipping and logistics capital, route knowledge, per origin province | 4,000 |
| Pop | size by age band, ethnic group, education, labour status, income moments, wealth, ideology, grievance, party attachment | 100,000 |
| Enterprise | sector, ownership type, capacity, capital, employment, inventories, owners, debts | 150,000 |
| Bank | balance sheet, interbank exposures | 2,000 |
| CentralBank | policy rate, reserves, balance sheet, mandate | about 160 (monetary unions share one) |
| State | institution vector, laws, budget, debt, currency, AI profile, relations | about 195 |
| InterestGroup / Party | members (pop shares), preferences, clout, organization, platform | about 10 groups and 2-8 parties per state |
| Army / Fleet | location, composition, supply, morale, orders | 2,000 |
| Shipment | good, quantity, origin, destination, arrival tick, owner | transient, 100,000s |
| FinancialClaim | loan, bond, deposit, equity: issuer, holder, terms | 1,000,000 |
| Treaty | type, parties, terms, expiry | 2,000 |

### 4.3 Time scales

| Frequency | Processes |
| --- | --- |
| Hourly (phase inside the daily sub-tick, sparse; ADR 0209) | Order arrival, movement and contact, combat rounds on engaged segments, sorties and strikes, naval search and interception |
| Daily (sub-tick, only while any war is active) | Supply, planners, intelligence, air and naval tasking, encirclement, morale, blockade status |
| Monthly (main tick) | Production, labour markets, consumption, price updates, trade flows, payments and credit, taxes, migration decisions, unrest, law-change progress, diplomacy |
| Quarterly | Enterprise investment and entry/exit, bond issuance |
| Annual | Cohort aging, fertility and mortality rate updates, productivity catch-up step, climate emulator step, elections calendar, census outputs, harvest realization for annual crops |

### 4.4 Monthly tick pipeline

1. Apply scheduled interventions (including any replay) for tick t.
2. Observation update: agents receive lagged, noisy signals of prices, wages and threats.
3. Demography: births, deaths, aging (annual components on their month).
4. Labour market: vacancies, matching, wages, occupational switching.
5. Production: enterprises choose output, buy inputs, produce; harvest and mining realizations.
6. Consumption demand: pops and states form demand at posted prices.
7. Province markets: local supply meets demand; inventories absorb gaps; prices update.
8. Inter-province trade: solve shipments, dispatch to transit pipeline, deliver arrivals.
9. Finance: settle payments, interest, loans, defaults, bank balance sheets, FX.
10. Fiscal: tax collection, government spending, debt service.
11. Investment and construction (quarterly months): capacity expansion, infrastructure.
12. Politics: clout, approval, ideology drift, unrest, law progress, regime events.
13. Diplomacy and military: crises, bargaining, mobilization, war start; daily sub-ticks if at war.
14. Migration: realized moves between provinces and abroad.
15. Accounting assertions; metrics; snapshot if t mod 12 = 0.

### 4.5 Determinism

- **RNG**: counter-based (Philox 4x32-10). Each draw is a pure function of (master seed, tick, system ID, entity ID, draw index). No global RNG state, so adding an entity or a system does not shift any other stream.
- **Floating point**: f64; no fast-math; parallel reductions use fixed-shape pairwise trees so results do not depend on thread count.
- **Ordering**: entities iterate in stable-ID order; ties broken by ID.
- **CI check**: two runs with the same inputs must produce identical state hashes at every snapshot.

### 4.6 Accounting invariants (asserted every tick)

| Invariant | Statement | Tolerance |
| --- | --- | --- |
| Stock-flow consistency | Every financial asset has a matching liability; sector net lending equals change in net financial wealth | 1e-9 relative |
| Money | Money stock changes only via bank lending/repayment, central bank operations, specie mining/minting | exact in ledger |
| Goods | Production + arrivals = consumption + intermediate use + inventory change + shipments + spoilage + destruction | 1e-9 relative |
| Population | Change = births - deaths + immigration - emigration, per province | exact (integers per cohort cell) |
| Land | Sum of land by use equals province land area | exact |

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

## 6. Population: Pops, Demography, Identity and Migration

Population is both the labour supply and the political substrate, so each pop carries economic state (education, labour status, income, wealth) and political state (ethnic identity, ideology, party attachment, grievance) in one record.

### 6.1 Pop definition

Key: (province, ethnic group, education, labour status), about 100,000 pops after dropping empty cells.

| Key dimension | Categories | Initialization source |
| --- | --- | --- |
| Ethnic group | Politically relevant groups per country, plus 'other' | GeoEPR settlement areas and group sizes |
| Education | Below secondary, secondary, tertiary | Barro-Lee / Wittgenstein Centre attainment, subnational where available |
| Labour status | Formal wage employee, informal worker, self-employed / smallholder, business owner / employer, public sector, unemployed, inactive (students, retirees, carers) | ILO labour statistics, household surveys |

Religion is a share vector within each pop rather than a key, initialized from available national and subnational religion data, to keep the pop count bounded. Components per pop: size by age band and sex (five-year bands), income mean and log-variance, wealth by asset type, consumption basket, employer links, ideology vector, party attachment, grievance, language and religious shares.

### 6.2 Fertility and mortality

Mortality uses the Brass relational logit life table: a standard schedule shifted by a level parameter and tilted by a shape parameter.

```latex
\operatorname{logit}\left(1 - l_x\right) = \alpha_{p,t} + \beta \, \operatorname{logit}\left(1 - l_x^{std}\right)
```

The level alpha responds to real income (a Preston-curve relation), health spending, epidemics, conflict and climate damages (heat). Fertility is a pop-level total fertility rate driven mainly by female education, income, urbanization and child survival, with a floor well below replacement so that sustained low-fertility regimes are reachable:

```latex
TFR_{p,t} = TFR^{min} + (TFR^{max} - TFR^{min}) \cdot \sigma\!\left(\gamma_0 - \gamma_e \, e^{f}_{p,t} - \gamma_y \ln y_{p,t} - \gamma_u \, \text{urban}_{p,t} + \gamma_m \, \text{child mortality}_{p,t}\right)
```

Rates are initialized to match UN World Population Prospects for 2000; 2000-2023 estimates are the validation target. UN projections beyond 2024 are a comparison benchmark, not a target. Aging feeds directly into labour supply and pension spending, which matters for open-ended runs.

### 6.3 Education and labour status

Cohorts move up education levels at enrollment rates set by public and private education spending and household income. Workers move between labour statuses each month: matching in formal labour markets, entry to informality when formal jobs are scarce, business formation and failure, retirement by age and pension law. Skill premia emerge from sector demand for educated labour.

### 6.4 Ethnicity, religion and identity politics

Ethnic groups carry a political status from the Ethnic Power Relations data (dominant, senior or junior partner, powerless, discriminated). Horizontal inequality between groups (income and political exclusion) feeds grievance, the mechanism documented by Cederman, Gleditsch and Buhaug (2013) for civil conflict. Language and religiosity shift through the cultural transmission model of Bisin and Verdier (2001), with state policy (schooling language, discrimination, autonomy) as levers.

### 6.5 Migration

Internal and international migration share one dynamic discrete-choice model in the Artuc-Chaudhuri-McLaren / Caliendo-Dvorkin-Parro tradition. Each month a share of each pop reconsiders location; destinations are chosen by logit over expected values net of moving costs:

```latex
\mu_{od} = \frac{\exp\left( (\beta \, \mathbb{E} V_d - m_{od}) / \nu \right)}{\sum_k \exp\left( (\beta \, \mathbb{E} V_k - m_{ok}) / \nu \right)}
```

Moving cost m depends on distance and travel cost, immigration law and visa regimes (free movement inside unions such as the EU), language and cultural distance, credit constraints, and diaspora networks. Expected value uses Harris-Todaro expected wages (wage times employment probability), amenities, conflict risk and climate damages. Forced displacement from conflict and disasters follows a separate hazard-driven flow toward safe neighbours. Remittances flow back as household transfers, a large financial flow for many economies. The dispersion parameter nu is calibrated to bilateral migration stocks and flow estimates; internal urbanization rates are a second target.

**Computation.** A dense logit is 100,000 pops x 4,594 destinations, about 5 x 10^8 exponentials a month, far over budget. Choice probabilities depend on a pop only through its origin province and the attributes that enter V (education, ethnic group), so they are computed per (origin province, education, group) cell over a sparse choice set: provinces within a cost radius, the largest cities, and foreign gateways, with remaining destinations pooled in one nest per country. Expected values follow the Caliendo-Dvorkin-Parro recursion, updated once per tick:

```latex
V_{o,t+1} = u_{o,t} + \nu \ln \sum_{d} \exp\left( \left( \beta V_{d,t} - m_{od} \right) / \nu \right)
```

so value iteration is spread over ticks like trade clearing (8.3), and its fixed point is the CDP steady state.

## 7. Production, Enterprises and Technology

Production follows the OECD input-output structure at province level, and technology is sector productivity that grows at the frontier and diffuses to followers; there are no named technologies.

### 7.1 Goods and sectors

The 45 industries of the OECD Inter-Country Input-Output tables define the goods, with a few splits where physical quantities matter for prices, food security or emissions.

| Group | Sectors (examples) | Splits |
| --- | --- | --- |
| Primary | Agriculture, forestry, fishing; energy mining; non-energy mining | Grains, other crops, livestock; crude oil, natural gas, coal |
| Manufacturing | Food, textiles, chemicals, pharmaceuticals, basic metals, electronics, machinery, motor vehicles, other transport | None initially |
| Energy and utilities | Electricity, gas, water | Fossil, nuclear and renewable generation |
| Construction | Construction | None |
| Services | Trade, transport, IT and telecom, finance, real estate, professional, public administration, education, health | Traded through frictions, not shipments |

Each good has weight per unit of value, perishability and a consumption category; services are mostly local, with cross-border service trade handled as frictions on demand rather than physical shipments.

### 7.2 Production function

Each (province, sector) enterprise combines a CES value-added core with intermediate inputs. Intermediates are Leontief in a non-energy bundle, initialized from the country's ICIO coefficients, plus a CES energy nest so firms can substitute between fuels and electricity:

```latex
Y = \min\left( \frac{VA}{a_{VA}}, \; \frac{M}{a_{M}}, \; \frac{E}{a_{E}} \right), \qquad VA = A_{s,i,t} \left[ \alpha_K K^{\rho} + \alpha_L L^{\rho} + \alpha_T T^{\rho} \right]^{1/\rho}
```

L is effective labour by education level; T is land (agriculture) or resource deposits (mining). A combines sector productivity, province agglomeration (rising with local employment density) and climate damages.

**Putty-clay intermediates (ADR 0020).** The coefficients a belong to installed capacity, not to the sector. Each capacity vintage fixes its input coefficients when it is built, by cost minimization over a CES with a long-run elasticity (register P38) at expected prices; once installed it runs Leontief. Short-run complementarity matches the near-zero elasticities measured for intermediates (Atalay 2017; Boehm, Flaaen and Pandalai-Nayar 2019) and keeps rationing along supply chains. Long-run substitution lets the input-output structure drift from its 2000 values over decades instead of staying frozen to 2150, and gives large shocks the non-linear propagation studied by Baqaee and Farhi (2019). Vintages are aggregated per enterprise into a capacity-weighted coefficient vector, so per-tick cost is unchanged; the choice happens at the quarterly investment step (7.4). Initial capacity is one vintage with the 2000 ICIO coefficients.

**Firms within a cell (ADR 0026).** Each (province, sector, ownership) enterprise is a mass of firms with Pareto-distributed productivity, shape k\_g, above a cell-specific lower bound, selling CES varieties with elasticity sigma\_g (k\_g > sigma\_g - 1), as in Melitz (2003) and Chaney (2008). The firm count, size distribution and exporter share of each cell are closed form, so the extensive margin of trade and a firm-size target come at no per-tick cost. The quarterly investment step (7.4) becomes free entry and exit in the mass of firms.

### 7.3 Enterprise behaviour (monthly)

1. Forecast demand with the heuristic expectation rules of Section 12.
2. Set an output target from forecast demand and an inventory-coverage rule (as in data-driven ABMs such as Assenza-Delli Gatti-Grazzini 2015 and Poledna et al. 2023).
3. Post vacancies or lay off; a province matching function pairs vacancies with job seekers by education.
4. Buy inputs at province prices; shortfalls cap output, so rationing propagates through the input-output network (the supply-chain mechanism behind 2021-22 shortages).
5. Sell output at the province market price; profits go to owners.

### 7.4 Investment, entry and exit (quarterly)

Owners (households via wealth, states, banks, foreign investors) allocate new capital across (province, sector) options by a logit over expected return net of risk, compared to the cost of capital (a Tobin-q style rule), financed from retained earnings, bank credit, bonds and equity. Persistent losses with negative equity trigger exit. Foreign ownership is simply foreign-owned capital, so FDI and profit repatriation are ordinary flows.

### 7.5 Technology as sector productivity

Technology has three parts, all at the sector level:

- **Frontier growth.** Each sector's frontier productivity grows with world research effort, with diminishing returns as in Jones (1995):

```latex
g^{*}_{s,t} = \delta_s \, R_{t}^{\lambda} \, \left(A^{*}_{s,t}\right)^{\phi - 1}, \qquad \phi < 1
```

Because growth depends on research effort, a shrinking or aging world population slows frontier growth, the semi-endogenous mechanism studied by Jones (2022). This keeps open-ended runs from exploding or stalling arbitrarily.

- **Catch-up.** Provinces converge toward the frontier at a rate that rises with education, trade exposure to frontier producers (Buera and Oberfield 2020), foreign ownership and institutional quality:

```latex
\Delta \ln A_{i,s,t} = g^{*}_{s,t} + \lambda_{i,t} \left( \ln A^{*}_{s,t} - \ln A_{i,s,t} \right) + \varepsilon_{i,s,t}
```

- **Learning by doing in energy.** Renewable generation and storage-intensive sectors follow Wright's law: unit costs fall with cumulative global output, the pattern documented for solar, wind and batteries by Way et al. (2022). This captures the energy transition without naming technologies.

Initial productivity gaps are backed out from 2000 output, inputs and prices; frontier growth rates per sector are calibrated to productivity accounts. A share of frontier growth arrives as fat-tailed sector jumps with the same mean (ADR 0242).

### 7.6 Structural change

Non-homothetic demand (Section 8.6) and unequal sector productivity growth (Baumol cost disease in services) jointly drive sector shares. Targets: 2000-2024 sector employment and value-added shares by country.

### 7.7 Energy and emissions

Energy sectors produce physical energy; fossil inputs carry emission factors, so CO2 and other greenhouse-gas emissions are computed from the input-output flows every month and passed to the climate module (Section 13). Carbon prices and fuel taxes are policy wedges on those inputs.

## 8. Markets, Price Formation and Trade

Every province prices every good, with inventories and adjustment lags, and goods move between provinces by structural gravity at posted prices, with origin markets clearing across ticks through inventories, so the equilibrium limit is entropic optimal transport (8.3).

### 8.1 Market topology

Each of about 4,000 provinces holds a price p(i,g) and a merchant inventory S(i,g) per good. Political borders do not partition markets: they act through tariffs, sanctions and frictions in the cost function. Services are priced locally and traded across provinces only through demand-side frictions.

### 8.2 Price formation (stock-flow rule)

Each month, supply (local output plus arrivals) meets demand at the posted price; inventories absorb the gap; the price then moves with excess demand and with deviation from target inventory coverage:

```latex
\Delta \ln p_{i,g,t+1} = \pi^{e}_{i,t} + \kappa_1 \frac{D_{i,g,t} - Q_{i,g,t}}{Q_{i,g,t}} - \kappa_2 \frac{S_{i,g,t} - S^{*}_{i,g,t}}{S^{*}_{i,g,t}}, \qquad S^{*} = \bar{s}_g \, D_{i,g}
```

pi^e is the merchants' expected inflation for the currency area (Section 12.2). It makes the rule homogeneous: after a permanent change in money growth, prices drift at the new rate with zero real excess demand, so the long-run Phillips curve is vertical and money is neutral in the long run (ADR 0019). The other two terms form a proportional-integral controller on the inventory gap: excess demand is the flow, the inventory deviation its integral. The P2 spec derives the stability region in (kappa1, kappa2) for the linearized market given local demand and supply elasticities; the step clip is a registered parameter, and every tick it binds is counted.

When inventory runs out, demand is rationed within the tick, which is how shortages propagate. The scheme follows the dynamic input-output model of Pichler et al. (2022), whose rationing rules were tested against 2020 data: priority by default (households' essential consumption, then intermediate users by contract, then the rest), with proportional and mixed rationing as switch S11. A Walrasian mode solves clearing prices by Newton iteration each tick, for equilibrium-nesting tests and as a robustness switch.

### 8.3 Inter-province trade: structural gravity cleared across ticks

Within each good, buyers in province d spread spending across origin provinces by a logit over delivered prices (A8). This is the structural gravity system of Eaton-Kortum and Anderson-van Wincoop with sector trade elasticity theta\_g (ADRs 0015, 0018). Flows are in value, and the kernel is built from iceberg factors, not from costs in currency:

```latex
X_{od,g} = \frac{K_{od,g} \, p_{o,g}^{-\theta_g}}{\sum_{k} K_{kd,g} \, p_{k,g}^{-\theta_g}} \, E_{d,g}, \qquad K_{od,g} = \tau_{od,g}^{-\theta_g}
```

E(d,g) is province d's expenditure on good g at posted prices (households, intermediate users, investment and government), and the denominator is the price-index term P(d,g)^(-theta). The kernel is dimensionless and prices enter only as ratios, so flows are homogeneous of degree zero in nominal prices and exchange rates: inflation or a currency move does not change trade dispersion. Quantities shipped are X / (p\_o tau); per-unit costs (5.4) are charged to the shipment. With firms heterogeneous within cells (ADR 0026), theta\_g is the Pareto shape of productivity and fixed export costs multiply the kernel on gateway links by f^(1 - theta/(sigma - 1)), so its form and factorization are unchanged.

**Clearing across ticks (ADR 0016).** Each tick applies the destination-side normalization above once, at posted prices. Origin sales are not forced to equal origin supply within the tick: the gap goes into the origin's merchant inventory and moves p\_o through the rule of 8.2 at the next tick. A destination normalization followed by an origin price adjustment is one Sinkhorn sweep, so the price dynamics are Sinkhorn spread over time. With adjustment speeds high and lags zero, the fixed point is the doubly constrained entropic transport solution, which is the structural-gravity equilibrium (Galichon 2016; Allen, Arkolakis and Takahashi 2020). The frictionless limit is therefore the GE reference model by construction, and the within-tick work is one kernel product per good class instead of 5 to 10 full Sinkhorn iterations.

Per-unit costs make tau depend on the origin price (tau = 1 + c / p\_o); the kernel uses last tick's prices, which adds no iteration. Supply that cannot be sold at the posted price stays in inventory, which replaces the unbalanced-transport relaxation. The full within-tick Sinkhorn solve (balanced, or unbalanced as in Chizat et al. 2018) stays available as structural switch S02 for nesting tests.

**Computation.** The kernel factorizes through about 600 gateways (ports, border crossings, airports), because every international route passes through a gateway pair. Under the least-cost legs P2 keeps (S05) the factorization approximates the path minimum; under route choice (5.4, deferred) it is exact, because route-choice costs sum over routes:

```latex
K_{od} = \mathbb{1}[o, d \text{ same country}] \, K^{\text{dom}}_{od} + \sum_{a,b} K^{\text{dom}}_{oa} \, K^{\text{int}}_{ab} \, K^{\text{dom}}_{bd}
```

Per tick and per (theta class, weight class) this is one sparse product with the province-to-gateway factor, one G x G product and one product back: O(P x k + G^2) with k gateways reachable per province, batched across goods in the class so the products are matrix-matrix (provinces x goods). Tariffs are constant within a country pair, so they scale blocks of the gateway product and are never materialized per good. The estimated cost is a few milliseconds per tick (19.2), to be replaced by the P2 benchmark.

### 8.4 Transit, shipping capacity and freight rates

Shipments arrive after the expected transit time over routes (5.4), so prices respond to news with realistic lags. Shipping capacity (container fleet, bulk carriers, tankers, rail and truck fleets) is capital owned by enterprises; freight rates rise with utilization and fuel prices and feed back into c. Chokepoint closures and port disruptions are shocks on specific edges. Validation: container and dry-bulk freight-rate indices, including the 2021 spike.

### 8.5 Trade policy

Policy wedges enter c(o,d,g): MFN and preferential tariffs (free trade agreements, customs unions such as the EU), anti-dumping and safeguard duties, quotas, export controls, export bans (as on food in 2008 and 2022), and sanctions (embargoes, secondary-sanction compliance costs). Tariff revenue accrues to the importing state. Optimal-tariff and retaliation logic, as quantified by Ossa (2014), is available to state AI.

### 8.6 Demand system

Households allocate spending across three consumption categories, agriculture A (goods 0 and 3), manufacturing M (goods 1, 2, 4, 5 and 6) and services S (goods 7, 8 and 9), the ones Alder, Boppart and Muller (2022) estimate, with PIGL preferences in their multi-sector form (ADR 0021). PIGL generates Engel's law and the rising services share and, unlike non-homothetic CES, aggregates exactly: a pop's category shares depend on its income distribution only through the moment E\[y^(1-epsilon)\], which for the lognormal income of A3 has a closed form:

```latex
\mathbb{E}\left[ y^{1-\varepsilon} \right] = \exp\left( (1-\varepsilon)\,\mu + \tfrac{1}{2}(1-\varepsilon)^2 \sigma^2 \right)
```

Category demand per pop is therefore closed form, with no root-finding or quadrature. Non-homothetic CES (Comin, Lashkari and Mestieri 2021) stays as switch S03; under it each pop needs a Newton solve for its utility index and a 5-node Gauss-Hermite quadrature over its income distribution, warm-started from the last tick. The sigma of a pop's lognormal income comes from its country's 2000 Gini, WDI's where it exists, else SWIID's (ADR 0021).

Within each category a CES at P06 aggregates goods and origins. A subsistence floor for food and energy enters through rationing priority, not preferences. Governments, armies and investment add their own demand.

**Consumption and saving (ADR 0021).** Total spending follows a buffer-stock rule: pops consume out of expected permanent income and correct toward a target ratio of wealth to income,

```latex
C_{p,t} = m_p \, \bar{y}^{e}_{p,t} + \lambda_p \left( W_{p,t} - w^{*}_{p} \, \bar{y}^{e}_{p,t} \right)
```

with propensities differing by wealth bracket and labour status (register P37). The rule is disciplined by intertemporal MPCs, the response of spending in every later month to a one-off transfer (Fagereng, Holm and Natvik 2021; Auclert, Rognlie and Straub 2024), not only by impact MPCs; this is the main channel through which fiscal and monetary policy reach demand. With static pops in P2 the rule runs with m = 1 and lambda = 0 (spend current income), which closes the accounting loop; it is calibrated in P5 together with money and banks.

### 8.7 Factor markets

- **Labour**: per province and education level, wage posting with a matching function; wages move with expected inflation plus a term in the vacancy-unemployment ratio (ADR 0019), with downward nominal rigidity and minimum wages where legislated.
- **Land and housing**: rents clear demand by use; housing is a durable stock with construction lags, so house-price booms and busts are possible.
- **Capital**: allocated by investment and credit (Sections 7.4 and 9).

### 8.8 Currencies in trade

Merchants invoice in a currency chosen by a dominant-currency rule: most international trade is invoiced in dollars or euros regardless of origin, as documented by Gopinath and coauthors. Exchange-rate changes therefore pass through to prices according to the invoicing currency, not just bilateral rates.

Dominant-currency pass-through needs export prices that are sticky in the invoicing currency, and the market prices of 8.2 move every tick. P5 therefore adds a thin posted-price layer for exporters: each exporting enterprise posts a price in its invoicing currency and resets it with a Calvo hazard calibrated to export-price change frequencies. Without that layer, the pass-through and price-change-frequency targets of 15.7 are dropped.

## 9. Money, Finance and the Fiscal State

The financial system is a stock-flow-consistent network of balance sheets in which central banks set policy, commercial banks create money by lending, asset prices move with expectations, and crises propagate through bank, housing and sovereign exposures.

### 9.1 Sectoral balance sheets

Each economy has households, non-financial enterprises, banks, government, central bank and the rest of the world (all other states, endogenous). The balance-sheet and transactions-flow matrices of Godley and Lavoie (2007) are computed every tick and must sum to zero across rows and columns. Agent-level claims (loans, mortgages, bonds, deposits, equity, reserves) aggregate exactly to the sector matrices, the standard SFC-ABM construction (Caverzasi and Godin 2015). Initial balance sheets come from national financial accounts where available and are imputed elsewhere.

### 9.2 Monetary and exchange-rate regimes

| Regime | Examples in 2000 | Policy rule |
| --- | --- | --- |
| Floating, inflation targeting | United States, United Kingdom, Brazil | Taylor-type rule on inflation and output gap, effective lower bound, asset purchases when at the bound |
| Managed float / crawling peg | China, many emerging markets | Exchange-rate band defended with reserves and capital controls |
| Hard peg / currency board | Hong Kong, Gulf states | Rate set to defend the peg; reserves constrain |
| Monetary union | Euro area, CFA franc zones | One central bank for many states, no national devaluation |
| Dollarization | Ecuador, Panama | No independent monetary policy |

Regimes are initialized from the de facto classification of Ilzetzki, Reinhart and Rogoff and change through transition hazards on inflation, reserve cover and recent peg breaks; entering or leaving a monetary union stays at t0 until 2.0 (ADR 0242). Floating exchange rates are cleared in one place, the global demand system of 9.9: current-account and income flows enter it as net currency demand absorbed by financiers with limited risk-bearing capacity (Gabaix and Maggiori 2015 is that special case), and central banks intervene from reserves as one more investor class.

### 9.3 Banks and asset markets

Banks hold reserves, loans, mortgages, bonds and interbank claims against deposits, wholesale funding and equity. Lending creates deposits up to capital (Basel-style) and liquidity constraints; loan rates are the policy rate plus a risk premium from borrower leverage. Bank runs follow a threshold rule on depositor and wholesale-funder confidence, softened by deposit insurance; contagion runs through interbank exposures (Gai-Kapadia). Housing and equity prices are formed by agents valuing expected rents and earnings with heuristic expectations (Section 12), so leveraged housing booms and busts, the core of 2008, can emerge.

### 9.4 Sovereign debt, default and the IMF

States issue bonds in local or foreign currency. Spreads use a reduced-form default-risk function inspired by Eaton-Gersovitz and Arellano (2008):

```latex
s_{j,t} = f\left( \frac{\text{debt}}{\text{revenue}}, \; \text{deficit}, \; \text{foreign-currency share}, \; \text{reserves}, \; \text{institutions}, \; \text{default history}, \; \text{lender of last resort} \right)
```

Membership in a monetary union without a lender of last resort raises spreads under stress, the mechanism of the euro crisis. Default is a strategic choice of the state AI; consequences are market exclusion, restructuring with haircuts, and financial-sector losses wherever the bonds are held. The IMF is a stylized agent that lends with conditionality (fiscal targets) to states that request it.

Pricing and default are tied: the default probability that prices sovereign debt in 9.10 is the hazard implied by the state AI's own default rule, evaluated on the same state variables (Eaton-Gersovitz consistency), so spreads and realized defaults cannot drift apart. The function above is the prior for that rule, not a second pricing mechanism.

### 9.5 Capital flows and reserves

Savers and banks allocate across domestic and foreign assets through the demand system of Section 9.9, with home bias; central banks accumulate reserves in safe assets, which creates the demand for dollar and euro bonds. Sudden stops occur when risk appetite drops and foreign holders sell. Targets: current-account and net foreign-asset paths, and capital-flow reversals in 2008 and 2013.

### 9.6 Fiscal state

Revenue instruments: personal income tax, payroll and social contributions, VAT and sales taxes, corporate tax, tariffs, excises (fuel, tobacco, carbon), resource royalties and state-enterprise profits, seigniorage. Collected revenue is

```latex
R_{j,i} = \tau_{j,i} \times \text{Base}_{j,i} \times \kappa_{j,i} \times \text{compliance}_{j}
```

where kappa is fiscal capacity per instrument (income tax needs administration and formal employment; tariffs and royalties need little), built by investment and depreciating slowly, following Besley and Persson (2009). Informality shrinks the base. Spending: pensions, health, education, social transfers, defence, infrastructure, energy subsidies, interest. Initial tax rates, revenue and spending come from international government revenue and finance statistics. Deficits are financed by borrowing or monetary financing; budgets are set by the state AI under institutional rules (Section 10).

### 9.7 Financial markets: design stance

Asset prices are set once a month by clearing the portfolio demands of heterogeneous, constrained investor classes against the stock of every instrument in the balance sheets of 9.1; there are no order books, named firms or daily markets (consistent with 15.7). Fidelity comes from three sources: an exact who-to-whom map of claims, demand curves estimated from holdings data, and balance-sheet constraints that turn price falls into forced sales. Sections 9.8 to 9.14 refine 9.3 to 9.5.

Within a tick, clearing and constraints iterate to a fixed point: prices clear demands, losses hit capital and margins, constrained holders sell, and prices clear again. The loop runs a fixed number of rounds in a fixed order, so it obeys the determinism rules of 4.5. This inner loop, not a daily time step, carries fire sales, runs and sudden stops.

&#91;embedded content: one month of financial clearing · 6 steps, 1 feedback loop\]

Prices clear, losses hit balance sheets, and binding constraints force sales that re-enter the demand system; only when no constraint binds does policy act and the month close.

### 9.8 Instruments and holders

Each state carries about ten instrument classes, each split by currency of denomination, held by about ten investor classes at home and abroad. Every asset is someone's liability, so the who-to-whom matrix sums to zero each tick (a V0 invariant).

| Instrument | Issuer | Main holders | Initial stocks from |
| --- | --- | --- | --- |
| Currency, bank reserves | Central bank | Households, banks | Central bank balance sheets, IMF IFS |
| Deposits | Banks | Households, enterprises, government | National financial accounts (OECD, ECB, Fed Z.1) |
| Loans, mortgages | Households, enterprises | Banks, nonbank lenders | BIS credit to the non-financial sector |
| Sovereign bills and bonds (local and foreign currency, by maturity bucket) | Governments | Domestic banks, central banks, pensions and insurers, funds, foreign reserve managers | BIS debt securities; IMF sovereign investor base (Arslanalp and Tsuda) |
| Corporate and bank bonds | Enterprises, banks | Insurers, pensions, funds, foreign investors | BIS debt securities |
| Equity (listed and unlisted) | Enterprises, banks | Households, funds, pensions, foreign investors | Financial accounts, market capitalization series |
| Official FX reserves | Reserve-currency sovereigns and banks | Central banks | IMF IFS, currency shares from IMF COFER |
| Cross-border claims by currency | All sectors | Banks, portfolio investors, direct investors | BIS locational and consolidated banking statistics, IMF CPIS and CDIS, Lane and Milesi-Ferretti (2018) |
| Housing (real asset) | Not a claim | Owner-occupiers, landlords | Residential property price series, WID wealth shares |

Investor classes per state: households (by pop wealth bracket, portfolio shares from household finance surveys), enterprises, banks (archetypes in 9.11), insurers and pension funds, investment funds, leveraged funds, central bank and reserve manager, government and sovereign wealth fund. Foreign investors are the same classes of other states. Derivatives are not instruments; FX swaps enter only as short-term dollar funding of non-US banks, and credit derivatives are omitted.

### 9.9 Price formation: a global demand system

Prices clear a characteristics-based demand system (Koijen and Yogo 2019), extended to many currencies as in Koijen and Yogo (2020), so exchange rates and asset prices clear jointly. Investor class i with assets A puts portfolio weight w on asset a:

```latex
w_{i,a,t} = \frac{\exp(\delta_{i,a,t})}{1 + \sum_{b \in \mathcal{A}_i} \exp(\delta_{i,b,t})}, \qquad \delta_{i,a,t} = \gamma_i \, \mathbb{E}_{i,t}\left[ r_{a,t+1} \right] + \beta_i' x_{a,t} + \eta_{i,a}
```

The 1 in the denominator is the outside asset (deposits or cash). Characteristics x are currency, home or foreign issuer, default probability, duration, liquidity and mandate eligibility; an ineligible asset (a junk bond for a constrained insurer) gets zero weight. Expected returns come from the asset blocks of 9.10 and the expectations of 9.13. Each month, prices P solve:

```latex
P_{a,t} \, Q_{a,t} = \sum_i A_{i,t} \, w_{i,a,t}(P_t) \quad \text{for every asset } a
```

- **Inelastic markets.** Demand elasticities follow the inelastic markets hypothesis (Gabaix and Koijen 2021): a dollar of flow into equity moves market value by several dollars. Flows from QE, reserve managers, pension rebalancing or fire sales therefore move prices realistically.
- **Exact initialization.** The latent demand term eta is backed out so that 2000 holdings are reproduced exactly, as in Koijen and Yogo; it is data-determined, not a free parameter.
- **QE and reserve demand for free.** Central-bank purchases and reserve accumulation are just another investor's demand, so their price effects need no special mechanism; euro-area QE estimates (Koijen, Koulischer, Nguyen and Yogo 2021) are a validation target.
- **Solver.** Damped Newton on log prices with a block-sparse Jacobian (home bias keeps most weights within a state), a fixed iteration count and fixed reduction order. Each investor class's demand is restricted to slope downward in the asset's own price, as Koijen and Yogo impose, since extrapolative expectations (9.13) could otherwise make it slope upward and leave clearing without a unique solution. A residual counter per market, like the clip-binding counters, records every tick where the fixed iteration count leaves clearing error above tolerance. Cost scales with roughly 2,000 assets and 2,000 investor classes worldwide, small next to trade clearing (8.3).

### 9.10 Asset-class blocks

Each asset class supplies the demand system with a fundamental anchor and a premium driver, so prices stay tied to the simulated real economy while flows and constraints move them around that anchor.

| Asset | Fundamental anchor | Premium or spread driver | References |
| --- | --- | --- | --- |
| Sovereign yields, local currency | Expected policy-rate path from the rules of 9.2 | Term premium from duration supply against price-insensitive holders | Vayanos and Vila (2021) |
| Sovereign spreads | Default probability from 9.4 (debt to revenue, r minus g, foreign-currency share, reserves) | Liquidity; redenomination risk in unions; self-fulfilling rollover runs when refinancing needs are high | Calvo (1988); De Grauwe and Ji (2013); Lorenzoni and Werning (2019) |
| Equity, per state and sector group | Dividends from simulated operating surplus by sector (Section 7) | Discount rate that rises as intermediary capital falls | He, Kelly and Manela (2017) |
| Corporate and bank credit | Merton default probability from sector leverage and earnings volatility | Excess bond premium tied to intermediary capital | Merton (1974); Gilchrist and Zakrajsek (2012) |
| Housing, per province | Rents from province income and population; user cost | Credit limits (LTV, DTI), mortgage rates; supply elasticity from land and terrain (5.1) | Mian and Sufi (2009); Saiz (2010) |
| Exchange rates | Cleared jointly in 9.9; pegs and bands defended per 9.2 | Limited risk-bearing capacity of financiers; dollar convenience yield | Gabaix and Maggiori (2015); Jiang, Krishnamurthy and Lustig (2021) |
| Commodities | Physical balance and competitive storage (15.7) | Small financial-investor demand | Deaton and Laroque (1992) |

Housing is the one market cleared outside the demand system: each province clears owner and landlord demand against a slowly adjusting stock, and mortgages link it back to bank balance sheets.

### 9.11 Banks, contagion and the safety net

Each state has up to five bank archetypes rather than named banks: a global systemic bank (holding the cross-border book), a domestic universal bank, regional savings and cooperative banks (weighted by province), a state-owned bank, and nonbank lenders. Balance sheets come from financial accounts and BIS banking statistics.

- **Constraints.** Risk-weighted capital, leverage and liquidity coverage ratios. As buffers shrink, lending tightens and the bank's eligibility terms in 9.9 push it to sell risky assets.
- **Funding.** Insured and uninsured deposits, wholesale funding, and short-term dollar funding of non-US banks through FX swaps (McGuire and von Peter 2009; Ivashina, Scharfstein and Stein 2015), the channel of the 2008 dollar shortage.

Contagion runs through four channels, all resolved inside the monthly fixed-point loop of 9.7:

1. Direct exposures on the interbank and cross-border network (Gai and Kapadia 2010, as in 9.3).
2. Fire sales on common holdings (Greenwood, Landier and Thesmar 2015).
3. Funding runs: uninsured and wholesale funders withdraw past a threshold (Diamond and Dybvig 1983), with seeded noise keyed by the rules of 14.4 so ensembles show run probabilities, not certainties.
4. The sovereign-bank loop: home-biased holdings of own-government debt tie bank capital to sovereign spreads and bailouts to public debt (Brunnermeier et al. 2016; Farhi and Tirole 2018).

Safety-net levers in the state and central-bank action space: lender-of-last-resort lending against haircut collateral, deposit insurance limits, guarantees, recapitalization, bail-in or resolution, asset purchases, swap lines, and macroprudential tools (countercyclical capital buffer, LTV and DTI caps). Their costs feed the fiscal state (9.6) and spreads (9.4). These levers make the scenarios 'Lehman rescued' and 'the euro without a lender of last resort' plain decision overrides (14.2). Levers without a 0210 rule hold their t0 values and move through tightening and loosening hazards driven by simulated crises and the credit gap (ADR 0242).

### 9.12 Global financial cycle, the dollar and capital controls

One global risk-appetite state, an analogue of the VIX, correlates every market, so stress spreads across borders the way the global financial cycle does (Rey 2013; Miranda-Agrippino and Rey 2020).

- **Risk appetite.** Its innovations follow a stochastic-volatility process with fat tails, drawn from the run's random streams like weather and written to its event log (ADR 0241). Its level responds endogenously to US monetary policy and to intermediary losses, and it scales the return sensitivity of every investor class in 9.9. Crises themselves stay endogenous (13.6).
- **Dollar centrality.** Dollar invoicing, dollar debt of emerging-market sovereigns and firms, and reserve-manager demand for Treasuries give the dollar a convenience yield, so it appreciates in stress. Fed swap lines are a lever that relieves dollar funding of foreign banks.
- **Capital flows.** Gross flows emerge from portfolio rebalancing in 9.9; nothing is scripted. Sudden stops (Calvo 1998) and flow surges and retrenchments (Forbes and Warnock 2012) are validation targets, not mechanisms.
- **Capital controls and sanctions.** Controls are taxes or caps on foreign classes' weights in a state's assets, initialized from the capital-control measures of Fernandez et al. (2016). Sanctions, reserve freezes and payment-system exclusion (11.8) set eligibility to zero.
- **Pegs under attack.** A peg is defended with reserves and rates (9.2). As reserves fall relative to short-term external debt, investor demand for the currency weakens and the state AI weighs abandoning the peg; the Argentine and Turkish crises of 2001 are test cases.

### 9.13 Expectations and endogenous crises

Financial expectations reuse the heuristic switching of 12.2, with one addition: a diagnostic rule that over-weights recent good news (Bordalo, Gennaioli and Shleifer 2018), which lets credit booms build and reverse without scripted shocks. Weights are disciplined by survey data: return expectations that extrapolate past returns (Greenwood and Shleifer 2014), plus SPF and Michigan inflation expectations.

No crisis is scripted. Each type below emerges from the mechanisms above and is scored as a frequency across ensembles, never as a date.

| Crisis type | Built-in trigger | Propagation | Validation cases, 2000-2022 |
| --- | --- | --- | --- |
| Banking | Capital depletion after rapid credit growth (Schularick and Taylor 2012) | Runs, fire sales, credit crunch | United States, United Kingdom, Ireland, Iceland 2008; Cyprus 2013 |
| Sovereign debt | Spread spiral when rollover needs are high and no lender of last resort stands behind the debt | Bank losses through the sovereign-bank loop; austerity through 9.6 | Argentina 2001; Greece 2010-2012; Lebanon 2020; Sri Lanka 2022 |
| Currency | Reserves low against short-term external debt; dollar-debt mismatch | Depreciation raises the burden of dollar debt | Turkey 2001 and 2018; Russia 2014; Argentina 2018 |
| Sudden stop | Drop in global risk appetite; foreign investor classes exit | Credit crunch, current-account reversal | Emerging markets 2008-09; taper tantrum 2013 |
| Housing bust | Leveraged price boom with extrapolative expectations | Mortgage defaults; consumption falls through household balance sheets | United States 2006-2009; Spain and Ireland 2008 |

Crisis frequencies and types by region are scored against the systemic banking crisis database of Laeven and Valencia (2018).

### 9.14 Calibration, validation and build order

Finance gets at most 12 of the 60 free behavioural parameters (Section 17): run thresholds, fire-sale impact, the diagnostic weight and the risk-appetite process. Demand elasticities are literature-anchored, and latent demand and initial balance sheets are data-determined.

**Targets**

- Yield-curve levels and term premia (Adrian, Crump and Moench 2013), sovereign spread distributions.
- Equity and FX volatility, cross-country return correlations, exchange-rate pass-through.
- Credit-to-GDP and house-price paths; gross capital flows and net international investment positions.
- Price effects of QE and reserve accumulation; crisis frequencies (9.13).

**Holdings data.** Commercial fund-flow and holdings data is not free, so investor holdings come from IMF CPIS, ECB securities holdings statistics and US Treasury TIC data. The Jorda-Schularick-Taylor Macrohistory database is likely non-commercial; use it for priors only and never bundle it. BIS, IMF and ECB terms for a commercial build are to be confirmed (Steam release plan).

**Build order.** Each stage passes the ablation test of 15.7 before the next starts.

1. Balance sheets and who-to-whom matrices, international investment positions, policy rates and exchange-rate regimes, sovereign yields from expected policy paths.
2. The demand system and investor classes; equity, credit and FX clearing; province housing markets.
3. Bank archetypes, the monthly fixed-point loop, the four contagion channels and safety-net levers.
4. Global risk appetite, capital controls and sanctions, then validation of endogenous crises.

## 10. Institutions, Politics and Government

Politics runs in three layers: interest groups hold resources and preferences, parties assemble them into electoral coalitions, and institutions decide how much elections, money and arms each count for. Government types emerge from that institutional vector rather than being picked from a menu.

### 10.1 Institutional state vector

Each state carries continuous indices in \[0, 1\], initialized directly from V-Dem for 2000:

| Dimension | Meaning | V-Dem / other anchor |
| --- | --- | --- |
| Electoral competition | Free and fair elections, office changes hands | Clean elections, electoral democracy index |
| Franchise and participation | Who can vote | Suffrage |
| Executive constraints | Legislative and judicial limits | Judicial and legislative constraints |
| Rule of law / property security | Expropriation risk, contract enforcement | Rule of law |
| Bureaucratic quality | Merit vs patronage administration | Rigorous and impartial public administration |
| Civil liberties | Press, association, protest rights | Freedom of expression, association |
| Centralization | Unitary vs federal authority, regional autonomy | Regional autonomy indicators |
| Corruption | Diversion of public resources | Political corruption index |
| Minority inclusion | Equal treatment across groups | Ethnic Power Relations status |

Laws and policies (tariffs, taxes, minimum wage, immigration rules, central bank independence) are separate variables governed by these institutions.

### 10.2 Government types as derived labels

A classifier maps the vector to Regimes of the World categories (Luhrmann, Tannenberg and Lindberg 2018): closed autocracy, electoral autocracy, electoral democracy, liberal democracy; plus head-of-state form and executive type (presidential, parliamentary, semi-presidential, monarchy, military, one-party). Labels never drive mechanics.

### 10.3 Political structure

States contain first-level provinces with autonomy levels (federal units, autonomous regions), and may belong to supranational bodies with delegated powers: the EU holds trade policy and, for euro members, monetary policy; other unions hold narrower competences. Dependencies and de facto states (for example Taiwan, Kosovo after 2008) are represented with their actual status as an attribute.

### 10.4 Interest groups and power

Interest groups (business by sector, finance, organized labour, public-sector workers, farmers, pensioners, security forces, religious organizations, ethnic blocs, urban professionals) draw members from pops by labour status, education, sector, identity and ideology. Political power shares come from a contest success function:

```latex
\pi_g = \frac{w_g \, x_g^{m}}{\sum_h w_h \, x_h^{m}}, \qquad x_g = \text{wealth}_g^{a} \cdot (\text{members}_g \cdot \text{organization}_g)^{b} \cdot \text{arms}_g^{c}
```

Institutional weights w translate resources into influence: votes count through elections, money through lobbying and patronage (heavier with high corruption and weak constraints), arms through the security forces' capacity to intervene.

### 10.5 Parties and elections

- **Parties** are platforms in a policy space (economic left-right, cultural liberal-conservative, nationalism and sovereignty, ethnic or religious appeal) backed by a coalition of interest groups. Initial parties, positions and vote shares come from open party datasets (ParlGov, V-Party), with stable generic names rather than real ones in v1.
- **Voting**: pops vote by spatial proximity to platforms, party attachment, and retrospective economic voting (incumbents punished for unemployment and inflation), with turnout tied to education and mobilization.
- **Electoral systems**: plurality, two-round, proportional (D'Hondt, Sainte-Lague) and mixed, initialized per country, converting votes to seats.
- **Government formation**: majority winners govern; otherwise minimal connected winning coalitions form by bargaining over portfolios and policy.
- **Party system dynamics**: parties shift platforms toward vote-rich positions with inertia; new parties enter when a large ideological mass is unrepresented (the channel for populist entry); parties split and merge.
- **Calendars**: election dates follow constitutional terms from 2000 data; snap elections occur when governments lose confidence.

### 10.6 Policy choice

| Setting | Mechanism | Source |
| --- | --- | --- |
| Democracies | Governing party or coalition maximizes its vote and coalition-partner support, subject to veto players | Probabilistic voting (Persson-Tabellini 2000); veto players |
| Autocracies | Leader maximizes survival given winning coalition W and selectorate S; small W favours private goods | Bueno de Mesquita et al. (2003) |
| Legislatures | Proposer-veto bargaining; bills progress monthly and may fail | Design choice |

### 10.7 Legitimacy, ideology and grievance

Pop approval combines material conditions relative to expectations, policy distance from preferred platforms, group exclusion, corruption, war weariness and repression. Ideology evolves with education, age, economic conditions and media freedom, and diffuses through bounded-confidence opinion dynamics (Hegselmann-Krause), which can generate polarization.

### 10.8 Unrest and regime change

- **Protest**: mobilization follows a threshold model (Granovetter 1978) with preference falsification (Kuran 1991), so protest waves can cascade across neighbouring states, the pattern of the Arab Spring.
- **Structural pressure**: a Political Stress Index in the Goldstone-Turchin tradition (mass mobilization potential, elite competition, state fiscal distress) raises instability hazards.
- **Outcomes**: concession and reform, repression, democratic breakdown or backsliding (executive aggrandizement eroding constraints gradually), coups (calibrated to coup datasets), revolution, civil war (Section 11), secession (by referendum or conflict).
- **Calibration targets**: V-Dem index paths 2000-2024, frequencies of coups, mass protest campaigns and regime transitions.

## 11. Diplomacy, Conflict and War

Interstate war is modeled as a failure of bargaining under private information and commitment problems, made rare by nuclear deterrence and economic interdependence; civil conflict, the dominant form of organized violence since 2000, grows from group grievance, weak state capacity and opportunity.

### 11.1 Diplomatic state

Per dyad: relations, trust (track record of honoured commitments), rivalry flag, territorial claims, and treaties: defence alliances (NATO-style collective defence), non-aggression and security partnerships, free trade agreements, customs and monetary unions, sanctions regimes, arms-supply relationships. Membership of international organizations (UN Security Council permanent seats, WTO, IMF) is an attribute with procedural effects (vetoes, dispute settlement, lending access).

### 11.2 Crisis bargaining

Disputes arise from territorial claims, ethnic kin across borders, resource disputes, security dilemmas, regime-change motives or alliance obligations. Each side holds a noisy estimate of its probability of winning p and of costs; the bargaining range follows Fearon (1995):

```latex
x \in \left[ \hat{p}_A - \hat{c}_A, \; \hat{p}_A + \hat{c}_B \right]
```

War occurs when private estimates leave no acceptable offer, when a projected power shift gives the declining side a preventive motive (Powell 2006), or when the stake is treated as indivisible. Costs include casualties, economic losses through trade and finance, sanctions, and for nuclear dyads an expected catastrophic-escalation term that makes direct war between nuclear powers extremely rare but lets proxy conflicts and limited wars occur. Power-estimate noise is calibrated to war-onset frequencies.

### 11.3 Alliances and interdependence

Alliance formation trades autonomy for security (Morrow 1991); reliability is learned from behaviour. Economic interdependence enters the cost of war: bilateral trade raises the opportunity cost of fighting a partner, while broad multilateral openness reduces dependence on any one partner (Martin, Mayer and Thoenig 2008). Both channels are in the model and tested (Section 18).

### 11.4 Military capability

Capability is computed in-model from military personnel, military spending, defence-industrial output (arms, aerospace, shipbuilding sectors), energy and population, and compared to Correlates of War capability data and military-spending series. Forces: land formations, air power, navies, missiles and air defence as aggregate capability stocks with readiness, equipment bought from domestic or foreign arms producers, and recruitment by law (conscription or volunteer). Nuclear status is a state attribute.

### 11.5 Combat, logistics and occupation

- **Battles**: Lanchester attrition with modifiers for terrain, fortification, air superiority, precision munitions and morale; loss-exchange ratios calibrated to modern battle data where available.
- **Logistics**: forces draw fuel, ammunition and food over the transport graph; interdicted supply lines raise attrition.
- **Occupation**: transfers control and taxation, disrupts production and trade, raises local grievance and insurgency risk.
- **Naval and air**: sea control and air superiority set blockade and strike risk on transport edges and infrastructure.

### 11.6 Civil conflict and insurgency

Civil war risk per province rises with group exclusion and horizontal inequality (Cederman, Gleditsch and Buhaug 2013), weak state capacity and rough terrain (Fearon and Laitin 2003), lootable resources, neighbouring conflict, and youth unemployment. Rebel factions are proto-states controlling provinces, financed by resources, diaspora and foreign sponsors. Conflict intensity is validated against georeferenced conflict-event data (UCDP GED) at province level, which is unusually fine-grained for a simulator.

### 11.7 The war economy and displacement

Mobilization removes workers; procurement shifts demand to arms, fuel and transport; war finance runs through debt and money creation; casualties reduce cohorts; capital and infrastructure are destroyed in contested provinces; refugees flow to neighbours (Section 6.5). War weariness feeds approval.

### 11.8 Termination, peace and coercion

Bargaining continues during war (Wagner; Slantchev 2003): battle outcomes update beliefs, and settlements occur when the range reopens; frozen conflicts are a possible outcome. Terms: territory, demilitarization, autonomy, reparations, regime change. Economic coercion (embargoes, financial sanctions, reserve freezes, export controls, exclusion from payment systems) uses the trade and finance modules directly; the 2022 sanctions on Russia are a validation case for trade rerouting and price effects.

## 12. Agent Decision-Making

Decisions are made in tiers of increasing cost and decreasing frequency: cheap behavioural rules for millions of pop and enterprise choices, utility-scoring planners for states, and optional learned or language-model policies for rare, high-stakes state decisions.

### 12.1 Tiers

| Tier | Agents | Method | Frequency | Status |
| --- | --- | --- | --- | --- |
| 0 | Pops, enterprises, merchants, banks | Behavioural rules with heuristic expectations | Monthly | Core, required |
| 1a | States, interest groups, parties | Utility-scoring planner (Paradox-like weights, but derived from coalition preferences) | Monthly / quarterly | Core, required |
| 1b | States | Learned policies (multi-agent RL) trained on a fast surrogate | Quarterly | Phase 10 |
| 2 | State leaders | LLM proposals for crises, treaties, war and peace, validated and logged | Event-driven, rare | Deferred beyond v1 |

### 12.2 Expectations (Tier 0)

Agents forecast prices, wages and demand using a set of simple heuristics (adaptive, trend-following, anchoring to a long-run mean) and switch between them by recent forecasting performance, the heuristic-switching model of Brock and Hommes (1997), whose parameters have laboratory estimates (Anufriev and Hommes 2012). The intensity of choice controls how quickly agents herd onto the best-performing rule, a known source of endogenous cycles.

### 12.3 State planner (Tier 1a)

The state objective is a weighted sum over regime survival, revenue, power, prestige, territorial goals and interest-group welfare, with weights equal to the ruling coalition's power shares from Section 10.4. Policy modules (budget, taxes, tariffs, investment, laws, diplomacy, military) each score a small set of candidate actions with a one-step model of their effects (using local elasticities estimated online from the simulation itself) and pick the best feasible one, with inertia and noise. This keeps state AI cheap, legible (the UI shows the score breakdown for every decision), and responsive to who holds power.

### 12.4 Learned policies (Tier 1b)

Following the two-level reinforcement-learning design of the AI Economist (Zheng et al. 2022), state policies can be trained against populations of other state policies on a fast surrogate of the full model (a neural emulator of monthly macro transitions per state). Learned policies address the Lucas critique: behaviour adapts when the policy regime changes. They are evaluated against Tier 1a on historical plausibility metrics before being enabled.

### 12.5 Language-model leaders (Tier 2)

LLM agents are an active research area for economic simulation, from EconAgent (Li et al. 2024) to multi-agent macro simulators reproducing Okun and Phillips relations. In Ceteris Paribus they are deferred beyond v1 (decision of 23 Oct 2026). If added later, they would cover rare decisions where qualitative reasoning matters (crisis diplomacy, treaty drafting, peace terms), under these rules:

- The LLM receives a structured situation report and returns a structured action from the same action space as Tier 1a; a validator rejects infeasible actions.
- **Anonymization**: states, people and dates are renamed (for example 'State 17, year 64') so the model cannot pattern-match to real history and railroad outcomes it knows happened.
- **Determinism**: decisions are cached by a hash of the situation report and recorded in the event log, so replays and branches are reproducible without re-querying.
- **Known limits**: Lin et al. (2025) find LLM agents' expectations more homogeneous than human respondents', and outputs are prompt-sensitive; Tier 2 results are always compared against Tier 1a baselines.

### 12.6 Steward mode

Steward mode lands in 2.0-alpha; 1.x exposes no seat (ADR 0239). In Steward mode the user replaces Tier 1a for one state. The same action space is exposed through the UI, so anything the user can do, the AI can do, and vice versa.

## 13. Climate, Shocks and Stochastic Processes

Climate is an endogenous feedback loop (the economy emits, temperature responds, damages hit the economy). After t0 a default run is calendar-blind: every other event comes from a hazard on simulated state, drawn by thinning on uniforms that branches share (ADRs 0241, 0242). Observed history enters only as an optional set of interventions.

### 13.1 Default runs and replay history

| Use | Content | Purpose |
| --- | --- | --- |
| Default | Hazards and fitted generators only, with climate-shifted weather once climate is on; commitments in force at t0 (tariff phase-ins, euro banknotes, projects under construction) are state with completion hazards | Ensembles, open-ended futures, unconditional scorecard |
| Replay | `replay-2000-2024`, applied whole or by family (shocks, policy): observed weather, pandemic seedings, disasters, chokepoint closures, policy and regime changes, named projects, discoveries | Calibration, natural experiments (15.5), conditional scorecard |
| Edits | User interventions on either | Counterfactuals ('no COVID', 'Suez closed for a year') |

Replay is a set of interventions (14.2), logged like any user edit, and never part of a default run (ADR 0241).

### 13.2 Climate emulator

The emulator and damages ship in 1.1; 1.0 computes emissions only (ADR 0207).

1. **Emissions**: CO2, methane and nitrous oxide from fossil energy use, industry, agriculture and land-use change, computed from simulated activity (Section 7.7).
2. **Global response**: a reduced-form carbon cycle and temperature model in the style of FaIR (open source), initialized to observed 2000 concentrations; it runs annually and costs microseconds.
3. **Regional climate**: province temperature and precipitation anomalies by pattern scaling of global mean temperature, using patterns from climate-model ensembles, plus interannual variability from the weather generator.
4. **Damages**: non-linear temperature effects on crop yields (Schlenker and Roberts style), on labour productivity and output (Burke, Hsiang and Miguel 2015), on mortality (Carleton et al. 2022), sea-level rise removing coastal land and raising flood risk, and more frequent extremes. Whether temperature lowers output levels or growth rates is switch S10, not a fixed prior: growth effects (Burke, Hsiang and Miguel 2015) dominate every run past 2050, and the evidence is divided (Newell, Prest and Sexton 2021; Bilal and Kanzig 2024). The reference model for P8 is the spatial integrated assessment model of Cruz and Rossi-Hansberg (2024), which resolves warming, migration and trade at a comparable spatial scale; P8 compares damage incidence by province with it, as P2 compares trade with the GE reference.
5. **Policy levers**: carbon prices, fuel taxes and subsidies, efficiency standards and clean-energy subsidies act through prices and sector productivity; adaptation spending reduces damage functions.

Validation: 2000-2024 emissions by country and global temperature; beyond 2024, emission and temperature paths are compared to IPCC scenario ranges as a plausibility check, not a target.

### 13.3 Disease

Pandemics start from a per-province zoonotic spillover hazard on livestock, forest edge, density and wildlife trade, with traits drawn from a prior over past pathogens; non-pharmaceutical interventions and vaccine arrival follow fitted rules (ADR 0242). Each pathogen class runs as a metapopulation SEIR model on the transport and air-travel network, so pandemics travel along trade, travel and migration routes. Health spending, vaccination and non-pharmaceutical interventions (lockdowns, border closures) are policy levers whose economic effects run through labour supply, sector demand and trade, as in the COVID application of Poledna et al. (2023).

### 13.4 Disasters and resources

Earthquakes, tropical cyclones and floods are drawn from hazard maps (with climate-scaled frequencies for weather extremes) and destroy capital and housing in affected provinces. Resource discoveries and extraction-technology gains (for example the shale boom) come from exploration on simulated prices (ADR 0178) and appear as productivity and reserve shocks in the energy-mining sectors.

### 13.5 Political hazards

Leader death uses age-specific mortality; assassinations and terror attacks have low base hazards scaled by radicalization and conflict. These are the only personal events in the model.

### 13.6 Endogenous vs exogenous

Financial crises, recessions, protest waves, wars and famines are endogenous outcomes and never sit in replay. Users can still force them as interventions (Section 14), which is recorded as such.

## 14. Counterfactual Engine: Ex Ante and Ex Post Interventions

Every experiment is a branch: a parent run plus an ordered list of timed edits, replayed under the same random streams across an ensemble of seeds, so the reported effect of an intervention is a paired distribution rather than an anecdote.

### 14.1 Objects

| Object | Definition |
| --- | --- |
| Scenario | Data pack version + initial state + parameter vector + optional replay set (ADR 0241) |
| Branch | Scenario + ordered interventions \[(tick, edit)\], with a parent branch and fork tick |
| Run | Branch x seed |
| Ensemble | Branch x K seeds (default K = 32) |
| Experiment | Set of branches compared against a baseline, with declared outcome metrics |

### 14.2 Edit types

| Type | Example | Timing |
| --- | --- | --- |
| Parameter | Trade elasticity 5 to 8; migration cost x 0.5 | Ex ante or ex post |
| State (god mode) | Transfer a province; add 1M pops; destroy a fleet; set literacy | Ex ante or ex post |
| Policy (in-world) | The United States does not impose the 2018 steel and aluminium tariffs | Ex post (as a forced decision) |
| Event injection | Declare war; assassinate a ruler; discover gold in a province | Ex post |
| Decision override | Force a state AI's choice at one decision point, or lock a law | Ex post |
| Event or replay | Remove the run's pandemic; add a multi-year US Midwest drought | Ex ante or ex post |
| Module switch | Walrasian vs inventory pricing; Tier 1a vs Tier 2 AI | Ex ante |

In-world edits are distinguished from god-mode edits in the log, because only in-world edits are things an agent could have done.

### 14.3 Snapshots and replay

Full state snapshots are written every 12 ticks as compressed columnar chunks, content-addressed so unchanged tables are shared across branches. Reaching any tick means loading the nearest earlier snapshot and replaying deterministically. Forking at tick t costs one snapshot load. A null intervention must reproduce the parent bit for bit (a CI test).

### 14.4 Common random numbers

Because each random draw is keyed by (seed, tick, system, entity, index), entities untouched by an intervention receive identical draws in parent and child runs, which sharply reduces the variance of estimated treatment effects. Entities created after the fork get IDs derived deterministically from (creator ID, tick, kind, ordinal) so equivalent entities in both branches stay aligned for as long as histories remain similar.

### 14.5 Effect estimation

For outcome Y and seed k, the paired effect is the difference between branch and baseline outcomes for that seed. Reported: mean effect with bootstrap intervals, quantiles, and event probabilities (for example, the probability that two states go to war before 2035 in each branch). Maps show the mean difference per province, masked where the sign is not robust across seeds.

### 14.6 Predictability horizons

For each variable the engine measures when across-seed dispersion of the baseline reaches its long-run spread, analogous to weather predictability. Interventions are only interpretable for outcomes within their horizon; beyond it the right summary is a shift in distributions, not a path. Expected pattern (to test): aggregate world output and trade predictable for decades, borders and war dates for only years.

### 14.7 Attribution and mechanisms

- **Multiple interventions**: Shapley decomposition over intervention subsets (exact up to five interventions, sampled beyond).
- **Channel shutdown**: rerun the branch with one mechanism frozen at baseline values (prices, migration, politics, finance) to measure how much of an effect flows through it, a simulation analogue of mediation analysis.
- **Parameter sensitivity**: Morris screening, then Sobol indices (Saltelli et al.) computed on an emulator of the model to keep the cost manageable.
- **Equilibrium comparison**: for trade, tariff and migration-cost interventions, the engine also runs the dynamic hat-algebra reference model (Caliendo-Dvorkin-Parro 2019; Kleinman-Liu-Redding 2023) from the same initial data. Where agent and equilibrium responses diverge, the UI flags the gap and the channel-shutdown tool identifies whether disequilibrium, finance or politics explains it.

### 14.8 Scripting API

Everything in the UI is available from scripts through the `ceteris` CLI driven by a scenario file (ADR 0113), for example: create a branch from a baseline at a tick, apply edits, run an ensemble. Runs write their outcome tables as Arrow IPC and Parquet, which Polars, DuckDB or any other language reads for analysis without glue code. A thin Python client that only drives runs and reads Arrow is added only if notebook-driven runs prove necessary, under its own ADR.

## 15. Calibration, Estimation and Validation

Parameters are set in three layers (data, literature priors, estimation), estimated on 2000-2010 with simulation-based Bayesian inference module by module, and validated out of sample on 2011-2024, including the model's responses to well-studied natural experiments.

### 15.1 Three-layer parameterization

This follows the data-driven ABM program: Poledna, Miess, Hommes and Rabitsch (2023) built the first ABM competitive with VAR and DSGE benchmarks in out-of-sample macro forecasting by initializing agents from national accounts, input-output tables and census data; Wiese et al. (2024) extended it to all 38 OECD countries with approximate Bayesian inference and beat both that model and AR(1) benchmarks out of sample.

1. **Data-determined**: initial populations, capital stocks, input-output coefficients, balance sheets, tax rates, borders, infrastructure, institutions. Not estimated.
2. **Literature-anchored**: structural elasticities with priors from published estimates. Estimated only within prior ranges.
3. **Free behavioural**: adjustment speeds, expectation weights, thresholds. Estimated. Target fewer than 60 in total (Section 17).

### 15.2 Estimation methods

| Method | Used for | Notes |
| --- | --- | --- |
| Modular calibration | Each module against its own targets, others held at data | Essential on one laptop |
| Method of simulated moments | Moments with clear targets (trade shares, unemployment, spreads, fertility) | Common random numbers across draws |
| Neural posterior estimation / ABC-SMC | Joint posterior of behavioural parameters | Simulation-based inference for ABMs as in Dyer et al. (2024), in Rust in `ceteris-calibrate` (ADR 0113) |
| History matching with GP emulators | Ruling out implausible regions first | A few hundred runs |
| Gradient-based variational inference | Behavioural parameters, through forward-mode derivatives of the Rust core (ADR 0096) | Same code as the golden runs; in Hamid et al. (2025) gradient-based variational inference reached in 300 evaluations a loss ABC had not reached in 30,000 |
| Direct structural estimation | Parameters whose estimating equation the model nests: theta\_g, border frictions (P05) and mode cost ratios (P04) by PPML gravity on BACI; migration dispersion (P16) by PPML on bilateral migration flows; Koijen-Yogo demand elasticities by their instrument | Outside the simulation (ADR 0022); frees the run budget for behavioural parameters |
| Multi-fidelity emulation and inference | Low-fidelity simulators (the European slice, gateway-coarsened worlds, short horizons) explore parameter space; full-globe runs correct them (Kennedy and O'Hagan 2000) | Cuts the number of full-globe runs the overnight budget must cover |

**Compute budget on the MacBook (estimate to verify in P0).** A headless 25-year run on one core is targeted at 2 to 3 minutes; running 10 to 12 in parallel gives roughly 250 to 350 runs per hour, about 3,000 overnight. Calibration is planned around that number: emulators and modular steps first, joint inference last.

### 15.3 Validation ladder

| Level | Test | Pass criterion |
| --- | --- | --- |
| V0 | Accounting invariants, determinism, null-branch identity | Zero violations in CI |
| V1 | Equilibrium nesting: frictions off, ABM steady state vs analytical GE; long-run neutrality of a permanent nominal shock | Prices and flows within 1% where the uniqueness condition holds (ADR 0022); real allocations unchanged in the long run; from P5, impulse responses to rate and transfer shocks within tolerance of the sequence-space HANK reference given matched intertemporal MPCs (ADR 0025) |
| V2 | Stylized facts | Gravity distance elasticity near the consensus around -0.9 (Disdier and Head 2008); Zipf's law for city sizes; Engel curves; Pareto firm-size tails within cells (ADR 0026); Okun's law; Phillips-curve and Beveridge-curve shapes |
| V3 | Out-of-sample 2011-2024 | See 15.4 |
| V4 | Natural-experiment replication | See 15.5 |
| V5 | Expert face validity | Structured review of baseline runs by economists and political scientists |

### 15.4 Out-of-sample targets (2011-2024)

| Target | Source | Metric |
| --- | --- | --- |
| GDP growth, inflation, unemployment by country | National accounts, IMF and World Bank series | Rolling-origin forecasts vs AR(1) and VAR (as in Poledna and Wiese) |
| Bilateral trade by sector | BACI, ICIO | PPML gravity coefficients, share RMSE |
| Subnational output and population | Subnational output data, gridded population | Log RMSE, rank correlation |
| Commodity prices | Commodity price series (oil, food, metals) | Volatility and co-movement |
| Migration stocks and flows | UN migrant stock, flow estimates | Flow RMSE |
| Sovereign spreads, defaults, banking crises | Market data, crisis databases | Distribution match |
| Emissions and temperature | National inventories, observational records | Path match |
| Regime indices, coups, civil conflict | V-Dem, coup data, UCDP GED | Frequencies and hazards by region, not dates |

Forecasts are scored with proper scoring rules (CRPS) and interval coverage: a 90% ensemble band should contain the realized series about 90% of the time. Political and military targets are scored on hazards and frequencies, never specific events. Validation also uses rolling origins: packs built for 2000, 2005, 2010 and 2015 each run forward and are scored on years outside the calibration window, so performance is judged across several starting points rather than one.

### 15.5 Natural-experiment replication

Each episode runs as a subset of replay (ADR 0241): the model is fed the historical shock and its response is compared with the published empirical estimate:

| Episode | Model test | Empirical benchmark |
| --- | --- | --- |
| China shock (2001-2010) | Manufacturing employment response to import exposure across US provinces | Autor, Dorn and Hanson (2013) |
| 2018-19 US-China tariffs | Import-price pass-through and trade diversion | Amiti, Redding and Weinstein (2019); Fajgelbaum et al. (2020) |
| 2008 financial crisis | Trade collapse and credit-crunch propagation | Great Trade Collapse literature |
| 2022 Russia sanctions and energy shock | Trade rerouting, European energy prices | Post-2022 trade data |
| COVID 2020 | Sector output collapse and recovery, supply-chain shortages | Sector data 2020-2022 |

### 15.6 Guarding against overfitting

Targets, holdout periods and natural experiments are fixed in a pre-registration file before estimation; every tuned parameter is in the register; models with more free parameters must beat simpler variants on holdout scores, not in-sample fit.

### 15.7 Accuracy and anti-overfitting

Accuracy comes first from how the model is fed and judged, second from targeted additions, and never from mechanisms that cannot prove their value out of sample.

**Methods (no new mechanisms)**

- **Data assimilation.** During the calibration window and at every rolling origin, the simulated state is nudged toward observed data with an ensemble Kalman or particle filter (sequential Monte Carlo for ABMs, as in Lux 2018), so forecasts start from data-consistent states. Each analysis step is followed by a projection through the consistency solver (23.2), so assimilated states satisfy the invariants of 4.6.
- **Posterior ensembles.** Ensemble members draw parameters from the posterior, so reported uncertainty includes parameter uncertainty, not only shock noise.
- **Model discrepancy.** Calibration includes an explicit discrepancy term (Kennedy and O'Hagan 2001) so parameters do not absorb structural error.
- **Stacking across structural switches.** Plausible module alternatives (17.2) run side by side and are weighted by out-of-sample skill (Bayesian stacking).
- **Two-way scorecard.** Validation runs from each origin with replay off (unconditional) and on (conditional); replay with and without its policy family separates economic-mechanism error from policy-rule error (ADRs 0210, 0241).

**Targets: micro facts over macro paths**

Many independent, well-identified moments constrain the model better than a few aggregate series: price-change frequencies of posted export prices (8.8), intertemporal marginal propensities to consume out of transfers, job-finding and separation rates, firm entry and exit rates, survey inflation expectations, and exchange-rate pass-through by invoicing currency. Targets span several scales at once, following pattern-oriented modelling (Grimm et al. 2005).

**Targeted additions**

- Wealth distribution per pop from distributional national accounts (World Inequality Database), for realistic spending heterogeneity and policy transmission.
- Competitive speculative storage for commodities (Deaton and Laroque), refining merchant inventories so food and energy price spikes are realistic.
- Cross-border bank claims (BIS locational banking statistics) to initialize foreign financial exposures.

**Benchmarks**

- Macro forecasts are scored against IMF World Economic Outlook forecasts as well as AR(1) and VAR baselines.
- Conflict hazards are scored against the ViEWS early-warning system (Hegre et al. 2019).

**Guardrails**

- **Ablation.** Every phase switches each module off and measures the change in out-of-sample skill; a mechanism that neither improves holdout scores nor passes a falsification test stays off by default.
- **Leave-one-region-out calibration.** Parameters are re-estimated without each world region and tested on it; parameters that only work where fitted are flagged.
- **Parameter freeze.** 2011-2024 data stays unseen until scoring.
- **Fuzz testing.** Random interventions and extreme shocks must never break invariants or produce non-finite values.
- **Documentation.** The model is described in the ODD protocol (Overview, Design concepts, Details; Grimm et al. 2020) so outsiders can review it.

**Excluded as bloat**

Named firms or individuals, sub-province geography, daily financial markets, power-grid dispatch, per-agent optimization, more sectors than the input-output tables, and LLM agents: each adds parameters faster than accuracy.

## 16. Data Sources and Pipeline

The 2000 pack is built from pinned sources; each is listed with its licence, evidence and release policy in the source manifest (crates/ceteris-pack/config/sources.toml, 98 sources). Non-commercial sources may be used because each user downloads them; sources that bar redistribution are fetched, never bundled. ADR 0005 lists the substitutions made in the P0 audit.

| Dataset | Content | Role | Licence | Bundle or fetch |
| --- | --- | --- | --- | --- |
| geoBoundaries | Admin boundaries | County layer, county-border splits | Per file; 28% of units share-alike | Fetch |
| Natural Earth | Coastlines, rivers | Map | Public domain | Bundle |
| Natural Earth admin-1 | First-level regions | Provinces | Public domain | Bundle |
| ETOPO 2022 | Elevation | Terrain, ruggedness | CC0 1.0 | Bundle aggregates |
| OpenStreetMap | Roads, rail, ports, pipelines | Transport graph | ODbL (share-alike) | Bundle under ODbL |
| WorldPop / GHSL | Gridded population, built-up area | Pops, urban share | CC BY 4.0 | Bundle aggregates |
| FAO GAEZ v4 | Crop suitability, yields | Land productivity | Possibly non-commercial | Fetch |
| OECD ICIO | Input-output, 45 industries, 1995-2022 (2025 edition), 80+ economies | Production, IO structure | OECD open terms | Fetch |
| CEPII BACI | Bilateral trade, product level | Trade | Open licence | Fetch |
| Subnational output (DOSE), gridded GDP | Regional output by sector | Province economies | CC BY 4.0 (likely) | Bundle aggregates |
| World Bank WDI | Macro, social indicators | Initialization, targets | CC BY 4.0 | Bundle extracts |
| UN World Population Prospects | Fertility, mortality, age structure | Demography | CC BY 3.0 IGO | Bundle extracts |
| UN international migrant stock; bilateral flow estimates | Migration | Migration | Varies | Fetch |
| Barro-Lee, Wittgenstein Centre | Education attainment | Pops | Varies | Fetch |
| ILOSTAT | Labour status | Pops | Open terms | Fetch |
| National financial accounts; IMF and BIS series | Balance sheets, rates, debt | Finance | Varies | Fetch |
| Exchange-rate regime classification (Ilzetzki-Reinhart-Rogoff) | De facto regimes | Monetary module | Free for research | Fetch |
| Government revenue and spending datasets | Taxes, spending | Fiscal | Varies | Fetch |
| V-Dem, V-Party | Institutions, parties | Politics | CC BY-SA 4.0 | Bundle extracts |
| ParlGov | Parties, elections, cabinets | Parties | Open (share-alike) | Fetch |
| Ethnic Power Relations / GeoEPR | Ethnic groups, status, areas | Identity, conflict | Open with attribution | Fetch |
| UCDP (including GED) | Conflict events, georeferenced | Conflict targets | CC BY 4.0 | Bundle extracts |
| Correlates of War; military spending series | Capabilities, alliances, spending | War module | Free with citation | Fetch |
| Global Energy Monitor trackers | Power plants, mines, pipelines | Energy assets | CC BY 4.0 | Bundle |
| ERA5 reanalysis | Weather 2000-2024 | Replay history (ADR 0241) | Copernicus licence (attribution) | Fetch, bundle aggregates |
| FaIR | Climate emulator (software) | Climate module | Apache 2.0 | Dependency |
| Disaster databases (e.g. EM-DAT) | Disaster events | Validation only | Restricted | User fetches; never bundled |

### 16.1 Pipeline

The `ceteris-pack` crate builds the pack reproducibly (`ceteris pack build`, ADR 0113): download with checksums, harmonize to the H3 grid and province map, impute with documented rules (for example, downscaling national ICIO output to provinces with subnational sector shares and night lights; imputing ICIO coefficients for the roughly 115 economies outside OECD ICIO from regional averages adjusted by income), reconcile to national totals, and emit versioned Parquet tables plus a manifest recording every source, version, licence and imputation flag. Fetch-only sources are downloaded on the user's machine at build time. The start year is a pipeline parameter (1995-2022).

## 17. Degrees of Freedom Register

The register lists every parameter that is not pinned by data, with its prior and how it is identified; ranges below are starting priors to be refined in Phase 9, not final values.

### 17.1 Continuous parameters

| ID | Parameter | Module | Starting prior | Identification / source | Class |
| --- | --- | --- | --- | --- | --- |
| P01 | Trade elasticity theta\_g by sector | Trade | 4 classes (ADR 0101, trade-weighted harmonic means): class 0 (food; chemicals and metals) 2.91, class 1 (machinery, electronics and vehicles) 4.14, class 2 (agriculture; light manufacturing) 7.68, class 3 (crude oil, gas and refined fuels; coal and other mining) 16.91; local goods 0 (3 classes 2.62, 5.19 and 10.57 before, ADR 0023) | Caliendo and Parro (2015); estimated directly by PPML structural gravity on 2000 BACI flows (ADR 0022); sets the gravity kernel | Literature |
| P02 | Merchant inventory target coverage | Markets | 1 to 6 months by good | Storage and perishability; price volatility | Free |
| P03a | Price adjustment speed kappa1 | Markets | 0.05 to 0.5 per month, start 0.05 | Price volatility and autocorrelation | Free |
| P03b | Inventory term kappa2 of the price rule | Markets | 0.05 to 0.5 per month when on; start 0 (off) | Price volatility and autocorrelation; stability region (ADR 0019) | Free |
| P03c | Step clip c\_max of the price and wage rules | Markets | 0.1 in log per month | Bounds one tick's move; binding ticks counted per province | Free |
| P03d | Share m of each market's stability limit used by its price gain (ADR 0263), and of its coupled block's, the country's linearized tick (ADR 0267) | Markets | 0 to 1, start 0.5 | Frictionless stability (V1); per-market speed; a capped block keeps the same margin of its own limit | Free |
| P04 | Mode cost ratios (water, rail, road, track) | Transport | Ordered, log-uniform | Freight rates, price gaps | Literature + calibrated |
| P05 | Non-tariff border frictions | Trade | Log-uniform | Gravity border effects; fitted per theta class to ICIO 2000 cross-border shares (ADR 0092), per good in class 3 (ADR 0276, proposed), a good below its own share at nu 0 clamped at 0 | Calibrated |
| P06 | Goods substitution sigma within a PIGL category | Demand | 0.3 to 0.9, start 0.6 | Comin-Lashkari-Mestieri (2021) | Literature |
| P07 | Income elasticities by category | Demand | Food below 1, services above 1 | Comin-Lashkari-Mestieri; budget surveys | Literature |
| P08 | Capital-labour substitution | Production | 0.5 to 1.2 | Oberfield and Raval (2021) and related | Literature |
| P09 | Agglomeration elasticity | Production | 0.02 to 0.10 | Rosenthal and Strange (2004) | Literature |
| P10 | Depreciation rates | Production | 2 to 8% per year by asset | Accounting conventions | Literature |
| P11 | Investment logit dispersion | Investment | Free | Capital allocation dispersion | Free |
| P12 | Ideas production lambda, phi | Technology | lambda 0.3 to 1, phi below 1 | Jones (1995) | Literature + calibrated |
| P13 | Catch-up rate and its determinants (education, trade exposure, FDI, institutions) | Technology | A few percent of the gap per year | Conditional-convergence evidence; Buera and Oberfield (2020) | Calibrated |
| P14 | Fertility response gammas | Demography | Signed priors | Crude birth rates, transition timing | Calibrated |
| P15 | Mortality level mapping (Preston curve) | Demography | Fitted curve | Life tables, crude death rates | Data + calibrated |
| P16 | Migration dispersion nu and cost weights | Migration | Literature range | Artuc et al. (2010); Caliendo et al. (2019); flow data | Literature + calibrated |
| P17 | Diaspora network weight | Migration | Positive | Chain-migration flows | Calibrated |
| P18 | Cultural transmission rates | Culture | Free | Assimilation and language shift rates | Calibrated |
| P19 | Heuristic-switching intensity of choice | Expectations | Laboratory estimates | Anufriev and Hommes (2012) | Literature |
| P20 | Bank leverage and reserve limits | Finance | By era | Historical bank balance sheets | Data |
| P21 | Sovereign spread function coefficients | Finance | Signed priors | Historical spreads | Calibrated |
| P22 | Fiscal capacity accumulation and decay | Fiscal | Free | Revenue/GDP paths | Calibrated |
| P23 | Contest success exponents m, a, b, c | Politics | Around 1 | Policy outcomes vs group strength | Free |
| P24 | Mobilization threshold distribution | Unrest | Free | Frequency of revolts and revolutions | Calibrated |
| P25 | Bounded-confidence threshold | Ideology | Free | Polarization paths | Free |
| P26 | Power-estimate noise in bargaining | War | Free | War onset frequencies (COW) | Calibrated |
| P27 | Lanchester coefficients and modifiers | War | By technology era | Battle datasets | Calibrated |
| P28 | Supply attrition curve | War | Free | Historical campaign attrition | Calibrated |
| P29 | State planner inertia and noise | AI | Free | Policy change frequency | Free |
| P30 | Discount factor | Several | 0.95 to 0.97 per year | Standard | Literature |

Added for the modern scenario:

| ID | Parameter | Module | Starting prior | Identification / source | Class |
| --- | --- | --- | --- | --- | --- |
| P31 | Climate damage coefficients (yields, labour, mortality) | Climate | Published dose-response curves | Schlenker and Roberts; Burke et al. (2015); Carleton et al. (2022) | Literature |
| P32 | Wright's-law learning rates for energy sectors | Technology | 10 to 30% cost decline per doubling | Way et al. (2022) | Literature |
| P33 | Economic-voting weight | Parties | Positive | Incumbent vote response to growth and inflation | Calibrated |
| P34 | New-party entry threshold | Parties | Free | Frequency of new parties reaching parliament | Calibrated |
| P35 | Taylor-rule coefficients by central bank | Monetary | Inflation above 1, output positive | Estimated policy rules | Literature + calibrated |
| P36 | Nuclear escalation cost term | War | Large | Absence of direct war between nuclear powers | Calibrated |
| P37 | Consumption propensity m, wealth correction lambda, target wealth ratio w\* | Households | m 0.5 to 1 by wealth bracket; lambda 0.01 to 0.05 per month | Intertemporal MPCs (Fagereng, Holm and Natvik 2021; Auclert, Rognlie and Straub 2024); wealth-to-income by bracket (WID) | Calibrated |
| P38 | Long-run input substitution elasticity (putty-clay) | Production | 0.3 to 1 | Drift of input-output coefficients across ICIO years 1995-2022 | Literature + calibrated |
| P39 | Wage adjustment speed kappa\_w | Labour | 0.05 to 0.5 per month, start 0.1 | Wage inflation persistence (P2 econ spec, ADR 0019) | Free |
| P40 | Downward wage rigidity delta\_w | Labour | 0 to 1, start 0.5 | Mass of nominal wage changes at zero | Free |
| P41 | Employment adjustment speed eta | Labour | 0.1 to 1 per month, start 0.3 | Job creation and destruction rates | Free |
| P42 | Education efficiency e\_k | Labour | Below secondary 1; secondary 1.4 to 1.6; tertiary 2.0 to 2.6 | Mincer returns of about 8 to 12% per year | Literature |
| P43 | Time cost of transit lambda\_g, ad valorem per day | Trade | Manufactures 0.006 to 0.021; food 0.003 to 0.010; primary 0 to 0.006 | Hummels and Schaur (2013); ADR 0013 | Literature + free |
| P44 | Route-choice dispersion theta\_r | Trade | 41 to 182, value 111.5; unused in P2 (route choice deferred, ADR 0017) | Fan, Lu and Luo (2021), routing elasticity from exporters' port choice | Literature |
| P45 | Expectation gain g of pi^e | Markets | 0.004 to 0.65 per month, start 0.006 | Learning gain of Poledna et al. (2023) to the adaptive heuristic of Anufriev and Hommes (2012); ADR 0019 | Literature |
| P46 | PIGL price parameters sigma\_B, phi, gamma, with the Dt weights theta | Demand | Start sigma\_B 0.26, phi 0, gamma 0.71, theta 0.634, 0.082, 0.284 | Alder, Boppart and Muller (2022), pooled PIGL; ADR 0021 | Literature |
| P47 | PIGL Engel parameter epsilon | Demand | Start 0.71; country estimates 0.34 to 0.90 | Alder, Boppart and Muller (2022); ADR 0021 | Literature |
| P50 | Within-cell elasticity of substitution sigma\_g | Production | 3 to 8, below theta\_g + 1 | Markup estimates; Broda and Weinstein (2006) | Literature |
| P51 | Fixed export costs by border crossing | Trade | Log-uniform | Exporter shares by sector and country | Calibrated |
| P52 | Entry costs by sector | Production | Log-uniform | Firm counts by size class; entry and exit rates | Calibrated |
| P56 | Storage loss delta\_g and money cost k\_g per month of the competitively stored goods 0, 1, 2, 5 (ADR 0091) | Markets | Storage off by default (owner ruling 2026-10-31: the approved values destabilize the fixture, ADR 0091 As built), so delta defaults to 0; approved values, set by `TradeParams::with_storage`: delta: agriculture 0.003 (0.001 to 0.01), fuels 0.0005 (0 to 0.002), coal and mining 0.001 (0 to 0.005), chemicals and metals 0 (0 to 0.001). k: 0.009 (0.0055 to 0.02), 0.011 (0.0075 to 0.02), 0.007 (0.005 to 0.012), 0.0067 (0.0055 to 0.01); k includes the interest part 0.0047, the 2000 mean of the US 3-month bill (FRED TB3MS, 5.82% a year) | delta: Hofstrand (ISU A2-33, 1995) and APHLIS post-harvest losses for grain (medium); judgement for fuels and coal (weak); no deterioration of base metals, Cafiero et al. (2011) (medium). k: interest (strong) plus physical storage: ISU extension 2017 grain rates, Cushing tank leases, LME rents (weak, not 2000-dated). Owner approval 2026-10-30T22:51Z; refit in P9 | Literature |
| P144 | Speculation intensity psi\_g of competitive storage (ADR 0091), months | Markets | 0 by default (storage off, owner ruling 2026-10-31); approved values: agriculture 20 (14 to 37), fuels 1.0 (0.6 to 2), coal and mining 3.571, chemicals and metals 5.714 | Derived, not estimated: psi = lambda s\_g / (zeta (k + delta)), lambda the monthly persistence target 1 - rho of de-trended log(I / I\*) in Gorton, Hayashi and Rouwenhorst (2013, Table 2): 0.14, 0.16, and 0.05 and 0.065 for the last two (weak). Goods 2 and 5 are the source sheet's 25 and 40, derived at zeta 0.5, rescaled to the approved zeta 3.5 (psi x 0.5 / 3.5): for coal and mining that is the formula's value at zeta 3.5, for chemicals and metals the formula gives 5.54 (0.065 x 2 / (3.5 x 0.0067)), so 5.714 is the rescaled sheet value, as approved. Owner approval 2026-10-30T22:51Z; refit in P9. Since ADR 0280 (owner ruling 2026-10-31) the storage order is a partial adjustment at lambda = psi zeta (k + delta) / s, the persistence rate itself: 0.144, 0.15, 0.05, 0.067 a month at the approved psi (the targets above, rounded through psi, and good 5's rescaled value); lambda is derived from P144, P145, P56 and P02, not a new parameter | Free |
| P145 | Convenience-yield elasticity zeta\_g (ADR 0091) | Markets | Agriculture 1.8 (0.7 to 2.7), fuels 13 (8 to 22), coal and mining 3.5, chemicals and metals 3.5 | Slope of the basis in inventory at I / I\* = 1, Gorton, Hayashi and Rouwenhorst (2013, Table 3) over k + delta: grains -0.255 a year, energies -1.861 (medium); 3.5 for goods 2 and 5 is the owner's choice between the level route (0.4 to 0.8) and the curvature route (3.2 to 4.3) of the metals group (weak). Owner approval 2026-10-30T22:51Z; refit in P9 | Free |
| P146 | Gain g\_s of the per-market expected price change pi^e\_og (ADR 0091) | Markets | 0.006 per month for all four goods (0.004 to 0.65, the P45 prior) | P45's start: the constant learning gain of Milani (2007, JME 54), 0.0183 a quarter. Was 0.08 (a 12-month window, Gorton, Hayashi and Rouwenhorst 2013), at which agriculture's storage loop leaves the region (psi limit 14.7 against 20); at 0.006 the limit is 43.5 (owner approval 2026-10-31, B7 follow-up); refit in P9 | Free |
| P148 | Loss write-off threshold M | Production | 3 months of base sales | Limited liability of enterprises (ADR 0112) | Free |
| P149 | Input stock cover | Production | 1 month | The cover of the input order rule (owner decision, 24 Oct 2026) | Free |
| P150 | Theta scale: multiplies every theta\_g where the kernel is built (ADR 0101) | Trade | 1; tariff experiments report the lane {0.6, 1, 1.4} until direct estimates replace the literature values | Experiment lane for the uncertainty of P01 (trade SPEC) | Free (experiment lane) |
| P151 | Price step clip c\_max\_storable of the storable commodities, goods 0, 1, 2 and 5 (ADR 0106) | Markets | 0.3 in log per month | Oil fell about 30% in one month in late 2008 (trade SPEC) | Free (numerical) |
| P154 | Bulk freight rate: multiplier on the sea cost per tonne-km, per theta class (ADR 0247) | Transport | Class 3 (fuels, mining) 0.026, classes 0 to 2 at 1 (on by default, owner ruling 2026-10-30) | Slope of 2000 tanker and dry-bulk voyage rates per tonne against sea km, UNCTAD Review of Maritime Transport 2001 and 2002 | Literature |
| P155 | Bulk freight rate: multiplier on the port handling fee, per theta class (ADR 0247) | Transport | Class 3 0.40, classes 0 to 2 at 1 (on, as P154) | Half the fixed part of the same rates over P04m's listed-port fee | Literature |
| P156 | Steady-state money tolerance per pop on the full pack (ADR 0089 acceptance, `p2_steady_state.rs`) | Validation | max(4 minor units, 2e-6 y_k), y_k the pop's steady monthly income (was 2e-6 of the quantity itself) | A balance's error accumulates from one tick's flows through it (wage, dividends and rebates in, spending out), each at the relative error eps of the real drift: \|dm\| <= eps (W + Div + Reb + S) ~ 2 eps y; a bound relative to the balance reads it against the non-wage share only (Albania 0.22, Pitcairn 0.32); the factor 2 is not taken, so the bound is the tighter of the two. Owner approval 2026-10-30T23:19Z | Free (numerical) |
| P157 | Block stability tolerance (ADR 0267, `block::TOLERANCE`): a block of markets counts as stable while the measured spectral radius of its linearized tick is at most 1 + this | Markets | 2e-3 (not tuned) | A market at its own per-market cap has radius 1 at gains / m; the growth-rate measurement over 2,000 ticks reads a radius to about 1e-3 (5.4e-4 against the Gelfand limit on the fixture's blocks); the tick grows at the map's radius to within it (`block_map_growth_matches_the_tick`) | Free (numerical) |
| P159 | Calibration pass tolerance (`steady::PASS_TOL`; P158 on this branch before, renumbered since main's ADR 0272 holds P158): the passes of stages 1, 2 and 4 stop when rebates, r, buyer prices and the PIGL weights change by at most this, relative (econ SPEC, Calibration) | Calibration | 1e-13 (was 1e-12; B8e, owner, 2026-10-31) | Derived from stage 5's check (1e-12): it compares Q, settled at the last pass's values, with D, the orders those values' spending gives at the settled prices, so its gap is about 3.8 times the last pass change (B8 pack: 1.34e-12 at a last change of 3.55e-13) plus about 3e-13 of rounding; the tolerance must be at most (1e-12 - 3e-13) / 3.8 = 1.8e-13, and 1e-13 is the decade below (5.25e-13 measured at it, B8d) | Free (numerical) |

Held or proposed, not built in P2: P143 kappa\_n (ADR 0090, proposed; its split price rule is parked on `p2-adr0090` and `pre-p3/b6-wip`, and deferred to P5 by owner ruling, 2026-10-30, ADR 0090 "Deferred to P5") and P147 M\_F (ADR 0112, second decision; reserved and held, `p2-r3-producer-stock`).

### 17.2 Structural switches (model uncertainty)

| ID | Switch | Options |
| --- | --- | --- |
| S01 | Node price formation | Stock-flow rule / Walrasian clearing |
| S02 | Trade solver | Gravity normalization cleared across ticks / full within-tick Sinkhorn (balanced or unbalanced) / Walrasian spatial price equilibrium |
| S03 | Demand system | PIGL / non-homothetic CES / Stone-Geary |
| S04 | Expectations | Heuristic switching / adaptive only / perfect foresight (reference) |
| S05 | Trade costs | Route choice / least-cost path; per-unit + iceberg / iceberg only |
| S06 | State AI | Tier 1a / 1b / 2 |
| S07 | Replay (ADR 0241) | Off / shocks / policy / all |
| S08 | System order within tick | Fixed / randomized (sensitivity) |
| S09 | Intermediate technology | Putty-clay vintages / fixed 2000 Leontief coefficients |
| S10 | Climate damages | Level effects / growth effects |
| S11 | Rationing | Priority / proportional / mixed (Pichler et al. 2022) |

A conclusion is reported as robust only if it survives the plausible settings of the relevant switches.

## 18. Testable Hypotheses

The simulator is built to adjudicate between explanations, so each hypothesis names the experiment that would falsify it; H1 to H3 are engineering acceptance tests, H4 onward are research questions.

| ID | Hypothesis | Experiment | Falsified if |
| --- | --- | --- | --- |
| H1 | Equilibrium nesting: with frictions off, the agent model converges to the GE reference | Frictionless runs vs hat-algebra solution | Gaps above 1% persist |
| H2 | Gravity emerges: PPML on simulated flows recovers each theta class's trade elasticity on the model's own trade costs; the distance elasticity is reported against the empirical range (ADR 0264) | Estimate gravity on simulated data (Santos Silva and Tenreyro 2006): per class, ln(1 + tariff) with the model's country-pair kernel as the cost control; pooled, ln distance and contiguity | A class's theta off its estimate by more than 2 pair-clustered standard errors and 1%; the distance elasticity (about -0.7 to -1.1 in the data) is a diagnostic until the log-distance crossing friction lands (P4, ADR 0272 on main) |
| H3 | Predictability: aggregates are predictable for a decade or more, elections, coups and wars for only a few years | Ensemble dispersion horizons (14.6) | No separation of horizons |
| H4 | The China shock's local labour-market effects emerge without being targeted | Feed China's productivity and WTO accession; compare US province employment responses | Responses far from Autor, Dorn and Hanson (2013) |
| H5 | 2018-19 tariffs pass almost fully into import prices | Tariff branch vs baseline | Pass-through well below empirical estimates |
| H6 | A monetary union without a lender of last resort amplifies sovereign stress | Euro branch with a 2010 lender of last resort | Spreads unchanged |
| H7 | Financial crises raise anti-incumbent and populist vote shares (Funke, Schularick and Trebesch 2016) | Event study on simulated crises | No vote shift |
| H8 | Bilateral trade lowers bilateral war risk; multilateral openness raises it (Martin, Mayer and Thoenig 2008) | Dyadic hazard model on simulated conflicts | Signs reversed or zero |
| H9 | Horizontal inequality and political exclusion drive civil conflict onset (Cederman, Gleditsch and Buhaug 2013) | Province hazard model; branch with inclusion reforms | No effect beyond income levels |
| H10 | Sanctions are substantially offset by trade rerouting through third countries | 2022 sanctions branch | Direct trade losses not offset |
| H11 | Non-linear heat damages widen income gaps between hot and temperate countries (Burke, Hsiang and Miguel 2015) | Climate on vs damages off | No divergence |
| H12 | Learning curves make clean energy dominate new capacity even without carbon prices; carbon prices mainly change timing | Branches with and without carbon pricing | Fossil capacity dominates through 2060 without pricing |
| H13 | A shrinking world population slows frontier growth in the long run (Jones 2022) | Open-ended runs under fertility branches | Growth unaffected |

## 19. Technology Stack, Performance and Repository

A Rust core gives deterministic, parallel speed; Rust crates also own the data pipeline and calibration, and analysis reads the outputs from outside; the desktop app renders the map, in its webview today and natively in Rust (wgpu map, Slint panels; ADR 0088) later; the three meet on Apache Arrow.

### 19.1 Stack

| Layer | Choice | Why |
| --- | --- | --- |
| Simulation core | Rust, plain structure-of-arrays, rayon for parallelism | Determinism control, speed, memory safety for a long-lived codebase |
| Graphs | petgraph plus a sparse least-cost-path cache | Transport costs recomputed only when edges change |
| GPU fallback | wgpu compute on Metal | Only if the trade solver misses its CPU budget |
| App shell | One native Rust process: winit window, Slint panels, the economy on a sim thread publishing views through a run-ahead ring (ADRs 0088, 0208) | One native macOS app, no server |
| Map UI | wgpu globe drawn into a texture shown in Slint: fills coloured from a per-feature texture, borders from an id target, labels as Slint text (ADRs 0010, 0088, 0244); globe only, with Equal Earth for flat exports (ADR 0250) 120 fps on the owner's Mac (frame p50 8.33 ms); a full province recolour uploads in 0.06 ms |
| Pipeline, reference model, calibration | Rust crates `ceteris-pack`, `ceteris-reference`, `ceteris-calibrate`, driven by the `ceteris` CLI; GEOS only on the owner-side map build (ADR 0113) | One toolchain; the restricted part of the pack builds on the user's machine with no Python runtime |
| Analysis | Outside the repo: Polars, DuckDB or any language reading the Arrow and Parquet outputs | No bindings to maintain; runs are driven by scenario files |
| Run store | Parquet partitioned by experiment / branch / seed / table; DuckDB queries | Cheap ensemble analysis on one disk |
| Snapshots | zstd-compressed Arrow, content-addressed | Branch sharing |
| Licence | Open source, non-commercial: Apache-2.0 / MIT dual for code; data under its source licences, downloaded by each user (ODbL and other share-alike tables shipped separately) | Non-commercial sources usable; restricted sources fetched, never bundled |

An alternative worth noting: the Julia ecosystem now hosts a data-driven macro ABM framework (BeforeIT.jl, associated with the Bank of Italy), useful as a reference implementation for the economic core even though Ceteris Paribus uses Rust.

### 19.2 Performance budget (single run, 8-core desktop)

Hardware: MacBook Pro with an M-series Pro chip (about 12 cores) and 18-36 GB of unified memory. Target 6 to 12 ticks per second (0.5 to 1 simulated year per second), about 80 to 160 ms per tick. Memory target: under 6 GB for one interactive run so ensembles can run alongside.

| System | Work per tick | Budget |
| --- | --- | --- |
| Demand and consumption | 100k pops x 8 categories x goods | 10 ms |
| Production and labour | 150k enterprises | 12 ms |
| Province markets | 4,000 provinces x 50 goods | 2 ms |
| Trade (one gravity normalization per tick, gateway-factorized kernel) | 45 goods in 3 to 5 theta classes, 600 gateways + 4,594 provinces x k gateways | 30 ms, of which source\_dispatch 8 ms (P2, full pack) |
| Finance, fiscal, monetary | 1M claims, mostly passive | 8 ms |
| Politics, migration, diplomacy | Pops, parties, states | 8 ms |
| Climate (annual) | Emulator + pattern scaling | under 1 ms amortized |
| Accounting assertions | Full ledger check | 5 ms |
| Headroom |  | to 160 ms |

During wars the daily sub-tick adds about 30 cheap steps per month, budgeted separately. All timings are estimates; P0 and P2 benchmarks replace them with measurements. Ensembles run headless across the desktop's cores; with desktop-only hardware, calibration depends on emulators and modular calibration to stay within one machine's compute.

### 19.3 Repository layout

```
ceteris/
  specs/                  module SPEC.md files: state, equations, order, invariants, parameter IDs
  adr/                    architecture decision records
  crates/
    ceteris-core/         entity storage, scheduler, RNG, ledger, snapshots, events
    ceteris-geo/          provinces, transport graph, trade costs
    ceteris-econ/         production, enterprises, labour, demand, productivity
    ceteris-trade/        province markets, two-level entropic OT, shipments
    ceteris-finance/      money, central banks, banks, sovereign debt, FX, fiscal
    ceteris-demo/         demography, education, identity, migration
    ceteris-polity/       institutions, interest groups, parties, elections, unrest
    ceteris-war/          diplomacy, forces, combat, civil conflict
    ceteris-climate/      emissions, emulator, damages
    ceteris-ai/           state planners, policy interfaces
    ceteris-pack/         data pipeline (any start year 1995-2022); config/ holds the licence manifest
    ceteris-geos-coverage/  GEOS coverage simplification, owner-side map build only
    ceteris-reference/    GE hat-algebra reference model
    ceteris-calibrate/    calibration, estimation, SBI, MSM, emulators
    ceteris-cli/          the ceteris binary: pack, reference, calibrate, runs, branch and ensemble runners
  app/                    native app (winit, Slint, wgpu), its own cargo workspace (ADR 0088)
  tests/golden/           golden runs with state hashes
```

### 19.4 Performance directives

These are binding rules for the coding agent: the speed comes from layout and algorithm choices, never from relaxing determinism.

**Data layout**

- Structure-of-arrays columns indexed by `u32`; no hash maps in hot loops.
- Entities sorted by province (CSR-style offsets), so per-province work is a contiguous slice; re-sort only at quarter boundaries when entities are created or removed.
- Sparse relations (trade partners, transport edges, financial claims by holder and issuer) in compressed-row form; 45x45 input-output blocks stay dense.
- Goods are the innermost dimension of every goods-indexed array, padded to a multiple of 4 (f32) or 2 (f64), aligned to 128 bytes (the M-series cache line).
- Ledger balances in fixed-point `i64`; prices and quantities in `f64`; kernel values in `f32` where the spec allows.
- Economic kernels are generic over a Real trait implemented by f64 and by forward-mode dual numbers (ADR 0096). Release builds instantiate f64 only, so derivatives cost nothing outside calibration.
- Ensemble members are lanes, the innermost array dimension (ADR 0024); interactive play runs one lane.

**Tick execution**

- Double-buffered state: each system reads tick t and writes t+1; no locks in hot paths.
- Zero allocation in hot loops: preallocated scratch per thread (rayon `map_init`), arenas for transient objects.
- Shipments in a timing wheel (ring buffer indexed by arrival tick): O(1) insert and delivery, no heap.
- Multi-rate scheduling; war sub-ticks only while a war is active; rare events by sampled next-event times, not per-tick coin flips.
- Closed-form PIGL demand (8.6) and a single kernel product for trade (8.3): goods markets have no within-tick iterative solver. Asset-market clearing (9.9) is the only within-tick iteration.

**Linear algebra**

- The trade kernel factorizes through about 600 gateways: O(P x k + G^2) per tick instead of O(P^2), exact under route choice because route sums decompose through gateways (5.4, 8.3), and an approximation of the path minimum under the least-cost legs P2 keeps (S05).
- Goods sharing a (theta class, weight class) pair share a kernel, so kernel products are matrix-matrix (provinces x goods), not one matrix-vector product per good; 3 to 5 theta classes (register P01).
- Tariffs are constant within a country pair: apply them as blockwise scaling of per-block products; never materialize per-good kernels.
- The gravity kernel tau^(-theta) is bounded and dimensionless, so no log-domain stabilization is needed. Route-choice solves (I - A)x = b, off in P2 (5.4), use sparse iterative methods warm-started from the previous solve and run only when A changes. Anderson acceleration on every remaining fixed-point loop (Walrasian mode, full within-tick Sinkhorn test mode, GE reference model, Koijen-Yogo clearing).
- Never invert the Leontief matrix in the agent model: shortages propagate one sparse matrix-vector product per tick. The GE reference model applies the global IO system matrix-free and solves with Krylov methods.
- Exponential distributed lags (climate boxes, adaptive expectations, depreciation) are O(1) state updates; cohort aging is an array shift.
- Correlated weather from k spatial principal components (O(P x k) per draw); route-choice solves from gateways (5.4), updated only when edges change; contagion by fictitious-default rounds on sparse exposures; bounded-confidence neighbours by sort-and-sweep.

**SIMD on Apple Silicon (NEON, 128-bit)**

- Write inner loops to auto-vectorize: slices with lengths asserted up front, `chunks_exact` and `zip` instead of indexing, no branches (min/max and select for clamps), separate input and output slices. Hand-written kernels use stable crates (`pulp` or `wide`), not nightly `std::simd`.
- NEON has no gather instruction, so sparse kernels vectorize across goods (contiguous) while iterating partners serially; never vectorize across partner indices.
- `exp` and `ln` in logit and migration choices use fixed-coefficient polynomial approximations implemented once in SIMD; utilities that change by small additive amounts update multiplicatively.
- Pops are already partitioned by labour status, so status-dependent logic runs as separate branch-free loops per partition.
- FMA is used explicitly and always (`mul_add`), never left to the compiler, so results match across machines.

**Parallelism**

- M-series chips mix performance and efficiency cores, so equal static chunks leave fast cores idle. Split work into many small chunks with fixed boundaries independent of thread count, let rayon work-steal, write results to fixed slots, and reduce in fixed order: scheduling varies, results do not.
- Country-local systems (fiscal, politics, labour) run one task per country, sorted largest first for load balance.
- Systems with disjoint read and write sets run concurrently, from a static dependency graph built from declared read/write sets.
- Snapshots, metrics and UI extraction run on a background thread from the frozen buffer, overlapping the next tick.
- macOS cannot pin threads to cores; use quality-of-service classes instead: user-interactive for the interactive run, background for ensemble processes, so the UI stays smooth during batch work.

**Ensembles and calibration**

- Ensemble members run as 8 to 32 lanes of one process, advancing in lockstep (ADR 0024); process count about equal to performance cores.
- Data packs are immutable Arrow files memory-mapped by every process, so ensembles share one physical copy through the page cache.
- Lanes share packs, kernels and route-choice factors; trade products widen from (provinces x goods) to (provinces x goods x lanes).
- Copy-on-write snapshots in about 64k-row content-hashed chunks with low-level zstd.
- Emulate principal components of output series; reuse Saltelli sample matrices; random Fourier features keep emulators linear in run count; analysis tools read runs only through Arrow and Parquet outputs, never per-entity calls.

**GPU (fallback only)**

- Metal via wgpu shares unified memory (no copies) but each dispatch costs tens of microseconds, so only large batched kernels qualify (gateway Sinkhorn, emulator evaluation).
- Shader fast-math must be disabled and reductions must use fixed trees; verify the wgpu Metal backend's math mode before enabling any GPU path.

**Determinism rules**

- Pure-Rust `libm` for transcendental functions; fixed reduction trees; no float atomics; fixed target CPU in release builds.
- Dense linear algebra uses the pure-Rust `faer` crate with fixed parallelism by default. Apple Accelerate (faster matrix units, but multithreaded reduction order not guaranteed) is allowed only behind a feature flag for runs that need statistical, not bitwise, reproducibility.

**Tooling**

- criterion benchmark per system in CI; regressions over 20% block merges.
- Profile with samply and Instruments; release builds with `lto = "fat"` and `codegen-units = 1`.

## 20. Interface and Visualization

The interface is a Paradox-style map with time controls, plus two things Paradox games lack: a branch manager for comparing histories and an explainer that decomposes any number into its causes. 1.0 has one neutral chrome; narration, country flavour and seat chrome come after 1.0 (ADR 0240).

### 20.1 Map modes

| Group | Modes |
| --- | --- |
| Political | Owner, controller (war), subjects, alliances, claims, customs and currency unions |
| Physical | Terrain, crop suitability, resources (discovered / latent in god mode), infrastructure, disease environment, temperature anomaly, climate damages, emissions |
| Population | Density, growth, age structure, ethnic groups, religion, education, labour status, urbanization, migration flows (arrows) |
| Economy | GDP per capita, sector shares, wages, price of good g, price gap vs reference node, inventories, market access |
| Trade | Animated flow arcs by good, trade node regions, tariff walls, freight costs from a selected node |
| Finance | Currency regimes, sovereign spreads, bank stress, capital flows |
| Politics | Regime type, governing parties and vote shares, interest-group dominance, approval, grievance, protest, Political Stress Index |
| War | Fronts, armies, supply, blockades, devastation |
| Comparison | Branch difference for any of the above, masked by robustness across seeds |

### 20.2 Panels

- **Province / state inspectors**: every component, editable in god mode, with an 'apply at tick' field.
- **Ledgers**: state budget, trade balance, balance sheets, interest groups and laws; market panel with supply, demand, inventory and price history per node and good.
- **Explainer**: for any price, wage, flow or decision, a waterfall of contributions (for a price: local supply shock, import cost, tariff, demand from wages; for a decision: the utility terms of each option).
- **Branch manager**: a tree of branches with fork ticks and intervention lists; select two to compare.
- **Ensemble view**: fan charts with quantile bands per branch; event-probability tables.
- **Timeline**: scrubber over snapshots; jump to any tick and fork.
- **Console**: CLI-equivalent commands for anything done by clicking, exported as a scenario file for reproducibility.

### 20.3 Time controls

Pause, five speeds, step one tick, run to date, and 'run ensemble from here' which forks K seeds in the background and streams summary statistics back to the map. A speed is simulated time per wall second (one day, week, month, quarter, then maximum); the simulation runs ahead of the presented time on its own threads, also while paused, and the map interpolates between ticks (ADR 0208).

## 21. Phased Roadmap and Agent Execution Protocol

Eleven phases take the project from an empty repository to a calibrated, playable world; the counterfactual workbench comes third because branching is the product, and every phase ends with a runnable world and passing invariants.

&#91;embedded content: roadmap · 11 phases, each with its gate\]

Each phase starts only when the previous gate passes; the highlighted workbench phase is where the project first becomes useful for experiments. Phases run in the order P0-P2, pre-P3, App shell, P3, P4, P5a, P5b, P8, P9, R, then P6, P7, P10; numbers are kept, and the order follows the major-version rule of ADR 0136 and the 1.0 phase plan of ADR 0207. Each phase builds only what its gate and the 2011-2024 scorecard need; other mechanisms land after 1.0 as minors behind a flag (ADR 0207 lists them).

**Full globe from P2 (ADR 0012).** P2 onward runs on all 4,594 provinces; the dev\_slice flag stays in the pack for ad hoc fast runs, and no gate depends on it. **First release** is 1.0 at the end of R: a calibrated, validated economic world (P0-P5, P8, P9), not an earlier partial build. 1.0 runs with the climate emulator and damages off; they arrive as 1.1 (ADR 0207). P6 is 2.0-alpha, P7 2.0-beta, and P10 lands with 2.0-alpha (Steward mode for economic levers) and 2.x (full Steward mode and Tier 1b); 1.x has no seats (ADR 0239).

### 21.1 Phases and acceptance criteria

1. **P0 Foundations.** Repository, spec template, ADR template, entity storage, scheduler, counter-based RNG, double-entry ledger, event log, snapshot and restore, CI.
   - Accept: a toy world of 10 provinces runs 1,200 ticks; identical hashes across two runs and across 1 vs 8 threads; restore-and-replay from any snapshot matches.
2. **P1 World and map.** Geography pipeline (H3 raster, provinces, sea zones), transport graph with mode costs, least-cost-path cache, 2000 borders, web map with political, terrain and resource modes.
   - Accept: about 4,000 provinces render at 60 fps; clicking a province shows attributes; cost-from-node map mode looks plausible to a reviewer.
3. **P2 Economic core (minimal).** 10 goods, static pops, one enterprise per (province, sector), labour market, province markets with the stock-flow rule, structural-gravity trade over least-cost legs through gateways (route choice deferred, ADR 0017), cleared across ticks, with transit, a single numeraire money, accounting assertions, and the GE reference model (in Python at P2, in Rust since ADR 0113).
   - Accept: V0 and V1 pass (frictionless convergence to the GE reference within 1% where the uniqueness condition holds; long-run neutrality); simulated gravity elasticities in range by theta class (H2; per class with the model's kernel as the cost control and distance reported, ADR 0264); no invariant violations over 100 years; ADRs 0015 to 0023, 0089 and 0112 implemented (Section 25); V1 is the run from the perturbed steady state (gate decision below).
   - As built (26 Oct 2026):
     - Trade elasticity in 3 classes (2.62, 5.19, 10.57), drawn from the OECD key's thetas by ADR 0023's rule, with a per-class H2 prior band of two standard errors (1.40 to 3.84, 3.36 to 7.02, 8.15 to 12.99).
     - Calibration to a data state that is the steady state, and a natural rate u\* in the wage rule (ADR 0089), replacing full employment; the productivity A is calibrated, not estimated.
     - National profit ownership (owner, 25 Oct 2026; ADR 0089): enterprises' profits go to the pops of their country by population; wages stay local.
     - Limited liability of enterprises and a loss write-off beyond M = 3 months of base sales (ADR 0112, P148); markets keep the signed rule.
     - Budgets on the full pack (owner, 26 Oct 2026): a whole tick 35 ms, trade 30 ms with source\_dispatch 8 ms (19.2); the gateway count and rule move to the ADR 0092 rework.
     - Gate decision (owner, 26 Oct 2026): V1 is the run from the perturbed steady state; the run from the shocked data state is a stress test (xfail(strict)), and ADR 0090's price rule is redesigned after the gate.
   - **Pre-P3** (ADR 0207): the Rust migration (ADR 0113), the economics review and audit branches with ADR 0200 built beside ADR 0092, P3 prerequisites and ensemble lanes. Accept: the P2 gate on the recalibrated pack, and the trade benchmark rerun with openness within 1.5 times the ICIO shares, the ACR formula holding and a x10 border shock converging (x10 reported, not asserted: beyond P2's valid range, its collapse deferred to P5's money anchor, ADRs 0090 and 0207).
   - **App shell** (ADR 0207): ADR 0088 stages 1 and 2, the wgpu map at parity with P1 and a Slint panel host. Accept: ADR 0088's exit criteria.
4. **P3 Counterfactual workbench.** Branches, interventions (all types in 14.2), common random numbers, ensembles, difference maps, fan charts, CLI and scenario files with Arrow exports (ADR 0113), the replay loader (ADR 0241).
   - Accept: null intervention reproduces parent; relabelling the start year leaves the golden hash unchanged (ADR 0241); a tariff intervention yields paired effects with tighter intervals under CRN than with independent seeds.
5. **P4 Population.** Demography with Brass mortality and fertility, education, occupational mobility, migration with dynamic discrete choice, and the detrended calibration that replaces the steady state once population grows. Culture and religion stay at their 2000 shares until P6 (ADR 0207).
   - Accept: fertility, mortality and urbanization within tolerance of UN data for 2000-2010; population conservation holds.
6. **P5 Money, finance and state**, in two gates (ADR 0207).
   - **P5a Money and fiscal.** Currencies and regimes, central banks with policy rules, taxes with fiscal capacity, budgets, SFC matrices, and the quarterly investment step (7.4, ADRs 0202, 0204). Accept: SFC matrices close every tick; a policy-rate shock moves credit, output, inflation and the exchange rate with plausible signs and lags.
   - **P5b Banks, debt and flows.** Banks, housing and equity prices, sovereign bonds and default, IMF, portfolio capital flows, the sequence-space HANK reference in Rust (ADR 0025), and regime and prudential hazards (ADR 0242). Accept: responses within tolerance of the HANK reference given matched intertemporal MPCs; spreads respond to debt and stress.
7. **P6 Politics and institutions.** Institution vector, interest groups, parties and elections, contest function, policy mechanisms, laws with momentum, legitimacy, ideology dynamics, unrest, reform, repression, revolution, coups, secession.
   - Accept: regime transitions, coups and protest waves at plausible frequencies; simulated 2000-2010 elections track vote shares; crisis-voting event study (H7) runs end to end.
8. **P7 Diplomacy and war.** Relations, treaties, crisis bargaining, alliances, nuclear deterrence, forces, daily combat, logistics, occupation, war economy, peace terms, sanctions, civil conflict.
   - Accept: interstate and civil conflict onset frequencies within COW and UCDP ranges in ensembles; conflicts visibly move prices, debt and populations.
9. **P8 Growth, energy and climate.** Frontier productivity growth with technology jumps, catch-up, learning curves in energy, structural change, emissions; pandemic and chokepoint hazards (ADR 0242). Endogenous infrastructure, the climate emulator and damages are minors after 1.0 (climate in 1.1, ADR 0207).
   - Accept: 2000-2010 growth, sector shares and emissions within tolerance; open-ended runs to 2150 stay numerically stable.
10. **P9 Calibration and hindcast.** Pre-registration, a joint check of the block calibrations made at each gate, SBI, emulators and Sobol analysis in Rust, hazard fits outside the simulation (ADR 0242), 2011-2024 out-of-sample validation with replay off and on (ADR 0241), natural-experiment replication.
    - Accept: out-of-sample scorecard (15.4) and natural-experiment table (15.5) published with CRPS and coverage; robustness across structural switches documented.
    - **R Release** (ADR 0207): the user-side fetch and build of the pack, including manual sources, a signed macOS build, hashes checked across architectures, a minimum pedia. Accept: a clean machine builds the pack and reproduces the published 1.0 hashes.
11. **P10 AI layers and Steward mode.** Learned state policies and playable single-state control; LLM leaders are deferred beyond v1.
    - Accept: Tier 1b compared with Tier 1a on historical base rates; Steward mode exposes the full state action space.

### 21.2 Agent execution protocol

The plan is written to be executed by coding agents in focused steps. Rules for every task:

1. **Spec first.** Before code, the agent writes or updates `specs/<module>/SPEC.md`: state components, equations, system order, invariants, parameter IDs from Section 17, and test plan. A human approves specs for modules in P2, P5 and P6.
2. **Tests before implementation.** Unit tests for each equation, property-based tests for invariants (proptest), and an equilibrium-nesting test where applicable.
3. **Small tasks.** One system or one data table per task; each task leaves the world runnable.
4. **Golden runs.** Each phase adds a golden scenario with stored state hashes; any change that alters them must state why in its commit and update the hashes deliberately.
5. **No silent tuning.** A parameter changed to make a test pass is recorded in the register with its justification; otherwise the change is rejected.
6. **Decisions recorded.** Any deviation from this document is an ADR with context, options and consequences.
7. **Performance gates.** Benchmarks per system run in CI; regressions over 20% block merge.
8. **Definition of done.** Spec updated, tests pass, invariants hold over a 100-year run, benchmarks within budget, UI exposes the new state where relevant. Every new mechanism also passes an ablation test (15.7): switching it off must lower out-of-sample skill or break a falsification test, or it stays off by default.

## 22. Risks and Open Decisions

The largest risk is not technical but epistemic: a model this rich can reproduce anything, so the mitigations centre on keeping free parameters few, nesting known equilibria, and scoring out of sample.

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Complexity explosion; modules interact in untestable ways | High | Two-tier modules, specs, invariants, channel-shutdown tests, phase gates |
| Identification: many parameter sets fit 2000-2010 equally well | High | Three-layer parameterization, literature priors, modular calibration, posterior reporting, natural-experiment tests |
| Province-level data gaps, especially for the \~115 economies outside OECD ICIO | High | Documented imputation, reconciliation to national totals, imputation flags down-weighted in calibration |
| Sensitive real-world content: real states, ethnic groups and ongoing conflicts in an open-source simulator | High | Neutral mechanistic modeling, generic party names, documented sources, outputs framed as distributions not predictions, no editorializing UI text |
| Trade solver misses the laptop budget at province level | Medium | One gravity normalization per tick (ADR 0016), gateway-factorized kernel, tunable gateway counts, Metal fallback |
| Open-ended runs drift or explode (debt spirals, price drift, demographic collapse) | Medium | Stabilizing policy rules, stability tests to 2150, balanced-growth checks in CI |
| Numerical instability (price oscillation, bank-run cascades) | Medium | Clipped adjustments, stability tests on adjustment speeds, Walrasian fallback switch |
| Railroading toward known post-2000 history | Medium | Calendar-blind default runs: events come from hazards on simulated state; history enters only as optional replay (ADRs 0241, 0242); LLM tier deferred |
| Dataset licences restrict redistribution | Medium | Bundle-or-fetch policy, licence manifest, P0 audit |
| Determinism broken by parallelism or library updates | Medium | Fixed reductions, pinned toolchains, hash checks in CI |
| Scope creep toward a full game | Medium | Non-goals list (2.4); features must serve experiments |

### 22.1 Decisions (resolved 23 Oct 2026)

| Decision | Choice | Consequence |
| --- | --- | --- |
| Scenario | Modern start, 1 Jan 2000 (supersedes 1836) | Data replaces judgment; 2011-2024 is the out-of-sample window |
| Start-year flexibility | Pipeline builds packs for any year 1995-2022 | Rolling-origin validation (2000, 2005, 2010, 2015) |
| Horizon | Open-ended | Demography and long-run stability are first-class requirements; climate joins them in 1.1 (ADR 0207) |
| Map scope | Global, full globe from P2 (ADR 0012) | dev\_slice kept in the pack for ad hoc fast runs; no gate depends on it |
| Provinces | Natural Earth first-level regions at 2000 borders, kept whole, 4,594 (ADR 0251) | Public domain; geoBoundaries counties add a display layer |
| Markets | Every province prices every good | Gravity kernel factorized through gateways (8.3) |
| UI priority | Observer / laboratory | Branches, diff maps, ensembles before Steward controls |
| App | Fully native Rust app, Slint panels over a wgpu map (ADR 0088) | Rust core in-process, no server |
| Core language | Rust from the start | Pipeline, reference model and calibration in Rust too (ADR 0113); no Python in the build, release or test path |
| Hardware | MacBook Pro, M-series Pro, 18-36 GB, desktop only | Budgets in 15.2 and 19.2; emulators and modular calibration mandatory |
| Pops | Province x ethnic group x education x labour status | About 100,000 pops; religion as shares |
| Climate | Simple emulator with damages, from 1.1 (ADR 0207) | Endogenous feedback for long runs; 1.0 runs with climate off |
| Technology | Aggregate sector productivity, no named technologies | Frontier growth, catch-up, energy learning curves |
| Parties | Explicit parties over interest groups | Elections, coalitions, party entry |
| Naming | Real states and provinces, generic party labels by family | Recognizable map, less curation and sensitivity |
| IO and trade data | OECD ICIO 2025 edition (1995-2022, 80+ economies) + BACI | About 115 economies imputed |
| Licence | Open source and non-commercial (owner, 28 Oct 2026; supersedes the closed-source fallback of ADR 0005) | Apache-2.0 / MIT code; each user downloads the data, so non-commercial sources are acceptable; nothing restricted is bundled |
| LLM leaders | Out of v1 | Tier 2 deferred |
| First release | 1.0 at the end of R, after P9 (calibrated, validated economic world; P6 and P7 are 2.0, ADR 0136; phase plan ADR 0207) | No public partial builds |
| Name | Ceteris Paribus; code name ceteris (supersedes WorldSim, which is crowded) | Crates ceteris-\*, CLI ceteris; Python package name dormant until bindings exist (ADRs 0003, 0113); check crates.io, PyPI and GitHub before P0 |

### 22.2 Defaults the coding agent may take (recorded as ADRs)

- Exact sector splits beyond the 45 ICIO industries.
- The province-splitting rule (population and area thresholds); resolved as no split (ADR 0251).
- The imputation model for input-output coefficients of countries outside OECD ICIO.
- Number of trade gateways and partner-set size, set by benchmark results.
- Generic naming scheme for parties.

## 23. Beyond v1: World Builder and Strategy-Game Depth

Version 1 is a calibrated economic laboratory; this section specifies what turns it into a world builder and a Paradox-depth strategy game without weakening that laboratory. Nothing here is scheduled before 1.0, and every item lands behind a feature flag (roadmap, release process). Version numbers follow ADR 0136: a major relaxes one ceteris paribus, and everything else is a minor. Seats, scoring, narration and flavour are 2.0 or later (ADRs 0239, 0240); 1.x minors are simulator, lab, editor and view work.

The gaps fall in three groups: building worlds other than the real 2000 start (23.2-23.3), giving the player something to do and be judged on (23.4), and adding the discrete texture of characters, commands, actions and stories that aggregate mechanisms lack (23.5-23.8).

### 23.1 Lab and extended profiles

Most additions here add free parameters faster than they add out-of-sample skill, so they would fail the ablation rule of 15.7. They ship anyway, under a second profile.

|  | Lab profile | Extended profile |
| --- | --- | --- |
| Purpose | Calibrated counterfactuals, scorecard, citable results | Play, exploration, worlds without data |
| Mechanisms | Only those that pass ablation | Lab mechanisms plus the additions in this section |
| Free parameters | At most 60 (Section 17) | Lab set plus an X-register (X01, X02, ...) with priors and provenance |
| Scoring | Scorecard (15.4), natural experiments (15.5) | Pattern tests only; never in the scorecard |
| UI label | None | "Exploratory" banner, as for 2.0 politics in lab mode |
| Determinism and invariants | Required | Required |

Rules: an extended mechanism must reduce to its lab counterpart when switched off (equilibrium nesting applied to profiles), and a mechanism that later passes ablation may join the lab profile as opt-in in a minor release; it becomes the lab default only at the next major (ADR 0136). Branches record their profile in the run manifest, and lab and extended runs are never compared as parent and child.

### 23.2 World authoring and the consistency solver

A consistency solver is the prerequisite for every authoring feature: it takes an edited or hand-made world and returns the nearest world that satisfies the invariants of 4.6. Without it, a large ex-ante edit (move a province, double a population, invent a state) either halts the run or silently breaks stock-flow consistency.

**Solver stages**, run in order on the edited state, each minimizing distance to the user's targets subject to the invariants:

1. **Pops and land.** Rescale pop cells so province totals, age structure and land use match the edit; integer cohorts by largest-remainder rounding.
2. **Production and IO.** Rebalance input-output and social accounting matrices by GRAS (Junius and Oosterhaven 2003) or cross-entropy (Golan, Judge and Robinson 1994), holding user-locked cells fixed.
3. **Trade.** Re-solve the entropic transport of 8.3 at the new costs and endowments; the result is the gravity-consistent flow set.
4. **Balance sheets.** Close the who-to-whom matrices of 9.8 by adjusting the least-locked counterparty (default: rest of world, then the central bank).
5. **Institutions and politics.** Recompute interest-group memberships, party bases and derived labels from the new pops.

Every adjustment is logged with its size, so the user sees how far the solver moved their edit. A null edit must return the input bit for bit (a CI test).

**Editors** (desktop app, extended and lab profiles):

- **Map:** split, merge and redraw provinces on the H3 base; reassign owner and controller; edit terrain, resources and infrastructure edges.
- **Polities:** create, rename or dissolve states, monetary unions and supranational bodies; set the institution vector by sliders with V-Dem anchors shown.
- **Peoples:** create ethnic, religious and language groups; paint settlement areas; set EPR status.
- **Economy:** lock or edit sector capacity, productivity gaps and balance-sheet lines.

Editors write an intervention list, not raw state, so every authored world is a branch of a data pack and stays reproducible (14.1).

### 23.3 Synthetic worlds, earlier eras and modding

**Synthetic worlds** (extended profile). A generator builds a fictional world in the same pack schema, then hands it to the consistency solver:

1. Terrain from noise plus plate-style uplift and erosion; rivers by flow accumulation.
2. Climate normals from latitude, elevation and prevailing winds; crop suitability from a GAEZ-style lookup on those normals.
3. Resources by geology priors; settlement by suitability and access, with city sizes drawn to Zipf's law (a V2 target already).
4. Provinces by Voronoi on population, states by seeded flood fill with border noise.
5. IO coefficients, institutions, education and balance sheets drawn from empirical priors conditional on income per head, estimated on the 2000 pack.

Because step 5 reuses 2000 cross-country relationships, synthetic worlds inherit modern-economy behaviour. That is intended; the generator is for alternate geographies, not alternate eras.

**Earlier eras.** The pipeline stops at 1995 because OECD ICIO and BACI do. A pre-1995 path needs:

- Long-run sources: Maddison Project (output, population), HYDE (gridded population and land use), Clio-Infra (education, wages, institutions), CShapes 2.0 (borders since 1886); licences to check for each.
- IO tables imputed from structural-change priors (Section 7.6) conditioned on income and sector shares.
- Capability milestones (23.7), because without them nothing distinguishes a railway economy from a container economy. This revisits the no-named-tech decision for earlier eras only.

Pre-1995 packs are extended-profile until a historical hindcast exists to score them.

**Modding.** The data-pack schema, scenario and intervention formats are already public API (roadmap, versioning policy). Modding adds:

- A documented mod manifest that layers packs, intervention lists and X-register parameter overrides, with load order and conflict reporting.
- Module hooks: a module ABI so a third-party crate can register systems at a fixed pipeline slot, declare its state components and parameters, and pass the V0 invariants.
- Mods run only in the extended profile unless they change data alone and pass the scorecard.

### 23.4 Player loop

The player loop lands in 2.0-alpha with Steward mode (ADR 0239). The player is scored by how much better their state does than the AI would have done with the same world and the same luck. The engine already computes exactly that: a paired effect under common random numbers (14.4-14.5), with the Tier 1a run as the baseline branch.

**Objectives.** At start the player picks or writes an objective as a weighted outcome vector over the state's own metrics, for example real income of the bottom 40%, debt to GDP, regime survival, territorial control or emissions. Presets cover common ambitions (catch up to the frontier, survive a hostile neighbour, keep the peg). The weights are fixed at start and shown on the scoreboard.

**Score.** Score = mean over K seeds of the objective in the player's branch minus the objective in the AI baseline, with a bootstrap interval. A shadow ensemble of the AI baseline runs in the background at low priority; the score is final only when both ensembles reach the end date. Single-seed play shows a provisional score with a wide band.

**Mandate and fail states.** The player is the government, not the state, so power can be lost:

- Democracies: losing an election hands the levers to the winning coalition's AI until the next election, unless the player chooses opposition play (proposing bills, campaigning, coalition offers).
- Autocracies: a coup, revolution or loss of the winning coalition ends the run, or continues it as the successor regime if the player opts in.
- Hard failures: sovereign default, state collapse, or loss of the capital province, each optional per scenario.

**Difficulty** changes information and frictions, never cheats: observation lag and noise (A13), the AI's planner horizon (23.8), and how much of the score breakdown the player sees during play.

**Ironman.** An Ironman run disables god-mode edits and branch forking, locks the seed list, and records only in-world edits (14.2), so its score is comparable across players.

### 23.5 Characters and the military command layer

**Characters** replace the stochastic leader attributes of 2.4 in the extended profile. People in office and in public life on 1 January 2000 carry their real names and public facts; everyone who enters later is generated from the population (ADRs 0183-0186, decision of 28 Oct 2026). The lab shows role labels, and a run-level switch replaces real people with generated stand-ins.

| Role | Latent traits | What the player sees | Main effect |
| --- | --- | --- | --- |
| Head of government | Competence, risk aversion, ideology, loyalty base, health | Record in office, platform, age | Planner weights and noise (12.3) |
| Ministers (finance, defence, foreign, interior) | Competence, faction, corruption | Faction, past posts | Implementation slippage on their policy module |
| Central bank governor | Hawkishness, independence | Speeches as noisy signals | Taylor-rule coefficients within the regime's band |
| Party leaders | Charisma, ideology, ambition | Polling, platform | Vote shares, coalition bargaining |
| Generals | See below | Dossier signals only | Combat, logistics, initiative |

Leader effects are bounded by evidence: growth effects are calibrated to the leader-death natural experiment of Jones and Olken (2005), tenure and exit hazards to Archigos (Goemans, Gleditsch and Chiozza 2009). Autocrats choose between loyal and competent appointees, the trade-off of Egorov and Sonin (2011), so small winning coalitions produce weaker cabinets and officer corps.

**Command layer** (extended profile; the lab keeps the aggregate Lanchester model of 11.5). It implements the war direction set in October 2026: HOI3-style but very realistic, simulation first, no unlockable technology, and a command spectrum from micromanagement to delegation.

- **Order of battle.** The capability stocks of 11.4 are organized into formations (brigade, division, corps, army, front) with HQs. Templates set the mix of manoeuvre, fires, air defence, engineers and logistics; restructuring costs readiness for months.
- **Command spectrum.** The player can order a single brigade, draw a front line with an objective, or hand a theatre to a general with a goal and a risk tolerance. Any level can be delegated.
- **Generals.** Latent operational skill, logistics skill, initiative and risk aversion. The player sees only a dossier of signals: staff-college scores, training results, combat record, peer reports and medals. Medals inflate with patronage and corruption (10.1), so they are gameable signals. Performance in battle updates a visible estimate.
- **Doctrine and training.** Force employment, after Biddle (2004), is a stock built by training spending and exercise time. It multiplies effectiveness more than equipment does, which is how quality differs without a tech tree.
- **Equipment.** Quality comes from defence-sector productivity (7.5) and the supplier state. Production lines follow Wright's law per equipment type; Wright (1936) first measured it on aircraft. Switching lines resets learning.
- **Logistics and fronts.** Supply hubs sit on the transport graph (5.3); fronts are contact lines between controlled provinces on the daily sub-tick. Encirclement cuts supply and raises surrender hazards.
- **Fog of war.** Enemy formations are seen through reconnaissance and intelligence (23.6), with the noise of A13.

Nesting test: a theatre delegated to an average general with default doctrine must reproduce the aggregate Lanchester outcome distribution of the lab profile. Battle-level coefficients are fitted to historical battle datasets such as CDB90 (licence to check).

### 23.6 Diplomacy as actions, intelligence and covert action

Section 11 models why states fight and settle; this section gives the player and the AI the same explicit verbs. Every verb is an action in the Tier 1a action space (12.6), so the AI uses them too.

**Diplomatic actions**

| Action | Mechanism it acts through | Data anchor |
| --- | --- | --- |
| Demand (territory, policy, basing, debt terms) | Opens a crisis in the bargaining model of 11.2 with a stated stake | Militarized dispute frequencies |
| Casus belli and war goals | Sets the indivisible-stake flag and the terms a settlement must meet; legitimacy cost scales with how unprovoked it looks | Interstate war onsets |
| Peace conference | Interactive bargaining over the terms list of 11.8 while beliefs update from the front | War termination terms |
| Client states and spheres | Hierarchy contracts trading autonomy for protection (Lake 2009): basing, foreign-policy alignment, currency or trade preference | Alliance and basing data |
| Arms deals | Transfers equipment stocks and creates dependence on spare parts from the supplier | Arms transfer series (licence to check) |
| International organizations | UN Security Council and General Assembly votes, WTO disputes, IMF programmes; EU accession as an endogenous process gated by thresholds on the institution vector | UN voting ideal points (Bailey, Strezhnev and Voeten 2017) |

A great-power ranking (capability, economy, financial centrality) is a derived view for the UI only, like regime labels in 10.2.

**Intelligence.** Each state has an intelligence service, a stock built by spending and bureaucratic quality. It reduces the noise of A13 on chosen targets: military capability, reserves, internal stability, true bargaining resolve. Better intelligence narrows the bargaining range of 11.2, which lowers war risk from private information.

**Covert action** (extended profile), each with a success hazard, an exposure hazard and blowback when exposed:

- Election interference, calibrated to partisan electoral interventions (Levin 2016).
- Coup sponsorship and support for rebels, raising the hazards of 10.8 and 11.6 in the target.
- Covert regime change, whose poor long-run record (O'Rourke 2018) is a validation target.
- Sabotage of infrastructure edges and cyber attacks on payment systems or utilities.
- Propaganda: shifts ideology diffusion (10.7) in the target, weighted by its civil-liberties index.

Exposure damages trust (11.1) with the target and third parties, so covert action trades short-run gain for long-run isolation.

### 23.7 Policy catalog, capability milestones, emerging sectors and non-state actors

**Policy catalog.** Section 10.1 says laws are separate variables; this is the list. Every law passes through the legislative process of 10.6, takes effect after a lag scaled by bureaucratic quality, and draws a stance from each interest group in proportion to its members' payoff.

| Domain | Levers | Acts through | Initial values from |
| --- | --- | --- | --- |
| Labour | Minimum wage, employment protection, union rules, retirement age | 8.7, 6.3 | OECD EPL; Botero et al. (2004) |
| Social protection | Pension design (pay-as-you-go or funded, replacement rate), unemployment insurance, cash transfers | 9.6, household income | OECD SOCX, ILO social protection data |
| Health | Public share of spending, coverage | Mortality level (6.2), SEIR response (13.3) | WHO health expenditure data |
| Education | Spending, compulsory years, tertiary access | Enrollment (6.3) | UNESCO data |
| Property and ownership | Land reform, nationalization, privatization | Ownership type (3.2), rule of law | V-Dem, privatization records |
| Markets | Price controls, subsidies, export bans, industrial policy, local content | Rationing (8.2), wedges (8.5) | Tariff and subsidy datasets |
| Finance | Capital controls, central bank independence, macroprudential tools | 9.11, 9.12 | Fernandez et al. (2016) |
| Citizenship and identity | Immigration law, naturalization, language and autonomy policy | 6.4, 6.5 | Migration policy indices |
| Information | Censorship, state media share | Civil liberties, ideology diffusion (10.7) | V-Dem |

**Capability milestones.** Discrete capabilities emerge from continuous investment; nothing is picked from a tree. A capability switches on when sector productivity and cumulative investment cross thresholds, and it unlocks actions or modifiers:

- Nuclear weapons, gated by nuclear latency (Fuhrmann and Tkach 2015) and a state decision with a sanctions response.
- Satellites and space launch: raise intelligence quality (23.6).
- Precision munitions and drones: raise Lanchester modifiers (11.5).
- Rapid vaccine platforms: shorten the vaccination lag in 13.3.
- Frontier AI: a services productivity shock plus cyber capability.

The 2000 state of each capability is data. This keeps the no-named-technology decision of 2.4 for 1.x while giving players milestones to race toward.

**Emerging sectors.** With 45 fixed sectors, a run to 2150 keeps the product space of 2000. Latent sectors exist from the start with zero output, an income elasticity above one in the demand system (8.6), and a parent sector. A latent sector activates when its frontier productivity crosses a viability threshold; provinces enter it in order of relatedness to their current output (Hidalgo et al. 2007). Extended profile only until a hindcast can test it.

**Non-state actors**

- **Criminal organizations.** Illicit production and trafficking as sectors with routes on the transport graph; crackdowns fragment groups and raise violence, the mechanism of Dell (2015). Initialized from UNODC estimates.
- **Armed and terrorist groups.** Extend the rebel factions of 11.6 with transnational networks that recruit from grievance and raise the attack hazards of 13.5.
- **Media.** An outlet layer per state (state, private, foreign ownership; reach) that weights ideology diffusion.
- **Conglomerates.** A few generated business groups per state as lobbying interest groups (10.4) that own foreign capital across borders.

### 23.8 Narrative layer and strategic AI

**Narrative layer.** Stories come from the event log, not from scripts, so "mechanism over script" (2.5) holds. The layer is a view: it never changes state, except through choices the player makes, which are recorded as in-world edits.

- **Event cards.** A card fires when a logged event or threshold crossing touches the player's state or a watched one: a bank run, a coup attempt, a protest cascade, a spread spike. It shows what happened, the decomposition of its drivers (explainability, 2.5) and, where relevant, two to four responses. Every response is an ordinary action from the action space, so a card adds no effect a menu could not.
- **Chronicle.** A per-branch history written from the event log with deterministic templates, readable as a timeline and diffable against the parent branch.
- **News feed and outliner.** World events ranked by size and proximity; the outliner lists the player's formations, crises, bills in progress and active covert operations.
- **Alerts.** User-set thresholds on any metric, such as reserves below three months of imports.

**Strategic AI.** The Tier 1a planner looks one step ahead, which is enough for budgets but too myopic for an opponent: it cannot prepare a war, build an alliance or save for a crisis. A strategic layer sits above it:

1. **Goals**, chosen quarterly from the coalition objective of 12.3: build fiscal capacity, deter neighbour X, join union Y, take province Z.
2. **Plans**, decomposed from goals by a hierarchical task network into constraints and targets for the one-step planner over several years (mobilize by a date, hold reserves above a floor, court a partner).
3. **Evaluation** by rollouts on the neural surrogate of 12.4, with a fixed rollout count and seeds keyed by (state, tick) so decisions stay deterministic.
4. **War planning** sets theatre objectives, force allocation and generals' risk tolerance for the command layer (23.5).

The rollout budget is set so the strategic layer stays within 10% of the tick budget of 19.2. Planning horizon and rollout count are the AI difficulty settings of 23.4. The layer runs in the extended profile until it passes the policy-forecast gate (roadmap, 0.6.0), at which point it may replace the one-step planner in the lab profile.

### 23.9 Placement in the roadmap

The first addition in this section is the consistency solver in 1.4 (climate, 1.1, is outside it); seats, scoring and most game depth arrive with 2.0 politics and war (ADR 0239); pre-1995 eras enter the lab at 3.0. Releases follow the roadmap's After 1.0 table, its versioning rules and ADR 0136; a 2.x or 3.x label below on an extended-profile item means its dependencies land then, and the item ships as a minor.

| Item | Section | Release | Depends on | Profile |
| --- | --- | --- | --- | --- |
| Objectives, score against the AI baseline, Ironman | 23.4 | 2.0-alpha | Steward mode, ensembles | Lab and extended |
| Steward mode and office seats (ADR 0237) | 12.6, 23.4 | 2.0-alpha | P6 | Lab and extended |
| Planned-economy seat (ADR 0237) | 23.4 | 2.0-alpha or later | Steward mode, ADRs 0084, 0206 | Lab and extended |
| Firm seat (ADR 0237) | 23.4 | 2.0-alpha or later | Steward mode | Extended |
| Event cards, chronicle, outliner, alerts | 23.8 | 1.3 | Event log | View only |
| Policy catalog: labour, social, health, education, markets, finance | 23.7 | 1.3 | God-mode edits (14.2) | Lab |
| Consistency solver and editors | 23.2 | 1.4 | P2, P4, P5 complete | Lab and extended |
| Mod manifest (data and parameters only) | 23.3 | 1.4 | Solver | Extended |
| Policy catalog: property, citizenship, information | 23.7 | 2.0-alpha | P6 legislative process | Lab if it passes pattern tests |
| Characters | 23.5 | 2.0-alpha | P6 | Extended |
| Neural surrogate (pulled forward from P10) and strategic AI | 23.8 | 2.0-beta | P5 policy gate | Extended until the gate |
| Diplomatic actions and intelligence | 23.6 | 2.0-beta | P7 | Lab if H7-H9 hold |
| Command layer | 23.5 | 2.0-beta | P7, characters | Extended |
| Capability milestones | 23.7 | 2.0 | P7, P8 | Extended; lab at 3.0 |
| Covert action, non-state actors | 23.6, 23.7 | 2.x | Intelligence, P6, P7 | Extended |
| Emerging sectors | 23.7 | 2.x | P8 | Extended; lab at 3.0 |
| Synthetic worlds, module ABI | 23.3 | 2.x | Solver; Rust crates published | Extended |
| Pre-1995 packs | 23.3 | 2.x (extended), 3.0 (lab) | Capabilities; reanalysis hindcast (ADR 0131) for lab | Extended until 3.0 |

Open decisions for this section:

- [ ] Size of the X-register, and whether it gets its own table in Section 17.
- [ ] Whether a lost mandate ends the run or hands control to the AI by default (23.4).
- [x] Whether the roadmap's After 1.0 table is updated to the releases above: yes, under ADR 0136 (28 Oct 2026).

## 24. References

Recent works checked for this draft are linked; the remainder are standard references cited from knowledge and should be verified when each module spec is written.

### 24.1 Checked for this draft

- Poledna, Miess, Hommes and Rabitsch (2023). [Economic forecasting with an agent-based model](https://irihs.ihs.ac.at/id/eprint/6453/). European Economic Review 151.
- Wiese, Kaszowska-Mojsa, Dyer, Moran, Pangallo, Lafond, Muellbauer, Calinescu and Farmer (2024). [Forecasting macroeconomic dynamics using a calibrated data-driven agent-based model](https://arxiv.org/pdf/2409.18760). INET Oxford Working Paper 2024-06.
- Kleinman, Liu and Redding (2023). [Dynamic spatial general equilibrium](https://collaborate.princeton.edu/en/publications/dynamic-spatial-general-equilibrium/). Econometrica 91(2).
- Dvorkin (2023). [Heterogeneous agents dynamic spatial general equilibrium](https://econpapers.repec.org/paper/fipfedlwp/95863.htm). St. Louis Fed WP 2023-005.
- Lin et al. (2025). [Simulating macroeconomic expectations using LLM agents](https://arxiv.org/abs/2505.17648v2). arXiv.
- Feng, Lu, Su and He (2025). [SimCity: multi-agent urban development simulation with rich interactions](https://arxiv.org/abs/2510.01297v1). arXiv.
- [Macroeconomic forecasting from input-output tables](https://arxiv.org/pdf/2603.12412) (2026), arXiv, for its survey of recent data-driven ABM work including Axtell and Farmer (2025, Journal of Economic Literature) and BeforeIT.jl (Glielmo et al. 2025).

### 24.2 Trade, spatial and growth economics

- Allen, R. C. (2009). The British Industrial Revolution in Global Perspective.
- Allen, T. and Arkolakis (2014). Trade and the topography of the spatial economy. QJE.
- Anderson and van Wincoop (2003). Gravity with gravitas. AER.
- Arkolakis, Costinot and Rodriguez-Clare (2012). New trade models, same old gains? AER.
- Artuc, Chaudhuri and McLaren (2010). Trade shocks and labor adjustment. AER.
- Barjamovic, Chaney, Cosar and Hortacsu (2019). Trade, merchants, and the lost cities of the Bronze Age. QJE.
- Buera and Oberfield (2020). The global diffusion of ideas. Econometrica.
- Caliendo and Parro (2015). Estimates of the trade and welfare effects of NAFTA. REStud.
- Caliendo, Dvorkin and Parro (2019). Trade and labor market dynamics. Econometrica.
- Comin and Hobijn (2010). An exploration of technology diffusion. AER.
- Comin, Lashkari and Mestieri (2021). Structural change with long-run income and price effects. Econometrica.
- Disdier and Head (2008). The puzzling persistence of the distance effect on bilateral trade. REStat.
- Donaldson (2018). Railroads of the Raj. AER.
- Donaldson and Hornbeck (2016). Railroads and American economic growth: a market access approach. QJE.
- Eaton and Kortum (2002). Technology, geography, and trade. Econometrica.
- Fajgelbaum and Schaal (2020). Optimal transport networks in spatial equilibrium. Econometrica.
- Galor (2011). Unified Growth Theory.
- Harris and Todaro (1970). Migration, unemployment and development. AER.
- Hatton and Williamson (1998). The Age of Mass Migration.
- Head and Mayer (2014). Gravity equations: workhorse, toolkit, and cookbook. Handbook of International Economics 4.
- Hummels and Skiba (2004). Shipping the good apples out? JPE.
- Jones (1995). R&D-based models of economic growth. JPE.
- Lopez-Cordova and Meissner (2003). Exchange-rate regimes and international trade: the classical gold standard era. AER.
- O'Rourke and Williamson (1999). Globalization and History.
- Ossa (2014). Trade wars and trade talks with data. AER.
- Pascali (2017). The wind of change: maritime technology, trade, and economic development. AER.
- Redding and Rossi-Hansberg (2017). Quantitative spatial economics. Annual Review of Economics.
- Rosenthal and Strange (2004). Evidence on the nature and sources of agglomeration economies. Handbook of Regional and Urban Economics 4.
- Santos Silva and Tenreyro (2006). The log of gravity. REStat.
- Simonovska and Waugh (2014). The elasticity of trade. JIE.
- Taylor and Williamson (1997). Convergence in the age of mass migration. European Review of Economic History.

### 24.3 Agent-based, monetary and computational methods

- Anufriev and Hommes (2012). Evolutionary selection of individual expectations and aggregate outcomes. AEJ: Microeconomics.
- Assenza, Delli Gatti and Grazzini (2015). Emergent dynamics of a macroeconomic agent based model with capital and credit. JEDC.
- Brock and Hommes (1997). A rational route to randomness. Econometrica.
- Caverzasi and Godin (2015). Post-Keynesian stock-flow-consistent modelling: a survey. Cambridge Journal of Economics.
- Chizat, Peyre, Schmitzer and Vialard (2018). Scaling algorithms for unbalanced optimal transport problems. Mathematics of Computation.
- Cuturi (2013). Sinkhorn distances. NeurIPS.
- Diamond and Dybvig (1983). Bank runs, deposit insurance, and liquidity. JPE.
- Dyer, Cannon, Farmer and Schmon (2024). Black-box Bayesian inference for agent-based models. JEDC.
- Gai and Kapadia (2010). Contagion in financial networks. Proceedings of the Royal Society A.
- Galichon (2016). Optimal Transport Methods in Economics.
- Godley and Lavoie (2007). Monetary Economics.
- Li, Gao, Li and Liao (2024). EconAgent: LLM-empowered agents for simulating macroeconomic activities. ACL.
- Saltelli et al. (2008). Global Sensitivity Analysis: The Primer.
- Zheng, Trott, Srinivasa, Parkes and Socher (2022). The AI Economist. Science Advances.

### 24.4 Finance and political economy

- Acemoglu and Robinson (2000). Why did the West extend the franchise? QJE.
- Acemoglu, Johnson and Robinson (2001). The colonial origins of comparative development. AER.
- Acemoglu and Robinson (2019). The Narrow Corridor.
- Alesina and Spolaore (1997). On the number and size of nations. QJE.
- Arellano (2008). Default risk and income fluctuations in emerging economies. AER.
- Besley and Persson (2009). The origins of state capacity. AER.
- Bisin and Verdier (2001). The economics of cultural transmission. JET.
- Bueno de Mesquita, Smith, Siverson and Morrow (2003). The Logic of Political Survival.
- Dell (2010). The persistent effects of Peru's mining mita. Econometrica.
- Eaton and Gersovitz (1981). Debt with potential repudiation. REStud.
- Goldstone (1991). Revolution and Rebellion in the Early Modern World.
- Granovetter (1978). Threshold models of collective behavior. AJS.
- Hegselmann and Krause (2002). Opinion dynamics and bounded confidence. JASSS.
- Kuran (1991). Now out of never. World Politics.
- Luhrmann, Tannenberg and Lindberg (2018). Regimes of the World. Politics and Governance.
- North and Weingast (1989). Constitutions and commitment. Journal of Economic History.
- Persson and Tabellini (2000). Political Economics.
- Reinhart and Rogoff (2009). This Time Is Different.
- Sen (1981). Poverty and Famines.
- Tilly (1990). Coercion, Capital, and European States.
- Turchin (2016). Ages of Discord.

### 24.5 Conflict

- Fearon (1995). Rationalist explanations for war. International Organization.
- Martin, Mayer and Thoenig (2008). Make trade not war? REStud.
- Morrow (1991). Alliances and asymmetry. AJPS.
- Powell (2006). War as a commitment problem. International Organization.
- Singer, Bremer and Stuckey (1972). Capability distribution, uncertainty, and major power war (CINC).
- Slantchev (2003). The principle of convergence in wartime negotiations. APSR.
- Van Creveld (1977). Supplying War.

### 24.6 Added for the modern scenario

- Amiti, Redding and Weinstein (2019). The impact of the 2018 tariffs on prices and welfare. Journal of Economic Perspectives.
- Autor, Dorn and Hanson (2013). The China syndrome. AER.
- Burke, Hsiang and Miguel (2015). Global non-linear effect of temperature on economic production. Nature.
- Carleton et al. (2022). Valuing the global mortality consequences of climate change. QJE.
- Cederman, Gleditsch and Buhaug (2013). Inequality, Grievances, and Civil War.
- Fajgelbaum, Goldberg, Kennedy and Khandelwal (2020). The return to protectionism. QJE.
- Fearon and Laitin (2003). Ethnicity, insurgency, and civil war. APSR.
- Funke, Schularick and Trebesch (2016). Going to extremes: politics after financial crises. European Economic Review.
- Gopinath et al. (2020). Dominant currency paradigm. AER.
- Ilzetzki, Reinhart and Rogoff (2019). Exchange arrangements entering the twenty-first century. QJE.
- Jones (2022). The end of economic growth? Unintended consequences of a declining population. AER.
- Schlenker and Roberts (2009). Nonlinear temperature effects indicate severe damages to US crop yields. PNAS.
- Smith et al. (2018). FAIR v1.3: a simple emissions-based impulse response and carbon cycle model. Geoscientific Model Development.
- Way, Ives, Mealy and Farmer (2022). Empirically grounded technology forecasts and the energy transition. Joule.

Several nineteenth-century references above are no longer cited after the switch to a 2000 start; they are kept as background and can be pruned.

### 24.7 Added for accuracy and close relatives

- Dawid et al. Eurace@Unibi: an agent-based macroeconomic model for economic policy analysis (with regions and interregional labour mobility).
- Deaton and Laroque (1992). On the behaviour of commodity prices. REStud.
- Grimm et al. (2005). Pattern-oriented modeling of agent-based complex systems. Science.
- Grimm et al. (2020). The ODD protocol for describing agent-based and other simulation models: a second update. JASSS.
- Hegre et al. (2019). ViEWS: a political violence early-warning system. Journal of Peace Research.
- Kennedy and O'Hagan (2001). Bayesian calibration of computer models. JRSS B.
- Lamperti et al. (2018). Faraway, so close: coupled climate and economic dynamics in an agent-based integrated assessment model. Ecological Economics.
- Lux (2018). Estimation of agent-based models using sequential Monte Carlo methods. JEDC.
- Yao, Vehtari, Simpson and Gelman (2018). Using stacking to average Bayesian predictive distributions. Bayesian Analysis.

## 25. Model migration (ADRs 0015-0026)

An economics review on 24 Oct 2026 changed P2's core: the trade objective, how markets clear, trade costs, the price and wage rules, production, demand and the P2 gate. Sections 5.4, 7.2, 8.2, 8.3, 8.6, 8.7, 15, 17, 19 and 21.1 already carry the new design; the ADRs below record the decisions and say how to move the P2 work in progress onto them. Later-phase findings (6.5, 8.8, 9.2, 9.4, 9.9, 13.2, 15.7) are folded into those sections and need no ADR until their phases start.

**How to apply.** Transcribe each ADR to adr/NNNN-slug.md in the repository format. P2 work had already used 0012 to 0014 (full globe, posted-price logit routing, data seeding), so these ADRs are numbered 0015 to 0022 in the repository and here; 0015 and 0016 supersede the repository's 0013. The ADRs were written against the P2 specification in this document, because the P2 code is not on the remote (it ends at P1, commit c848142); where the code differs, map each migration step to what exists and record the mapping in the ADR.

**Landing order.** 0015 and 0016 together (one rewrite of the trade system), then 0017 (ceteris-geo), 0018, 0019, 0021, 0020, and 0022 last because it defines the gate. Every one of them changes simulation output, so regenerate the golden hashes once after all eight land, with one BEHAVIOUR changelog entry listing them.

**Second round (0024-0026, 0096).** A follow-up review against the computational and economic state of the art added four ADRs. 0096 (forward-mode derivatives) and 0024 (ensemble lanes) fix P2's numeric types and array layout, so they were meant to land before P2's code settled, ideally with 0015 and 0016; P2 shipped in f64 on p2-econ without them, so adopting them now means a retrofit. 0026 (firms within cells) lands with the trade rewrite. 0025 (HANK reference) lands in P5 and asks only for a test harness in P2. 0096, 0024 and 0025 leave release-mode output unchanged; 0026 changes it and joins the single golden-hash regeneration.

| ADR | Decision | Crates and specs | Changes P2 output |
| --- | --- | --- | --- |
| 0015 | Trade flows in value, kernel tau^(-theta) | trade | Yes |
| 0016 | Origin markets clear across ticks; full Sinkhorn becomes switch S02 | trade, markets | Yes |
| 0017 | Route-choice trade costs (Allen-Arkolakis); deferred past P2 | geo, trade | No (deferred; least-cost legs in P2) |
| 0018 | Sector trade elasticities in 3 to 5 classes | trade, register | Yes |
| 0019 | Expected inflation in price and wage rules | markets, labour | Yes (small in P2) |
| 0020 | Putty-clay intermediates; within-tick rationing | econ | Rationing only |
| 0021 | PIGL demand; consumption rule | econ | Yes |
| 0022 | Uniqueness check, static nesting gate, direct estimation | reference and calibrate (`ceteris-reference`, `ceteris-calibrate`) | Gate only |
| 0096 | Forward-mode derivatives through a Real trait | core, econ, trade, geo | No (f64 path identical) |
| 0024 | Ensemble lanes as the innermost array dimension | core, all systems | No (lane k equals seed k) |
| 0025 | Sequence-space HANK reference for the macro block | reference (`ceteris-reference`); P2 test harness | No |
| 0026 | Pareto firm heterogeneity within cells | econ, trade, register | Yes |

### 0015. Trade flows in value with a dimensionless gravity kernel

Status: proposed. Date: 2026-10-24. Design sections: 1, 8, 8.3, 17.1 (P01).

**Context.** Section 8.3 maximized the sum of X(E p\_d - p\_o - c) minus epsilon times the entropy of X, with epsilon in currency units, and then wrote c = theta ln tau to claim structural gravity. Price levels and log costs cannot be added. With epsilon in currency units, flows depend on the nominal price level and exchange rates: inflation or a depreciation changes trade dispersion, which no gravity model allows, and V1 fails for reasons unrelated to frictions.

**Options.** (1) Keep the level objective and rescale epsilon by a price index each tick. (2) Write flows in value with kernel tau^(-theta) and destination expenditure as the demand side, so the potentials are p\_o^(-theta) and the importer price index.

**Decision.** Option 2. Option 1 restores homogeneity only approximately and keeps a parameter, epsilon, with no literature counterpart.

**Migration.**

- Flows X(o,d,g) become values; quantities shipped are X / (p\_o tau). Per-unit costs are booked to the shipment owner in the ledger, not netted from the price.
- Build the kernel from iceberg factors, K = tau^(-theta\_g), not exp(-c / epsilon). Remove epsilon and the export-supply and import-demand KL weights from the register.
- Destination expenditure E(d,g) comes from the demand systems at posted prices: consumption, intermediate use, investment and government.
- Rewrite the objective section of the trade SPEC.md.

**Tests.**

- Property test: scaling all prices and expenditures by lambda > 0 leaves trade shares unchanged to 1e-12.
- Three-province toy: flows equal the closed-form Eaton-Kortum shares.
- Goods conservation (V0) holds after converting value flows to quantities.

**Consequences.** Trade no longer depends on the price level. Epsilon leaves the register; theta\_g is the only trade dispersion parameter.

### 0016. Clear origin markets across ticks

Status: proposed. Date: 2026-10-24. Design sections: 8.2, 8.3, 17.2 (S02), 19.2, 19.4.

**Context.** Running Sinkhorn to convergence each month solves a within-tick spatial equilibrium, and the inventory rule of 8.2 then moves prices on top of it: two adjustment mechanisms for one gap. It also costs 5 to 10 iterations of two half-steps per good class per tick, the largest line in the 19.2 budget.

**Options.** (1) Full Sinkhorn each tick. (2) One destination normalization per tick at posted prices; origin excess supply goes to inventory and moves p\_o through 8.2. (3) A fixed small number of sweeps per tick.

**Decision.** Option 2 by default; option 1 stays as switch S02 for nesting tests. A destination normalization followed by an origin price update is one Sinkhorn sweep, so option 2 is Sinkhorn spread over ticks, and its frictionless fixed point is the same structural-gravity equilibrium. Option 3 adds a free iteration count that changes the dynamics.

**Migration.**

- Replace the per-tick solver loop with one normalization per (theta class, weight class).
- Remove the Sinkhorn scaling vectors from tick state and snapshots; keep them only inside the S02 test path.
- Route origin excess supply (output plus arrivals minus sales) into merchant inventory. Take the unbalanced-transport path out of the default system order.
- The 8.2 rule is now the only origin clearing mechanism; ADR 0019 covers its stability.

**Tests.**

- Small worlds (5 to 20 provinces, 3 goods): with kappa large and transit zero, the fixed point matches the S02 full solve to 1e-6.
- Common-random-numbers steady-state comparison of the default and S02 on the European slice.
- Criterion benchmark: the trade system within its 19.2 budget of 30 ms on the full globe, with source\_dispatch within 8 ms.

**Consequences.** Trade responds to shocks over months rather than within the month, the intended disequilibrium. Tariff impulse responses are checked against the timing in Amiti, Redding and Weinstein (2019).

### 0017. Route-choice trade costs

Status: proposed. Date: 2026-10-24. Design sections: 5.3, 5.4, 5.5, 8.3, 8.4, 17.2 (S05), 19.4.

**Context.** P1 computes least-cost paths by multi-source Dijkstra from gateways and caches them (ceteris-geo, paths.rs). A minimum over paths is not differentiable in edge costs, makes the gateway factorization of the trade kernel an approximation, and reroutes trade discontinuously when a chokepoint closes.

**Options.** (1) Keep least-cost paths. (2) k shortest paths with logit weights. (3) Route choice with Frechet route shocks (Allen and Arkolakis 2022): tau^(-theta) = (I - A)^(-1), with A = t^(-theta) edge by edge.

**Decision.** Option 3. It sums over all routes in closed form, makes the gateway factorization exact, is differentiable, and gives closed-form welfare effects for infrastructure (5.5). Option 2 is heuristic and still jumps when the set of k paths changes. Least-cost paths remain as S05 and drive the freight-cost map mode.

**Deferral (owner, 24 Oct 2026).** Route choice is deferred past P2: P2 keeps least-cost legs (S05) with the gateway kernel, and the route-choice module stays in ceteris-geo and ceteris-trade, tested and off (Network.route\_taus = None; P44 unused). On the P1 graph no route dispersion in P44's prior brings the spectral radius of A below 1. The revisit trigger is the P8 corridor network.

**Migration.**

- ceteris-geo: add a route-choice module beside paths.rs. Edge iceberg factor t(k,l) = 1 + c(k,l) / p\_ref(g) from the existing per-edge cost components (P04), at each good's reference price, updated annually.
- Build A per (theta class, weight class). Assert spectral radius below 1 at pack build and on every edge change, and fail loudly: the fix is coarser edges or a different class, never clipping.
- Solve (I - A)x = b with a sparse iterative method warm-started from the last solve, from each gateway (international factor) and within each country (domestic factor). Never form the inverse.
- Expected transit time per (o,d) is the route-probability-weighted time, from one extra pair of solves with the time-weighted matrix; it feeds the shipment timing wheel.
- Tariffs and non-policy frictions apply on gateway-to-gateway links only.
- Keep paths.rs and its cache for S05 and the map mode.

**Tests.**

- On a toy graph, route-choice costs converge to least-cost path costs as theta grows.
- Finite-difference check of the derivative of tau with respect to an edge cost.
- Scaling one edge's cost up to closure changes tau continuously.
- Spectral-radius assertion in the pack property tests.

**Consequences.** Deferred past P2, so no golden hash changes in P2. When route choice is revisited, ceteris-geo golden hashes change, the trade kernel (0016) uses route-choice factors directly, and 5.5's infrastructure planner can score links with the Allen-Arkolakis welfare formula.

### 0018. Sector-specific trade elasticities

Status: proposed. Date: 2026-10-24. Design sections: 8.3, 17.1 (P01), 19.4, 18 (H2).

**Context.** One theta for every sector is below current practice. Sector estimates differ by an order of magnitude (Caliendo and Parro 2015), and that dispersion drives the welfare and reallocation effects of tariff counterfactuals, the product's core experiments.

**Options.** (1) A single theta. (2) Theta per sector, each with its own kernel. (3) Theta per sector, grouped into 3 to 5 classes that share kernels.

**Decision.** Option 3. Kernels and route-choice matrices are built per (theta class, weight class), so cost scales with classes, not sectors. Values are estimated outside the simulation (0022).

**Migration.**

- P01 becomes a per-sector vector with a class map. Until the estimation runs, use Caliendo-Parro values mapped to ICIO sectors for the 10 P2 goods.
- Key the kernel cache by (theta class, weight class).
- Class boundaries are an agent default under 22.2, recorded in their own ADR.

**Tests.** H2 per class: PPML on simulated flows recovers each class's theta within its prior band.

**Consequences.** More kernels in memory (classes x weight classes); the 19.2 trade line is budgeted for 5 theta classes.

### 0019. Expected inflation in price and wage rules

Status: proposed. Date: 2026-10-24. Design sections: 8.2, 8.7, 12.2, 15.3 (V1).

**Context.** The price rule (8.2) and the wage rule (8.7) responded only to real excess demand. With explicit money (A9), nothing anchors the nominal side: the long-run Phillips curve is not vertical, a permanent change in money growth has permanent real effects, and P5's monetary policy would act only through demand.

**Options.** (1) Keep the rules real and rely on the central bank. (2) Add expected inflation from the heuristic expectations of 12.2, as Poledna et al. (2023) do for firm prices.

**Decision.** Option 2 in both rules, with unit pass-through. Unit pass-through is a homogeneity restriction, not a parameter.

**Migration.**

- Price rule: change in ln p = pi^e + kappa1 (D - Q)/Q - kappa2 (S - S\*)/S\*.
- Wage rule: wage growth = pi^e + f(vacancies / unemployed); downward rigidity applies to nominal wages.
- In P2, with numeraire money, pi^e is the adaptive expectation of the numeraire price index. It stays near zero, but the rules are homogeneous now and P5 needs no rewrite.
- P2 spec: derive the stability region in (kappa1, kappa2) for the linearized market (the rule is a proportional-integral controller on the inventory gap), register the step clip, and count every tick it binds.

**Tests.**

- Homogeneity: multiplying every nominal stock and price by lambda at a tick leaves the real path unchanged to 1e-9.
- V1 neutrality (from P5): a permanent change in money growth leaves real allocations unchanged after the transition.
- Stability: kappa pairs inside and outside the derived region behave as predicted on the linearized toy market.

**Consequences.** Inflation persistence now comes partly from expectations, which the 12.2 heuristics and survey targets discipline.

### 0020. Putty-clay intermediates and within-tick rationing

Status: proposed. Date: 2026-10-24. Design sections: 7.2, 7.4, 8.2, 17.1 (P38), 17.2 (S09, S11), 19.4.

**Context.** Leontief intermediates fixed at 2000 ICIO coefficients are right for months and wrong for decades: an open-ended run would carry the 2000 input-output structure to 2150. Separately, shortages propagated one input-output layer per month (19.4), a timing artifact rather than a model of supply chains.

**Options.** (1) Fixed Leontief. (2) CES intermediates with one elasticity at all horizons. (3) Putty-clay: coefficients chosen per capacity vintage at investment time, Leontief once installed.

**Decision.** Option 3, with option 1 kept as S09. Rationing follows Pichler et al. (2022) within the tick: priority by default, proportional and mixed as S11.

**Migration.**

- Enterprise state holds a capacity-weighted coefficient vector aggregated over vintages; input purchases read it instead of the national ICIO row.
- P2 has one vintage (2000 ICIO), so the vintage structure leaves P2 output unchanged; the coefficient choice lands with investment (7.4).
- Replace one-layer-per-tick shortage propagation with the within-tick rationing pass, using inventories as the buffer.

**Tests.**

- With one vintage, golden hashes equal S09 (fixed Leontief) bit for bit.
- In a toy supply chain, an upstream shortage reaches downstream output within the tick, and the priority order is respected.

**Consequences.** The rationing change alters P2 output; the vintage change does not until investment exists. P38 enters the register.

### 0021. PIGL demand and a household consumption rule

Status: proposed. Date: 2026-10-24. Design sections: 8.6, 17.1 (P06, P07, P37), 17.2 (S03), 19.4.

**Context.** Non-homothetic CES (Comin, Lashkari and Mestieri 2021) has no closed form: each pop needs a root-find for its utility index and quadrature over its lognormal income, which contradicts the closed-form demand directive of 19.4. The design also had no rule splitting income between spending and saving, though MPCs are a target and this is the main channel through which fiscal and monetary policy reach demand.

**Options.** (1) Non-homothetic CES with Newton and Gauss-Hermite quadrature. (2) PIGL in the multi-sector form of Alder, Boppart and Muller (2022), which aggregates exactly through E\[y^(1-epsilon)\], closed form under lognormal income. (3) Stone-Geary.

**Decision.** Option 2 by default, option 1 as S03. Total spending follows the buffer-stock rule of 8.6 (P37).

**Migration.**

- Replace the category-demand kernel with PIGL shares computed from each pop's income moments (mu, sigma^2).
- At initialization, back out the PIGL category weights from 2000 budget shares by exact inversion, so they are data-determined. The PIGL price and Engel parameters take P46 and P47; P06 stays the goods substitution within a PIGL category and P07 the S03 row.
- Add the consumption rule with m = 1 and lambda = 0 in P2 (spend current income); its parameters switch on in P5.

**Tests.**

- A pop's aggregate demand equals Monte Carlo integration over its lognormal income to 1e-6.
- Adding-up: category spending sums to total spending exactly.
- Engel curves across the 2000 cross-section: the food share falls and the services share rises with income.

**Consequences.** Demand stays closed form and cheap. Structural change runs through PIGL's Engel parameter rather than per-category income elasticities.

### 0022. Uniqueness check, static nesting gate and direct estimation

Status: proposed. Date: 2026-10-24. Design sections: 3.4, 15.2, 15.3 (V1), 17.1, 21.1 (P2); roadmap P2 gate.

**Context.** With agglomeration (A5) and trade, and with migration from P4, the spatial equilibrium can be non-unique; then "within 1% of the GE reference" is ill-posed, since both models can be right and sit in different equilibria. Reference models with perfect foresight (Caliendo-Dvorkin-Parro, Kleinman-Liu-Redding) cannot be matched on transition paths by agents with heuristic expectations. And theta\_g, border frictions and mode costs are identified by gravity equations the model nests, so estimating them by simulation wastes the run budget.

**Decision.**

1. Before V1, evaluate the uniqueness condition of Allen, Arkolakis and Li (2024), a spectral-radius bound on the matrix of elasticities built from theta, agglomeration and (from P4) migration dispersion, at the tested parameters. Where it fails, V1 starts both models from the same point and reports which equilibrium each reaches instead of a 1% gap.
2. V1 compares steady states with the custom static GE. Comparisons with CDP and KLR are sign and rank only, as the roadmap already states.
3. theta\_g (0018), border frictions (P05) and mode cost ratios (P04) are estimated by PPML structural gravity on 2000 BACI with the route-choice cost structure, in the ceteris-calibrate crate (`ceteris gravity`), outside simulation-based inference.

**Migration.**

- Add the spectral check to the ceteris-reference crate (`uniqueness`) and call it from the V1 test (`p2_gate`).
- Add the PPML estimation; its outputs enter the pack with provenance "data", not "calibrated".
- Update the gate text in the P2 SPEC.md.

**Tests.** The gate itself; the PPML code reproduces published estimates on a published sample before it runs on the pack.

**Consequences.** The P2 gate is well posed, and fewer parameters enter joint inference in P9.

### 0096. Forward-mode derivatives in the Rust core

Status: proposed. Date: 2026-10-24. Design sections: 15.2, 15.7, 17, 19.4.

**Context.** Section 15.2 planned gradient-based tuning on JAX relaxations of the price, demand and trade kernels: a second implementation of the core, in Python, that would drift from the first. Gradient-based calibration of agent-based models is now practical: Hamid et al. (2025) calibrate a differentiable supply-chain ABM by variational inference and reach in 300 model evaluations a loss that approximate Bayesian computation had not reached in 30,000. After ADRs 0015 to 0021 the per-tick economic kernels are smooth and closed form: the gravity normalization, route-choice solves, PIGL demand, and the price and wage rules.

**Options.** (1) Reimplement the kernels in JAX. (2) Reverse-mode automatic differentiation in Rust with Enzyme. (3) Forward-mode dual numbers through a float trait. (4) Finite differences under common random numbers.

**Decision.** Option 3, with option 4 as its test. Calibration needs derivatives with respect to about 10 parameters (the top Sobol indices), where forward mode is cheap and needs no tape. Reverse mode over a 300-tick run with 100,000 pops would store the whole trajectory, and Enzyme needs a nightly LLVM plugin against the pinned stable toolchain. Option 1 duplicates the core.

**Migration.**

- Add a Real trait in ceteris-core (arithmetic, mul\_add, exp, ln, powf, smooth max and min), implemented by f64 and by Dual\<N>: a value plus N tangents, N up to 16, stored contiguously for SIMD.
- Write the P2 economic kernels and the route-choice solve generic over Real. Release builds instantiate f64 only, so derivatives cost nothing outside calibration.
- Give non-smooth operations smooth versions in calibration builds only: the price-step clip becomes a soft clip, and priority rationing a softmax ordering with a temperature. Discrete events (default, entry, exit) are held at their primal outcome, and gradients are reported as conditional on that event path.
- Ledger balances stay fixed-point i64 without tangents; tangents live on prices, quantities and parameters.
- Gradients of summary statistics reach analysis through the same Arrow boundary as other outputs.

**Tests.**

- The f64 instantiation reproduces the non-generic golden hashes bit for bit.
- Dual gradients match central finite differences under common random numbers to 1e-6 relative on the P2 golden scenario.
- A criterion benchmark of Dual<10> against f64 sets the calibration-build budget in the P2 spec.

**Consequences.** Variational inference and gradient-based MCMC become available in P9 for the smooth blocks without emulators; no JAX model remains (the GE reference is `ceteris-reference`, ADR 0113). Calibration builds differ from release builds in the soft clips and rationing, so each posterior is checked by re-running its mean in release mode.

### 0024. Ensemble lanes

Status: proposed. Date: 2026-10-24. Design sections: 4.5, 14.1, 14.4, 15.2, 19.4.

**Context.** Section 19.4 ran one single-threaded run per process. Ensembles (32 seeds by default), common-random-number pairs and calibration batches are most of the compute, and every process reads the same kernels and route-choice factors from memory again. Current agent-based frameworks vectorize across runs instead; ABMax, for example, runs many similar models in parallel with JAX's vmap.

**Options.** (1) One process per run. (2) Lanes: an inner array dimension indexing ensemble members, advanced in lockstep in one process. (3) GPU-resident ensembles.

**Decision.** Option 2, with the lane count L a const generic: L = 1 for interactive play, 8 to 32 for ensembles. Option 3 stays the Metal fallback for the trade products only, under the GPU rules of 19.4.

**Migration.**

- State arrays gain lane as the innermost dimension, after goods. P2's entities are static, so every table is rectangular in lanes.
- RNG streams already key on the seed, so lane k uses seed k and no stream changes.
- Read-only data (pack, kernels, route-choice factors) is stored once; a lane whose edges differ after an intervention gets a copy-on-write factor.
- Trade products widen from (provinces x goods) to (provinces x goods x lanes), so gateway products become larger matrix-matrix products.
- From P4, lanes can diverge in their entity sets (enterprise entry, wars): entity tables carry per-lane existence masks, and work for absent entities is masked, not skipped.
- Lanes compose with 0096: tangents and lanes are both inner dimensions, and a calibration build uses lanes for parameter draws or tangents, not both at once.

**Tests.**

- Lane k of a vectorized run has the same state hashes as a scalar run with seed k at every snapshot.
- A common-random-number pair in two lanes reproduces the paired effects of two scalar runs.
- A benchmark of run-ticks per second against the process-per-run baseline.

**Consequences.** Snapshots store lanes as separate branches. The throughput gain is measured in P2; it is expected from memory traffic rather than arithmetic.

### 0025. Sequence-space HANK reference for the macro block

Status: proposed. Date: 2026-10-24. Design sections: 3.4, 8.6, 9, 15.3 (V1), 21.1 (P5).

**Context.** Trade and migration nest into Caliendo-Dvorkin-Parro and Kleinman-Liu-Redding, but household saving, money and fiscal policy had no reference model, so P5's acceptance rested on plausible signs and lags. The standard tool for heterogeneous-agent macroeconomics is the sequence-space Jacobian method (Auclert, Bardoczy, Rognlie and Straub 2021), which solves linearized HANK models in seconds and has an open-source Python implementation. Auclert, Rognlie and Straub (2024) show that intertemporal MPCs summarize the household block for aggregate demand.

**Options.** (1) No reference; plausibility only. (2) A representative-agent New Keynesian reference. (3) A small-open-economy HANK per state, solved in sequence space.

**Decision.** Option 3. A representative agent cannot match MPCs and would fail the transfer experiments that matter most for fiscal counterfactuals.

**Migration.**

- P2: add a harness that computes the intertemporal MPC matrix of the consumption rule (8.6) from simulated one-off transfers to each pop group. It runs even with m = 1 and lambda = 0.
- P5: build the reference in the ceteris-reference crate with each state's income process, Taylor rule (P35), fiscal rule and trade openness from its initial data, and calibrate its household block to the same intertemporal MPC targets as P37.
- V1 macro test: with perfect foresight in place of heuristics (S04) and frictions off, impulse responses of output, consumption and inflation to policy-rate and transfer shocks lie within tolerance of the reference for small shocks; with heuristics on, signs and peak timing only.

**Tests.** The harness reproduces the closed-form case: with m = 1 and lambda = 0 the intertemporal MPC matrix is the identity.

**Consequences.** P5 gets a quantitative gate. The reference is linear, so it checks small shocks only; crises stay judged by the frequency targets of 9.13.

### 0026. Firm heterogeneity within cells

Status: proposed. Date: 2026-10-24. Design sections: 7.2, 7.4, 8.3, 15.3 (V2), 17.1.

**Context.** One enterprise per (province, sector, ownership) has no extensive margin of trade, no firm-size distribution and no exporter selection. The V2 firm-size target had to be dropped, and trade liberalization worked only through the intensive margin.

**Options.** (1) One representative enterprise per cell. (2) Explicit firms. (3) A Pareto distribution of productivity within each cell, aggregated in closed form (Melitz 2003; Chaney 2008).

**Decision.** Option 3. Explicit firms multiply agents and parameters for little out-of-sample gain, which 15.7 already excludes. With Pareto productivity of shape k\_g and CES varieties of elasticity sigma\_g (k\_g > sigma\_g - 1), aggregate flows keep the gravity form of 8.3 with trade elasticity k\_g, times a fixed-export-cost term:

```latex
X_{od,g} \propto \tau_{od,g}^{-k_g} \, f_{od,g}^{\,1 - k_g / (\sigma_g - 1)}
```

Fixed export costs vary by border crossing, so they multiply the kernel on gateway links and the factorization of 8.3 is unchanged. Each cell's firm count, size distribution and exporter share are closed form.

**Migration.**

- Reinterpret theta\_g (P01) as the Pareto shape k\_g; the PPML estimates of 0022 are unchanged.
- Add sigma\_g (P50), fixed export costs by border (P51, identified by exporter shares) and entry costs (P52, identified by firm counts by size class) to the register.
- Gateway links carry the fixed-cost factor beside tariffs.
- Each enterprise cell stores its mass of firms and productivity lower bound; the quarterly investment step (7.4) becomes free entry and exit in that mass.
- Restore the V2 firm-size target as the Pareto tail within cells.

**Tests.**

- A toy cell's exporter share and size distribution match the closed form.
- With fixed export costs at zero, flows equal the 8.3 kernel.
- PPML on simulated flows recovers k\_g.

**Consequences.** Liberalization now moves the extensive margin. Granular shocks from individual large firms (Gabaix 2011) are still absent; they could later come from drawing each country-sector's largest firms explicitly from the tail.

## 26. Gap analysis, 24 Oct 2026 (ADRs 0027-0054)

A review of the design against current literature and the most complete existing models (data-driven macro ABMs such as Poledna et al. 2023 and Wiese et al. 2024, multi-country policy models, integrated assessment models, dynamic spatial models, WCDE demographic projections, ViEWS, and the strategy games the project draws on) produced 28 proposed ADRs. Their full text is in the repository's adr folder, in pull requests 3 to 7; this section indexes them. Sections of this document change when each ADR is accepted.

**Corrections to existing decisions.** The single climate damage default with a level-versus-growth switch (13.2, S10) is replaced by three damage lanes (0036): Kotz, Levermann and Wenz (2024) was retracted in December 2025, and the remaining estimates differ by up to an order of magnitude. V-Dem v16 (March 2026) supersedes v15. One pack cannot serve both research validation and release, because the best validation data are often non-commercial, so the pipeline gets two profiles (0050). Wright's law alone sets costs but not adoption speed (0035). A single long-run trade elasticity cannot reproduce short-run tariff and sanctions responses (0027).

| ADR | Decision | Phase | PR |
| --- | --- | --- | --- |
| 0027 | Short- and long-run trade elasticities (sourcing inertia) | P2 | 3 |
| 0028 | Oil and gas market structure (OPEC rule, storage, LNG) | P2, P8 | 3 |
| 0029 | Food: competitive storage, export restrictions, famine | P2, P4 | 3 |
| 0030 | Variable markups within cells (ablation-gated) | P2 | 3 |
| 0031 | Structural transformation and informality | P2, P4 | 3 |
| 0032 | Cohort demography: education transitions, fertility tempo, households | P4 | 3 |
| 0033 | Mortality by cause group | P4, P8 | 3 |
| 0034 | Power sector via residual load duration curves | P2 stub, P8 | 4 |
| 0035 | Technology diffusion (FTT) and energy policy instruments | P8 | 4 |
| 0036 | Climate damage lanes | P8 | 4 |
| 0037 | MESMER regional climate, tipping elements, sea level | P8 | 4 |
| 0038 | Water, air pollution and land use | P8 | 4 |
| 0039 | Automation and AI as a scenario lane | P8 | 4 |
| 0040 | Housing supply elasticity and mortgage underwriting | P5 | 5 |
| 0041 | Multinational production and profit shifting | P5 | 5 |
| 0042 | Public capital and fiscal dynamics | P5 | 5 |
| 0043 | Sovereign debt restructuring and creditors | P5 | 5 |
| 0044 | Pensions, life-cycle saving and wealth dynamics | P5 | 5 |
| 0045 | Economic statecraft: critical minerals, export controls, sanctions evasion | P5, P6 | 5 |
| 0046 | Political economy channels | P6 | 6 |
| 0047 | Trade agreements and international organizations | P6 | 6 |
| 0048 | Grounding the war module in modern evidence | P7 | 6 |
| 0049 | Explainability panels and event chronicle | P3 onward | 6 |
| 0050 | Research and release pack profiles; sources to audit | Pipeline, now | 7 |
| 0051 | Fat-tailed hazard library | P7 to P9 | 7 |
| 0052 | Forecast scorecard: vintages, hard benchmarks, proper tests | P3, P9 | 7 |
| 0053 | Robust inference and model selection | P9 | 7 |
| 0054 | Explicit exclusions | All | 7 |

**Register and switches.** These ADRs take register IDs P53 to P92 and switches S12 to S18; P44 to P47 went to P2 (17.1), P48 and P49 stay free, and P50 to P52 belong to 0026. Most new entries are data or literature priors. Free behavioural parameters still count against the cap of 60: a mechanism that fails the ablation rule (15.7) moves to the extended profile (23.1) instead of growing the lab register.

**Highest leverage, in order.** Cohort education demography (0032); the power sector and oil market (0034, 0028); technology diffusion (0035); climate damage lanes (0036); sourcing inertia (0027); the scorecard with vintages (0052); pack profiles (0050); the food system (0029); sovereign restructuring (0043); housing underwriting (0040); trade agreements (0047); political economy channels (0046); the hazard library (0051); and robust inference (0053).

## 27. Beyond the economic core (ADRs 0055 to 0087)

A second research pass looked outside the economic core for missing mechanisms and missing couplings between modules. ADRs 0055 to 0080 come from that scan; 0081 to 0087 fill gaps between existing modules. Each PR adds only new files under adr/, so they merge in any order.

| ADR | Decision | Phase | PR |
| --- | --- | --- | --- |
| 0055 | Cohort values: secular and emancipative orientations | P4, P6 | 8 |
| 0056 | Generalized trust and civil society | P6 (P4 pandemic compliance) | 8 |
| 0057 | Gender norms, sex ratios at birth, marriage markets | P4 | 8 |
| 0058 | Religion switching and identity salience | P4, P6 | 8 |
| 0059 | Learning quality, not only years of schooling | P4 (P2 reads 2000 values) | 8 |
| 0060 | Crime, homicide and organized crime | P6, P7 | 8 |
| 0061 | Health-system capacity | P4, P8 | 9 |
| 0062 | Chronic disease risk and antimicrobial resistance | P4, P8 | 9 |
| 0063 | Addiction epidemics | P6 | 9 |
| 0064 | Child nutrition and the care burden of ageing | P4 | 9 |
| 0065 | True and reported statistics | P3, P5, P6, P9 | 10 |
| 0066 | Leaders, tenure and succession | P6 | 10 |
| 0067 | State capacity: building, eroding, collapsing | P6 | 10 |
| 0068 | Subnational governance, land tenure, clientelism, electoral integrity | P6 | 10 |
| 0069 | Non-state armed actors, insurgency finance, terrorism | P7 | 10 |
| 0070 | Development finance actors | P5, P6 | 10 |
| 0071 | Information technology and the information environment | P4, P6 | 11 |
| 0072 | International order, peace operations, courts, soft power | P6, P7 | 11 |
| 0073 | Proliferation, the Arctic, frozen conflicts, intelligence | P7, P8 | 11 |
| 0074 | Settlement hierarchy and urban systems | P4 | 12 |
| 0075 | Infrastructure interdependence and cascading failures | P7, P8 | 12 |
| 0076 | Groundwater, shared river basins, glaciers | P8 | 12 |
| 0077 | Heat, wildfire, fisheries, deforestation frontiers | P8 | 12 |
| 0078 | Chronicle narration, storyteller camera, legends | P3 onward | 13 |
| 0079 | Red-team branches and surrogate sliders | P3, P9 | 13 |
| 0080 | Polycrisis coupling audit | Gates from P5 | 13 |
| 0081 | Seasonality and weather-driven demand | P2, P8 | 14 |
| 0082 | Cohort experience effects on expectations and risk-taking | P5 | 14 |
| 0083 | Policy implementation and diffusion | P5, P6 | 14 |
| 0084 | Price controls, subsidies, parallel markets, currency substitution | P5 | 14 |
| 0085 | Strategic reserves and stockpiles | P5, P7, P8 | 14 |
| 0086 | Reconstruction and recovery | P7, P8 | 14 |
| 0087 | Synthetic observables for validation | P3, P9 | 14 |

Register: P93 to P142 (society P93 to P104, health P105 to P110, state P111 to P119, information and geopolitics P120 to P125, cities and environment P126 to P133, cross-cutting P134 to P142). Switches: S19 cohort values (0055), S20 religion switching (0058), S21 published statistics differ from truth (0065); with each off, golden hashes are unchanged. Amendments to earlier PRs: 0033 gains the WHO CC BY-NC-SA 3.0 IGO note (PR 3); 0050 adds the second licence scan, 0052 non-economic out-of-sample targets and a subnational satellite track, 0054 seven more exclusions (PR 7).

Highest leverage: 0065 changes the data layer from P3, so it should land before any calibration code reads observed series as truth; 0080's coupling register gates every phase from P5. Licence action now: GeoEPR carries no licence and WCDE a custom one, so request permission before P4 needs them (ADR 0050).
