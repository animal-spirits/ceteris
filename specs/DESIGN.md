# Ceteris Paribus: Design Document for a Paradox-Style Economic World Simulation

2026-10-23 · @Antonio

## 1. Executive Summary

Ceteris Paribus (code name `ceteris`) is a deterministic, forkable, agent-based world simulator that starts on 1 January 2000 and runs open-ended. Its economic core is a province-level production-and-trade model disciplined by quantitative trade theory, presented through a Paradox-style map in a native desktop app. Any state variable or parameter can be edited before a run (ex ante) or at any tick during it (ex post). It stays closed source until the restricted data sources it uses are replaced or cleared.

Six design bets carry the project:

- **Hybrid microstructure.**  Population groups (pops) and enterprises are agents trading in out-of-equilibrium markets with inventories and lags. A structural dynamic spatial general-equilibrium model (Caliendo-Dvorkin-Parro / Kleinman-Liu-Redding class) runs alongside as a reference model: in the frictionless limit the agent model must reproduce it.
- **Every province is a market.**  Each of about 4,000 provinces prices every good. Shipments between provinces are solved each tick as an entropy-regularized optimal transport problem that is mathematically equivalent to structural gravity, so literature trade elasticities map one-to-one onto model parameters.
- **Institutions are a continuous state vector** , initialized from V-Dem, with explicit political parties built from interest-group coalitions. Labels such as 'electoral autocracy' are derived views, never mechanics.
- **Every run is reproducible and forkable.**  Counter-based random numbers keyed by entity and tick, periodic snapshots and an event log let any branch be replayed exactly, and let interventions be compared against their parent under common random numbers. Ensembles report distributions, never single trajectories.
- **Data-driven initial conditions, few free parameters.**  Following the data-driven ABM program of Poledna et al. (2023) and Wiese et al. (2024), the world is initialized from 2000 data (OECD ICIO, BACI, gridded population, V-Dem). Behavioural parameters are estimated on 2000-2010 and tested out of sample on 2011-2024, a window containing the 2008 crisis, the euro crisis, COVID and two tariff wars.
- **Accounting is law.**  Stock-flow consistency, goods conservation and population conservation are asserted every tick; a violation halts the run.

Headline specification:

| Dimension | Default | Rationale |
|---|---|---|
| Start / horizon | 1 Jan 2000, open-ended | 25 years of out-of-sample history; long runs need climate, demography and stable growth |
| Economic tick | 1 month | Captures harvests, prices, credit and policy cycles |
| Military sub-tick | 1 day | Movement, battles, sieges |
| Provinces | 4,796 first-level regions from Natural Earth at 2000 borders; those over 250,000 km² split along county borders | Modern admin units carry the subnational data |
| Markets | Every province prices every good | Finest price geography; costs about 10x more trade-solver work than hubs |
| Goods | 45 sectors aligned with OECD ICIO, with energy and food staples split out | Direct mapping to input-output data |
| States | About 195 | Current sovereign states plus a handful of de facto entities |
| Pops | About 100,000: province x ethnic group x education x labour status | Matches modern data (GeoEPR, census education, labour surveys) |
| Hardware | MacBook Pro, M-series Pro, 18-36 GB | Desktop only; sets all budgets |
| Performance target | 0.5 to 1 simulated year per second, single run | Interactive play; ensembles and calibration overnight |

Stack: a Rust simulation core embedded in a Tauri desktop app. The map is drawn with MapLibre and a GPU fill layer today and moves to a native Rust renderer on wgpu. Python (uv) runs the data pipeline, the reference model and calibration via PyO3. The project is closed source for now; it opens under Apache-2.0 / MIT once every bundled dataset permits redistribution. Build order puts the map, accounting, markets, trade and the counterfactual workbench first, because those define the product (Section 21).

## 2. Vision, Scope and Design Principles

The product is a laboratory first and a game second: a living world map where the user watches economies, states and populations co-evolve from 2000 onward, intervenes, and compares branches of history quantitatively.

### 2.1 Modes of use

| Mode | User role | Typical question |
|---|---|---|
| Observer | Watches; no control | What does the world look like in 2060 under baseline dynamics? |
| God mode (ex ante) | Edits initial state or parameters, then runs an ensemble | What if China never joins the WTO, or the trade elasticity were 8 instead of 5? |
| God mode (ex post) | Pauses at tick t, edits anything, forks a branch | What if Lehman is rescued in September 2008, given everything up to then? |
| Steward (later) | Controls one state's policy levers; AI runs the rest | Can Brazil raise its growth rate with a different tariff and infrastructure policy? |
| Batch / scripted | Python API runs experiment grids headless | Distribution of 2030 outcomes across 64 seeds under three tariff regimes |

### 2.2 Era choice

The start is **1 January 2000** . The engine stays era-agnostic (goods, institutions and maps live in versioned data packs), and the pipeline can build a pack for any start year from 1995 to 2022 (the span of the OECD ICIO 2025 edition); 2000 is the primary scenario. Reasons:

- **Data replaces judgment.**  OECD ICIO input-output tables and CEPII BACI trade flows start in 1995; gridded population, subnational GDP, V-Dem, UCDP conflict events and UN demographic data all cover 2000. Few hand-made decisions are needed.
- **A long out-of-sample window.**  2000-2024 contains China's WTO accession, the commodity supercycle, the 2008 financial crisis, the euro crisis, the Arab Spring, COVID, the 2022 energy shock and sanctions, and the 2018 and 2025 tariff rounds: natural tests of whether the model responds like the world did.
- **Open-ended future.**  From 2025 onward the model runs without a script, so long-run stability, demography and climate feedback become first-class requirements.

### 2.3 In scope

Economy (production, consumption, input-output, trade, transport, money, banking, sovereign debt, fiscal and monetary policy), population (demography, education, labour status, ethnicity, migration), technology (sector productivity growth and diffusion), politics (institutions, interest groups, parties, elections, legitimacy, unrest, regime change, secession), diplomacy and conflict (treaties, alliances, sanctions, interstate and civil war), climate (emissions, temperature, damages) and exogenous shocks (pandemics, disasters, commodity discoveries).

### 2.4 Non-goals for version 1

- Named individuals and leaders' personalities; leaders exist only as stochastic attributes of states and parties.
- Named technologies; technology is sector productivity (decision of 23 Oct 2026).
- LLM-driven leaders (deferred), tactical battle maps, 3D graphics, multiplayer, cloud hosting.
- Scripted events and historical railroading: post-2000 history is a validation target, not a script.

### 2.5 Design principles

- **Mechanism over script.**  No event fires because of a date; it fires because state variables crossed a threshold or a hazard realized.
- **Every number has provenance.**  Each parameter is tagged data, literature prior, or calibrated, with its source and an ID in the degrees-of-freedom register (Section 17).
- **Conservation laws.**  Money, goods and people are never created or destroyed except by explicit, logged processes.
- **Equilibrium nesting.**  Behavioural rules collapse to a known optimizing equilibrium when frictions are removed, giving a built-in correctness test and a bridge to the literature.
- **Modularity.**  Each module is a set of systems over shared component arrays with a written spec; alternative modules can be swapped to test model uncertainty.
- **Explainability.**  Every price, flow and decision can be decomposed in the UI into its drivers.
- **Reproducibility.**  Same data pack, parameters, seed and intervention list give bit-identical output.
- **Two tiers per module.**  Minimal first (closes the accounting loop), rich later. The world is always runnable.
- **Open by construction.**  Code, specs and every bundled dataset are redistributable; restricted data is excluded rather than worked around.

## 3. Modeling Philosophy and Global Assumptions

The model is a hybrid: agent-based dynamics for everything that moves, anchored to a structural equilibrium model for everything that can be measured against the trade and spatial literature.

### 3.1 Why hybrid

| Approach | Strength | Fatal weakness for this project |
|---|---|---|
| Quantitative spatial GE / dynamic hat algebra | Clean, data-matched counterfactuals for trade and migration | No disequilibrium, crises, path dependence, wars or revolutions; politics is exogenous |
| DSGE / HANK | Expectations, monetary policy | Representative or limited heterogeneity, linearized around a steady state, no geography |
| Classical ABM (stylized) | Emergence, crises, rich heterogeneity | Many free parameters, weak calibration, hard to trust |
| Data-driven ABM (Poledna 2023 lineage) | Calibrated from accounts, competitive forecasts | Built for single economies over short horizons, no politics or war |
| Paradox games (Victoria 3) | Legible, playable, multi-system | Ad hoc mechanics, not calibrated, not reproducible for research |

Ceteris Paribus takes the agent architecture and legibility of the last row, the data discipline of the fourth, and uses the first as its reference model and validation target.

### 3.2 Unit of agency

Individuals are too many (about 6.1 billion people in 2000) and add little. The behavioural unit is the **pop** : a representative group defined by (province, ethnic group, education level, labour status). Within-pop heterogeneity is carried as moments (lognormal income with tracked mean and dispersion, age structure), so inequality and poverty headcounts are computable without simulating individuals. Enterprises are aggregated per (province, sector, ownership type: domestic private, state, foreign). States, central banks, commercial banks, interest groups and parties are explicit agents.

### 3.3 Global assumptions

Each assumption has an ID referenced by module specs. Assumptions marked (T) are testable switches: the alternative is implemented and compared.

| ID | Assumption | Alternative / test |
|---|---|---|
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

Every behavioural rule is specified as a relaxation of an optimizing benchmark with an explicit friction parameter. Setting adjustment speeds high, information lags to zero and dispersion parameters to their structural values must reproduce the benchmark. Examples: merchant logit routing collapses to structural gravity; inventory-based pricing converges to Walrasian clearing; dynamic migration choice collapses to the Caliendo-Dvorkin-Parro migration block. This gives every module a closed-form correctness test and makes literature elasticities usable as priors.

