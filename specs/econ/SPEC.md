# Econ SPEC

Status: approved (owner, 2026-10-23)
Phase: P2
Design sections: 3.2, 3.3, 3.4, 4.2, 4.4, 4.6, 6.1, 7.1, 7.2, 7.3, 8.6, 8.7, 9.1, 15.3 (V0, V1), 16, 17, 18 (H1, H2), 19.2, 19.4, 20, 21.1 (P2); ADRs 0012 to 0016, 0018, 0019 and 0023

## Purpose

`ceteris-econ` owns the minimal economic core of P2: static pops, one enterprise per
(province, sector), the province labour market, final demand, income distribution and a single
numeraire money. It assembles the P2 world (`EconWorld`) and its tick schedule from
`ceteris-core`, `ceteris-geo` and `ceteris-trade` (`specs/trade/SPEC.md`). This spec also
covers the P2 additions to the pack pipeline (2000 economic data, gateways), the GE reference
model in `python/ceteris/reference`, the P2 gate, and what the app shows. Demography,
investment, finance, the state and politics are later phases.

## Deviations from DESIGN.md (approved with the spec)

| Design | P2 | Why |
|---|---|---|
| Enterprises set output from demand forecasts and inventory coverage (7.3) | Price-taking enterprises move employment toward the profit-maximizing level at expected (current) prices; inventory coverage lives with the province market (trade SPEC) | Province markets absorb all output at the posted price, so firms face prices, not demand curves; this rule nests the GE optimum exactly. The forecast rule can return as a switch when firms get pricing power |
| Non-homothetic CES demand over categories (8.6, A4) | Homothetic CES over the 10 goods | Non-homotheticity matters only with income growth, which P2 does not have; it would be a mechanism without a test (21.2, ablation rule) |
| Intermediates Leontief plus a CES energy nest (7.2) | All intermediates Leontief | The energy nest comes with energy and emissions (P8) |
| VA over K, effective L by education, land or resources T (7.2) | Normalized CES over one fixed factor F (capital, land and resources together) and effective labour | No investment in P2, so K and T never move separately; one fixed factor keeps the reference model small |
| Pops keyed by province, ethnic group, education, labour status (6.1) | Province x education (3 levels); labour status as employed and unemployed counts inside each pop | Ethnicity and status transitions are P4 |
| Wage posting and matching function (8.7) | Wage moves with excess labour demand, with downward rigidity; employment rationed pro rata | Matching and occupational mobility are P4 |
| Explicit fiat money through banks (A9) | Numeraire-only mode (A9 alternative); every account may run a negative balance at an interest-free clearing facility | No banks before P5 |
| Final demand split into households, government, investment | One final demander per pop, with country shares from all ICIO final demand | No state or investment in P2 |
| Observed 2000 trade imbalances | Balanced trade: pops spend their income each month | Deficits need finance (P5) |
| Agglomeration in productivity (A5, P09) | Off by default | Ablation rule: on only once it improves a test |
| Buyers face posted prices (8.6, 7.3) | Traded goods are bought at the delivered unit price q_d = 1 / alpha_d of the buyer's province (trade SPEC, Sourcing), in household demand, input orders and the value-added price (owner decision, 2026-10-24) | Destination sourcing gives no single posted price at a destination; at the CES index P_d an order of a unit delivers fewer than one |
| Inventory coverage at the market (7.3) | Enterprises order inputs up to an inventory position (stock plus goods on order) of (1 + cover + L) months of capacity use, cover = 1, L the expected months in transit (owner decision, 2026-10-24) | Goods paid at dispatch and delivered after transit must be counted, and the target must hold them: a two-month position cannot hold a month in transit |

## Goods (energy-forward, owner choice)

Aggregated from the ICIO 2025 edition industries. If the edition splits coal (B05) from oil and
gas (B06), use the split; otherwise split B05_06 by EIA 2000 production values at Pink Sheet
prices, per country.

