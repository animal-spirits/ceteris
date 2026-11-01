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

