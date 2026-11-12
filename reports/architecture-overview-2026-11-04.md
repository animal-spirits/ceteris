# Ceteris Paribus: architecture overview, 4 November 2026

As built on `origin/main` plus `origin/pre-p3/audit`. Module specs: `specs/{core,geo,trade,econ,app}/SPEC.md`;
design: `spec/04` (architecture and loop), `spec/08` (markets and trade), `spec/15` (calibration),
`spec/19` (stack). Section 7 lists where the build departs from the 23 Oct design.

## 1. Shape of the system

```
sources (fetch.toml, checksums, licences)          owner-side, once per pack
   -> ceteris-pack build  -> pack/2000/*.arrow + web/ (fills, tiles, json) + manifest
   -> ceteris calibrate   -> calibration tables installed into the pack (steady state)
   -> ceteris run | app   -> Sim<EconWorld>: deterministic monthly ticks, snapshots, history
   -> ceteris export      -> Arrow directory read by ceteris-reference (independent GE check)
```

Paradigm: a deterministic, data-oriented simulation (ECS-like structure-of-arrays, systems run
in a fixed order over flat arrays) whose economics are disciplined by quantitative trade theory:
the agent-based dynamics must converge to an independently coded general-equilibrium reference
when frictions are off (equilibrium nesting). Everything is f64 and bit-reproducible across
thread counts; runs are forkable because the whole dynamic state is a canonical byte string.

## 2. Crates

| Crate | Lines | Role |
| --- | --- | --- |
| `ceteris-core` | 2.4k | Entity ids, scheduler, Philox RNG, double-entry ledger, event log, fixed-order reductions, snapshots, `Sim` |
| `ceteris-geo` | 2.0k | Multimodal transport graph (CSR), least-cost paths with a cache, route choice (built, off) |
| `ceteris-trade` | 6.3k | Goods, trade parameters, network, two-level kernel, destination sourcing, origin markets, pipeline |
| `ceteris-econ` | 20.4k | `EconWorld`: enterprises, pops, labour, production, local chain, PIGL demand, distribution, transfers, drift, history, export |
| `ceteris-calibrate` | 14.6k | Steady-state calibration (5 stages), stability tables, block gain caps, gravity/border fits, benchmark harness and shocks |
| `ceteris-reference` | 7.3k | Independent GE model from the spec: Newton-Krylov solve, Sinkhorn, PPML/HDFE, uniqueness test |
| `ceteris-pack` | 25.1k | Fetch, licence check, geo/raster/web builders, econ table builders (ICIO, BACI, WDI, PWT, SEA, AAMNE, WID readers; RAS; downscaling) |
| `ceteris-cli` | 2.4k | `ceteris` binary: pack, calibrate, run, export, reference, gate tests |
| `ceteris-geos-coverage` | 0.1k | FFI to GEOS coverage simplification (owner-side feature) |
| `app/` (own workspace) | 4.5k | Slint 1.18.1 panels over a wgpu 30 map |

## 3. Core abstractions (`ceteris-core`)

- **`Sim<W: State>`**: owns the world, a `Schedule<W>`, a `Seed`, a `SnapshotStore` and an
  intervention log. `step`, `run_until`, `seek(tick)`; `intervene(closure)` records a
  between-tick edit that `seek` replays, so a fork is "restore snapshot, replay edits"
  (ADRs 0291-0293). An edit after a seek back drops later snapshots and edits.
- **`Schedule` / `SystemDef` / `Cadence`**: systems with stable `SystemId`s run in a fixed order;
  cadence Monthly, Quarterly or Annual{month}. A tick is one month.
- **RNG**: Philox4x32-10 counter-based; `Seed::stream(tick, system, kind, entity)` gives an
  independent stream per draw site, so results do not depend on execution order or threads.
  Common random numbers across branches fall out of this. (P2 draws no random numbers; P3's
  ensembles need member parameter draws for spread.)