| ID | Good | ICIO industries | Traded between provinces |
|---|---|---|---|
| 0 | Agriculture | A01_02, A03 | yes |
| 1 | Crude oil and gas | B06 (or share of B05_06), B09 | yes |
| 2 | Coal and other mining | B05 (or share of B05_06), B07_08 | yes |
| 3 | Food | C10T12 | yes |
| 4 | Light manufacturing | C13T15, C16, C17_18, C31T33 | yes |
| 5 | Chemicals, refined fuels and metals | C19, C20, C21, C22, C23, C24 (all splits), C25 | yes |
| 6 | Machinery, electronics and vehicles | C26, C27, C28, C29, C30 (all splits) | yes |
| 7 | Electricity and utilities | D, E | no (P2) |
| 8 | Construction | F | no (P2) |
| 9 | Services | G to T | no (P2) |

## Pack additions (pack format 3)

Built by the pipeline after the P1 tables; same rules as P1 (pinned inputs in `fetch.toml`,
licences in `sources.toml`, byte-identical rebuilds, `tests/golden/p2-pack.txt`).

| Table | Rows | Content | Sources |
|---|---|---|---|
| `econ_goods.arrow` | 10 | name, traded flag, weight per value w_g (t per unit), inventory coverage prior | BACI HS92 2000 (tonnes and values, HS to ISIC concordance) |
| `econ_countries.arrow` | 242 | ICIO economy or imputation donor, final demand shares (10), labour share, unemployment rate, participation rate, education shares (3), GDP 2000 | ICIO, PWT 11 (labsh), ILOSTAT, WIC reconstruction (Barro-Lee cross-check), WDI |
| `econ_io.arrow` | countries x 10 x 10 | input coefficient a_kg (units of k per unit of g) | ICIO 2000, aggregated |
| `econ_provinces.arrow` | 4,796 | population 2000, working-age population, VA by good (10) | WorldPop 2000 aggregated through the H3 cells; VA downscaled as below |
| `econ_tariffs.arrow` | importer x exporter x 2 groups | applied tariff, primary and manufactured | WDI weighted applied rates by importer; zero inside agreements and customs unions (CEPII gravity, WB DTA) |
| `gateways.arrow` | about 600 | node, country, kind (port, airport, land crossing) | P1 graph |

**Downscaling VA to provinces (traded goods only).** National VA by good from ICIO, then
province shares from the best proxy available: agriculture by MapSPAM 2000 value of production;
oil and gas by GEM extraction assets; coal and other mining by GEM coal mines and P1 mineral
sites; manufacturing goods by DOSE manufacturing GVA where a DOSE region matches a province by
ISO 3166-2 code or exact name within its country (never through GADM geometry), else by Kummu
gridded GDP. Each fallback falls back to population. Local goods get a base point only:
national VA split by population (their equilibrium output follows local demand). Provinces with
fewer than 1,000 working-age people get no VA; theirs is reallocated within the country by
the same proxy, so every enterprise has workers to hire. Every row carries an imputation flag.

**Economies outside ICIO.** IO coefficients and final demand shares from the ICIO economy
nearest in (region, log GDP per capita); national VA by good from WDI sector shares
(agriculture, industry, manufacturing, services) split within each group by the donor's
proportions. Agent default under 22.2, ADR 0014.

**Gateways.** Per country: listed ports and major airports clustered by k-medoids on path cost,
plus every land crossing (graph edge whose endpoints lie in different 2000 countries, merged
within 50 km). Count per country proportional to the square root of 2000 trade, at least one,
about 600 in total. Agent default under 22.2, set by benchmark.

**Calibration table** `econ_calib.arrow` (not built by the pack pipeline; written by
`ceteris calibrate p2`, below): productivity A per enterprise, base factor quantities, base
employment.

## State components

