# RFCs

A proposal to change the design. Copy [0000-template.md](0000-template.md) to
`rfcs/NNNN-slug.md` with the next free RFC number (the highest file here plus one), open a draft
PR, and label each choice D1, D2, and so on. The owner answers each one; the answer and its date
go in the RFC. Accepted items take ADR numbers in the same PR, as rows in
[spec/rows.toml](../spec/rows.toml), and the PR edits the sections they change.
Rejected and deferred RFCs are merged too, so the reasoning stays on record.

RFCs 0001 to 0281 are the former ADR files, moved here with their numbers; for them RFC n
decided ADR n. From 0282 on the two counters are separate: an RFC is cited as RFC NNNN, a
decision as ADR NNNN, and one RFC may decide several rows. The older files keep their own
header format (Status, Date, Design sections; Context, Options, Decision, Consequences).

The full procedure is in [CLAUDE.md](../CLAUDE.md), "Decisions, specs and research".

## Index

| RFC | Title | Status | Rows |
| --- | --- | --- | --- |
| 0001 | [Record architecture decisions](0001-record-architecture-decisions.md) | accepted | ADR 0001 |
| 0002 | [Canonical binary snapshots before Arrow](0002-p0-snapshot-codec.md) | accepted | ADR 0002 |
| 0003 | [Python distribution name](0003-python-distribution-name.md) | proposed | ADR 0003 |
| 0004 | [Benchmark regression gate](0004-benchmark-gate.md) | accepted | ADR 0004 |
| 0005 | [Data licence audit and closed-source fallbacks](0005-data-licence-audit.md) | accepted | ADR 0005 |
| 0006 | [Province set at 2000 borders](0006-province-set.md) | accepted | ADR 0006 |
| 0007 | [Transport graph construction](0007-transport-graph.md) | accepted | ADR 0007 |
| 0008 | [Pack format and map delivery in P1](0008-pack-format-and-map.md) | accepted | ADR 0008 |
| 0009 | [County display layer](0009-county-display-layer.md) | accepted | ADR 0009 |
| 0010 | [GPU fill layer, pack protocol and globe](0010-gpu-fills-and-pack-protocol.md) | accepted | ADR 0010 |
| 0011 | [Rewrite the map in Rust](0011-native-rust-map.md) | accepted | ADR 0011 |
| 0012 | [Simulate the full globe from P2](0012-full-globe-from-p2.md) | accepted | ADR 0012 |
| 0013 | [Trade routing as a posted-price logit on a two-level kernel](0013-trade-routing-logit-two-level-kernel.md) | superseded by 0015 and 0016 | ADR 0013 |
| 0014 | [P2 data seeding, downscaling and imputation](0014-p2-data-seeding-and-imputation.md) | accepted | ADR 0014 |
| 0015 | [Trade flows in value with a dimensionless gravity kernel](0015-trade-flows-in-value.md) | accepted | ADR 0015 |
| 0016 | [Clear origin markets across ticks](0016-clear-origin-markets-across-ticks.md) | accepted | ADR 0016 |
| 0017 | [Route-choice trade costs](0017-route-choice-trade-costs.md) | accepted | ADR 0017 |
| 0018 | [Sector-specific trade elasticities](0018-sector-trade-elasticities.md) | accepted | ADR 0018 |
| 0019 | [Expected inflation in price and wage rules](0019-expected-inflation-in-price-and-wage-rules.md) | accepted | ADR 0019 |
| 0020 | [Putty-clay intermediates and within-tick rationing](0020-putty-clay-and-within-tick-rationing.md) | accepted | ADR 0020 |
| 0021 | [PIGL demand and a household consumption rule](0021-pigl-demand-and-consumption-rule.md) | accepted | ADR 0021 |
| 0022 | [Uniqueness check, static nesting gate and direct estimation](0022-uniqueness-nesting-gate-and-direct-estimation.md) | accepted | ADR 0022 |
| 0023 | [Trade elasticity classes](0023-trade-elasticity-classes.md) | accepted | ADR 0023 |
| 0024 | [Ensemble lanes](0024-ensemble-lanes.md) | proposed | ADR 0024 |
| 0025 | [Sequence-space HANK reference for the macro block](0025-sequence-space-hank-reference.md) | proposed | ADR 0025 |
| 0026 | [Firm heterogeneity within cells](0026-firm-heterogeneity-within-cells.md) | proposed | ADR 0026 |
| 0027 | [Short- and long-run trade elasticities](0027-short-and-long-run-trade-elasticities.md) | proposed | ADR 0027 |
| 0028 | [Oil and gas market structure](0028-oil-and-gas-market-structure.md) | proposed | ADR 0028 |
| 0029 | [Food system: competitive storage, export restrictions and famine](0029-food-system-storage-export-bans-and-famine.md) | proposed | ADR 0029 |
| 0030 | [Variable markups within cells](0030-variable-markups.md) | proposed | ADR 0030 |
| 0031 | [Structural transformation and informality dynamics](0031-structural-transformation-and-informality.md) | proposed | ADR 0031 |
| 0032 | [Cohort demography: education transitions, fertility timing and households](0032-cohort-demography-and-education-transitions.md) | proposed | ADR 0032 |
| 0033 | [Mortality causes and health](0033-mortality-causes-and-health.md) | proposed | ADR 0033 |
| 0034 | [Power sector via residual load duration curves](0034-power-sector-residual-load-duration-curves.md) | proposed | ADR 0034 |
| 0035 | [Technology diffusion and energy policy instruments](0035-technology-diffusion-and-energy-policy-instruments.md) | proposed | ADR 0035 |
| 0036 | [Climate damage lanes](0036-climate-damage-lanes.md) | proposed | ADR 0036 |
| 0037 | [Regional climate emulation, tipping elements and sea level](0037-regional-climate-tipping-and-sea-level.md) | proposed | ADR 0037 |
| 0038 | [Water, air pollution and land use](0038-water-air-and-land.md) | proposed | ADR 0038 |
| 0039 | [Automation and AI as a scenario lane](0039-automation-and-ai-scenario-lane.md) | proposed | ADR 0039 |
| 0040 | [Housing supply elasticity and mortgage underwriting](0040-housing-supply-and-mortgage-underwriting.md) | proposed | ADR 0040 |
| 0041 | [Multinational production and profit shifting](0041-multinational-production-and-profit-shifting.md) | proposed | ADR 0041 |
| 0042 | [Public capital and fiscal dynamics](0042-public-capital-and-fiscal-dynamics.md) | proposed | ADR 0042 |
| 0043 | [Sovereign debt restructuring and creditor structure](0043-sovereign-debt-restructuring-and-creditors.md) | proposed | ADR 0043 |
| 0044 | [Pensions, life-cycle saving and wealth dynamics](0044-pensions-life-cycle-and-wealth-dynamics.md) | proposed | ADR 0044 |
| 0045 | [Economic statecraft: critical minerals, export controls and sanctions evasion](0045-economic-statecraft-critical-minerals-and-sanctions-evasion.md) | proposed | ADR 0045 |
| 0046 | [Political economy channels: populism, information, polarization and institutions](0046-political-economy-channels.md) | proposed | ADR 0046 |
| 0047 | [Trade agreements and international organizations](0047-trade-agreements-and-international-organizations.md) | proposed | ADR 0047 |
| 0048 | [Grounding the war module in modern evidence](0048-military-realism-grounding.md) | proposed | ADR 0048 |
| 0049 | [Explainability panels and event chronicle](0049-explainability-panels-and-event-chronicle.md) | proposed | ADR 0049 |
| 0050 | [Research and release pack profiles, and sources to audit](0050-research-and-release-pack-profiles.md) | proposed | ADR 0050 |
| 0051 | [A fat-tailed hazard library for rare events](0051-fat-tailed-hazard-library.md) | proposed | ADR 0051 |
| 0052 | [Forecast scorecard: real-time vintages, hard benchmarks and proper tests](0052-forecast-scorecard-vintages-and-benchmarks.md) | proposed | ADR 0052 |
| 0053 | [Robust inference and model selection](0053-robust-inference-and-model-selection.md) | proposed | ADR 0053 |
| 0054 | [Explicit exclusions](0054-explicit-exclusions.md) | proposed | ADR 0054 |
| 0055 | [Cohort values: secular and emancipative orientations](0055-cohort-values.md) | proposed | ADR 0055 |
| 0056 | [Generalized trust and civil society](0056-generalized-trust-and-civil-society.md) | proposed | ADR 0056 |
| 0057 | [Gender norms, sex ratios at birth and marriage markets](0057-gender-norms-sex-ratios-and-marriage-markets.md) | proposed | ADR 0057 |
| 0058 | [Religion switching and identity salience](0058-religion-switching-and-identity-salience.md) | proposed | ADR 0058 |
| 0059 | [Learning quality, not only years of schooling](0059-learning-quality.md) | proposed | ADR 0059 |
| 0060 | [Crime, homicide and organized crime](0060-crime-homicide-and-organized-crime.md) | proposed | ADR 0060 |
| 0061 | [Health-system capacity](0061-health-system-capacity.md) | proposed | ADR 0061 |
| 0062 | [Chronic disease risk and antimicrobial resistance](0062-chronic-disease-and-antimicrobial-resistance.md) | proposed | ADR 0062 |
| 0063 | [Addiction epidemics](0063-addiction-epidemics.md) | proposed | ADR 0063 |
| 0064 | [Child nutrition and the care burden of ageing](0064-child-nutrition-and-care-burden.md) | proposed | ADR 0064 |
| 0065 | [True and reported statistics](0065-true-and-reported-statistics.md) | proposed | ADR 0065 |
| 0066 | [Leaders, tenure and succession](0066-leader-tenure-and-succession.md) | proposed | ADR 0066 |
| 0067 | [State capacity: building, eroding and collapsing](0067-state-capacity-dynamics.md) | proposed | ADR 0067 |
| 0068 | [Subnational governance, land tenure, clientelism and electoral integrity](0068-subnational-governance-land-tenure-and-clientelism.md) | proposed | ADR 0068 |
| 0069 | [Non-state armed actors, insurgency finance and terrorism](0069-non-state-armed-actors-and-insurgency-finance.md) | proposed | ADR 0069 |
| 0070 | [Development finance actors](0070-development-finance-actors.md) | proposed | ADR 0070 |
| 0071 | [Information technology and the information environment](0071-information-technology-and-the-information-environment.md) | proposed | ADR 0071 |
| 0072 | [International order, peace operations, courts and soft power](0072-international-order-and-soft-power.md) | proposed | ADR 0072 |
| 0073 | [Proliferation, the Arctic, frozen conflicts and intelligence](0073-proliferation-arctic-frozen-conflicts-and-intelligence.md) | proposed | ADR 0073 |
| 0074 | [Settlement hierarchy and urban systems](0074-settlement-hierarchy-and-urban-systems.md) | proposed | ADR 0074 |
| 0075 | [Infrastructure interdependence and cascading failures](0075-infrastructure-interdependence-and-cascades.md) | proposed | ADR 0075 |
| 0076 | [Groundwater, shared river basins and glacier water towers](0076-groundwater-river-basins-and-glaciers.md) | proposed | ADR 0076 |
| 0077 | [Heat, wildfire, fisheries and deforestation frontiers](0077-heat-wildfire-fisheries-and-deforestation.md) | proposed | ADR 0077 |
| 0078 | [Chronicle narration, storyteller camera and legends](0078-chronicle-narration-storyteller-camera-and-legends.md) | proposed | ADR 0078 |
| 0079 | [Red-team branches and surrogate sliders](0079-red-team-branches-and-surrogate-sliders.md) | proposed | ADR 0079 |
| 0080 | [Polycrisis coupling audit](0080-polycrisis-coupling-audit.md) | proposed | ADR 0080 |
| 0081 | [Seasonality and weather-driven demand](0081-seasonality-and-weather-driven-demand.md) | proposed | ADR 0081 |
| 0082 | [Cohort experience effects on expectations and risk-taking](0082-cohort-experience-effects.md) | proposed | ADR 0082 |
| 0083 | [Policy implementation and diffusion](0083-policy-implementation-and-diffusion.md) | proposed | ADR 0083 |
| 0084 | [Price controls, subsidies, parallel markets and currency substitution](0084-price-controls-parallel-markets-and-currency-substitution.md) | proposed | ADR 0084 |
| 0085 | [Strategic reserves and stockpiles](0085-strategic-reserves-and-stockpiles.md) | proposed | ADR 0085 |
| 0086 | [Reconstruction and recovery](0086-reconstruction-and-recovery.md) | proposed | ADR 0086 |
| 0087 | [Synthetic observables for validation](0087-synthetic-observables.md) | proposed | ADR 0087 |
| 0088 | [Fully native app with Slint panels](0088-native-app-slint.md) | accepted | ADR 0088 |
| 0089 | [Data-state steady-state calibration and natural-rate unemployment](0089-data-state-steady-state-calibration.md) | accepted | ADR 0089 |
| 0090 | [Split price rule and inventory gain](0090-split-price-rule-and-inventory-gain.md) | proposed | ADR 0090 |
| 0091 | [Competitive storage in P2 with per-market price expectations](0091-competitive-storage-in-p2.md) | proposed | ADR 0091 |
| 0092 | [Trade frictions consistent with theta](0092-trade-frictions-consistent-with-theta.md) | accepted (RFC 0312) | ADR 0092 |
| 0093 | [Numerical robustness of the P2 core](0093-numerical-robustness-of-the-p2-core.md) | accepted (RFC 0312) | ADR 0093 |
| 0094 | [PIGL aggregation across pops](0094-pigl-aggregation-across-pops.md) | accepted (RFC 0312) | ADR 0094 |
| 0095 | [Consistency amendments to proposed design ADRs](0095-consistency-amendments-to-design-adrs.md) | proposed | ADR 0095 |
| 0096 | [Forward-mode derivatives in the Rust core](0096-forward-mode-derivatives.md) | proposed | ADR 0096 |
| 0097 | [Determinism lints and the post-gate dispatch speed plan](0097-determinism-lints-and-dispatch-speed-plan.md) | accepted | ADR 0097 |
| 0098 | [Price levels and source corrections at calibration](0098-calibration-price-levels-and-source-corrections.md) | accepted (RFC 0312) | ADR 0098 |
| 0099 | [Split final demand and a fixed saving rate in P2](0099-final-demand-split-and-saving-rate.md) | proposed | ADR 0099 |
| 0100 | [Trade imbalances as exogenous transfers until finance](0100-trade-imbalances-as-transfers.md) | accepted (RFC 0312) | ADR 0100 |
| 0101 | [Harmonic aggregation of trade elasticities](0101-harmonic-aggregation-of-trade-elasticities.md) | accepted (RFC 0312) | ADR 0101 |
| 0102 | [Bilateral, transit and internal trade frictions](0102-bilateral-and-internal-trade-frictions.md) | proposed | ADR 0102 |
| 0103 | [Initial drift: 2000 is not a rest point](0103-initial-drift-off-steady-state.md) | accepted (RFC 0312) | ADR 0103 |
| 0104 | [Sector labour shares and the distribution of capital income](0104-sector-factor-shares-and-capital-income.md) | proposed | ADR 0104 |
| 0105 | [Skill groups and sector switching costs](0105-skill-groups-and-sector-switching-costs.md) | proposed | ADR 0105 |
| 0106 | [Price clip on the gap only, and money width](0106-price-clip-and-money-width.md) | accepted (RFC 0312) | ADR 0106 |
| 0107 | [Holdout protocol, Monte Carlo error and parameter lanes](0107-holdout-protocol-and-monte-carlo-error.md) | proposed | ADR 0107 |
| 0108 | [Estimation windows per module and long-run lanes](0108-estimation-windows-and-long-run-lanes.md) | proposed | ADR 0108 |
| 0109 | [International migration under admission caps](0109-international-migration-under-admission-caps.md) | proposed | ADR 0109 |
| 0110 | [Reduced-form conflict hazard as the lab default](0110-reduced-form-conflict-hazard-default.md) | proposed | ADR 0110 |
| 0111 | [Amendments from the assumption audit](0111-amendments-from-the-assumption-audit.md) | proposed | ADR 0111 |
| 0112 | [Loss write-off in P2](0112-loss-write-off-in-p2.md) | accepted | ADR 0112 |
| 0113 | [Full migration of the Python code to Rust](0113-full-migration-to-rust.md) | accepted | ADR 0113 |
| 0114 | [Merge Natural Earth units below a country's first level](0114-merge-sub-first-level-units.md) | rejected | ADR 0114 |
| 0115 | [Territory as partitions of a fixed cell grid](0115-territory-as-cell-partitions.md) | proposed | ADR 0115 |
| 0116 | [Version-control verbs for branches](0116-version-control-verbs-for-branches.md) | proposed | ADR 0116 |
| 0117 | [Goal-seek interventions and leverage maps](0117-goal-seek-and-leverage-maps.md) | proposed | ADR 0117 |
| 0118 | [Contrastive explanations and history's percentile](0118-contrastive-explanations-and-historys-percentile.md) | proposed | ADR 0118 |
| 0119 | [Multiverse results and built-in preregistration](0119-multiverse-results-and-preregistration.md) | proposed | ADR 0119 |
| 0120 | [Fuzzing the model against stylized facts](0120-model-fuzzing-and-llm-auditor.md) | proposed | ADR 0120 |
| 0121 | [Friction dial and debate packs](0121-friction-dial-and-debate-packs.md) | proposed | ADR 0121 |
| 0122 | [Element and embodied-energy ledgers](0122-element-and-energy-ledgers.md) | proposed | ADR 0122 |
| 0123 | [Reflexive forecasters and alternative monetary regimes](0123-reflexive-forecasters-and-alternative-monetary-regimes.md) | proposed | ADR 0123 |
| 0124 | [Economic views: trade-cost cartogram, receipt, atlas and diegetic eras](0124-economic-views-cartogram-receipt-atlas-eras.md) | proposed | ADR 0124 |
| 0125 | [LLM tools outside the core](0125-llm-tools-outside-the-core.md) | proposed | ADR 0125 |
| 0126 | [Public benchmarks: CeterisBench and an agent gym](0126-public-benchmarks-ceterisbench-and-agent-gym.md) | proposed | ADR 0126 |
| 0127 | [Research infrastructure: living papers, parameter wiki, volunteer ensembles, model court](0127-research-infrastructure.md) | proposed | ADR 0127 |
| 0128 | [Distribution: runs as hashes, reproducible builds, annual report, engine crate](0128-distribution-runs-as-hashes-and-reproducible-builds.md) | accepted in part (RFC 0318) | ADR 0128 |
| 0129 | [Constitutional wind tunnel and veil-of-ignorance scoring](0129-constitutions-and-veil-of-ignorance-scoring.md) | proposed | ADR 0129 |
| 0130 | [New seats: idea, founder, coalition partner, mediator, quartermaster, citizen](0130-new-seats.md) | proposed | ADR 0130 |
| 0131 | [Deep history: economic reanalysis, agrarian worlds, period advisers, long runs](0131-deep-history.md) | proposed | ADR 0131 |
| 0132 | [Stakes, memory and community play](0132-stakes-memory-and-community-play.md) | proposed | ADR 0132 |
| 0133 | [Other worlds: Earth variants, importers, LLM-drafted packs, nested zoom](0133-other-worlds.md) | proposed | ADR 0133 |
| 0134 | [Sandboxed mechanism modules](0134-sandboxed-mechanism-modules.md) | proposed | ADR 0134 |
| 0135 | [Model intercomparison protocol and deterministic coupling](0135-model-intercomparison-and-coupling-protocol.md) | proposed | ADR 0135 |
| 0136 | [What earns a major version](0136-what-earns-a-major-version.md) | accepted | ADR 0136 |
| 0137 | [Art and sound direction: Modern Atlas](0137-art-and-sound-direction-modern-atlas.md) | accepted | ADR 0137 |
| 0138 | [Natural capital as extended-profile minors](0138-natural-capital-as-extended-minors.md) | proposed | ADR 0138 |
| 0139 | [Narrative state: endogenous belief content as a major candidate](0139-narrative-state-as-a-major-candidate.md) | proposed | ADR 0139 |
| 0140 | [Seat chrome by country and regime](0140-seat-chrome-by-country-and-regime.md) | accepted | ADR 0140 |
| 0141 | [Place and regime flavour from data](0141-place-and-regime-flavour-from-data.md) | proposed | ADR 0141 |
| 0142 | [Flags: real first, then AI-assisted and editable](0142-flags-real-first-ai-assisted-and-editable.md) | proposed | ADR 0142 |
| 0143 | [State names: localized, dynamic and editable](0143-state-names-localized-dynamic-editable.md) | proposed | ADR 0143 |
| 0144 | [Replay hooks](0144-replay-hooks.md) | proposed | ADR 0144 |
| 0145 | [War architecture: clocks, grains and profiles](0145-war-architecture-clocks-and-profiles.md) | proposed | ADR 0145 |
| 0146 | [Forces, order of battle and the forces of 2000](0146-forces-order-of-battle-and-initial-forces.md) | proposed | ADR 0146 |
| 0147 | [Command, control and delegation](0147-command-control-and-delegation.md) | proposed | ADR 0147 |
| 0148 | [Physical logistics on a military overlay](0148-physical-logistics-on-a-military-overlay.md) | proposed | ADR 0148 |
| 0149 | [Land combat resolver](0149-land-combat-resolver.md) | proposed | ADR 0149 |
| 0150 | [Air power, drones and air defence](0150-air-power-drones-and-air-defence.md) | proposed | ADR 0150 |
| 0151 | [Naval forces and sea control](0151-naval-forces-and-sea-control.md) | proposed | ADR 0151 |
| 0152 | [Strategic strike, nuclear forces and escalation](0152-strategic-strike-nuclear-forces-and-escalation.md) | proposed | ADR 0152 |
| 0153 | [Intelligence and fog of war as belief states](0153-intelligence-and-fog-of-war-as-belief-states.md) | proposed | ADR 0153 |
| 0154 | [Irregular war, occupation, civilian harm and atrocities](0154-irregular-war-occupation-and-atrocities.md) | proposed | ADR 0154 |
| 0155 | [Economic lenses](0155-economic-lenses.md) | proposed | ADR 0155 |
| 0156 | [Influence web from derivatives](0156-influence-web-from-derivatives.md) | proposed | ADR 0156 |
| 0157 | [Static and dynamic scoring of policy packages](0157-static-and-dynamic-scoring-of-policy-packages.md) | proposed | ADR 0157 |
| 0158 | [Onboarding: lessons and in-app pedia](0158-onboarding-lessons-and-in-app-pedia.md) | proposed | ADR 0158 |
| 0159 | [Situations as tracked processes](0159-situations-as-tracked-processes.md) | proposed | ADR 0159 |
| 0160 | [Trade-core seats: corridor planner and commodity trader](0160-trade-core-seats-corridor-planner-and-commodity-trader.md) | proposed | ADR 0160 |
| 0161 | [Ambitions, Pareto histograms, ascension and policy paths](0161-ambitions-pareto-histograms-ascension-and-policy-paths.md) | proposed | ADR 0161 |
| 0162 | [Commitment devices](0162-commitment-devices.md) | proposed | ADR 0162 |
| 0163 | [State reach](0163-state-reach.md) | proposed | ADR 0163 |
| 0164 | [Whip count and private ledger objective](0164-whip-count-and-private-ledger-objective.md) | proposed | ADR 0164 |
| 0165 | [Mobilization and the war economy](0165-mobilization-and-the-war-economy.md) | proposed | ADR 0165 |
| 0166 | [War interaction surfaces and modding](0166-war-interaction-surfaces-and-modding.md) | proposed | ADR 0166 |
| 0167 | [Endemic disease: HIV/AIDS, malaria and tuberculosis](0167-endemic-disease.md) | proposed | ADR 0167 |
| 0168 | [Cross-border services trade and tourism](0168-cross-border-services-and-tourism.md) | proposed | ADR 0168 |
| 0169 | [Hours worked and short-time work](0169-hours-and-short-time-work.md) | proposed | ADR 0169 |
| 0170 | [Enterprise input stocks and inventory investment](0170-enterprise-input-stocks.md) | proposed | ADR 0170 |
| 0171 | [State-owned enterprise behaviour](0171-state-owned-enterprise-behaviour.md) | proposed | ADR 0171 |
| 0172 | [Bank forbearance and trade finance](0172-forbearance-and-trade-finance.md) | proposed | ADR 0172 |
| 0173 | [Consumer durables](0173-consumer-durables.md) | proposed | ADR 0173 |
| 0174 | [Temporary labour migration](0174-temporary-labour-migration.md) | proposed | ADR 0174 |
| 0175 | [Offshore household wealth, bargaining coverage and space weather](0175-offshore-wealth-bargaining-space-weather.md) | proposed | ADR 0175 |
| 0176 | [Endogenous neutral rate in the policy rule](0176-endogenous-neutral-rate.md) | proposed | ADR 0176 |
| 0177 | [Natural unemployment rate from matching](0177-emergent-natural-unemployment.md) | proposed | ADR 0177 |
| 0178 | [Endogenous exploration and reserves](0178-endogenous-exploration-and-reserves.md) | proposed | ADR 0178 |
| 0179 | [Research effort and its direction](0179-research-effort-and-direction.md) | proposed | ADR 0179 |
| 0180 | [Endogenous shipping costs and water-limited waterways](0180-endogenous-transport-costs.md) | proposed | ADR 0180 |
| 0181 | [Small endogenizations: invoicing, piracy, housing regulation, pandemic spillover](0181-small-endogenizations.md) | proposed | ADR 0181 |
| 0182 | [Data sources to add from the gap audit](0182-data-sources-from-the-gap-audit.md) | proposed | ADR 0182 |
| 0183 | [Characters and the real 2000 roster](0183-characters-and-the-real-2000-roster.md) | proposed | ADR 0183 |
| 0184 | [Character traits from data, and the ablation gate](0184-character-traits-from-data.md) | proposed | ADR 0184 |
| 0185 | [Character memory and relations](0185-character-memory-and-relations.md) | proposed | ADR 0185 |
| 0186 | [Careers, pools and generated successors](0186-careers-pools-and-generated-successors.md) | proposed | ADR 0186 |
| 0187 | [Leadership dossiers instead of portraits](0187-leadership-dossiers-instead-of-portraits.md) | accepted | ADR 0187 |
| 0188 | [Keyboard-first input: command registry, two keymaps, palette and query language](0188-keyboard-first-input.md) | proposed | ADR 0188 |
| 0189 | [Emergent organizations](0189-emergent-organizations.md) | proposed | ADR 0189 |
| 0190 | [Administrative divisions as an open partition registry](0190-administrative-divisions-as-an-open-partition-registry.md) | proposed | ADR 0190 |
| 0191 | [Wellbeing: material welfare and life evaluation](0191-wellbeing-material-welfare-and-life-evaluation.md) | proposed | ADR 0191 |
| 0192 | [Approval and legitimacy as separate stocks](0192-approval-and-legitimacy-as-separate-stocks.md) | proposed | ADR 0192 |
| 0193 | [Vote intention, turnout and polls](0193-vote-intention-turnout-and-polls.md) | proposed | ADR 0193 |
| 0194 | [Legislative voting, discipline and veto gates](0194-legislative-voting-discipline-and-veto-gates.md) | proposed | ADR 0194 |
| 0195 | [Government as bodies with scopes and competences](0195-government-bodies-competences-and-appointments.md) | proposed | ADR 0195 |
| 0196 | [Multiple windows and workspaces](0196-multiple-windows-and-workspaces.md) | proposed | ADR 0196 |
| 0197 | [Local API, exports and deep links](0197-local-api-and-exports.md) | proposed | ADR 0197 |
| 0198 | [Regime balance by seat-relative play](0198-regime-balance-by-seat-relative-play.md) | proposed | ADR 0198 |
| 0199 | [Loyalty belief state and coup-proofing](0199-loyalty-belief-state-and-coup-proofing.md) | proposed | ADR 0199 |
| 0200 | [Selection on trade links](0200-selection-on-trade-links.md) | accepted | ADR 0200 |
| 0201 | [Ownership cells from OECD Analytical AMNE](0201-ownership-cells-from-oecd-amne.md) | proposed | ADR 0201 |
| 0202 | [Firm entry, exit and the zombie band](0202-firm-entry-exit-and-zombie-band.md) | proposed | ADR 0202 |
| 0203 | [Named firms: view, granular opt-in, political actors](0203-named-firms-view-granular-and-political.md) | proposed | ADR 0203 |
| 0204 | [Capital formation by asset class and time to build](0204-capital-formation-and-time-to-build.md) | proposed | ADR 0204 |
| 0205 | [Construction capacity as a priced bottleneck, with priority](0205-construction-capacity-and-priority.md) | proposed | ADR 0205 |
| 0206 | [Named projects, the 2000 pipeline and the project tape](0206-named-projects-pipeline-and-project-tape.md) | proposed | ADR 0206 |
| 0207 | [The 1.0 phase plan and scope rule](0207-1.0-phase-plan-and-scope.md) | accepted | ADR 0207 |
| 0208 | [Presentation pipeline and time controls](0208-presentation-pipeline-and-time-controls.md) | accepted | ADR 0208 |
| 0209 | [Sparse hourly war phase](0209-sparse-hourly-war-phase.md) | proposed | ADR 0209 |
| 0210 | [Policy in 1.0: estimated rules per lever and the policy tape](0210-policy-rules-and-the-policy-tape-in-1.0.md) | proposed | ADR 0210 |
| 0211 | [Run files and compatibility across versions](0211-run-files-and-compatibility-across-versions.md) | accepted in part (RFC 0318) | ADR 0211 |
| 0212 | [History retention and the disk budget](0212-history-retention-and-disk-budget.md) | accepted (RFC 0318) | ADR 0212 |
| 0213 | [Data licences in shared runs and exports](0213-data-licences-in-shared-runs-and-exports.md) | proposed | ADR 0213 |
| 0214 | [Failure handling, repro bundles and opt-in usage statistics](0214-failure-handling-repro-bundles-and-usage-stats.md) | proposed | ADR 0214 |
| 0215 | [First-run data fetch and the source archive](0215-first-run-data-fetch-and-source-archive.md) | proposed | ADR 0215 |
| 0216 | [Platforms, signing, updates and minimum hardware](0216-platforms-signing-updates-and-minimum-hardware.md) | proposed | ADR 0216 |
| 0217 | [Pack profiles as prebuilt and user-built, and the credits screen](0217-pack-profiles-as-prebuilt-and-user-built-and-credits.md) | proposed | ADR 0217 |
| 0218 | [Repository opening and contributions](0218-repository-opening-and-contributions.md) | proposed | ADR 0218 |
| 0219 | [Documentation set](0219-documentation-set.md) | proposed | ADR 0219 |
| 0220 | [Localization pipeline and 1.0 languages](0220-localization-pipeline-and-1.0-languages.md) | proposed | ADR 0220 |
| 0221 | [Colour-vision-safe map modes and UI scale](0221-colour-vision-safe-map-modes-and-ui-scale.md) | proposed | ADR 0221 |
| 0222 | [Disputed borders and neutral presentation](0222-disputed-borders-and-neutral-presentation.md) | proposed | ADR 0222 |
| 0223 | [Language repertoires and shift](0223-language-repertoires-and-shift.md) | proposed | ADR 0223 |
| 0224 | [Religious change: affiliation and religiosity](0224-religious-change-affiliation-and-religiosity.md) | proposed | ADR 0224 |
| 0225 | [Assimilation policy, backlash and group boundaries](0225-assimilation-policy-backlash-and-group-boundaries.md) | proposed | ADR 0225 |
| 0226 | [Atrocity risk, onset and consequences](0226-atrocity-risk-onset-and-consequences.md) | proposed | ADR 0226 |
| 0227 | [The full financial design in 1.0](0227-full-financial-design-in-1.0.md) | proposed | ADR 0227 |
| 0228 | [Gabaix-Maggiori exchange rates in P5a](0228-gabaix-maggiori-exchange-rates-in-p5a.md) | proposed | ADR 0228 |
| 0229 | [Crisis levers as state-triggered rules in 1.0](0229-crisis-levers-as-state-triggered-rules.md) | proposed | ADR 0229 |
| 0230 | [Latent demand dynamics in the demand system](0230-latent-demand-dynamics.md) | proposed | ADR 0230 |
| 0231 | [Cost of capital for investment](0231-cost-of-capital-for-investment.md) | proposed | ADR 0231 |
| 0232 | [Daily calendar over monthly ticks](0232-daily-calendar-over-monthly-ticks.md) | proposed | ADR 0232 |
| 0233 | [Margin-constrained leverage and LDI spirals](0233-margin-constrained-leverage-and-ldi-spirals.md) | proposed | ADR 0233 |
| 0234 | [Late-game flatness gate](0234-late-game-flatness-gate.md) | proposed | ADR 0234 |
| 0235 | [Budget lines from measured per-good cost](0235-budget-lines-from-measured-per-good-cost.md) | proposed | ADR 0235 |
| 0236 | [Bounded state tables](0236-bounded-state-tables.md) | proposed | ADR 0236 |
| 0237 | [Seat registry: office, firm and planned-economy seats](0237-seat-registry-office-firm-and-planned-economy-seats.md) | proposed | ADR 0237 |
| 0238 | [Data revisions and real-time vintages](0238-data-revisions-and-real-time-vintages.md) | proposed | ADR 0238 |
| 0239 | [No player seats in 1.x: game mechanics start at 2.0](0239-no-player-seats-in-1x.md) | accepted | ADR 0239 |
| 0240 | [Narration, chrome, flavour and replay hooks after 1.0](0240-presentation-game-layer-after-1.0.md) | accepted | ADR 0240 |
| 0241 | [Initial conditions only: calendar-blind runs and replay history](0241-initial-conditions-only-and-replay-history.md) | accepted | ADR 0241 |
| 0242 | [Endogenous hazards: pandemics, policy regimes and technology jumps](0242-endogenous-hazards-pandemics-regimes-technology.md) | accepted | ADR 0242 |
| 0243 | [Balanced-growth calibration](0243-balanced-growth-calibration.md) | accepted | ADR 0243 |
| 0244 | [Map borders from the id buffer, labels in Slint](0244-map-borders-from-ids-and-slint-labels.md) | accepted | ADR 0244 |
| 0245 | [Control labels, irregular shares and how advances relabel cells](0245-control-labels-irregular-shares-and-advance-relabel.md) | accepted | ADR 0245 |
| 0246 | [Rear security: computed demand and key-node orders](0246-rear-security-demand-and-key-node-orders.md) | accepted | ADR 0246 |
| 0247 | [Bulk freight rate per theta class](0247-bulk-freight-rate-per-theta-class.md) | accepted (RFC 0312) | ADR 0247 |
| 0248 | [Re-estimate of the 1.0 plan after ADRs 0208 to 0247](0248-1.0-re-estimate-after-0207.md) | accepted | ADR 0248 |
| 0249 | [Partial WDI sector shares and oil and gas from rents](0249-wdi-partial-shares-and-oil-gas-rents.md) | accepted | ADR 0249 |
| 0250 | [Globe-only map, Equal Earth for print](0250-globe-only-map-equal-earth-for-print.md) | accepted | ADR 0250 |
| 0251 | [Whole first-level provinces](0251-whole-first-level-provinces.md) | accepted | ADR 0251 |
| 0252 | [Map and notice cues borrowed from grand strategy games](0252-map-and-notice-cues.md) | accepted | ADR 0252 |
| 0253 | [Royal houses, claimants and emergent restorations](0253-royal-houses-claimants-and-restorations.md) | accepted | ADR 0253 |
| 0254 | [Organic conflict onset: crisis hazard, actors decide](0254-organic-conflict-onset.md) | accepted | ADR 0254 |
| 0255 | [Relief globe](0255-relief-globe.md) | accepted | ADR 0255 |
| 0256 | [World technology for crude oil and gas set by rents](0256-world-technology-for-oil-and-gas-from-rents.md) | accepted (RFC 0312) | ADR 0256 |
| 0257 | [Literature-fidelity tiers and a benchmark rerun at every phase gate](0257-literature-fidelity-tiers-and-phase-gate-benchmark.md) | accepted | ADR 0257 |
| 0258 | [Indicator registry, display transforms and the headline set](0258-indicator-registry-and-display-transforms.md) | accepted | ADR 0258 |
| 0259 | [Difficulty settings for seat play](0259-difficulty-settings-for-seat-play.md) | accepted | ADR 0259 |
| 0260 | [Frosted panels, theme from the OS, SDF map labels](0260-frosted-panels-os-theme-sdf-labels.md) | accepted | ADR 0260 |
| 0261 | [Glass loupe and glass edges on floating overlays](0261-glass-loupe-and-overlay-edges.md) | accepted | ADR 0261 |
| 0262 | [UI and UX polish touches](0262-ui-polish-touches.md) | accepted | ADR 0262 |
| 0263 | [Price gain capped at a share of each market's stability limit](0263-price-gain-capped-per-market.md) | accepted | ADR 0263 |
| 0264 | [H2 distance a reported diagnostic, theta per class gated with the model's kernel](0264-h2-distance-diagnostic-kernel-controlled-theta.md) | proposed | ADR 0264 |
| 0265 | [The ACR criterion with input-output linkages](0265-acr-criterion-with-input-output-linkages.md) | proposed | ADR 0265 |
| 0266 | [Labour capped at what inputs allow](0266-labour-capped-at-what-inputs-allow.md) | accepted (RFC 0312) | ADR 0266 |
| 0267 | [Price gain capped per coupled block of commodity markets](0267-price-gain-capped-per-coupled-block.md) | accepted (RFC 0312) | ADR 0267 |
| 0268 | [The ACR criterion graded per province](0268-acr-graded-per-province.md) | accepted | ADR 0268 |
| 0269 | [P04 mode cost ratios borrowed from freight rates](0269-p04-mode-cost-ratios-borrowed.md) | accepted | ADR 0269 |
| 0270 | [Extraction column for crude oil and gas set by rents](0270-extraction-column-for-rent-economies.md) | accepted | ADR 0270 |
| 0271 | [Goods at the ICIO industry level](0271-full-icio-goods-list.md) | proposed | ADR 0271 |
| 0272 | [Log-distance trade cost on the border crossing](0272-log-distance-crossing-friction.md) | accepted | ADR 0272 |
| 0273 | [Mining support services are a local good](0273-mining-support-services-local.md) | accepted | ADR 0273 |
| 0274 | [Up to seven theta classes](0274-up-to-seven-theta-classes.md) | accepted | ADR 0274 |
| 0275 | [Manufacturing province spread from plant and census seeds, fitted by RAS](0275-manufacturing-spread-by-seeds-and-ras.md) | accepted | ADR 0275 |
| 0276 | [Border friction per good in theta class 3](0276-border-friction-per-good-in-class-3.md) | accepted (RFC 0312) | ADR 0276 |
| 0277 | [Air freight rate from 2000 mode shares](0277-air-freight-rate-from-mode-shares.md) | accepted | ADR 0277 |
| 0278 | [Energy scope for 1.0: grid zones, plant capacity, state energy cells, frozen energy policy, gas markets](0278-energy-scope-for-1.0.md) | accepted | ADR 0278 |
| 0279 | [Leg paths minimise the full iceberg cost per good](0279-least-iceberg-paths-per-good.md) | accepted | ADR 0279 |
| 0280 | [Storage order by partial adjustment toward a target stock](0280-storage-order-by-partial-adjustment.md) | accepted (RFC 0312) | ADR 0280 |
| 0281 | [Rents-to-VA factor from extraction value added](0281-rents-to-value-added-from-extraction.md) | accepted (RFC 0312) | ADR 0281 |
| 0282 | [Price level source, market rates and donor table targets (amends ADR 0098)](0282-price-level-source-market-rates-and-donor-table-targets.md) | proposed | ADR 0282 to 0286, 0289, 0290, 0294 |
| 0283 | [Manufacturing census seeds: nearest year for Eurostat gaps, Statistics Finland for Finland](0283-manufacturing-seeds-nearest-year-and-finland.md) | accepted | ADR 0287 |
| 0284 | [Ensemble lanes move from pre-P3 into P3 (amends ADR 0207)](0284-ensemble-lanes-in-p3.md) | accepted | ADR 0288 |
| 0285 | [Interventions replay on seek, and parameters are snapshot state](0285-interventions-and-parameters-as-snapshot-state.md) | accepted | ADR 0291 to 0293 |
| 0286 | [Storage-on frictionless V1 misses: cause recorded, fix deferred to P5](0286-storage-on-frictionless-misses-deferred.md) | deferred | none |
| 0287 | [Tick speed programme: bit-identical speed-ups first, then narrow exceptions](0287-tick-speed-programme.md) | accepted | ADR 0294 to 0298 |
| 0301 | [Three final demanders in P2: who buys, inventory change, shares, balance, composition and the accelerator (accepts and amends ADR 0099)](0301-three-final-demanders-in-p2.md) | accepted | ADR 0301 to 0306 |
| 0302 | [Sector labour shares, wealth-weighted and foreign dividends: sources, bounds, ranks and routing (accepts and amends ADR 0104)](0302-sector-labour-shares-and-wealth-weighted-dividends.md) | accepted | ADR 0307 to 0310 |
| 0303 | [Wealth-bracket shares from the income Gini, WID as a check only (amends ADR 0308)](0303-open-wealth-shares-from-the-income-gini.md) | accepted | ADR 0311, 0312 |
| 0304 | [The largest income-effect weight the household shares allow (amends ADR 0094)](0304-largest-income-effect-weight-the-shares-allow.md) | accepted | ADR 0313 |
| 0305 | [New work defaults to 1.x, not 1.0 (amends ADR 0207)](0305-new-work-defaults-to-1x.md) | accepted | ADR 0314 |
| 0306 | [Transfers as the current account's share of national income (amends ADR 0100)](0306-transfers-as-a-share-of-national-income.md) | accepted | ADR 0315 |
| 0307 | [Invert PIGL on the shares the run spends, clamps included (amends ADR 0094)](0307-invert-pigl-on-the-clamped-shares.md) | accepted | ADR 0316 |
| 0308 | [Trade imbalances: balanced final demand, current-account shares and the observed path (amends ADR 0100)](0308-trade-imbalances-balanced-final-demand-and-the-current-account-path.md) | accepted | ADR 0318 to 0321, 0299, 0300 |
| 0309 | [Ensemble spread at P3 comes from register priors](0309-ensemble-spread-from-register-priors.md) | accepted | ADR 0322 |
| 0310 | [P3 ships the gate's workbench views; the rest of the lab UI follows the gate (amends ADR 0207)](0310-p3-ships-gate-views-lab-ui-after.md) | accepted | ADR 0323 |
| 0311 | [Accept the late-game flatness gate, per-good budget lines and bounded state tables (accepts ADRs 0234 to 0236)](0311-accept-flatness-budget-and-bounds-adrs.md) | accepted | ADR 0234, ADR 0235, ADR 0236 |
| 0312 | [Accept the built pre-P3 ADRs that still read proposed](0312-accept-built-pre-p3-adrs.md) | accepted | none new |
| 0313 | [Re-rate the 1.0 plan at the pre-P3 gate (amends ADR 0248)](0313-re-rate-the-1-0-plan-at-the-pre-p3-gate.md) | accepted | ADR 0324, ADR 0325 |
| 0314 | [Held-out sources register and the C7 guard](0314-held-out-sources-register-and-c7-guard.md) | proposed | none yet |
| 0315 | [Snapshot chunks are zstd pieces of the canonical bytes, not Arrow tables](0315-snapshot-chunks-are-zstd-canonical-bytes.md) | accepted | ADR 0326 |
| 0316 | [T2b posts netted legs; posting stops copying and zeroing per-chunk vectors](0316-net-the-t2b-ledger-journal.md) | accepted | ADR 0331 |
| 0317 | [Canonical summation order, province slices of the ledger and goods-innermost dispatch blocks](0317-summation-order-and-goods-layout.md) | accepted | ADR 0327, ADR 0328, ADR 0329, ADR 0330 |
| 0318 | [Accept ADR 0212 and the built parts of ADRs 0211 and 0128 as built](0318-accept-history-retention-and-run-files-as-built.md) | accepted | none new |
| 0319 | [Good-major pipeline slots and parallel tick checks for the 45-goods budget](0319-good-major-pipeline-and-parallel-tick-checks.md) | proposed | none yet |
| 0332 | [Status of ADR 0108 (estimation windows and long-run lanes)](0332-status-of-adr-0108-estimation-windows.md) | accepted | none new |
