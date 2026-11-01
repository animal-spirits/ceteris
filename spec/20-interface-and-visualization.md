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

