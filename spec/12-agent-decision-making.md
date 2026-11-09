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

The inflation that the price and wage rules expect (ADR 0019) is that of a world price index over the buyer prices of all goods, weighted by base consumption, local goods included at their country's price level p_c (ADR 0284).

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

