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

