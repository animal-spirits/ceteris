## 26. Gap analysis, 24 Oct 2026 (ADRs 0027-0054)

A review of the design against current literature and the most complete existing models (data-driven macro ABMs such as Poledna et al. 2023 and Wiese et al. 2024, multi-country policy models, integrated assessment models, dynamic spatial models, WCDE demographic projections, ViEWS, and the strategy games the project draws on) produced 28 proposed ADRs. Their full text is in the repository's adr folder, in pull requests 3 to 7; this section indexes them. Sections of this document change when each ADR is accepted.

**Corrections to existing decisions.** The single climate damage default with a level-versus-growth switch (13.2, S10) is replaced by three damage lanes (0036): Kotz, Levermann and Wenz (2024) was retracted in December 2025, and the remaining estimates differ by up to an order of magnitude. V-Dem v16 (March 2026) supersedes v15. One pack cannot serve both research validation and release, because the best validation data are often non-commercial, so the pipeline gets two profiles (0050). Wright's law alone sets costs but not adoption speed (0035). A single long-run trade elasticity cannot reproduce short-run tariff and sanctions responses (0027).

| ADR | Decision | Phase | PR |
| --- | --- | --- | --- |
| 0027 | Short- and long-run trade elasticities (sourcing inertia) | P2 | 3 |
| 0028 | Oil and gas market structure (OPEC rule, storage, LNG) | P2, P8 | 3 |
| 0029 | Food: competitive storage, export restrictions, famine | P2, P4 | 3 |
| 0030 | Variable markups within cells (ablation-gated) | P2 | 3 |
| 0031 | Structural transformation and informality | P2, P4 | 3 |
| 0032 | Cohort demography: education transitions, fertility tempo, households | P4 | 3 |
| 0033 | Mortality by cause group | P4, P8 | 3 |
| 0034 | Power sector via residual load duration curves | P2 stub, P8 | 4 |
| 0035 | Technology diffusion (FTT) and energy policy instruments | P8 | 4 |
| 0036 | Climate damage lanes | P8 | 4 |
| 0037 | MESMER regional climate, tipping elements, sea level | P8 | 4 |
| 0038 | Water, air pollution and land use | P8 | 4 |
| 0039 | Automation and AI as a scenario lane | P8 | 4 |
| 0040 | Housing supply elasticity and mortgage underwriting | P5 | 5 |
| 0041 | Multinational production and profit shifting | P5 | 5 |
| 0042 | Public capital and fiscal dynamics | P5 | 5 |
| 0043 | Sovereign debt restructuring and creditors | P5 | 5 |
| 0044 | Pensions, life-cycle saving and wealth dynamics | P5 | 5 |
| 0045 | Economic statecraft: critical minerals, export controls, sanctions evasion | P5, P6 | 5 |
| 0046 | Political economy channels | P6 | 6 |
| 0047 | Trade agreements and international organizations | P6 | 6 |
| 0048 | Grounding the war module in modern evidence | P7 | 6 |
| 0049 | Explainability panels and event chronicle | P3 onward | 6 |
| 0050 | Research and release pack profiles; sources to audit | Pipeline, now | 7 |
| 0051 | Fat-tailed hazard library | P7 to P9 | 7 |
| 0052 | Forecast scorecard: vintages, hard benchmarks, proper tests | P3, P9 | 7 |
| 0053 | Robust inference and model selection | P9 | 7 |
| 0054 | Explicit exclusions | All | 7 |

**Register and switches.** These ADRs take register IDs P53 to P92 and switches S12 to S18; P44 to P47 went to P2 (17.1), P48 and P49 stay free, and P50 to P52 belong to 0026. Most new entries are data or literature priors. Free behavioural parameters still count against the cap of 60: a mechanism that fails the ablation rule (15.7) moves to the extended profile (23.1) instead of growing the lab register.

**Highest leverage, in order.** Cohort education demography (0032); the power sector and oil market (0034, 0028); technology diffusion (0035); climate damage lanes (0036); sourcing inertia (0027); the scorecard with vintages (0052); pack profiles (0050); the food system (0029); sovereign restructuring (0043); housing underwriting (0040); trade agreements (0047); political economy channels (0046); the hazard library (0051); and robust inference (0053).

