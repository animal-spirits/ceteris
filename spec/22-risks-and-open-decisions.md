## 22. Risks and Open Decisions

The largest risk is not technical but epistemic: a model this rich can reproduce anything, so the mitigations centre on keeping free parameters few, nesting known equilibria, and scoring out of sample.

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Complexity explosion; modules interact in untestable ways | High | Two-tier modules, specs, invariants, channel-shutdown tests, phase gates |
| Identification: many parameter sets fit 2000-2010 equally well | High | Three-layer parameterization, literature priors, modular calibration, posterior reporting, natural-experiment tests |
| Province-level data gaps, especially for the \~115 economies outside OECD ICIO | High | Documented imputation, reconciliation to national totals, imputation flags down-weighted in calibration |
| Sensitive real-world content: real states, ethnic groups and ongoing conflicts in an open-source simulator | High | Neutral mechanistic modeling, generic party names, documented sources, outputs framed as distributions not predictions, no editorializing UI text |
| Trade solver misses the laptop budget at province level | Medium | One gravity normalization per tick (ADR 0016), gateway-factorized kernel, tunable gateway counts, Metal fallback |
| Open-ended runs drift or explode (debt spirals, price drift, demographic collapse) | Medium | Stabilizing policy rules, stability tests to 2150, balanced-growth checks in CI |
| Numerical instability (price oscillation, bank-run cascades) | Medium | Clipped adjustments, stability tests on adjustment speeds, Walrasian fallback switch |
| Railroading toward known post-2000 history | Medium | Calendar-blind default runs: events come from hazards on simulated state; history enters only as optional replay (ADRs 0241, 0242); LLM tier deferred |
| Dataset licences restrict redistribution | Medium | Bundle-or-fetch policy, licence manifest, P0 audit |
| Determinism broken by parallelism or library updates | Medium | Fixed reductions, pinned toolchains, hash checks in CI |
| Scope creep toward a full game | Medium | Non-goals list (2.4); features must serve experiments |

### 22.1 Decisions (resolved 23 Oct 2026)

| Decision | Choice | Consequence |
| --- | --- | --- |
| Scenario | Modern start, 1 Jan 2000 (supersedes 1836) | Data replaces judgment; 2011-2024 is the out-of-sample window |
| Start-year flexibility | Pipeline builds packs for any year 1995-2022 | Rolling-origin validation (2000, 2005, 2010, 2015) |
| Horizon | Open-ended | Demography and long-run stability are first-class requirements; climate joins them in 1.1 (ADR 0207) |
| Map scope | Global, full globe from P2 (ADR 0012) | dev\_slice kept in the pack for ad hoc fast runs; no gate depends on it |
| Provinces | Natural Earth first-level regions at 2000 borders, kept whole, 4,594 (ADR 0251) | Public domain; geoBoundaries counties add a display layer |
| Markets | Every province prices every good | Gravity kernel factorized through gateways (8.3) |
| UI priority | Observer / laboratory | Branches, diff maps, ensembles before Steward controls |
| App | Fully native Rust app, Slint panels over a wgpu map (ADR 0088) | Rust core in-process, no server |
| Core language | Rust from the start | Pipeline, reference model and calibration in Rust too (ADR 0113); no Python in the build, release or test path |
| Hardware | MacBook Pro, M-series Pro, 18-36 GB, desktop only | Budgets in 15.2 and 19.2; emulators and modular calibration mandatory |
| Pops | Province x ethnic group x education x labour status | About 100,000 pops; religion as shares |
| Climate | Simple emulator with damages, from 1.1 (ADR 0207) | Endogenous feedback for long runs; 1.0 runs with climate off |
| Technology | Aggregate sector productivity, no named technologies | Frontier growth, catch-up, energy learning curves |
| Parties | Explicit parties over interest groups | Elections, coalitions, party entry |
| Naming | Real states and provinces, generic party labels by family | Recognizable map, less curation and sensitivity |
| IO and trade data | OECD ICIO 2025 edition (1995-2022, 80+ economies) + BACI | About 115 economies imputed |
| Licence | Open source and non-commercial (owner, 28 Oct 2026; supersedes the closed-source fallback of ADR 0005) | AGPL-3.0-or-later code (ADR 0346, 11 Nov 2026); each user downloads the data, so non-commercial sources are acceptable; nothing restricted is bundled |
| LLM leaders | Out of v1 | Tier 2 deferred |
| First release | 1.0 at the end of R, after P9 (calibrated, validated economic world; P6 and P7 are 2.0, ADR 0136; phase plan ADR 0207) | No public partial builds |
| Name | Ceteris Paribus; code name ceteris (supersedes WorldSim, which is crowded) | Crates ceteris-\*, CLI ceteris; Python package name dormant until bindings exist (ADRs 0003, 0113); check crates.io, PyPI and GitHub before P0 |

### 22.2 Defaults the coding agent may take (recorded as ADRs)

- Exact sector splits beyond the 45 ICIO industries.
- The province-splitting rule (population and area thresholds); resolved as no split (ADR 0251).
- The imputation model for input-output coefficients of countries outside OECD ICIO.
- Number of trade gateways and partner-set size, set by benchmark results.
- Generic naming scheme for parties.