- **`Ledger`**: double-entry money in integer minor units (i64, with i128 "wide" accounts for
  aggregates, ADR 0106) plus f64 goods holdings; every transfer carries a `Flow` tag; a journal
  of `Entry`s; `check` asserts money sums to zero and goods are conserved each tick.
- **`EventLog`**: append-only discrete decisions, inside the snapshot.
- **`reduce::fixed_sum`** and friends: reductions whose order depends only on input length;
  clippy bans on hash maps and order-dependent reductions (`clippy.toml`, ADR 0097).
- **Snapshots**: `Encode`/`Decode` traits write a canonical byte encoding; the state hash is
  over those bytes; snapshots every 12 ticks, content-addressed. Static inputs (pack, kernel)
  stay outside the snapshot and are named by digest; parameters are state (format 3).
- **`Violation` / invariants**: accounting assertions run as the last system of every tick.

## 4. World and geography (`ceteris-geo`, `ceteris-pack`)

- Provinces are whole first-level regions at 2000 borders (ADR 0251; 4,368 provinces with value
  added on the B8 pack), counties as a display layer (about 53k), H3 cells for rasters.
- Transport graph: multimodal (road, rail, sea, inland water, air), priced per tonne from mode
  parameters P04 (fit never converged, borrowed ratios per ADR 0269), stored CSR. Least-cost
  paths with a cache recomputed only when edge costs change. Least-iceberg leg paths per good
  (ADR 0279); air share estimated from 2000 mode shares (0277).
- Route choice (Allen-Arkolakis 2022, `z = (I - A)^-1` without forming the inverse, log-domain)
  is built and tested but off (ADR 0017).
- Pack: Arrow tables with recorded digests, `manifest.json`, map fills and PMTiles; the pack is
  built owner-side (GEOS 3.15, tippecanoe), and each user fetches sources themselves (licences
  in `sources.toml`, research and release profiles, ADR 0050).
- Econ pack tables: ICIO 2000 at country level downscaled to provinces (population, gridded
  GDP, plant seeds for manufacturing with RAS to DOSE rows and ICIO columns, ADR 0275); BACI
  trade by good; tariffs; PWT/WDI price levels with a Penn-effect screen (0282); SEA labour
  shares; AAMNE foreign ownership; wealth brackets from the income Gini (0311); drift terms from
  PWT 1995-2000 (0103).

## 5. Economic core: algorithms

### 5.1 Tick order (system ids 201-210, monthly)

T1 deliver, E1 labour, E2 produce (with the same-tick local chain), E3 demand, T2 source,
T3 clear, T2b dispatch, T4 reprice, E4 distribute (profits, dividends, transfers, write-off),
E5 assert. Drift factors scale productivity per tick when on (C5).

### 5.2 Trade: structural gravity on a two-level kernel

- Goods: 10 in P2 (agriculture, fuels, mining, food, light mfg, chemicals/metals, machinery
  traded; utilities, construction, services local). The goods count is now a pack value (G2);
  the 50-goods ICIO-level list (0271-0275) is P3 work.
- **Kernel** (ADRs 0015, 0092): domestic pairs `K_od = tau_od^-theta_g` dense within a country;
  cross-border pairs go through gateways, `K_od = sum_ab K_oa m_a K_ab m_b K_bd` with gateway
  masses m, never materialized: origin access is evaluated through gateway aggregates
  (low-rank evaluation, about 623k nonzeros per good). Log-domain weights and stored
  reciprocals keep it finite across 1e12 price ratios (ADR 0093). Border friction is a
  log-distance term on the crossing (0272), per good in class 3 (0276).
- **Destination sourcing** (Eaton-Kortum, ADRs 0015, 0016, 0200): destination d spends E_d over
  origins as `X_od = E_d K_od p_o^-theta / Phi_d`; delivered index `P_d = Phi_d^(-1/theta)`;
  buyers pay `q_d = Gamma_d P_d` with a selection constant per cell fixed at calibration, so
  welfare obeys the ACR formula. Orders clear at origin markets across ticks; shipments sit in a
  4-slot per-buyer **pipeline** (transit lags 1-4 months). Gather-parallel dispatch per target,
  proven bit-identical to the scatter.
