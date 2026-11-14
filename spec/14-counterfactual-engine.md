## 14. Counterfactual Engine: Ex Ante and Ex Post Interventions

Every experiment is a branch: a parent run plus an ordered list of timed edits, replayed under the same random streams across an ensemble of seeds (members differ by their draws of the uncertain parameters from the register priors, shared with the baseline; ADR 0322), so the reported effect of an intervention is a paired distribution rather than an anecdote.

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
| Parameter | Trade elasticity 5 to 8; migration cost x 0.5; border iceberg of one good (`trade.border_nu.<good>`, ADR 0336) | Ex ante or ex post |
| State (god mode) | Transfer a province; add 1M pops; destroy a fleet; set literacy | Ex ante or ex post |
| Policy (in-world) | The United States does not impose the 2018 steel and aluminium tariffs; a pair's non-tariff barrier (`ntb.<importer>.<exporter>.<group>`, an iceberg factor, default 1, ADR 0337) | Ex post (as a forced decision) |
| Event injection | Declare war; assassinate a ruler; discover gold in a province | Ex post |
| Decision override | Force a state AI's choice at one decision point, or lock a law | Ex post |
| Event or replay | Remove the run's pandemic; add a multi-year US Midwest drought | Ex ante or ex post |
| Module switch | Walrasian vs inventory pricing; Tier 1a vs Tier 2 AI | Ex ante |

In-world edits are distinguished from god-mode edits in the log, because only in-world edits are things an agent could have done.

### 14.3 Snapshots and replay

Full state snapshots are written every 12 ticks as compressed chunks of the canonical encoding, content-addressed so unchanged regions are shared across branches (ADR 0326). Reaching any tick means loading the nearest earlier snapshot and replaying deterministically. Forking at tick t costs one snapshot load. A null intervention must reproduce the parent bit for bit (a CI test). A snapshot carries the run's parameters and names the network it was taken on by the network's content digest, so a restore returns to both; seek reapplies the branch's timed edits as it replays past their ticks, a snapshot at a tick holding the world before that tick's edits; an edit made after seeking back starts a new timeline and drops the later snapshots and edits (ADRs 0291 to 0293).

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

Everything in the UI is available from scripts through the `ceteris` CLI driven by a scenario file (ADR 0113) and, after the P3 gate, through the read-only `ceteris serve` local API (ADRs 0197, 0323), for example: create a branch from a baseline at a tick, apply edits, run an ensemble. Runs write their outcome tables as Arrow IPC, which Polars, DuckDB or any other language reads for analysis without glue code. A thin Python client that only drives runs and reads Arrow is added only if notebook-driven runs prove necessary, under its own ADR.