## 4. System Architecture and Simulation Loop

The engine is a data-oriented entity-component system: entity tables stored as structure-of-arrays, advanced by an ordered pipeline of systems, with every discrete decision written to an event log and every flow written to a double-entry ledger.

*[Diagram: system architecture · core, data, clients, calibration loop]*

The map client sends edits straight into the systems pipeline and reads results back through the query API; calibration closes the loop by writing posterior parameters into the next data pack.

### 4.1 Layers

- **Data packs** : versioned, built offline by the Python pipeline (geography, initial populations, capital, technology tree, institutions, parameter priors).
- **World state** : component arrays keyed by stable entity IDs.
- **Systems pipeline** : pure functions of (state, parameters, RNG streams) that mutate state in a fixed order.
- **Ledger and event log** : every monetary and goods flow as a balanced transaction; every discrete decision (law change, war declaration, firm entry) as an event.
- **Snapshot and branch store** : periodic full snapshots plus logs; any tick of any branch is reconstructible.
- **Query API** : Arrow-based reads for the map client and Python.
- **Clients** : web map, Python notebooks, CLI batch runner.

### 4.2 Entity model

| Entity | Key components | Approx. count |
|---|---|---|
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
|---|---|
| Daily (sub-tick, only while any war is active) | Army movement, battles, sieges, naval engagements, blockade status |
| Monthly (main tick) | Production, labour markets, consumption, price updates, trade flows, payments and credit, taxes, migration decisions, unrest, law-change progress, diplomacy |
| Quarterly | Enterprise investment and entry/exit, bond issuance |
| Annual | Cohort aging, fertility and mortality rate updates, productivity catch-up step, climate emulator step, elections calendar, census outputs, harvest realization for annual crops |

### 4.4 Monthly tick pipeline

- Apply scheduled interventions and exogenous shock tape for tick t.
- Observation update: agents receive lagged, noisy signals of prices, wages and threats.
- Demography: births, deaths, aging (annual components on their month).
- Labour market: vacancies, matching, wages, occupational switching.
- Production: enterprises choose output, buy inputs, produce; harvest and mining realizations.
- Consumption demand: pops and states form demand at posted prices.
- Province markets: local supply meets demand; inventories absorb gaps; prices update.
- Inter-province trade: solve shipments, dispatch to transit pipeline, deliver arrivals.
- Finance: settle payments, interest, loans, defaults, bank balance sheets, FX.
- Fiscal: tax collection, government spending, debt service.
- Investment and construction (quarterly months): capacity expansion, infrastructure.
- Politics: clout, approval, ideology drift, unrest, law progress, regime events.
- Diplomacy and military: crises, bargaining, mobilization, war start; daily sub-ticks if at war.
- Migration: realized moves between provinces and abroad.
- Accounting assertions; metrics; snapshot if t mod 12 = 0.

### 4.5 Determinism

- **RNG** : counter-based (Philox 4x32-10). Each draw is a pure function of (master seed, tick, system ID, entity ID, draw index). No global RNG state, so adding an entity or a system does not shift any other stream.
- **Floating point** : f64; no fast-math; parallel reductions use fixed-shape pairwise trees so results do not depend on thread count.
- **Ordering** : entities iterate in stable-ID order; ties broken by ID.
- **CI check** : two runs with the same inputs must produce identical state hashes at every snapshot.

### 4.6 Accounting invariants (asserted every tick)

| Invariant | Statement | Tolerance |
|---|---|---|
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

Provinces are first-level administrative regions from Natural Earth (public domain; 4,594 units at mixed levels), remapped to 1 January 2000: South Sudan in Sudan, Serbia and Montenegro in Yugoslavia, Crimea in Ukraine. Regions over 250,000 km² are split along county boundaries from geoBoundaries (82 regions), or by Voronoi cells where counties are too few (11), giving 4,796. A county layer of 53,084 units is drawn on the map and carries raster statistics; it is not simulated. The post-2000 sovereignty changes (Timor-Leste, Montenegro, Kosovo, South Sudan) remain validation events, not scripted ones.

| Attribute | Source | Dynamic? |
|---|---|---|
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

The 2000 graph has 4,796 province anchors, 2,757 rail nodes, 2,284 ports (1,001 listed, 1,283 minor), 1,154 airports and 29,543 open-water H3 resolution-3 cells, grouped into 725 sea zones. Ten chokepoints tag their sea cells; Suez, Panama, the Turkish Straits, Gibraltar and Bab-el-Mandeb are explicit links. Edge costs are priced in the core from P04 parameters, not stored in the pack.

| Mode | Edge existence | Cost driver | Notes |
|---|---|---|---|
| Road | Network between adjacent provinces | Distance, road class, congestion, fuel | Dominant within continents |
| Rail freight | Built lines | Distance, gauge breaks, electrification | Bulk goods |
| Inland waterway | Navigable rivers, canals | Distance, seasonal depth | Bulk goods; not yet built |
| Maritime (container, bulk, tanker) | Sea lanes via chokepoints (Suez, Panama, Hormuz, Malacca, Bosporus) | Distance, bunker fuel, port capacity, freight rates | Chokepoint closures are shocks |
| Air freight | Airport pairs | Distance, jet fuel | High value-to-weight goods |
| Pipeline | Built oil and gas pipelines | Throughput, transit fees | Geopolitically exposed; not yet built (P8, with energy assets) |

Relative mode costs are calibrated priors: the ordering (sea cheapest per ton-km, then rail and waterway, then road, air far above) is imposed; magnitudes are estimated against international transport-cost data and price gaps (Section 15).

### 5.4 Generalized trade cost

For good g from province o to province d, the cost per unit is the least-cost path over the graph:

```math
c_{od,g} = \min_{\text{paths}} \sum_{e} \left( f_{e,m} \, w_g \, \ell_e + h_{e} \right) + p_{o,g} \, r \, T_{od} + \rho_{od} \, p_{o,g} + t_{od,g} + \nu_{od}
```

where f is the per-ton-km mode cost, w the good's weight per unit of value, l edge length, h transshipment and port cost, r the interest rate times transit time T (inventory carrying cost), rho the loss and disruption risk, t tariffs and other policy wedges (applied tariffs initialized from WTO and World Bank tariff data), and nu non-policy frictions (language, legal system, currency, sanctions compliance). For comparison with the GE reference model the cost maps to an iceberg factor tau = 1 + c / p. Per-unit components matter: Hummels and Skiba (2004) show per-unit shipping costs change the composition of trade, which pure iceberg models miss.

### 5.5 Endogenous infrastructure

States and enterprises propose projects (highways, rail lines, ports, pipelines) scored by expected market-access gains, following Donaldson and Hornbeck (2016), against cost. Network design is informed by Fajgelbaum and Schaal (2020) optimal transport networks: an AI planner ranks candidate links by the marginal welfare gain implied by current flows and congestion. Built infrastructure lowers edge costs, which raises market access, which changes location choices: the main spatial feedback loop of the model.

### 5.6 Licensing note

OpenStreetMap is licensed under ODbL, whose share-alike terms attach to derived databases. While the project is closed source nothing is distributed; once it opens, the OSM-derived tables ship as a separate artifact under ODbL, attributed. Every source's licence is recorded in the pack manifest (Section 16).

## 6. Population: Pops, Demography, Identity and Migration

Population is both the labour supply and the political substrate, so each pop carries economic state (education, labour status, income, wealth) and political state (ethnic identity, ideology, party attachment, grievance) in one record.

### 6.1 Pop definition

Key: (province, ethnic group, education, labour status), about 100,000 pops after dropping empty cells.

| Key dimension | Categories | Initialization source |
|---|---|---|
| Ethnic group | Politically relevant groups per country, plus 'other' | GeoEPR settlement areas and group sizes |
| Education | Below secondary, secondary, tertiary | Barro-Lee / Wittgenstein Centre attainment, subnational where available |
| Labour status | Formal wage employee, informal worker, self-employed / smallholder, business owner / employer, public sector, unemployed, inactive (students, retirees, carers) | ILO labour statistics, household surveys |

Religion is a share vector within each pop rather than a key, initialized from available national and subnational religion data, to keep the pop count bounded. Components per pop: size by age band and sex (five-year bands), income mean and log-variance, wealth by asset type, consumption basket, employer links, ideology vector, party attachment, grievance, language and religious shares.

### 6.2 Fertility and mortality

Mortality uses the Brass relational logit life table: a standard schedule shifted by a level parameter and tilted by a shape parameter.

```math
\operatorname{logit}\left(1 - l_x\right) = \alpha_{p,t} + \beta \, \operatorname{logit}\left(1 - l_x^{std}\right)
```

The level alpha responds to real income (a Preston-curve relation), health spending, epidemics, conflict and climate damages (heat). Fertility is a pop-level total fertility rate driven mainly by female education, income, urbanization and child survival, with a floor well below replacement so that sustained low-fertility regimes are reachable:

```math
TFR_{p,t} = TFR^{min} + (TFR^{max} - TFR^{min}) \cdot \sigma\!\left(\gamma_0 - \gamma_e \, e^{f}_{p,t} - \gamma_y \ln y_{p,t} - \gamma_u \, \text{urban}_{p,t} + \gamma_m \, \text{child mortality}_{p,t}\right)
```

Rates are initialized to match UN World Population Prospects for 2000; 2000-2023 estimates are the validation target. UN projections beyond 2024 are a comparison benchmark, not a target. Aging feeds directly into labour supply and pension spending, which matters for open-ended runs.

### 6.3 Education and labour status