- Theta: per good from its class; up to 7 classes (0274), harmonic class aggregation (0101).
- Costs: iceberg only, `tau = 1 + c w_g + lambda_g T` (time cost of transit).

### 5.3 Markets and prices

- Origin markets with rationing between buyer classes (Pichler et al. style, S11 switch),
  perishing of local goods, competitive storage of storables at origin by partial adjustment
  toward a target stock (ADRs 0091, 0280).
- **Stock-flow price rule**: `dlnp = pi^e + clip(gap terms, +-c_max)`, kappa1 0.05, inventory
  term off (kappa2 0), wider clip 0.3 for storables (0106). Expected inflation from the world
  price index. Gains are capped per market at a share of its stability limit (0263) and per
  coupled country block (0267), from Jury/Schur-Cohn tests on linearized transition matrices
  (`ceteris-calibrate::stability`, `block`).
- Wage rule: `dlnw = pi^e + clip(kappa_w (x + u*))` with downward rigidity, natural rate u*
  (0089); employment pro rata; a Walrasian clearing wage in frictionless mode.

### 5.4 Production and demand

- Enterprises, one per (province, sector), price-taking; employment moves toward the
  profit-maximizing level, capped by what inputs allow (0266). Value added: normalized CES of
  one fixed factor and effective labour. Intermediates Leontief with ICIO 2000 cost shares at
  base delivered prices (one putty-clay vintage). Inputs ordered up to (1 + cover + L) months.
- **Local chain** (ADRs 0020, 0093): utilities, construction and services produced, used and
  rationed in one same-tick pass per province; exact fill with bounded passes.
- **PIGL demand** (Alder-Boppart-Mueller 2022) over 3 categories with CES within each; a pop's
  lognormal income integrated in closed form; aggregation across pops inverts weights per
  country by a 1-D root (0094).
- Final demand: households, government and investment (0099, 0301-0306); trade imbalances as
  transfers through a world account (0100, 0295-0300); sector labour shares, wealth-weighted
  domestic and foreign dividends through a world dividend account (0307-0310); wealth
  brackets (0311). Money: a single numeraire with interest-free clearing (no banks before P5);
  loss write-off beyond 3 months of base sales (0112).

### 5.5 Calibration (`ceteris-calibrate::steady`, ADR 0089)

The 2000 data state is made the steady state: values fixed, prices solved.
Stage 1 output values from VA and ICIO; stage 2 origin prices by a balanced Sinkhorn scaling per
traded good (pinned from both sides); stage 3 technology as 2000 cost shares at solved prices;
stage 4 PIGL weights inverted per country; stage 5 the start-of-tick state (stocks, pipeline,
merchant stocks at coverage times own output), checked stationary under the stock rules to
1e-12 (passes settled to 1e-13). Balanced-growth calibration with country trends replaces it in
P4 (0243).

### 5.6 Reference model and gates

- `ceteris-reference` reads only the `ceteris export` Arrow directory, never the simulator's
  structs (independent implementation from the spec). Newton-Krylov (restarted GMRES on
  finite-difference Jacobian products), Anderson-accelerated tatonnement fallback.
- Uniqueness check before V1: Allen-Arkolakis-Li (2024) spectral-radius test; V1 excludes
  cells outside the one-market J2 region and reports them.
- V0/V1: frictionless runs converge to the reference within 1%; H2 gravity elasticities by
  theta class by PPML with HDFE; benchmark harness (Japan, Brexit, border shocks) measures
  welfare, ACR ratio and openness against ICIO.
- Goldens: per-crate `tests/golden.rs` against `tests/golden/*.txt` hashes; re-blessed in own
  commits only.

## 6. App (`app/`)