| Component | Entity | Type | Units | Initialized from |
|---|---|---|---|---|
| Population, working-age, labour force | pop (province, education) | `f64` | persons | econ pack; static in P2 |
| Employed | pop | `f64` | persons | (1 - unemployment rate) x labour force |
| Ownership share | pop | `f64` | share of province profit pool | base wage income share |
| Income last tick | pop | `f64` | numeraire | |
| Sector, productivity A, base VA0, base employment L0, labour share s_L | enterprise (province, sector) | `f64` | | calibration table |
| Employment L | enterprise | `f64` | effective labour units | base L0 |
| Output, sales, profit last tick | enterprise | `f64` | | |
| Input stock | enterprise x 10 goods | `f64`, held in the ledger | units | one month of base input use |
| Wage w | province | `f64` | numeraire per effective unit | base w0 = sum_g s_L VA0_ig / ((1 - u_2000) L^s_i) |
| Wage clip ticks: ticks at which the wage step clip bound | province | `u32` | ticks | zero |
| Effective labour supply L^s | province | `f64` | effective units | sum_k e_k LF_k |
| Accounts | pops, enterprises, rebate (one per country), mint | ledger | money `i64` (1 minor unit = 1e-4 numeraire), goods `f64` | |

Enterprises exist for every (province, traded good) with VA > 0 and every (province, local
good). About 4,796 x 10 rows at most, sorted by province (CSR offsets, 19.4). Every pop and
enterprise belongs to one province; per-province work is a contiguous slice.

## Equations

**Production.** For enterprise (i, g) with country input coefficients a_kg and VA coefficient
a_VA = 1 - sum_k a_kg (at reference prices):

```
Y  = min( VA / a_VA,  min_k  M_k / a_kg )                         (Leontief)
VA = A * VA0 * [ (1 - s_L) + s_L (L/L0)^rho ]^(1/rho)              (normalized CES, F fixed at F0)
rho = (sigma_FL - 1) / sigma_FL                                    (P08)
```

sigma_FL = 1 uses the Cobb-Douglas limit VA = A VA0 (L/L0)^s_L. Inputs M_k come from the
enterprise's input stock, bought at the previous tick's market clearing; a shortfall caps Y,
so rationing propagates through the IO network (7.3) with a one-month lag, and local goods
(services feed services) need no same-tick solve. Each tick the enterprise orders up to an inventory position
(stock plus goods on order, the paid shipments in its pipeline):

```
order_k = max(0, (1 + cover + L_k) a_kg Y_cap - stock_k - on_order_k),    cover = 1
```

where Y_cap = VA(L) / a_VA is capacity output at this tick's employment and L_k the expected
months in transit of input k to the enterprise's province: the arrival slots of its sourcing
(0 home, 1 domestic, 1 + lag a crossing) weighted by the units arriving at current shares; 0 for
local goods (bought at the province market and delivered the same tick). Where L = 0 everywhere
this is the two-month rule (owner decisions, 2026-10-24): planning on output cut by a short input
would cut the next order and the next output with it, a ratchet that emptied the fixture economy
from the data state; and a fixed target that counts goods on order cannot hold a month in
transit. Traded inputs are bought at the delivered unit price q_k, spend = order_k q_k, so the
units ordered are the units that arrive. At a steady state inputs do not bind and Y_cap = Y.

**Labour target.** With value-added price p_VA = (p_ig - sum_k a_kg q_ik) / a_VA, the own
(origin or province market) price at current prices and the buyer prices q_ik of last tick's
E3 (the delivered unit price of traded inputs, the posted price of local ones), the profit-maximizing employment solves p_VA dVA/dL = w_i, in closed form:

```
q  = w L0 / (p_VA A VA0 s_L)
L* = L0 * ( (q^(rho/(1-rho)) - s_L) / (1 - s_L) )^(-1/rho)
```

L* = 0 if p_VA <= 0. When the bracket is not positive the target is a corner: zero if rho < 0,
the province's labour supply if rho > 0. Cobb-Douglas: L* = L0 q^(1/(s_L - 1)). A unit test
checks the closed form against numerical maximization. Employment moves toward the target:
L_t = L_{t-1} + eta (L* - L_{t-1}) (P41). If sum_g L_t exceeds L^s_i, all are scaled down pro
rata.

**Wages.** Excess labour demand x_i = (sum_g L*_ig - L^s_i) / L^s_i, expected inflation pi^e
(trade SPEC, Origin market; ADR 0019):

```
dlnw = clip( pi^e + kappa_w x_i, -c_max, c_max ),  times (1 - delta_w) when negative    (P39, P40)
```

