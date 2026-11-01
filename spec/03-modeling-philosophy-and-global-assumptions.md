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

