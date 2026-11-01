## 1. Executive Summary

Ceteris Paribus (code name `ceteris`) is a deterministic, forkable, agent-based world simulator that starts on 1 January 2000 and runs open-ended. Its economic core is a province-level production-and-trade model disciplined by quantitative trade theory, presented through a Paradox-style map in a native desktop app. Any state variable or parameter can be edited before a run (ex ante) or at any tick during it (ex post). It is open source and non-commercial, and each user downloads the source data.

Six design bets carry the project:

1. **Hybrid microstructure.** Population groups (pops) and enterprises are agents trading in out-of-equilibrium markets with inventories and lags. A structural dynamic spatial general-equilibrium model (Caliendo-Dvorkin-Parro / Kleinman-Liu-Redding class) runs alongside as a reference model: in the frictionless limit the agent model must reproduce it.
2. **Every province is a market.** Each of about 4,000 provinces prices every good. Shipments between provinces follow structural gravity at posted prices, and origin markets clear across ticks through inventories, so the price dynamics are entropic optimal transport spread over time and literature trade elasticities map one-to-one onto model parameters.
3. **Institutions are a continuous state vector**, initialized from V-Dem, with explicit political parties built from interest-group coalitions. Labels such as 'electoral autocracy' are derived views, never mechanics.
4. **Every run is reproducible and forkable.** Counter-based random numbers keyed by entity and tick, periodic snapshots and an event log let any branch be replayed exactly, and let interventions be compared against their parent under common random numbers. Ensembles report distributions, never single trajectories.
5. **Data-driven initial conditions, few free parameters.** Following the data-driven ABM program of Poledna et al. (2023) and Wiese et al. (2024), the world is initialized from 2000 data (OECD ICIO, BACI, gridded population, V-Dem). Behavioural parameters are estimated on 2000-2010 and tested out of sample on 2011-2024, a window containing the 2008 crisis, the euro crisis, COVID and two tariff wars.
6. **Accounting is law.** Stock-flow consistency, goods conservation and population conservation are asserted every tick; a violation halts the run.

Headline specification:

| Dimension | Default | Rationale |
| --- | --- | --- |
| Start / horizon | 1 Jan 2000, open-ended | 25 years of out-of-sample history; long runs need climate, demography and stable growth |
| Economic tick | 1 month | Captures harvests, prices, credit and policy cycles |
| Military sub-tick | 1 day | Movement, battles, sieges |
| Provinces | 4,594 first-level regions from Natural Earth at 2000 borders, kept whole (ADR 0251) | Modern admin units carry the subnational data |
| Markets | Every province prices every good | Finest price geography; costs about 10x more trade-solver work than hubs |
| Goods | 45 sectors aligned with OECD ICIO, with energy and food staples split out | Direct mapping to input-output data |
| States | About 195 | Current sovereign states plus a handful of de facto entities |
| Pops | About 100,000: province x ethnic group x education x labour status | Matches modern data (GeoEPR, census education, labour surveys) |
| Hardware | MacBook Pro, M-series Pro, 18-36 GB | Desktop only; sets all budgets |
| Performance target | 0.5 to 1 simulated year per second, single run | Interactive play; ensembles and calibration overnight |

Stack: a Rust simulation core embedded in a desktop app. The app is fully native Rust, Slint panels over a wgpu globe, with no React, webview or Tauri (ADR 0088). The data pipeline, the reference model and calibration are Rust crates driven by the `ceteris` CLI (ADR 0113). The project is open source (Apache-2.0 / MIT) and non-commercial; each user's pipeline downloads the source data, so non-commercial sources are acceptable inputs and restricted data is never bundled (owner, 28 Oct 2026). Build order puts the map, accounting, markets, trade and the counterfactual workbench first, because those define the product (Section 21).