Cohorts move up education levels at enrollment rates set by public and private education spending and household income. Workers move between labour statuses each month: matching in formal labour markets, entry to informality when formal jobs are scarce, business formation and failure, retirement by age and pension law. Skill premia emerge from sector demand for educated labour.

### 6.4 Ethnicity, religion and identity politics

Ethnic groups carry a political status from the Ethnic Power Relations data (dominant, senior or junior partner, powerless, discriminated). Horizontal inequality between groups (income and political exclusion) feeds grievance, the mechanism documented by Cederman, Gleditsch and Buhaug (2013) for civil conflict. Language and religiosity shift through the cultural transmission model of Bisin and Verdier (2001), with state policy (schooling language, discrimination, autonomy) as levers.

### 6.5 Migration

Internal and international migration share one dynamic discrete-choice model in the Artuc-Chaudhuri-McLaren / Caliendo-Dvorkin-Parro tradition. Each month a share of each pop reconsiders location; destinations are chosen by logit over expected values net of moving costs:

```math
\mu_{od} = \frac{\exp\left( (\beta \, \mathbb{E} V_d - m_{od}) / \nu \right)}{\sum_k \exp\left( (\beta \, \mathbb{E} V_k - m_{ok}) / \nu \right)}
```

Moving cost m depends on distance and travel cost, immigration law and visa regimes (free movement inside unions such as the EU), language and cultural distance, credit constraints, and diaspora networks. Expected value uses Harris-Todaro expected wages (wage times employment probability), amenities, conflict risk and climate damages. Forced displacement from conflict and disasters follows a separate hazard-driven flow toward safe neighbours. Remittances flow back as household transfers, a large financial flow for many economies. The dispersion parameter nu is calibrated to bilateral migration stocks and flow estimates; internal urbanization rates are a second target.

## 7. Production, Enterprises and Technology

Production follows the OECD input-output structure at province level, and technology is sector productivity that grows at the frontier and diffuses to followers; there are no named technologies.

### 7.1 Goods and sectors

The 45 industries of the OECD Inter-Country Input-Output tables define the goods, with a few splits where physical quantities matter for prices, food security or emissions.

| Group | Sectors (examples) | Splits |
|---|---|---|
| Primary | Agriculture, forestry, fishing; energy mining; non-energy mining | Grains, other crops, livestock; crude oil, natural gas, coal |
| Manufacturing | Food, textiles, chemicals, pharmaceuticals, basic metals, electronics, machinery, motor vehicles, other transport | None initially |
| Energy and utilities | Electricity, gas, water | Fossil, nuclear and renewable generation |
| Construction | Construction | None |
| Services | Trade, transport, IT and telecom, finance, real estate, professional, public administration, education, health | Traded through frictions, not shipments |

Each good has weight per unit of value, perishability and a consumption category; services are mostly local, with cross-border service trade handled as frictions on demand rather than physical shipments.

### 7.2 Production function

Each (province, sector) enterprise combines a CES value-added core with intermediate inputs. Intermediates are Leontief in a non-energy bundle, initialized from the country's ICIO coefficients, plus a CES energy nest so firms can substitute between fuels and electricity:

```math
Y = \min\left( \frac{VA}{a_{VA}}, \; \frac{M}{a_{M}}, \; \frac{E}{a_{E}} \right), \qquad VA = A_{s,i,t} \left[ \alpha_K K^{\rho} + \alpha_L L^{\rho} + \alpha_T T^{\rho} \right]^{1/\rho}
```

L is effective labour by education level; T is land (agriculture) or resource deposits (mining). A combines sector productivity, province agglomeration (rising with local employment density) and climate damages.

### 7.3 Enterprise behaviour (monthly)

- Forecast demand with the heuristic expectation rules of Section 12.
- Set an output target from forecast demand and an inventory-coverage rule (as in data-driven ABMs such as Assenza-Delli Gatti-Grazzini 2015 and Poledna et al. 2023).
- Post vacancies or lay off; a province matching function pairs vacancies with job seekers by education.
- Buy inputs at province prices; shortfalls cap output, so rationing propagates through the input-output network (the supply-chain mechanism behind 2021-22 shortages).
- Sell output at the province market price; profits go to owners.

### 7.4 Investment, entry and exit (quarterly)

Owners (households via wealth, states, banks, foreign investors) allocate new capital across (province, sector) options by a logit over expected return net of risk, compared to the cost of capital (a Tobin-q style rule), financed from retained earnings, bank credit, bonds and equity. Persistent losses with negative equity trigger exit. Foreign ownership is simply foreign-owned capital, so FDI and profit repatriation are ordinary flows.

### 7.5 Technology as sector productivity

Technology has three parts, all at the sector level:

- **Frontier growth.**  Each sector's frontier productivity grows with world research effort, with diminishing returns as in Jones (1995):

```math
g^{*}_{s,t} = \delta_s \, R_{t}^{\lambda} \, \left(A^{*}_{s,t}\right)^{\phi - 1}, \qquad \phi < 1
```

Because growth depends on research effort, a shrinking or aging world population slows frontier growth, the semi-endogenous mechanism studied by Jones (2022). This keeps open-ended runs from exploding or stalling arbitrarily.

- **Catch-up.**  Provinces converge toward the frontier at a rate that rises with education, trade exposure to frontier producers (Buera and Oberfield 2020), foreign ownership and institutional quality:

```math
\Delta \ln A_{i,s,t} = g^{*}_{s,t} + \lambda_{i,t} \left( \ln A^{*}_{s,t} - \ln A_{i,s,t} \right) + \varepsilon_{i,s,t}
```

- **Learning by doing in energy.**  Renewable generation and storage-intensive sectors follow Wright's law: unit costs fall with cumulative global output, the pattern documented for solar, wind and batteries by Way et al. (2022). This captures the energy transition without naming technologies.

Initial productivity gaps are backed out from 2000 output, inputs and prices; frontier growth rates per sector are calibrated to productivity accounts.

### 7.6 Structural change

Non-homothetic demand (Section 8.6) and unequal sector productivity growth (Baumol cost disease in services) jointly drive sector shares. Targets: 2000-2024 sector employment and value-added shares by country.

### 7.7 Energy and emissions

Energy sectors produce physical energy; fossil inputs carry emission factors, so CO2 and other greenhouse-gas emissions are computed from the input-output flows every month and passed to the climate module (Section 13). Carbon prices and fuel taxes are policy wedges on those inputs.

## 8. Markets, Price Formation and Trade

Every province prices every good, with inventories and adjustment lags, and goods move between provinces as the solution of an entropy-regularized transport problem that reproduces structural gravity in its equilibrium limit.

### 8.1 Market topology

Each of about 4,000 provinces holds a price p(i,g) and a merchant inventory S(i,g) per good. Political borders do not partition markets: they act through tariffs, sanctions and frictions in the cost function. Services are priced locally and traded across provinces only through demand-side frictions.

### 8.2 Price formation (stock-flow rule)

Each month, supply (local output plus arrivals) meets demand at the posted price; inventories absorb the gap; the price then moves with excess demand and with deviation from target inventory coverage:

```math
\Delta \ln p_{i,g,t+1} = \kappa_1 \frac{D_{i,g,t} - Q_{i,g,t}}{Q_{i,g,t}} - \kappa_2 \frac{S_{i,g,t} - S^{*}_{i,g,t}}{S^{*}_{i,g,t}}, \qquad S^{*} = \bar{s}_g \, D_{i,g}
```