Unit pass-through of pi^e is a homogeneity restriction, not a parameter; downward rigidity
applies to nominal cuts, so a real cut that is a nominal rise is not damped. pi^e tracks the
world price index over buyer prices with base consumption weights; cells whose buyer price is
infinite (no reachable origin) are left out of the index (owner ruling, MIGRATION Task 6), so a
change in reachability moves its weights. c_max is P03c; every tick the clip binds is counted
per province (`clip_ticks` in `run_wages`).

Frictionless mode (V1): w_i solves sum_g L*_ig(w) = L^s_i exactly (monotone in w; safeguarded
Newton in ln w), and eta = 1. The unemployment rate u_i = 1 - sum_g L_ig / L^s_i applies equally
to each education level. L^s is the whole labour force, so 2000 unemployment rates are only the
starting point: with the wage rule, unemployment is cyclical and the long-run steady state has
full employment (owner choice; structural unemployment arrives with matching in P4).

**Income.** Pop k in province i receives wages w_i e_k employed_k, a share of the province profit
pool (enterprise profits plus market cash flow, both signed, trade SPEC), and a share of its
country's tariff rebate by population. Shares are fixed at load.

**Final demand.** Each pop spends its whole money balance m each month (when positive) with CES
over goods at buyer prices (q_d for traded goods, the province price for local goods; a good
with no reachable origin has q_d infinite and gets nothing), Omega from its country's 2000 final demand shares and
sigma = P06:

```
x_g = Omega_g p_g^-sigma m / sum_k Omega_k p_k^(1 - sigma)
```

Spending on traded goods is sourced over origins (trade SPEC) and paid at dispatch for what is
filled; rationed purchases leave money unspent until the next tick.

**Money.** The mint issues one month of base income to each pop at tick 0. Enterprises and
markets pay out their signed cash flow every tick, so their balances return to zero; any
account may be negative within or across ticks (clearing facility, no interest). Every payment
is a ledger transfer rounded half to even to minor units. Prices and wages scale freely: the
dynamics are homogeneous of degree zero in nominal values, so the price level is set by
initial money and comparisons use a world price index.

## Calibration (`ceteris calibrate p2`)

1. Rust loads the pack, builds the kernel, and exports kernel legs, enterprises, pops and
   parameters as Arrow.
2. Base point per enterprise: L0_ig = s_L VA0_ig / w0_i (base wage from the State table), so
   base wages clear base employment. The GE reference model (below) then solves in one joint
   system for prices, wages, the productivity A of every traded enterprise (equilibrium VA
   equals the pack's downscaled VA) and one A per (country, local good) (equilibrium national
   VA of that good equals ICIO).
3. The result is `econ_calib.arrow`, hashed in `tests/golden/p2-pack.txt`. Residuals (where no
   positive A can match) are reported, not hidden.

The 2000 state used for runs is the data state (prices 1, base wages and employment, 2000
unemployment, inventories and input stocks at target) with the calibrated A. It is not the
equilibrium: runs start out of it and converge, so V1 tests convergence, not calibration fit.

## GE reference model (`python/ceteris/reference`)

Static equilibrium of the same economy with all frictions off: unknowns are ln p for every
active (province, good) cell (origin markets of traded goods, province markets of local goods)
and ln w for every province. Conditions: every origin sells its output, output = sum_d X_od /
(p_o (1 + t)) with X_od = E_d K_od w_o / Phi_d, destination spending E_d of households (CES at
q_d) and enterprises (a Y q_d); local goods clear locally; labour clears with p_VA at q; pops
spend income (wages, profits, rebates; market cash flow is zero at a fixed point); the world
price index over consumer prices (q_d for traded goods, 2000 final demand weights) equals 1.
Solved with Newton-Krylov (scipy) from the data state, Anderson-accelerated tatonnement as a
fallback; tolerance 1e-10 on scaled residuals. Balanced Sinkhorn on the dense kernel is the S02
full solve and reproduces the equilibrium's sourcing flows. numpy and scipy only (JAX stays
optional, 19.1).

## System order (monthly unless noted)

