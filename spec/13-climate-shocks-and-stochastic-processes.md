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