Slint panels over a wgpu map drawn into a texture on Slint's device (ADR 0088). Map pass with a
second R/G uint target holding province and county ids for picking and ID-buffer borders
(0244); orthographic globe only (0250); 2-degree chunks for culling, county LOD, SDF labels.
Sim thread steps the world and publishes one `econ::View` per tick into a bounded run-ahead
ring (0208); the frame loop drains it without blocking. Map modes: political, terrain,
resources, freight cost, price, output, net exports, wage, unemployment. Command registry and
clock (1, 7, 30.4, 91.3 days/s).

## 7. Patterns and engineering rules

- Spec before code; every deviation is an RFC; decision ids allocated by `ceteris-rows`; CI
  `rows check` on every PR.
- Determinism as a tested contract: hashes equal across runs, 1/3/all threads, and
  restore-and-replay; proptests for bit-identity when changing parallel structure.
- Structure of arrays indexed `[province * n_goods + g]`; static data shared via `Arc`, outside
  snapshots, named by digest.
- Switches with an "Off" setting for every mechanism (ablation rule: on only once it improves a
  test); switches S01-S23 in the register.
- Register of parameters (P01-P162) with priors; register of testable hypotheses (spec 18).
- Two-implementation discipline (simulator vs reference) and literature-fidelity tiers with a
  benchmark rerun at every phase gate (0257).
- `unsafe_code = "forbid"`, toolchain pinned 1.93.0, `cargo deny` licences.

## 8. Changes from the design architecture

| Design (23 Oct) | As built | Decision |
| --- | --- | --- |
| Python model and tooling beside Rust core | All Rust | 0113 |
| Web map, later Tauri hybrid | Native Slint + wgpu | 0011, 0088 |
| European development slice | Full globe from P2 | 0012 |
| Unbalanced Sinkhorn per tick over both marginals | Destination sourcing in value, cleared across ticks | 0015, 0016 (0013 superseded) |
| Posted prices for buyers | CES index times selection constant (Eaton-Kortum selection) | 0200 |
| Route-choice costs | Least-cost legs; route choice built but off | 0017 |
| Per-unit plus iceberg costs | Iceberg only with time cost | trade SPEC |
| Kernels per (theta class, weight class), 3 classes | Per good, up to 7 classes, harmonic aggregation | 0018, 0023, 0101, 0274 |
| PIGL over 5 categories | 3 categories, CES within | 0021 |
| CES energy nest | Leontief until P8 | econ SPEC |
| Firms forecast demand and hold coverage | Price-taking firms; coverage at markets and input orders | econ SPEC |
| Price rule kappa1 0.2, kappa2 0.1 | kappa1 0.05, kappa2 0, gain caps per market and block | 0019, 0263, 0267 |
| Explicit fiat money via banks | Numeraire with clearing facility until P5 | econ SPEC |
| One final demander | Households, government, investment | 0099, 0301-0306 |
| Full employment | Natural rate u* in the wage rule | 0089 |
| Calibrated with estimated productivity | Data state is the steady state; A calibrated | 0089 |
| Ensembles via lanes in pre-P3, `Real` generic | Lanes in P3; `Real`/duals in P9 | 0288, 0096 |
| Climate first-class in 1.0 | Climate in 1.1 | 0207 |
| Calendar events and historical paths as inputs | Only 1 Jan 2000 data; endogenous hazards | 0241, 0242 |

Counts of deviations: 26 approved module deviations in the econ (15) and trade (11) SPECs;
105 RFCs amend, supersede or replace 75 earlier ADRs, 35 of them aimed at 22 design-era ADRs
(details in the progress report, Section 6).

## 9. Known architectural weak points

- Price rule is gap-only at low gain; the split rule with a traded-market anchor (0090) and the
  storage-on frictionless fix (RFC 0286) wait for P5's money anchor.
- Dispatch dominates the tick (about 19-24 ms of 41); 7 traded goods cap per-good
  parallelism; serial tail about 7.9 ms (journal copy, goods moves).
- The gravity fit (distance elasticity, P04 costs) rests on borrowed priors and reported
  diagnostics rather than gates.
- The steady-state calibration has to be redone in P4 as balanced growth (0243), and every pack
  change forces recalibrate plus golden re-bless, which keeps main CI red between rebuilds.