| System ID | Name | Reads | Writes | Position (4.4) |
|---|---|---|---|---|
| T1 | deliver (trade) | pipeline | pops' consumption, enterprise input stocks | first |
| E1 | labour | prices, last tick's buyer prices, wages, pi^e, A | L*, L, wages, wage clip counts, unemployment | labour market |
| E2 | produce | L, input stocks | input use, output to the province's market (Q), wages paid | production |
| E3 | demand | pop money, prices, output, pipeline | buyer prices q and leads L, household demand, enterprise input orders | consumption demand |
| T2 | source (trade) | traded spending, prices, kernel | orders at origin markets | inter-province trade |
| T3 | clear (trade) | orders, output, inventory; local demand | fills, sales, inventory; local sales and payments | markets |
| T2b | dispatch (trade) | fills | shipments, pipeline, payments, tariffs | inter-province trade |
| T4 | reprice (trade) | D, Q, S, pi^e, buyer prices | prices, clip counts; then pi^e from the world price index | markets |
| E4 | distribute | enterprise and market cash flow, rebates | pop money for next tick | finance (P2 stand-in) |
| E5 | assert | ledger, state | | accounting assertions |

System IDs 201 to 210 in this order.

Pops spend at E3 the money they hold: this tick's wages plus last tick's dividends and rebates.

Snapshots every 12 ticks (core). No RNG draws in P2; streams are reserved per system ID.

## Invariants

| Invariant | Statement | Tolerance | Checked where |
|---|---|---|---|
| Money | Sum of balances zero; every flow tagged | exact | `Ledger::check`, every tick |
| Goods | Created - destroyed = change in holdings (incl. transit) | 1e-9 relative | every tick |
| Population | Pops unchanged | exact | every tick |
| Labour | 0 <= employed <= labour force; sum L <= L^s | 1e-12 relative | every tick |
| Leontief | Inputs used = a_kg Y | 1e-12 relative | every tick |
| Payments | Each buyer pays its share of its class's filled spending, half to even per good; gateways net to zero | exact in ledger | property test |
| Income | Pop income = wages + dividends + rebates paid this tick | exact in ledger | every tick |
| Determinism | Same inputs, same hashes; 1 vs 8 threads; replay from snapshots | exact | P2 acceptance |
| Long run | 1,200 ticks on the full pack: no violation, all prices and wages finite and positive | exact | P2 acceptance |

## Parameters

New register IDs (Section 17 extension, applied to the design doc with the next snapshot):

| ID | Name | Prior / value | Class |
|---|---|---|---|
| P06 | Goods substitution sigma | 0.3 to 0.9, start 0.6 | Literature |
| P08 | F-L substitution sigma_FL | 0.5 to 1.2, start 0.8 | Literature |
| P39 | Wage adjustment speed kappa_w | 0.05 to 0.5 per month, start 0.1 | Free |
| P40 | Downward wage rigidity delta_w | 0 to 1, start 0.5 | Free |
| P41 | Employment adjustment speed eta | 0.1 to 1 per month, start 0.3 | Free |
| P42 | Education efficiency e_k | below secondary 1; secondary 1.4 to 1.6; tertiary 2.0 to 2.6 (Mincer returns about 8 to 12% per year) | Literature |

Data, not parameters: IO coefficients, final demand shares, labour shares, VA targets,
populations, education and labour-force shares. Calibrated: A (inversion, not estimation).

## Equilibrium nesting

H1/V1: with frictionless settings (Walrasian wage, eta = 1, delta_w = 0) and any stable kappa,
the run from the data state reaches the GE reference: every price and wage (index-normalized)
within 1%, and every country-pair x good flow larger than 1e-6 of world trade within 1%. Checked
on generated small worlds in CI and on the full pack locally.

## P2 gate (21.1)

- V0: zero invariant violations; determinism, 1 vs 8 threads, replay.
- V1: as above, full pack, within 1,200 ticks.
- H2: PPML of simulated country-pair trade (traded goods, FOB value, steady state of the default
  run) on ln distance (CEPII population-weighted) and contiguity, with exporter and importer
  effects, excluding domestic pairs: distance coefficient in -1.1 to -0.7. H2 blocks the gate
  (owner decision). Nothing is fitted to it: costs come from P04 and the literature time cost
  P43. If it misses, work stops and the miss is reported to the owner before any parameter
  moves; any change then goes through the register.
