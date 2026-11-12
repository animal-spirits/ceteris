## 15. Calibration, Estimation and Validation

Parameters are set in three layers (data, literature priors, estimation), estimated on 2000-2010 with simulation-based Bayesian inference module by module, and validated out of sample on 2011-2024, including the model's responses to well-studied natural experiments.

### 15.1 Three-layer parameterization

This follows the data-driven ABM program: Poledna, Miess, Hommes and Rabitsch (2023) built the first ABM competitive with VAR and DSGE benchmarks in out-of-sample macro forecasting by initializing agents from national accounts, input-output tables and census data; Wiese et al. (2024) extended it to all 38 OECD countries with approximate Bayesian inference and beat both that model and AR(1) benchmarks out of sample.

1. **Data-determined**: initial populations, capital stocks, input-output coefficients, balance sheets, tax rates, borders, infrastructure, institutions. Not estimated.
2. **Literature-anchored**: structural elasticities with priors from published estimates. Estimated only within prior ranges.
3. **Free behavioural**: adjustment speeds, expectation weights, thresholds. Estimated. Target fewer than 60 in total (Section 17).

### 15.2 Estimation methods

| Method | Used for | Notes |
| --- | --- | --- |
| Modular calibration | Each module against its own targets, others held at data | Essential on one laptop |
| Method of simulated moments | Moments with clear targets (trade shares, unemployment, spreads, fertility) | Common random numbers across draws |
| Neural posterior estimation / ABC-SMC | Joint posterior of behavioural parameters | Simulation-based inference for ABMs as in Dyer et al. (2024), in Rust in `ceteris-calibrate` (ADR 0113) |
| History matching with GP emulators | Ruling out implausible regions first | A few hundred runs |
| Gradient-based variational inference | Behavioural parameters, through forward-mode derivatives of the Rust core (ADR 0096) | Same code as the golden runs; in Hamid et al. (2025) gradient-based variational inference reached in 300 evaluations a loss ABC had not reached in 30,000 |
| Direct structural estimation | Parameters whose estimating equation the model nests: theta\_g, border frictions (P05) and mode cost ratios (P04) by PPML gravity on BACI; migration dispersion (P16) by PPML on bilateral migration flows; Koijen-Yogo demand elasticities by their instrument | Outside the simulation (ADR 0022); frees the run budget for behavioural parameters |
| Multi-fidelity emulation and inference | Low-fidelity simulators (the European slice, gateway-coarsened worlds, short horizons) explore parameter space; full-globe runs correct them (Kennedy and O'Hagan 2000) | Cuts the number of full-globe runs the overnight budget must cover |

**Compute budget on the MacBook (estimate to verify in P0).** A headless 25-year run on one core is targeted at 2 to 3 minutes; running 10 to 12 in parallel gives roughly 250 to 350 runs per hour, about 3,000 overnight. Calibration is planned around that number: emulators and modular steps first, joint inference last.

### 15.3 Validation ladder

| Level | Test | Pass criterion |
| --- | --- | --- |
| V0 | Accounting invariants, determinism, null-branch identity | Zero violations in CI |
| V1 | Equilibrium nesting: frictions off, ABM steady state vs analytical GE; long-run neutrality of a permanent nominal shock | Prices and flows within 1% where the uniqueness condition holds (ADR 0022); real allocations unchanged in the long run; from P5, impulse responses to rate and transfer shocks within tolerance of the sequence-space HANK reference given matched intertemporal MPCs (ADR 0025) |
| V2 | Stylized facts | Gravity distance elasticity near the consensus around -0.9 (Disdier and Head 2008); Zipf's law for city sizes; Engel curves; Pareto firm-size tails within cells (ADR 0026); Okun's law; Phillips-curve and Beveridge-curve shapes |
| V3 | Out-of-sample 2011-2024 | See 15.4 |
| V4 | Natural-experiment replication | See 15.5 |
| V5 | Expert face validity | Structured review of baseline runs by economists and political scientists |

### 15.4 Out-of-sample targets (2011-2024)

| Target | Source | Metric |
| --- | --- | --- |
| GDP growth, inflation, unemployment by country | National accounts, IMF and World Bank series | Rolling-origin forecasts vs AR(1) and VAR (as in Poledna and Wiese) |
| Bilateral trade by sector | BACI, ICIO | PPML gravity coefficients, share RMSE |
| Subnational output and population | Subnational output data, gridded population | Log RMSE, rank correlation |
| Commodity prices | Commodity price series (oil, food, metals) | Volatility and co-movement |
| Migration stocks and flows | UN migrant stock, flow estimates | Flow RMSE |
| Sovereign spreads, defaults, banking crises | Market data, crisis databases | Distribution match |
| Emissions and temperature | National inventories, observational records | Path match |
| Regime indices, coups, civil conflict | V-Dem, coup data, UCDP GED | Frequencies and hazards by region, not dates |

Forecasts are scored with proper scoring rules (CRPS) and interval coverage: a 90% ensemble band should contain the realized series about 90% of the time. Political and military targets are scored on hazards and frequencies, never specific events. Validation also uses rolling origins: packs built for 2000, 2005, 2010 and 2015 each run forward and are scored on years outside the calibration window, so performance is judged across several starting points rather than one.

Runs on the observed current-account path (ADR 0100, S22) take the 2000 to 2024 imbalances as given, so their scores are conditional forecasts and are labelled so.

### 15.5 Natural-experiment replication

Each episode runs as a subset of replay (ADR 0241): the model is fed the historical shock and its response is compared with the published empirical estimate:

| Episode | Model test | Empirical benchmark |
| --- | --- | --- |
| China shock (2001-2010) | Manufacturing employment response to import exposure across US provinces | Autor, Dorn and Hanson (2013) |
| 2018-19 US-China tariffs | Import-price pass-through and trade diversion | Amiti, Redding and Weinstein (2019); Fajgelbaum et al. (2020) |
| 2008 financial crisis | Trade collapse and credit-crunch propagation | Great Trade Collapse literature |
| 2022 Russia sanctions and energy shock | Trade rerouting, European energy prices | Post-2022 trade data |
| COVID 2020 | Sector output collapse and recovery, supply-chain shortages | Sector data 2020-2022 |

### 15.6 Guarding against overfitting

Targets, holdout periods and natural experiments are fixed in a pre-registration file before estimation; every tuned parameter is in the register; models with more free parameters must beat simpler variants on holdout scores, not in-sample fit.

### 15.7 Accuracy and anti-overfitting

Accuracy comes first from how the model is fed and judged, second from targeted additions, and never from mechanisms that cannot prove their value out of sample.

**Free-moment gates.** From the P4 gate, the free moments of RFC 0350 are hard gates with fixed pass bands (ADRs 0364, 0365): H2 distance in [-1.1, -0.7]; UK welfare under hard and soft Brexit within 0.5x to 1.5x of Dhingra et al. (2017); Japan and EU27 welfare negative under costlier borders; ACR-IO with no wrong sign, median in [0.8, 1.25] and correlation at least 0.9; openness per class within 1.5x of ICIO; border x10 survives 1,500 ticks with index at most 1.07 and welfare within 0.5x to 2x of the ACR prediction; US-China 2018 pass-through in [0.8, 1.2] and welfare in [-0.1%, -0.02%]; PPML theta class spread at least 3; short-run trade elasticity in [0.5, 1.5] with long-run over short-run at least 1.5, from the gate after #200. A band moves only by RFC.

**Methods (no new mechanisms)**

- **Data assimilation.** During the calibration window and at every rolling origin, the simulated state is nudged toward observed data with an ensemble Kalman or particle filter (sequential Monte Carlo for ABMs, as in Lux 2018), so forecasts start from data-consistent states. Each analysis step is followed by a projection through the consistency solver (23.2), so assimilated states satisfy the invariants of 4.6.
- **Posterior ensembles.** Ensemble members draw parameters from the posterior, so reported uncertainty includes parameter uncertainty, not only shock noise.
- **Model discrepancy.** Calibration includes an explicit discrepancy term (Kennedy and O'Hagan 2001) so parameters do not absorb structural error.
- **Stacking across structural switches.** Plausible module alternatives (17.2) run side by side and are weighted by out-of-sample skill (Bayesian stacking).
- **Two-way scorecard.** Validation runs from each origin with replay off (unconditional) and on (conditional); replay with and without its policy family separates economic-mechanism error from policy-rule error (ADRs 0210, 0241).

**Targets: micro facts over macro paths**

Many independent, well-identified moments constrain the model better than a few aggregate series: price-change frequencies of posted export prices (8.8), intertemporal marginal propensities to consume out of transfers, job-finding and separation rates, firm entry and exit rates, survey inflation expectations, and exchange-rate pass-through by invoicing currency. Targets span several scales at once, following pattern-oriented modelling (Grimm et al. 2005).

**Targeted additions**

- Wealth distribution per pop from distributional national accounts (World Inequality Database), for realistic spending heterogeneity and policy transmission.
- Competitive speculative storage for commodities (Deaton and Laroque), refining merchant inventories so food and energy price spikes are realistic.
- Cross-border bank claims (BIS locational banking statistics) to initialize foreign financial exposures.

**Benchmarks**

- Macro forecasts are scored against IMF World Economic Outlook forecasts as well as AR(1) and VAR baselines.
- Conflict hazards are scored against the ViEWS early-warning system (Hegre et al. 2019).

**Guardrails**

- **Ablation.** Every phase switches each module off and measures the change in out-of-sample skill; a mechanism that neither improves holdout scores nor passes a falsification test stays off by default.
- **Leave-one-region-out calibration.** Parameters are re-estimated without each world region and tested on it; parameters that only work where fitted are flagged.
- **Parameter freeze.** 2011-2024 data stays unseen until scoring.
- **Fuzz testing.** Random interventions and extreme shocks must never break invariants or produce non-finite values.
- **Documentation.** The model is described in the ODD protocol (Overview, Design concepts, Details; Grimm et al. 2020) so outsiders can review it.

**Excluded as bloat**

Named firms or individuals, sub-province geography, daily financial markets, power-grid dispatch, per-agent optimization, more sectors than the input-output tables, and LLM agents: each adds parameters faster than accuracy.

