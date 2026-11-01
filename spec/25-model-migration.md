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