- H2 per theta class (ADR 0018): PPML of each class's simulated country-pair trade on ln(1 +
  tariff) and ln distance with exporter and importer effects (`ppml.trade_elasticity`) recovers
  the class's theta (ADR 0023) within its prior band.
- 100 years (1,200 ticks) of the default run on the full pack without violation.
- Budgets (19.2, M4 Pro): demand 10 ms, production and labour 12 ms, markets 2 ms, trade 30 ms
  per tick; criterion benchmarks in CI with the 20% regression gate.
- Golden: `tests/golden/p2-econ.txt`, hashes of a generated 3-country, 24-province world over
  1,200 ticks (runs in CI); `tests/golden/p2-pack.txt` for the pack tables and calibration.
- UI: below.

## App (after the native app reaches parity, ADR 0011 option 3)

The owner chose a fully native app (no React, webview or Tauri), panels in iced or Slint, picked
by the stage 1 spike. P2's UI is built there, not in `app/`. Map modes: price by good
(index-normalized), output by sector, net exports by good, wage, unemployment. Province panel:
market rows per good (price, inventory, supply, demand), its enterprises (sector, employment,
output, profit), its pops (education, labour force, employed, income). Time controls: play,
pause, step, speed, driving the core in the same process. Colouring goes through the feature
texture fed from core columns.

## Test plan

- Unit per equation: Leontief binding input, normalized CES and the closed-form L* against
  numerical maximization, wage step and clip, rigidity on nominal cuts
  (`wage_rigidity_applies_to_nominal_cuts`), frictionless wage root, CES demand shares,
  income split, rounding of payments.
- Property (proptest): money and goods conservation through a full tick on random small
  worlds; employment bounds; homogeneity (scaling all nominal values leaves real outcomes
  identical; pi^e is a log difference and stays); pi^e follows the world price index
  (`expected_inflation_follows_the_world_index`); clip counts (`world_counts_clip_bindings`).
- Python (pytest, hypothesis): ICIO aggregation preserves totals; downscaling preserves national
  VA; imputation flags; GE reference residuals; reference versus balanced Sinkhorn on small
  worlds; PPML recovers a known elasticity on synthetic gravity data.
- Acceptance: `tests/p2_acceptance.rs` (V0, V1, 100 years, determinism) and
  `python/tests/test_p2_gate.py` (V1 comparison, H2); full-pack parts skip without a pack, as
  in P1.
- Benchmarks: `labour`, `produce`, `consume` (criterion, full pack).

## Open questions

- ICIO 2025 edition industry list (whether B05 and B06 are split) is checked at fetch.
- Sector-specific labour shares (P2 uses the PWT country share for every sector).
- DOSE region matching rate to Natural Earth provinces without GADM geometry; if too low,
  manufacturing falls back to gridded GDP for those countries.
- 2000 tariff detail beyond importer x two groups (WTO IDB is closed-source only, fetch).
- Stability on the full pack: on the fixture under origin clearing small deviations decay by
  0.9963 (frictionless) and 0.9969 (default) per tick at the P2 defaults (kappa1 0.05, kappa2
  0, expectation gain 0.006; trade SPEC, Stability region and P03a, P03b, P45). The slowest
  mode is the world price level, which expected inflation turns into a damped oscillation
  (trade SPEC, Nominal mode). Re-measure on the pack in Task 20.
- The run from the data state passes through a deep coal-led shortage before it converges (the
  equilibrium's coal output is about twice base output and intermediate demand is inelastic in
  units); V1 holds on the fixture at 1,200 ticks.
- Frictionless clearing wage: each labour target is capped at L^s, so where one enterprise's
  target reaches L^s, capped demand equals L^s on a whole interval of low wages; the search
  then finds no sign change and the wage is indeterminate below the true root. Searching on
  uncapped targets (capping only the allocation) would follow the closed form. No fixture
  province is affected.