Steps are clipped to prevent explosive oscillation. When inventory runs out, demand is rationed by priority (households' essential consumption, then intermediate users by contract, then the rest), which is how shortages propagate. A Walrasian mode solves clearing prices by Newton iteration each tick, for equilibrium-nesting tests and as a robustness switch.

### 8.3 Inter-province trade as entropic optimal transport

For each good, merchants choose shipments X(o,d). Merchant heterogeneity (contacts, contracts, idiosyncratic costs) is represented by an entropy term:

```math
\max_{X \geq 0} \sum_{o,d} X_{od} \left( \mathbb{E}p_{d} - p_{o} - c_{od} \right) - \varepsilon \sum_{o,d} X_{od} \left( \ln X_{od} - 1 \right) - \text{KL penalties on export supply and import demand}
```

The solution has the gravity form

```math
X_{od} = a_{o} \, b_{d} \, \exp\left( -\frac{c_{od}}{\varepsilon} \right)
```

with scaling vectors from Sinkhorn iterations (Cuturi 2013); the unbalanced variant (Chizat et al. 2018) lets supply stay home when trade is unprofitable. Galichon (2016) shows the equivalence of entropic transport and logit choice. Writing costs in log-iceberg units, c = theta ln tau, makes flows exactly the structural gravity of Eaton-Kortum and Anderson-van Wincoop with trade elasticity theta, so the temperature is pinned by literature estimates rather than free.

**Computation at province scale.**  A dense problem (4,000 x 4,000 x 45 goods) is too large for a laptop every month, so the solver is two-level:

- **International level** : about 600 gateway clusters (ports, border crossings, airports) exchange flows by entropic OT over the transport graph.
- **Domestic level** : each country distributes its gateway flows and its internal trade across provinces by a sparse entropic OT restricted to each province's cheapest partners (about 150).

Both levels are warm-started from last month's scaling vectors, so 5 to 10 iterations usually suffice. Estimated cost: a few hundred million operations per tick, about 30 to 60 ms on a 12-core M-series Pro, with a Metal compute path (via wgpu) as the fallback if CPU timing misses budget. The nesting is checked against the single-level solution on small worlds.

### 8.4 Transit, shipping capacity and freight rates

Shipments arrive after the path's transit time, so prices respond to news with realistic lags. Shipping capacity (container fleet, bulk carriers, tankers, rail and truck fleets) is capital owned by enterprises; freight rates rise with utilization and fuel prices and feed back into c. Chokepoint closures and port disruptions are shocks on specific edges. Validation: container and dry-bulk freight-rate indices, including the 2021 spike.

### 8.5 Trade policy

Policy wedges enter c(o,d,g): MFN and preferential tariffs (free trade agreements, customs unions such as the EU), anti-dumping and safeguard duties, quotas, export controls, export bans (as on food in 2008 and 2022), and sanctions (embargoes, secondary-sanction compliance costs). Tariff revenue accrues to the importing state. Optimal-tariff and retaliation logic, as quantified by Ossa (2014), is available to state AI.

### 8.6 Demand system

Households allocate spending across consumption categories (food, energy, housing, manufactures, services) with non-homothetic CES (Comin, Lashkari and Mestieri 2021), which separates income from price effects and generates Engel's law and the rising services share:

```math
\frac{\omega_{i}}{\omega_{j}} = \frac{\Omega_i}{\Omega_j} \left( \frac{p_i}{p_j} \right)^{1-\sigma} C^{(1-\sigma)(\varepsilon_i - \varepsilon_j)}
```

Within each category a CES aggregates goods and origins. A subsistence floor for food and energy enters through rationing priority, not preferences. Governments, armies and investment add their own demand.

### 8.7 Factor markets

- **Labour** : per province and education level, wage posting with a matching function; wages adjust with vacancy-unemployment ratios, with downward rigidity and minimum wages where legislated.
- **Land and housing** : rents clear demand by use; housing is a durable stock with construction lags, so house-price booms and busts are possible.
- **Capital** : allocated by investment and credit (Sections 7.4 and 9).

### 8.8 Currencies in trade

Merchants invoice in a currency chosen by a dominant-currency rule: most international trade is invoiced in dollars or euros regardless of origin, as documented by Gopinath and coauthors. Exchange-rate changes therefore pass through to prices according to the invoicing currency, not just bilateral rates.

## 9. Money, Finance and the Fiscal State

The financial system is a stock-flow-consistent network of balance sheets in which central banks set policy, commercial banks create money by lending, asset prices move with expectations, and crises propagate through bank, housing and sovereign exposures.

### 9.1 Sectoral balance sheets

Each economy has households, non-financial enterprises, banks, government, central bank and the rest of the world (all other states, endogenous). The balance-sheet and transactions-flow matrices of Godley and Lavoie (2007) are computed every tick and must sum to zero across rows and columns. Agent-level claims (loans, mortgages, bonds, deposits, equity, reserves) aggregate exactly to the sector matrices, the standard SFC-ABM construction (Caverzasi and Godin 2015). Initial balance sheets come from national financial accounts where available and are imputed elsewhere.

### 9.2 Monetary and exchange-rate regimes

| Regime | Examples in 2000 | Policy rule |
|---|---|---|
| Floating, inflation targeting | United States, United Kingdom, Brazil | Taylor-type rule on inflation and output gap, effective lower bound, asset purchases when at the bound |
| Managed float / crawling peg | China, many emerging markets | Exchange-rate band defended with reserves and capital controls |
| Hard peg / currency board | Hong Kong, Gulf states | Rate set to defend the peg; reserves constrain |
| Monetary union | Euro area, CFA franc zones | One central bank for many states, no national devaluation |
| Dollarization | Ecuador, Panama | No independent monetary policy |

Regimes are initialized from the de facto classification of Ilzetzki, Reinhart and Rogoff and can change by law (abandoning a peg, joining or leaving a union). Floating exchange rates clear net currency demand from trade, income and capital flows each month, with central bank intervention from reserves.

### 9.3 Banks and asset markets

Banks hold reserves, loans, mortgages, bonds and interbank claims against deposits, wholesale funding and equity. Lending creates deposits up to capital (Basel-style) and liquidity constraints; loan rates are the policy rate plus a risk premium from borrower leverage. Bank runs follow a threshold rule on depositor and wholesale-funder confidence, softened by deposit insurance; contagion runs through interbank exposures (Gai-Kapadia). Housing and equity prices are formed by agents valuing expected rents and earnings with heuristic expectations (Section 12), so leveraged housing booms and busts, the core of 2008, can emerge.

### 9.4 Sovereign debt, default and the IMF

States issue bonds in local or foreign currency. Spreads use a reduced-form default-risk function inspired by Eaton-Gersovitz and Arellano (2008):

```math
s_{j,t} = f\left( \frac{\text{debt}}{\text{revenue}}, \; \text{deficit}, \; \text{foreign-currency share}, \; \text{reserves}, \; \text{institutions}, \; \text{default history}, \; \text{lender of last resort} \right)
```

Membership in a monetary union without a lender of last resort raises spreads under stress, the mechanism of the euro crisis. Default is a strategic choice of the state AI; consequences are market exclusion, restructuring with haircuts, and financial-sector losses wherever the bonds are held. The IMF is a stylized agent that lends with conditionality (fiscal targets) to states that request it.

### 9.5 Capital flows and reserves

Savers and banks allocate across domestic and foreign assets by a mean-variance rule with home bias; central banks accumulate reserves in safe assets, which creates the demand for dollar and euro bonds. Sudden stops occur when risk appetite drops and foreign holders sell. Targets: current-account and net foreign-asset paths, and capital-flow reversals in 2008 and 2013.

### 9.6 Fiscal state

Revenue instruments: personal income tax, payroll and social contributions, VAT and sales taxes, corporate tax, tariffs, excises (fuel, tobacco, carbon), resource royalties and state-enterprise profits, seigniorage. Collected revenue is

```math
R_{j,i} = \tau_{j,i} \times \text{Base}_{j,i} \times \kappa_{j,i} \times \text{compliance}_{j}
```

where kappa is fiscal capacity per instrument (income tax needs administration and formal employment; tariffs and royalties need little), built by investment and depreciating slowly, following Besley and Persson (2009). Informality shrinks the base. Spending: pensions, health, education, social transfers, defence, infrastructure, energy subsidies, interest. Initial tax rates, revenue and spending come from international government revenue and finance statistics. Deficits are financed by borrowing or monetary financing; budgets are set by the state AI under institutional rules (Section 10).

## 10. Institutions, Politics and Government

Politics runs in three layers: interest groups hold resources and preferences, parties assemble them into electoral coalitions, and institutions decide how much elections, money and arms each count for. Government types emerge from that institutional vector rather than being picked from a menu.

### 10.1 Institutional state vector

Each state carries continuous indices in [0, 1], initialized directly from V-Dem for 2000:

| Dimension | Meaning | V-Dem / other anchor |
|---|---|---|
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

```math
\pi_g = \frac{w_g \, x_g^{m}}{\sum_h w_h \, x_h^{m}}, \qquad x_g = \text{wealth}_g^{a} \cdot (\text{members}_g \cdot \text{organization}_g)^{b} \cdot \text{arms}_g^{c}
```

Institutional weights w translate resources into influence: votes count through elections, money through lobbying and patronage (heavier with high corruption and weak constraints), arms through the security forces' capacity to intervene.

### 10.5 Parties and elections

- **Parties**  are platforms in a policy space (economic left-right, cultural liberal-conservative, nationalism and sovereignty, ethnic or religious appeal) backed by a coalition of interest groups. Initial parties, positions and vote shares come from open party datasets (ParlGov, V-Party), with stable generic names rather than real ones in v1.
- **Voting** : pops vote by spatial proximity to platforms, party attachment, and retrospective economic voting (incumbents punished for unemployment and inflation), with turnout tied to education and mobilization.
- **Electoral systems** : plurality, two-round, proportional (D'Hondt, Sainte-Lague) and mixed, initialized per country, converting votes to seats.
- **Government formation** : majority winners govern; otherwise minimal connected winning coalitions form by bargaining over portfolios and policy.
- **Party system dynamics** : parties shift platforms toward vote-rich positions with inertia; new parties enter when a large ideological mass is unrepresented (the channel for populist entry); parties split and merge.
- **Calendars** : election dates follow constitutional terms from 2000 data; snap elections occur when governments lose confidence.

### 10.6 Policy choice

| Setting | Mechanism | Source |
|---|---|---|
| Democracies | Governing party or coalition maximizes its vote and coalition-partner support, subject to veto players | Probabilistic voting (Persson-Tabellini 2000); veto players |
| Autocracies | Leader maximizes survival given winning coalition W and selectorate S; small W favours private goods | Bueno de Mesquita et al. (2003) |
| Legislatures | Proposer-veto bargaining; bills progress monthly and may fail | Design choice |

### 10.7 Legitimacy, ideology and grievance

Pop approval combines material conditions relative to expectations, policy distance from preferred platforms, group exclusion, corruption, war weariness and repression. Ideology evolves with education, age, economic conditions and media freedom, and diffuses through bounded-confidence opinion dynamics (Hegselmann-Krause), which can generate polarization.

### 10.8 Unrest and regime change

- **Protest** : mobilization follows a threshold model (Granovetter 1978) with preference falsification (Kuran 1991), so protest waves can cascade across neighbouring states, the pattern of the Arab Spring.
- **Structural pressure** : a Political Stress Index in the Goldstone-Turchin tradition (mass mobilization potential, elite competition, state fiscal distress) raises instability hazards.
- **Outcomes** : concession and reform, repression, democratic breakdown or backsliding (executive aggrandizement eroding constraints gradually), coups (calibrated to coup datasets), revolution, civil war (Section 11), secession (by referendum or conflict).
- **Calibration targets** : V-Dem index paths 2000-2024, frequencies of coups, mass protest campaigns and regime transitions.

## 11. Diplomacy, Conflict and War

Interstate war is modeled as a failure of bargaining under private information and commitment problems, made rare by nuclear deterrence and economic interdependence; civil conflict, the dominant form of organized violence since 2000, grows from group grievance, weak state capacity and opportunity.

### 11.1 Diplomatic state

Per dyad: relations, trust (track record of honoured commitments), rivalry flag, territorial claims, and treaties: defence alliances (NATO-style collective defence), non-aggression and security partnerships, free trade agreements, customs and monetary unions, sanctions regimes, arms-supply relationships. Membership of international organizations (UN Security Council permanent seats, WTO, IMF) is an attribute with procedural effects (vetoes, dispute settlement, lending access).

### 11.2 Crisis bargaining

Disputes arise from territorial claims, ethnic kin across borders, resource disputes, security dilemmas, regime-change motives or alliance obligations. Each side holds a noisy estimate of its probability of winning p and of costs; the bargaining range follows Fearon (1995):

```math
x \in \left[ \hat{p}_A - \hat{c}_A, \; \hat{p}_A + \hat{c}_B \right]
```

War occurs when private estimates leave no acceptable offer, when a projected power shift gives the declining side a preventive motive (Powell 2006), or when the stake is treated as indivisible. Costs include casualties, economic losses through trade and finance, sanctions, and for nuclear dyads an expected catastrophic-escalation term that makes direct war between nuclear powers extremely rare but lets proxy conflicts and limited wars occur. Power-estimate noise is calibrated to war-onset frequencies.

### 11.3 Alliances and interdependence

Alliance formation trades autonomy for security (Morrow 1991); reliability is learned from behaviour. Economic interdependence enters the cost of war: bilateral trade raises the opportunity cost of fighting a partner, while broad multilateral openness reduces dependence on any one partner (Martin, Mayer and Thoenig 2008). Both channels are in the model and tested (Section 18).

### 11.4 Military capability

Capability is computed in-model from military personnel, military spending, defence-industrial output (arms, aerospace, shipbuilding sectors), energy and population, and compared to Correlates of War capability data and military-spending series. Forces: land formations, air power, navies, missiles and air defence as aggregate capability stocks with readiness, equipment bought from domestic or foreign arms producers, and recruitment by law (conscription or volunteer). Nuclear status is a state attribute.

### 11.5 Combat, logistics and occupation

- **Battles** : Lanchester attrition with modifiers for terrain, fortification, air superiority, precision munitions and morale; loss-exchange ratios calibrated to modern battle data where available.
- **Logistics** : forces draw fuel, ammunition and food over the transport graph; interdicted supply lines raise attrition.
- **Occupation** : transfers control and taxation, disrupts production and trade, raises local grievance and insurgency risk.
- **Naval and air** : sea control and air superiority set blockade and strike risk on transport edges and infrastructure.

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
|---|---|---|---|---|
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
- **Anonymization** : states, people and dates are renamed (for example 'State 17, year 64') so the model cannot pattern-match to real history and railroad outcomes it knows happened.
- **Determinism** : decisions are cached by a hash of the situation report and recorded in the event log, so replays and branches are reproducible without re-querying.
- **Known limits** : Lin et al. (2025) find LLM agents' expectations more homogeneous than human respondents', and outputs are prompt-sensitive; Tier 2 results are always compared against Tier 1a baselines.

### 12.6 Steward mode

In Steward mode the user replaces Tier 1a for one state. The same action space is exposed through the UI, so anything the user can do, the AI can do, and vice versa.

## 13. Climate, Shocks and Stochastic Processes

Climate is an endogenous feedback loop (the economy emits, temperature responds, damages hit the economy), while other exogenous shocks sit on a shock tape that can be replayed from 2000-2024 history, drawn stochastically, or edited, and is held fixed across branches.

### 13.1 Shock tape modes

| Mode | Content | Use |
|---|---|---|
| Historical | Observed weather 2000-2024, pandemic seeding (SARS 2003, H1N1 2009, COVID 2020), major disasters, chokepoint disruptions | Out-of-sample validation |
| Stochastic | Draws from fitted generators, with climate-shifted weather after 2024 | Ensembles, open-ended futures |
| Scripted | User edits on top of either | Counterfactuals ('no COVID', 'Suez closed for a year') |

### 13.2 Climate emulator

- **Emissions** : CO2, methane and nitrous oxide from fossil energy use, industry, agriculture and land-use change, computed from simulated activity (Section 7.7).
- **Global response** : a reduced-form carbon cycle and temperature model in the style of FaIR (open source), initialized to observed 2000 concentrations; it runs annually and costs microseconds.
- **Regional climate** : province temperature and precipitation anomalies by pattern scaling of global mean temperature, using patterns from climate-model ensembles, plus interannual variability from the weather generator.
- **Damages** : non-linear temperature effects on crop yields (Schlenker and Roberts style), on labour productivity and output (Burke, Hsiang and Miguel 2015), on mortality (Carleton et al. 2022), sea-level rise removing coastal land and raising flood risk, and more frequent extremes.
- **Policy levers** : carbon prices, fuel taxes and subsidies, efficiency standards and clean-energy subsidies act through prices and sector productivity; adaptation spending reduces damage functions.

Validation: 2000-2024 emissions by country and global temperature; beyond 2024, emission and temperature paths are compared to IPCC scenario ranges as a plausibility check, not a target.

### 13.3 Disease

Each pathogen class runs as a metapopulation SEIR model on the transport and air-travel network, so pandemics travel along trade, travel and migration routes. Health spending, vaccination and non-pharmaceutical interventions (lockdowns, border closures) are policy levers whose economic effects run through labour supply, sector demand and trade, as in the COVID application of Poledna et al. (2023).

### 13.4 Disasters and resources

Earthquakes, tropical cyclones and floods are drawn from hazard maps (with climate-scaled frequencies for weather extremes) and destroy capital and housing in affected provinces. Resource discoveries and extraction-technology gains (for example the shale boom) appear as productivity and reserve shocks in the energy-mining sectors.

### 13.5 Political hazards

Leader death uses age-specific mortality; assassinations and terror attacks have low base hazards scaled by radicalization and conflict. These are the only personal events in the model.

### 13.6 Endogenous vs exogenous

Financial crises, recessions, protest waves, wars and famines are endogenous outcomes and never sit on the tape. Users can still force them as interventions (Section 14), which is recorded as such.

## 14. Counterfactual Engine: Ex Ante and Ex Post Interventions

Every experiment is a branch: a parent run plus an ordered list of timed edits, replayed under the same random streams across an ensemble of seeds, so the reported effect of an intervention is a paired distribution rather than an anecdote.

### 14.1 Objects

| Object | Definition |
|---|---|
| Scenario | Data pack version + initial state + parameter vector + shock tape |
| Branch | Scenario + ordered interventions [(tick, edit)], with a parent branch and fork tick |
| Run | Branch x seed |
| Ensemble | Branch x K seeds (default K = 32) |
| Experiment | Set of branches compared against a baseline, with declared outcome metrics |

### 14.2 Edit types

| Type | Example | Timing |
|---|---|---|
| Parameter | Trade elasticity 5 to 8; migration cost x 0.5 | Ex ante or ex post |
| State (god mode) | Transfer a province; add 1M pops; destroy a fleet; set literacy | Ex ante or ex post |
| Policy (in-world) | The United States does not impose the 2018 steel and aluminium tariffs | Ex post (as a forced decision) |
| Event injection | Declare war; assassinate a ruler; discover gold in a province | Ex post |
| Decision override | Force a state AI's choice at one decision point, or lock a law | Ex post |
| Shock tape | Remove COVID; add a multi-year US Midwest drought | Ex ante (tape) |
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

- **Multiple interventions** : Shapley decomposition over intervention subsets (exact up to five interventions, sampled beyond).
- **Channel shutdown** : rerun the branch with one mechanism frozen at baseline values (prices, migration, politics, finance) to measure how much of an effect flows through it, a simulation analogue of mediation analysis.
- **Parameter sensitivity** : Morris screening, then Sobol indices (Saltelli et al.) computed on an emulator of the model to keep the cost manageable.
- **Equilibrium comparison** : for trade, tariff and migration-cost interventions, the engine also runs the dynamic hat-algebra reference model (Caliendo-Dvorkin-Parro 2019; Kleinman-Liu-Redding 2023) from the same initial data. Where agent and equilibrium responses diverge, the UI flags the gap and the channel-shutdown tool identifies whether disequilibrium, finance or politics explains it.

### 14.8 Scripting API

Everything in the UI is available in Python, for example: create a branch from a baseline at a tick, apply edits, run an ensemble, and query outcome tables as Polars data frames for analysis.

## 15. Calibration, Estimation and Validation

Parameters are set in three layers (data, literature priors, estimation), estimated on 2000-2010 with simulation-based Bayesian inference module by module, and validated out of sample on 2011-2024, including the model's responses to well-studied natural experiments.

### 15.1 Three-layer parameterization

This follows the data-driven ABM program: Poledna, Miess, Hommes and Rabitsch (2023) built the first ABM competitive with VAR and DSGE benchmarks in out-of-sample macro forecasting by initializing agents from national accounts, input-output tables and census data; Wiese et al. (2024) extended it to all 38 OECD countries with approximate Bayesian inference and beat both that model and AR(1) benchmarks out of sample.

- **Data-determined** : initial populations, capital stocks, input-output coefficients, balance sheets, tax rates, borders, infrastructure, institutions. Not estimated.
- **Literature-anchored** : structural elasticities with priors from published estimates. Estimated only within prior ranges.
- **Free behavioural** : adjustment speeds, expectation weights, thresholds. Estimated. Target fewer than 60 in total (Section 17).

### 15.2 Estimation methods

| Method | Used for | Notes |
|---|---|---|
| Modular calibration | Each module against its own targets, others held at data | Essential on one laptop |
| Method of simulated moments | Moments with clear targets (trade shares, unemployment, spreads, fertility) | Common random numbers across draws |
| Neural posterior estimation / ABC-SMC | Joint posterior of behavioural parameters | Simulation-based inference for ABMs as in Dyer et al. (2024); sbi toolkit (PyTorch, runs on Apple MPS) |
| History matching with GP emulators | Ruling out implausible regions first | A few hundred runs |
| Gradient-based tuning on differentiable relaxations | Prices, demand and trade kernels | JAX on CPU (Apple GPU support for JAX is limited) |

**Compute budget on the MacBook (estimate to verify in P0).**  A headless 25-year run on one core is targeted at 2 to 3 minutes; running 10 to 12 in parallel gives roughly 250 to 350 runs per hour, about 3,000 overnight. Calibration is planned around that number: emulators and modular steps first, joint inference last.

### 15.3 Validation ladder

| Level | Test | Pass criterion |
|---|---|---|
| V0 | Accounting invariants, determinism, null-branch identity | Zero violations in CI |
| V1 | Equilibrium nesting: frictions off, ABM steady state vs analytical GE | Prices and flows within 1% |
| V2 | Stylized facts | Gravity distance elasticity near the consensus around -0.9 (Disdier and Head 2008); Zipf's law for city sizes; Engel curves; fat-tailed firm sizes; Okun's law; Phillips-curve and Beveridge-curve shapes |
| V3 | Out-of-sample 2011-2024 | See 15.4 |
| V4 | Natural-experiment replication | See 15.5 |
| V5 | Expert face validity | Structured review of baseline runs by economists and political scientists |

### 15.4 Out-of-sample targets (2011-2024)

| Target | Source | Metric |
|---|---|---|
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

The model is fed each historical shock and its response is compared with the published empirical estimate:

| Episode | Model test | Empirical benchmark |
|---|---|---|
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

- **Data assimilation.**  During the calibration window and at every rolling origin, the simulated state is nudged toward observed data with an ensemble Kalman or particle filter (sequential Monte Carlo for ABMs, as in Lux 2018), so forecasts start from data-consistent states.
- **Posterior ensembles.**  Ensemble members draw parameters from the posterior, so reported uncertainty includes parameter uncertainty, not only shock noise.
- **Model discrepancy.**  Calibration includes an explicit discrepancy term (Kennedy and O'Hagan 2001) so parameters do not absorb structural error.
- **Stacking across structural switches.**  Plausible module alternatives (17.2) run side by side and are weighted by out-of-sample skill (Bayesian stacking).
- **Historical policy tape.**  In validation, actual policy rates and fiscal decisions can be fed exogenously, separating economic-mechanism error from state-AI policy error.

**Targets: micro facts over macro paths** 

Many independent, well-identified moments constrain the model better than a few aggregate series: price-change frequencies, marginal propensities to consume out of transfers, job-finding and separation rates, firm entry and exit rates, survey inflation expectations, and exchange-rate pass-through by invoicing currency. Targets span several scales at once, following pattern-oriented modelling (Grimm et al. 2005).

**Targeted additions** 

- Wealth distribution per pop from distributional national accounts (World Inequality Database), for realistic spending heterogeneity and policy transmission.
- Competitive speculative storage for commodities (Deaton and Laroque), refining merchant inventories so food and energy price spikes are realistic.
- Cross-border bank claims (BIS locational banking statistics) to initialize foreign financial exposures.

**Benchmarks** 

- Macro forecasts are scored against IMF World Economic Outlook forecasts as well as AR(1) and VAR baselines.
- Conflict hazards are scored against the ViEWS early-warning system (Hegre et al. 2019).

**Guardrails** 

- **Ablation.**  Every phase switches each module off and measures the change in out-of-sample skill; a mechanism that neither improves holdout scores nor passes a falsification test stays off by default.
- **Leave-one-region-out calibration.**  Parameters are re-estimated without each world region and tested on it; parameters that only work where fitted are flagged.
- **Parameter freeze.**  2011-2024 data stays unseen until scoring.
- **Fuzz testing.**  Random interventions and extreme shocks must never break invariants or produce non-finite values.
- **Documentation.**  The model is described in the ODD protocol (Overview, Design concepts, Details; Grimm et al. 2020) so outsiders can review it.

**Excluded as bloat** 

Named firms or individuals, sub-province geography, daily financial markets, power-grid dispatch, per-agent optimization, more sectors than the input-output tables, and LLM agents: each adds parameters faster than accuracy.

## 16. Data Sources and Pipeline

The 2000 pack is built from pinned sources; each is listed with its licence, evidence and release policy in the source manifest (python/ceteris/packs/sources.toml, 98 sources). While the project is closed source it may use restricted sources, which must be replaced or cleared before it opens. ADR 0005 lists the substitutions made in the P0 audit.

| Dataset | Content | Role | Licence | Bundle or fetch |
|---|---|---|---|---|
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
| ERA5 reanalysis | Weather 2000-2024 | Historical shock tape | Copernicus licence (attribution) | Fetch, bundle aggregates |
| FaIR | Climate emulator (software) | Climate module | Apache 2.0 | Dependency |
| Disaster databases (e.g. EM-DAT) | Disaster events | Validation only | Restricted | User fetches; never bundled |

### 16.1 Pipeline

A Python package builds the pack reproducibly: download with checksums, harmonize to the H3 grid and province map, impute with documented rules (for example, downscaling national ICIO output to provinces with subnational sector shares and night lights; imputing ICIO coefficients for the roughly 115 economies outside OECD ICIO from regional averages adjusted by income), reconcile to national totals, and emit versioned Parquet tables plus a manifest recording every source, version, licence and imputation flag. Fetch-only sources are downloaded on the user's machine at build time. The start year is a pipeline parameter (1995-2022).

## 17. Degrees of Freedom Register

The register lists every parameter that is not pinned by data, with its prior and how it is identified; ranges below are starting priors to be refined in Phase 9, not final values.

### 17.1 Continuous parameters

| ID | Parameter | Module | Starting prior | Identification / source | Class |
|---|---|---|---|---|---|
| P01 | Trade elasticity theta | Trade | 4 to 8, centre 5 | Head and Mayer (2014); Simonovska and Waugh (2014); sets entropic temperature | Literature |
| P02 | Merchant inventory target coverage | Markets | 1 to 6 months by good | Storage and perishability; price volatility | Free |
| P03 | Price adjustment speeds kappa1, kappa2 | Markets | 0.05 to 0.5 per month | Price volatility and autocorrelation | Free |
| P04 | Mode cost ratios (water, rail, road, track) | Transport | Ordered, log-uniform | Freight rates, price gaps | Literature + calibrated |
| P05 | Non-tariff border frictions | Trade | Log-uniform | Gravity border effects | Calibrated |
| P06 | Category substitution sigma | Demand | 0.3 to 0.9 | Comin-Lashkari-Mestieri (2021) | Literature |
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
|---|---|---|---|---|---|
| P31 | Climate damage coefficients (yields, labour, mortality) | Climate | Published dose-response curves | Schlenker and Roberts; Burke et al. (2015); Carleton et al. (2022) | Literature |
| P32 | Wright's-law learning rates for energy sectors | Technology | 10 to 30% cost decline per doubling | Way et al. (2022) | Literature |
| P33 | Economic-voting weight | Parties | Positive | Incumbent vote response to growth and inflation | Calibrated |
| P34 | New-party entry threshold | Parties | Free | Frequency of new parties reaching parliament | Calibrated |
| P35 | Taylor-rule coefficients by central bank | Monetary | Inflation above 1, output positive | Estimated policy rules | Literature + calibrated |
| P36 | Nuclear escalation cost term | War | Large | Absence of direct war between nuclear powers | Calibrated |

### 17.2 Structural switches (model uncertainty)

| ID | Switch | Options |
|---|---|---|
| S01 | Node price formation | Stock-flow rule / Walrasian clearing |
| S02 | Trade solver | Entropic OT (unbalanced) / closed-form gravity / Walrasian spatial price equilibrium |
| S03 | Demand system | Non-homothetic CES / Stone-Geary |
| S04 | Expectations | Heuristic switching / adaptive only / perfect foresight (reference) |
| S05 | Trade costs | Per-unit + iceberg / iceberg only |
| S06 | State AI | Tier 1a / 1b / 2 |
| S07 | Shock tape | Historical / stochastic / scripted |
| S08 | System order within tick | Fixed / randomized (sensitivity) |

A conclusion is reported as robust only if it survives the plausible settings of the relevant switches.

## 18. Testable Hypotheses

The simulator is built to adjudicate between explanations, so each hypothesis names the experiment that would falsify it; H1 to H3 are engineering acceptance tests, H4 onward are research questions.

| ID | Hypothesis | Experiment | Falsified if |
|---|---|---|---|
| H1 | Equilibrium nesting: with frictions off, the agent model converges to the GE reference | Frictionless runs vs hat-algebra solution | Gaps above 1% persist |
| H2 | Gravity emerges: PPML on simulated flows gives distance elasticities near the empirical range | Estimate gravity on simulated data (Santos Silva and Tenreyro 2006) | Elasticity outside about -0.7 to -1.1 |
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

A Rust core gives deterministic, parallel speed; Python owns data, calibration and analysis; the desktop app renders the map, in its webview today and natively in Rust on wgpu later; the three meet on Apache Arrow.

### 19.1 Stack

| Layer | Choice | Why |
|---|---|---|
| Simulation core | Rust, plain structure-of-arrays, rayon for parallelism | Determinism control, speed, memory safety for a long-lived codebase |
| Graphs | petgraph plus a sparse least-cost-path cache | Transport costs recomputed only when edges change |
| GPU fallback | wgpu compute on Metal | Only if the trade solver misses its CPU budget |
| App shell | Tauri 2: Rust core in-process, raw buffers to the UI over IPC (Arrow with tick state), pack files served from disk through pack:// | One native macOS app, no server |
| Map UI | MapLibre GL for tiles, borders and labels; a GPU fill layer coloured from a per-feature texture; PMTiles; React panels, ECharts; flat map or globe. To be rewritten in Rust on wgpu | A full recolour of 53,084 counties reaches the screen in 19 ms (p50) |
| Bindings | PyO3 + maturin | Python drives headless runs and reads Arrow tables with zero copy |
| Python tooling | uv; polars, duckdb, numpy, jax (CPU), torch (MPS) + sbi, SALib, optuna, geopandas, h3, pytest, hypothesis | Pipeline, reference model, inference, analysis |
| Run store | Parquet partitioned by experiment / branch / seed / table; DuckDB queries | Cheap ensemble analysis on one disk |
| Snapshots | zstd-compressed Arrow, content-addressed | Branch sharing |
| Licence | Closed source for now; Apache-2.0 / MIT dual for code on opening; data under its source licences (ODbL and other share-alike tables shipped separately) | Restricted sources usable until the project opens |

An alternative worth noting: the Julia ecosystem now hosts a data-driven macro ABM framework (BeforeIT.jl, associated with the Bank of Italy), useful as a reference implementation for the economic core even though Ceteris Paribus uses Rust.

### 19.2 Performance budget (single run, 8-core desktop)

Hardware: MacBook Pro with an M-series Pro chip (about 12 cores) and 18-36 GB of unified memory. Target 6 to 12 ticks per second (0.5 to 1 simulated year per second), about 80 to 160 ms per tick. Memory target: under 6 GB for one interactive run so ensembles can run alongside.

| System | Work per tick | Budget |
|---|---|---|
| Demand and consumption | 100k pops x 8 categories x goods | 10 ms |
| Production and labour | 150k enterprises | 12 ms |
| Province markets | 4,000 provinces x 50 goods | 2 ms |
| Trade (two-level sparse Sinkhorn, warm-started) | 50 goods, 600 gateways + 4,000 provinces x 150 partners | 30 to 60 ms |
| Finance, fiscal, monetary | 1M claims, mostly passive | 8 ms |
| Politics, migration, diplomacy | Pops, parties, states | 8 ms |
| Climate (annual) | Emulator + pattern scaling | under 1 ms amortized |
| Accounting assertions | Full ledger check | 5 ms |
| Headroom |  | to 160 ms |

During wars the daily sub-tick adds about 30 cheap steps per month, budgeted separately. All timings are estimates; P0 and P2 benchmarks replace them with measurements. Ensembles run headless across the desktop's cores; with desktop-only hardware, calibration depends on emulators and modular calibration to stay within one machine's compute.

### 19.3 Repository layout

```math
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
    ceteris-py/           PyO3 bindings
  app/                    Tauri shell (Rust) + TypeScript map UI
  python/ceteris/
    packs/                data pipeline (any start year 1995-2022), licence manifest
    calibrate/            SBI, MSM, emulators
    experiments/          branch and ensemble runners
    reference/            GE hat-algebra reference model
    analysis/             plots, metrics, reports
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

**Tick execution** 

- Double-buffered state: each system reads tick t and writes t+1; no locks in hot paths.
- Zero allocation in hot loops: preallocated scratch per thread (rayon `map_init`), arenas for transient objects.
- Shipments in a timing wheel (ring buffer indexed by arrival tick): O(1) insert and delivery, no heap.
- Multi-rate scheduling; war sub-ticks only while a war is active; rare events by sampled next-event times, not per-tick coin flips.
- Closed-form CES demand; the trade solve is the only per-tick iterative solver.

**Linear algebra** 

- The two-level trade solver is a low-rank factorization of the province kernel through about 600 gateways: O(P x k + G^2) per Sinkhorn step instead of O(P^2).
- Goods sharing a transport-weight class share a kernel, so Sinkhorn steps are matrix-matrix products (provinces x goods), not one matrix-vector product per good.
- Tariffs are constant within a country pair: apply them as blockwise scaling of per-block products; never materialize per-good kernels.
- Stabilized scaling Sinkhorn with absorption into log-potentials only near overflow (Schmitzer), so the inner loop has no `exp`; Anderson acceleration on every fixed-point loop (Sinkhorn, Walrasian mode, GE reference model).
- Never invert the Leontief matrix in the agent model: shortages propagate one sparse matrix-vector product per tick. The GE reference model applies the global IO system matrix-free and solves with Krylov methods.
- Exponential distributed lags (climate boxes, adaptive expectations, depreciation) are O(1) state updates; cohort aging is an array shift.
- Correlated weather from k spatial principal components (O(P x k) per draw); shortest paths by multi-source Dijkstra from gateways, updated only when edges change; contagion by fictitious-default rounds on sparse exposures; bounded-confidence neighbours by sort-and-sweep.

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

- One single-threaded run per process; process count about equal to performance cores.
- Data packs are immutable Arrow files memory-mapped by every process, so ensembles share one physical copy through the page cache.
- Seeds that share a kernel can be batched as extra columns in the gateway-level Sinkhorn products.
- Copy-on-write snapshots in about 64k-row content-hashed chunks with low-level zstd.
- Emulate principal components of output series; reuse Saltelli sample matrices; random Fourier features keep emulators linear in run count; Python crosses the boundary only with zero-copy Arrow, never per-entity calls.

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

The interface is a Paradox-style map with time controls, plus two things Paradox games lack: a branch manager for comparing histories and an explainer that decomposes any number into its causes.

### 20.1 Map modes

| Group | Modes |
|---|---|
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

- **Province / state inspectors** : every component, editable in god mode, with an 'apply at tick' field.
- **Ledgers** : state budget, trade balance, balance sheets, interest groups and laws; market panel with supply, demand, inventory and price history per node and good.
- **Explainer** : for any price, wage, flow or decision, a waterfall of contributions (for a price: local supply shock, import cost, tariff, demand from wages; for a decision: the utility terms of each option).
- **Branch manager** : a tree of branches with fork ticks and intervention lists; select two to compare.
- **Ensemble view** : fan charts with quantile bands per branch; event-probability tables.
- **Timeline** : scrubber over snapshots; jump to any tick and fork.
- **Console** : Python-equivalent commands for anything done by clicking, exported as a script for reproducibility.

### 20.3 Time controls

Pause, five speeds, step one tick, run to date, and 'run ensemble from here' which forks K seeds in the background and streams summary statistics back to the map.

## 21. Phased Roadmap and Agent Execution Protocol

Eleven phases take the project from an empty repository to a calibrated, playable world; the counterfactual workbench comes third because branching is the product, and every phase ends with a runnable world and passing invariants.

*[Diagram: roadmap · 11 phases, each with its gate]*

Each phase starts only when the previous gate passes; the highlighted workbench phase is where the project first becomes useful for experiments.

**Development slice.**  P1 to P3 run on a European slice (about 600 provinces, the rest of the world reduced to trade gateways) for fast debugging; P4 switches to the full globe. **First release**  is the end of P9: a calibrated, validated world, not an earlier partial build.

### 21.1 Phases and acceptance criteria

- **P0 Foundations.**  Repository, spec template, ADR template, entity storage, scheduler, counter-based RNG, double-entry ledger, event log, snapshot and restore, CI.
  - Accept: a toy world of 10 provinces runs 1,200 ticks; identical hashes across two runs and across 1 vs 8 threads; restore-and-replay from any snapshot matches.
- **P1 World and map.**  Geography pipeline (H3 raster, provinces, sea zones), transport graph with mode costs, least-cost-path cache, 2000 borders, web map with political, terrain and resource modes.
  - Accept: about 4,000 provinces render at 60 fps; clicking a province shows attributes; cost-from-node map mode looks plausible to a reviewer.
- **P2 Economic core (minimal).**  10 goods, static pops, one enterprise per (province, sector), labour market, province markets with the stock-flow rule, two-level entropic OT trade with transit, a single numeraire money, accounting assertions, and the GE reference model in Python.
  - Accept: V0 and V1 pass (frictionless convergence to the GE reference within 1%); simulated gravity elasticity in range (H2); no invariant violations over 100 years.
- **P3 Counterfactual workbench.**  Branches, interventions (all types in 14.2), common random numbers, ensembles, difference maps, fan charts, Python experiment API.
  - Accept: null intervention reproduces parent; a tariff intervention yields paired effects with tighter intervals under CRN than with independent seeds.
- **P4 Population.**  Demography with Brass mortality and fertility, education, occupational mobility, culture and religion dynamics, migration with dynamic discrete choice.
  - Accept: fertility, mortality and urbanization within tolerance of UN data for 2000-2010; population conservation holds.
- **P5 Money, finance and state.**  Currencies and regimes, central banks with policy rules, banks, housing and equity prices, sovereign bonds and default, IMF, portfolio capital flows, taxes with fiscal capacity, budgets, SFC matrices.
  - Accept: SFC matrices close every tick; a policy-rate shock moves credit, output, inflation and the exchange rate with plausible signs and lags; spreads respond to debt and stress.
- **P6 Politics and institutions.**  Institution vector, interest groups, parties and elections, contest function, policy mechanisms, laws with momentum, legitimacy, ideology dynamics, unrest, reform, repression, revolution, coups, secession.
  - Accept: regime transitions, coups and protest waves at plausible frequencies; simulated 2000-2010 elections track vote shares; crisis-voting event study (H7) runs end to end.
- **P7 Diplomacy and war.**  Relations, treaties, crisis bargaining, alliances, nuclear deterrence, forces, daily combat, logistics, occupation, war economy, peace terms, sanctions, civil conflict.
  - Accept: interstate and civil conflict onset frequencies within COW and UCDP ranges in ensembles; conflicts visibly move prices, debt and populations.
- **P8** **Growth, energy and climate** **.**  Frontier productivity growth, catch-up, learning curves in energy, structural change, endogenous infrastructure, emissions, climate emulator and damages.
  - Accept: 2000-2010 growth, sector shares and emissions within tolerance; open-ended runs to 2150 stay numerically stable.
- **P9 Calibration and hindcast.**  Pre-registration, modular calibration on 2000-2010, joint SBI, emulators, Sobol analysis, 2011-2024 out-of-sample validation, natural-experiment replication.
  - Accept: out-of-sample scorecard (15.4) and natural-experiment table (15.5) published with CRPS and coverage; robustness across structural switches documented.
- **P10 AI layers and Steward mode.**  Learned state policies and playable single-state control; LLM leaders are deferred beyond v1.
  - Accept: Tier 1b compared with Tier 1a on historical base rates; Steward mode exposes the full state action space.

### 21.2 Agent execution protocol

The plan is written to be executed by coding agents in focused steps. Rules for every task:

- **Spec first.**  Before code, the agent writes or updates `specs/<module>/SPEC.md`: state components, equations, system order, invariants, parameter IDs from Section 17, and test plan. A human approves specs for modules in P2, P5 and P6.
- **Tests before implementation.**  Unit tests for each equation, property-based tests for invariants (proptest in Rust, hypothesis in Python), and an equilibrium-nesting test where applicable.
- **Small tasks.**  One system or one data table per task; each task leaves the world runnable.
- **Golden runs.**  Each phase adds a golden scenario with stored state hashes; any change that alters them must state why in its commit and update the hashes deliberately.
- **No silent tuning.**  A parameter changed to make a test pass is recorded in the register with its justification; otherwise the change is rejected.
- **Decisions recorded.**  Any deviation from this document is an ADR with context, options and consequences.
- **Performance gates.**  Benchmarks per system run in CI; regressions over 20% block merge.
- **Definition of done.**  Spec updated, tests pass, invariants hold over a 100-year run, benchmarks within budget, UI exposes the new state where relevant. Every new mechanism also passes an ablation test (15.7): switching it off must lower out-of-sample skill or break a falsification test, or it stays off by default.

## 22. Risks and Open Decisions

The largest risk is not technical but epistemic: a model this rich can reproduce anything, so the mitigations centre on keeping free parameters few, nesting known equilibria, and scoring out of sample.

| Risk | Impact | Mitigation |
|---|---|---|
| Complexity explosion; modules interact in untestable ways | High | Two-tier modules, specs, invariants, channel-shutdown tests, phase gates |
| Identification: many parameter sets fit 2000-2010 equally well | High | Three-layer parameterization, literature priors, modular calibration, posterior reporting, natural-experiment tests |
| Province-level data gaps, especially for the ~115 economies outside OECD ICIO | High | Documented imputation, reconciliation to national totals, imputation flags down-weighted in calibration |
| Sensitive real-world content: real states, ethnic groups and ongoing conflicts in an open-source simulator | High | Neutral mechanistic modeling, generic party names, documented sources, outputs framed as distributions not predictions, no editorializing UI text |
| Trade solver misses the laptop budget at province level | Medium | Two-level solver, warm starts, tunable gateway and partner counts, Metal fallback |
| Open-ended runs drift or explode (debt spirals, price drift, demographic collapse) | Medium | Stabilizing policy rules, stability tests to 2150, balanced-growth checks in CI |
| Numerical instability (price oscillation, bank-run cascades) | Medium | Clipped adjustments, stability tests on adjustment speeds, Walrasian fallback switch |
| Railroading toward known post-2000 history | Medium | No date-triggered events; historical shocks enter only as exogenous tape items; LLM tier deferred |
| Dataset licences restrict redistribution | Medium | Bundle-or-fetch policy, licence manifest, P0 audit |
| Determinism broken by parallelism or library updates | Medium | Fixed reductions, pinned toolchains, hash checks in CI |
| Scope creep toward a full game | Medium | Non-goals list (2.4); features must serve experiments |

### 22.1 Decisions (resolved 23 Oct 2026)

| Decision | Choice | Consequence |
|---|---|---|
| Scenario | Modern start, 1 Jan 2000 (supersedes 1836) | Data replaces judgment; 2011-2024 is the out-of-sample window |
| Start-year flexibility | Pipeline builds packs for any year 1995-2022 | Rolling-origin validation (2000, 2005, 2010, 2015) |
| Horizon | Open-ended | Climate, demography and long-run stability are first-class requirements |
| Map scope | Global, developed on a European slice in P1-P3 | Full globe from P4 |
| Provinces | Natural Earth first-level regions at 2000 borders, oversized ones split along county borders, 4,796 | Public domain; geoBoundaries counties add a display layer |
| Markets | Every province prices every good | Two-level trade solver (8.3) |
| UI priority | Observer / laboratory | Branches, diff maps, ensembles before Steward controls |
| App | Tauri desktop app; map moving to native Rust on wgpu (ADR 0011) | Rust core in-process, no server |
| Core language | Rust from the start | Python only for pipeline, reference model, calibration |
| Hardware | MacBook Pro, M-series Pro, 18-36 GB, desktop only | Budgets in 15.2 and 19.2; emulators and modular calibration mandatory |
| Pops | Province x ethnic group x education x labour status | About 100,000 pops; religion as shares |
| Climate | Simple emulator with damages | Endogenous feedback for long runs |
| Technology | Aggregate sector productivity, no named technologies | Frontier growth, catch-up, energy learning curves |
| Parties | Explicit parties over interest groups | Elections, coalitions, party entry |
| Naming | Real states and provinces, generic party labels by family | Recognizable map, less curation and sensitivity |
| IO and trade data | OECD ICIO 2025 edition (1995-2022, 80+ economies) + BACI | About 115 economies imputed |
| Licence | Closed source for now (supersedes open source; ADR 0005) | Restricted data allowed until it is replaced or cleared; Apache-2.0 / MIT on opening |
| LLM leaders | Out of v1 | Tier 2 deferred |
| First release | End of P9 (calibrated, validated world) | No public partial builds |
| Name | Ceteris Paribus; code name ceteris (supersedes WorldSim, which is crowded) | Crates ceteris-*, Python package ceteris, CLI ceteris; check crates.io, PyPI and GitHub before P0 |

### 22.2 Defaults the coding agent may take (recorded as ADRs)

- Exact sector splits beyond the 45 ICIO industries.
- The province-splitting rule (population and area thresholds).
- The imputation model for input-output coefficients of countries outside OECD ICIO.
- Number of trade gateways and partner-set size, set by benchmark results.
- Generic naming scheme for parties.

## 23. References

Recent works checked for this draft are linked; the remainder are standard references cited from knowledge and should be verified when each module spec is written.

### 23.1 Checked for this draft

- Poledna, Miess, Hommes and Rabitsch (2023). Economic forecasting with an agent-based model. European Economic Review 151.
- Wiese, Kaszowska-Mojsa, Dyer, Moran, Pangallo, Lafond, Muellbauer, Calinescu and Farmer (2024). Forecasting macroeconomic dynamics using a calibrated data-driven agent-based model. INET Oxford Working Paper 2024-06.
- Kleinman, Liu and Redding (2023). Dynamic spatial general equilibrium. Econometrica 91(2).
- Dvorkin (2023). Heterogeneous agents dynamic spatial general equilibrium. St. Louis Fed WP 2023-005.
- Lin et al. (2025). Simulating macroeconomic expectations using LLM agents. arXiv.
- Feng, Lu, Su and He (2025). SimCity: multi-agent urban development simulation with rich interactions. arXiv.
- Macroeconomic forecasting from input-output tables (2026), arXiv, for its survey of recent data-driven ABM work including Axtell and Farmer (2025, Journal of Economic Literature) and BeforeIT.jl (Glielmo et al. 2025).

### 23.2 Trade, spatial and growth economics

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

### 23.3 Agent-based, monetary and computational methods

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

### 23.4 Finance and political economy

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

### 23.5 Conflict

- Fearon (1995). Rationalist explanations for war. International Organization.
- Martin, Mayer and Thoenig (2008). Make trade not war? REStud.
- Morrow (1991). Alliances and asymmetry. AJPS.
- Powell (2006). War as a commitment problem. International Organization.
- Singer, Bremer and Stuckey (1972). Capability distribution, uncertainty, and major power war (CINC).
- Slantchev (2003). The principle of convergence in wartime negotiations. APSR.
- Van Creveld (1977). Supplying War.

### 23.6 Added for the modern scenario

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

### 23.7 Added for accuracy and close relatives

- Dawid et al. Eurace@Unibi: an agent-based macroeconomic model for economic policy analysis (with regions and interregional labour mobility).
- Deaton and Laroque (1992). On the behaviour of commodity prices. REStud.
- Grimm et al. (2005). Pattern-oriented modeling of agent-based complex systems. Science.
- Grimm et al. (2020). The ODD protocol for describing agent-based and other simulation models: a second update. JASSS.
- Hegre et al. (2019). ViEWS: a political violence early-warning system. Journal of Peace Research.
- Kennedy and O'Hagan (2001). Bayesian calibration of computer models. JRSS B.
- Lamperti et al. (2018). Faraway, so close: coupled climate and economic dynamics in an agent-based integrated assessment model. Ecological Economics.
- Lux (2018). Estimation of agent-based models using sequential Monte Carlo methods. JEDC.
- Yao, Vehtari, Simpson and Gelman (2018). Using stacking to average Bayesian predictive distributions. Bayesian Analysis.

