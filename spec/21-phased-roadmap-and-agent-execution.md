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
11. **P10 AI layers and Steward mode** (2.0-alpha for economic levers, full Steward and Tier 1b in 2.x; ADR 0239). Learned state policies and playable single-state control; LLM leaders are deferred beyond v1.
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

