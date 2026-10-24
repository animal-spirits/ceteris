# Econ SPEC

Status: approved (owner, 2026-10-23)
Phase: P2
Design sections: 3.2, 3.3, 3.4, 4.2, 4.4, 4.6, 6.1, 7.1, 7.2, 7.3, 8.6, 8.7, 9.1, 15.3 (V0, V1), 16, 17, 18 (H1, H2), 19.2, 19.4, 20, 21.1 (P2); ADRs 0012 to 0016 and 0018 to 0023

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
| PIGL over five categories: food, energy, housing, manufactures, services (8.6) | PIGL over the three categories the form was estimated on, A (goods 0, 3), M (goods 1, 2, 4, 5, 6), S (goods 7, 8, 9), with a CES over goods within each (ADR 0021, controller ruling 2026-10-24) | Alder, Boppart and Mueller (2022) estimate agriculture, manufacturing and services only; crude oil, coal and utilities have no clean counterpart there and are placed by the nearest category |
| Intermediates Leontief plus a CES energy nest (7.2) | All intermediates Leontief | The energy nest comes with energy and emissions (P8) |
| Putty-clay intermediates: coefficients per capacity vintage, chosen at investment by a CES at P38, Leontief once installed (7.2, ADR 0020) | Each enterprise runs on its own coefficient vector (`EntData.coef`, `Layout.coef`), one vintage: its country's 2000 ICIO column, filled at load (`EconInputs::set_coefficients`). S09 `Technology::FixedLeontief` reads the country column instead. With one vintage both settings read the same numbers and run bit for bit alike, state hashes included: the switch enters the static digest only once some enterprise's coefficients differ from its country's (`putty_clay_one_vintage_equals_fixed_leontief`) | No investment in P2, so no vintage beyond the first; the vintage choice and P38 arrive with investment (7.4) |
| Within-tick rationing, inventories as the buffer (8.2, 19.4, ADR 0020) | Local goods (utilities, construction, services): a same-tick chain at E2 per province (`local.rs`, Production below): a shortage of a local good cuts its users' output in the tick it happens. Traded goods keep their stocks and transit lags. Departures from the migration brief's chain, each needed for exact Leontief use: (1) a producer's own use of its good comes out of its output first, since cutting the producer pro rata with its users leaves the ratio of output to need unchanged and with empty stocks drives the pair to zero; (2) each user covers its residue from its own stock (the ledger holds stocks per enterprise), so the stock that counts for a good is sum min(b, n), and output is split so that each user's residue fits its stock; (3) each good's common fill is solved exactly for its users (the limit of repeating the brief's f_k), so a single shortage settles in two iterations; (4) a cycle between two local goods can still converge slowly (loop gain near 1 when both are fully used): at the bound the remaining shortage is cleared by cutting the users that produce no local good, which changes no local output, and where the producers of local goods alone do not fit, by stopping the users whose stock cannot cover them, so input use always equals coef Y exactly (MIGRATION Task 8) | ADR 0020 Mapping (owner, 2026-10-24): local goods have no transit, so their production, input use and rationing run as one pass within the tick |
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
| `econ_countries.arrow` | 242 | ICIO economy or imputation donor, final demand shares (10), labour share, unemployment rate, participation rate, education shares (3), GDP 2000, Gini 2000 (imputed flag) | ICIO, PWT 11 (labsh), ILOSTAT, WIC reconstruction (Barro-Lee cross-check), WDI; Gini: WDI SI.POV.GINI nearest year 1995 to 2005, else WIID |
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
| Input coefficients coef_k (putty-clay, ADR 0020; static in P2) | enterprise x 10 goods | `f64` | units per unit of output | one vintage: the country's 2000 ICIO column |
| Input stock | enterprise x 10 goods | `f64`, held in the ledger | units | one month of base input use |
| Wage w | province | `f64` | numeraire per effective unit | base w0 = sum_g s_L VA0_ig / ((1 - u_2000) L^s_i) |
| Wage clip ticks: ticks at which the wage step clip bound | province | `u32` | ticks | zero |
| Effective labour supply L^s | province | `f64` | effective units | sum_k e_k LF_k |
| Accounts | pops, enterprises, rebate (one per country), mint | ledger | money `i64` (1 minor unit = 1e-4 numeraire), goods `f64` | |

Enterprises exist for every (province, traded good) with VA > 0 and every (province, local
good). About 4,796 x 10 rows at most, sorted by province (CSR offsets, 19.4). Every pop and
enterprise belongs to one province; per-province work is a contiguous slice.

## Equations

**Production.** For enterprise e = (i, g) with input coefficients coef_k (`Layout.coef`: its
putty-clay coefficients, one vintage in P2, or its country's a_kg under S09 fixed Leontief) and
VA coefficient a_VA = 1 - sum_k coef_k (at reference prices):

```
Y  = min( VA / a_VA,  min_k M_k / coef_k )                       (Leontief)
VA = A * VA0 * [ (1 - s_L) + s_L (L/L0)^rho ]^(1/rho)              (normalized CES, F fixed at F0)
rho = (sigma_FL - 1) / sigma_FL                                    (P08)
```

sigma_FL = 1 uses the Cobb-Douglas limit VA = A VA0 (L/L0)^s_L. Traded inputs M_k come from the
enterprise's input stock, bought at earlier ticks and delivered after transit; a shortfall caps Y
in the tick the stock runs out, so a traded shortage reaches its users with their stocks and
transit as the lag. Local inputs (goods 7 to 9) come from this tick's local output first and the
enterprise's stock second, in the same-tick chain (ADR 0020, `local.rs`), run per province:

```
Y_e = min(VA / a_VA, min over traded k of M_k / coef_k)        every enterprise of the province
repeat up to N_LOCAL = 32 times, for each local good k:
    R_k  = (1 - coef_kk) Y_p                                    p the producer of k: own use first
    n_ek = coef_ek Y_e, b_ek = e's stock of k                   every other user e
    f_k  = largest x <= 1 with x sum n_ek <= R_k + sum min(b_ek, x n_ek)
         = the limit of f_k <- min(1, (R_k + sum min(b_ek, n_ek)) / sum n_ek) with every user cut alike
  Y'_e = Y_e min over local k used by e (its own good left out) of f_k
  stop when max_e |Y'_e - Y_e| <= 1e-12 Y_e; Y = Y'
after the bound: a good still short is cleared by cutting the users that produce no local good
(no local output moves); if its producers alone do not fit, the users their stock cannot cover stop
draws: with output enough, every user draws n_ek from output; short, each draws max(n_ek - b_ek, 0)
plus the rest of R_k pro rata to min(b_ek, n_ek), its stock covering the remainder
```

Y never rises. The draws are paid at the posted price at E2, count in the market's demand and
sales, and come before household demand under every S11 setting (the chain runs before E3); at
T3 the rest of local output is shared between enterprise orders (stock refills) and households by
S11, and what is left perishes. Use equals coef Y exactly. On the fixture, from the data state over
1,200 ticks, the chain binds in 6 of 30,000 province ticks in the default run (5 settle in 2
iterations, 1 in 3; the island, ticks 27 to 33) and never in the frictionless run; with services
feeding services at 0.3 and construction and services each other at 0.2 it binds in 287 of 3,000
province ticks over 120 ticks, 8 of them at the bound (a utilities and construction pair with loop
gain about 0.86; `local_chain_terminates_on_cycles`). Each tick the enterprise orders up to an
inventory position (stock plus goods on order, the paid shipments in its pipeline):

```
order_k = max(0, (1 + cover + L_k) coef_k Y_cap - stock_k - on_order_k),    cover = 1
```

where Y_cap = VA(L) / a_VA is capacity output at this tick's employment and L_k the expected
months in transit of input k to the enterprise's province: the arrival slots of its sourcing
(0 home, 1 domestic, 1 + lag a crossing) weighted by the units arriving at current shares; 0 for
local goods (bought at the province market and delivered the same tick, so their stock is a buffer
that the chain draws only in a shortage). Where L = 0 everywhere this is the two-month rule (owner
decisions, 2026-10-24): planning on output cut by a short input would cut the next order and the
next output with it, a ratchet that emptied the fixture economy from the data state; and a fixed
target that counts goods on order cannot hold a month in transit. Traded inputs are bought at the
delivered unit price q_k, spend = order_k q_k, so the units ordered are the units that arrive. At a
steady state inputs do not bind, Y_cap = Y, local stocks stay at their target and their orders are
zero, so the steady state (and the GE reference) is that of the lagged rule.

**Labour target.** With value-added price p_VA = (p_ig - sum_k coef_k q_ik) / a_VA, the own
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

**Final demand (ADR 0021).** Each pop spends its whole money balance each month (when positive;
the consumption rule of 8.6 with m = 1, lambda = 0) at buyer prices (q_d for traded goods, the
province price for local goods; a good with no reachable origin has q_d infinite and gets
nothing): PIGL across three categories, a CES over goods within each.

*Source.* Alder, Simon, Timo Boppart and Andreas Mueller (2022), "A Theory of Structural Change
That Can Fit the Data", American Economic Journal: Macroeconomics 14(2): 160-206. Equation and
page numbers below are those of the accepted manuscript (Essex repository,
`repository.essex.ac.uk/29270/1/AEJMacro2019-0303_Manuscript.pdf`), read in full.

- Period indirect utility, eq. (12), p. 14: `v(e, P) = (1 - eps) / eps * (e / B(P) - A(P))^eps
  - D(P)`, A and D homogeneous of degree 0, B linearly homogeneous. PIGL is Special Case I,
  A(P) = 0 (p. 18).
- Price functions, Section IV, p. 17: eq. (19) `B(P) = (sum_c omega_c p_c^(1 - sigma_B))^(1 /
  (1 - sigma_B))`, sigma_B > 0, sum omega = 1; eq. (21) `D(P) = (1 - eps) nu / (kappa gamma) *
  ((Dt(P) / B(P))^gamma - 1)`, `Dt(P) = (sum_c theta_c p_c^(1 - phi))^(1 / (1 - phi))`, nu >= 0,
  phi > 0, sum theta = 1; eps < 1, and 0 < gamma <= eps < 1 in estimation (Prop. 3, p. 18).
- Aggregate PIGL shares, eq. (22), p. 18: `eta_c = B_c p_c / B + kappa D_c p_c / (1 - eps) *
  B^eps (E/N)^-eps`, kappa = 1/N int ((E/N) / e_i)^(eps - 1) di. EIS 1 / (1 - eps).
- Individual shares (Roy's identity on (12) with A = 0, from (15), p. 15), with sB_c =
  omega_c p_c^(1 - sigma_B) / B^(1 - sigma_B) and sD_c = theta_c p_c^(1 - phi) / Dt^(1 - phi):

```
eta_c(e) = sB_c + lambda (Dt / B)^gamma (sD_c - sB_c) (e / B)^-eps,    lambda = nu / kappa
```

- Roles: eps (P47) is the Engel parameter: the income-effect term falls with e at rate eps, so
  categories with theta_c > omega_c (A) are necessities and those with theta_c < omega_c are
  luxuries. sigma_B, phi and gamma (P46) are price parameters: the substitution of the
  homothetic aggregator B, of the income-effect aggregator Dt, and the curvature of D in Dt /
  B. nu scales the income-effect term (absorbed in lambda, backed out per country); omega and
  theta are the two weight vectors.
- phi = 0, the pooled estimate, is the limit phi -> 0 of (21): the exponents 1 - phi and 1 / (1
  - phi) tend to 1, so `Dt(P) = sum_c theta_c p_c`, linear and smooth (the Leontief cost
  function), and sD_c = theta_c p_c / Dt. The formula is continuous there, so phi = 0 is used
  as is.

*Categories and goods.* Categories c: A (goods 0 agriculture, 3 food), M (goods 1, 2, 4, 5,
6), S (goods 7, 8, 9). The paper has no counterpart for crude oil, coal and utilities; they sit
in M, M and S. Within category c the pop spends by CES at P06 = sigma:
P_c = (sum_(g in c) alpha_g p_g^(1 - sigma))^(1 / (1 - sigma)), good g getting alpha_g
p_g^(1 - sigma) / sum of the category. Goods without weight or at an infinite price are left
out; a category with none of its goods reachable is left out, and B and Dt run over the rest
with omega and theta renormalized.

*Pop demand.* A pop's income per person is lognormal with mean m = money / population and log
dispersion s = sqrt(2) Phi^-1((G + 1) / 2) = 2 erfinv(G), G its country's 2000 Gini. With
E[e^(1 - eps)] = m^(1 - eps) exp(-eps (1 - eps) s^2 / 2), the pop's category shares are, in
closed form,

```
s_c = sB_c + lambda (Dt / B)^gamma (sD_c - sB_c) (m / B)^-eps exp(-eps (1 - eps) s^2 / 2)
```

Shares below 0 (a pop far poorer than its country's base) are set to 0 and the rest rescaled to
sum to 1 (Review Focus 4). Spending on good g is s_c(g) times its within-category share times
the money balance: spending equals money.

*Inversion (at load, per country).* Parameters theta, sigma_B, phi, gamma, eps and omega_A = 0
from the pooled PIGL column of the paper (Tables 3 and A3, column 3, pp. 24 and 48). At prices 1
(B = Dt = 1, sB = omega, sD = theta) and the country's base spending per person m_0 (base
household income over population), Omega_c = (1 - z) omega_c + z theta_c with z = lambda m_0^-eps
exp(-eps (1 - eps) s^2 / 2). Hence z = Omega_A / theta_A, omega_c = (Omega_c - z theta_c) / (1 -
z) for M and S, lambda = z m_0^eps exp(eps (1 - eps) s^2 / 2), alpha_g = Omega_g / Omega_c(g):
the 2000 final demand shares are reproduced exactly at prices 1 and m_0. A country with z >= 1
or a negative omega_c stops the load (never clipped). On the fixture z = 0.284, omega = (0,
0.247, 0.753) in every country.

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
   parameters as Arrow. theta per class, the border friction P05 and the mode cost ratios P04 are
   estimated before this, directly by PPML structural gravity on 2000 BACI with least-cost (S05)
   costs between countries (ADR 0022, PLAN Task 18), with provenance "data".
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
(p_o (1 + t)) with X_od = E_d K_od w_o / Phi_d, destination spending E_d of households (PIGL at
q_d) and enterprises (a Y q_d); local goods clear locally; labour clears with p_VA at q; each pop
spends its income by PIGL (`reference/demand.py`, the mirror of `demand.rs`): its effective
labour share e_k LF_k / L^s of its province's wages and profits, and its population share of its
country's rebate (market cash flow is zero at a fixed point); the rebate is the tariff share of
spending, so income is iterated to a fixed point (relative 1e-15) inside each evaluation; the
world price index over consumer prices (q_d for traded goods, 2000 final demand weights) equals
1. Solved with Newton-Krylov (scipy) from the data state, Anderson-accelerated tatonnement as a
fallback (under PIGL the fixture needs the fallback); tolerance 1e-10 on scaled residuals. Balanced Sinkhorn on the dense kernel is the S02
full solve and reproduces the equilibrium's sourcing flows. numpy and scipy only (JAX stays
optional, 19.1).

## System order (monthly unless noted)

| System ID | Name | Reads | Writes | Position (4.4) |
|---|---|---|---|---|
| T1 | deliver (trade) | pipeline | pops' consumption, enterprise input stocks | first |
| E1 | labour | prices, last tick's buyer prices, wages, pi^e, A | L*, L, wages, wage clip counts, unemployment | labour market |
| E2 | produce | L, input stocks, local output (same-tick chain per province) | input use (local inputs drawn from this tick's output first, paid at the posted price), output to the province's market (Q), wages paid | production |
| E3 | demand | pop money, prices, output, pipeline, E2 local draws | buyer prices q and leads L, household demand and enterprise input orders on top of the local draws | consumption demand |
| T2 | source (trade) | traded spending, prices, kernel | orders at origin markets | inter-province trade |
| T3 | clear (trade) | orders, output, inventory; local demand | fills, sales, inventory; local sales of what the chain left (S11) and payments | markets |
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
| Leontief | Inputs used = coef_k Y, traded from stock, local through the same-tick chain | 1e-12 relative | every tick |
| Payments | Each buyer pays its share of its class's filled spending, half to even per good; gateways net to zero | exact in ledger | property test |
| Income | Pop income = wages + dividends + rebates paid this tick | exact in ledger | every tick |
| Determinism | Same inputs, same hashes; 1 vs 8 threads; replay from snapshots | exact | P2 acceptance |
| Long run | 1,200 ticks on the full pack: no violation, all prices and wages finite and positive | exact | P2 acceptance |

## Parameters

New register IDs (Section 17 extension, applied to the design doc with the next snapshot):

| ID | Name | Prior / value | Class |
|---|---|---|---|
| P06 | Goods substitution sigma within a PIGL category | 0.3 to 0.9, start 0.6 | Literature |
| P46 | PIGL price parameters: sigma_B (aggregator B), phi (aggregator Dt), gamma (curvature of D), with the Dt weights theta_A, theta_M, theta_S | start sigma_B 0.26 (s.e. 0.05), phi 0.00 (at its bound), gamma 0.71 (s.e. 0.01), theta 0.634, 0.082, 0.284; country estimates sigma_B 0.09 to 0.55, gamma 0 to 0.90, phi 0 to 7.32 | Literature: Alder, Boppart and Mueller (2022), pooled PIGL, Tables 3 and A3 column 3 |
| P47 | PIGL Engel parameter eps | start 0.71 (s.e. 0.01); country estimates 0.34 to 0.90 | Literature: same column |
| P08 | F-L substitution sigma_FL | 0.5 to 1.2, start 0.8 | Literature |
| P39 | Wage adjustment speed kappa_w | 0.05 to 0.5 per month, start 0.1 | Free |
| P40 | Downward wage rigidity delta_w | 0 to 1, start 0.5 | Free |
| P41 | Employment adjustment speed eta | 0.1 to 1 per month, start 0.3 | Free |
| P42 | Education efficiency e_k | below secondary 1; secondary 1.4 to 1.6; tertiary 2.0 to 2.6 (Mincer returns about 8 to 12% per year) | Literature |

Data, not parameters: IO coefficients, final demand shares, labour shares, VA targets,
populations, education and labour-force shares, Gini. Calibrated: A (inversion, not
estimation); the PIGL weights omega_M, omega_S, lambda and alpha per country (exact inversion
of the 2000 final demand shares, above). P07 stays the S03 (non-homothetic CES) row of the
design register.

## Equilibrium nesting

H1/V1: with frictionless settings (Walrasian wage, eta = 1, delta_w = 0) and any stable kappa,
the run from the data state reaches the GE reference: every price and wage (index-normalized)
within 1%, and every country-pair x good flow larger than 1e-6 of world trade within 1%. Checked
on generated small worlds in CI and on the full pack locally. The 1% comparison presumes one
equilibrium: before V1 the uniqueness check below runs at the tested parameters, and V1 uses
these tolerances only where it holds (tolerance mode; P2 gate).

### Uniqueness check (ADR 0022)

Source: Allen, Arkolakis and Li (2024), "On the Equilibrium Properties of Spatial Models",
American Economic Review: Insights 6(4), 472-89. Page numbers below are those of the authors'
January 2024 version, the one read for this spec.

**Theorem 1** (p. 6), restated. N locations, H types of interaction, outcomes x in R_{++}^{N x
H} solving

    x_ih = sum_j f_ijh(x_j1, ..., x_jH)                                    (1)

with f_ijh: R_{++}^H -> R_{++} differentiable, and elasticities eps_{ijh,jh'}(x_j) = d ln f_ijh /
d ln x_jh'. Suppose an H x H matrix A bounds them: |eps_{ijh,jh'}(x_j)| <= A_hh' for all i, j,
h, h' and all x_j > 0. rho(A) is its spectral radius. Then:

- (i) if rho(A) < 1, (1) has a unique solution, and iterating (1) from any positive start
  converges to it at rate rho(A);
- (ii) if rho(A) = 1 and (a) for every i and every h, h' with A_hh' != 0 some j has
  |eps_{ijh,jh'}(x_j)| < A_hh' for all x_j, then (1) has at most one solution; or (b) the
  elasticities are constant, eps_{ijh,jh'} = alpha_hh' with |alpha_hh'| = A_hh' (f_ijh = K_ijh
  prod_h' x_jh'^alpha_hh', K_ijh > 0), then solutions are unique column-wise up to scale (x'_.h
  = c_h x_.h, c_h > 0);
- (iii) if rho(A) > 1 and N >= 2H + 1, some K_ijh > 0 make the constant-elasticity system with
  |alpha_hh'| = A_hh' have two solutions that differ beyond column-wise scale.

Remark 4 (p. 10): with endogenous scalars lambda_h x_ih = sum_j f_ijh and constant elasticities,
rho(A) <= 1 gives the conclusion of (ii.b). Remark 5 (p. 11): the constant-elasticity system
prod_h' x_ih'^gamma_hh' = sum_j K_ijh prod_h' x_ih'^rho_hh' x_jh'^beta_hh' (equation 8) maps
onto (1) with A = |B (Gamma - R)^-1| when Gamma - R is invertible.

**Special case used.** None of the paper's examples is a multi-sector trade model with an
input-output table, so no example's A is P2's. The nearest is Online Appendix A.3 (pp. 6-9):
one tradeable sector, Eaton-Kortum with tariffs, tradeables made from labour (share beta) and the
tradeables aggregate, under quasi-symmetric trade costs (kappa_ij omega_ij = tau_ij c_i r_j with
tau symmetric). Its system in the tradeables price index and the wage (equations 29 and 30) has,
with theta the trade elasticity (the paper's 1/theta),

    Gamma = [[-theta, 0], [(1 - beta) theta, 1 + beta theta]]
    B     = [[-(1 - beta) theta, -beta theta], [-(1 - beta) theta, -beta theta]]
    rho(B Gamma^-1) = (1 + beta (theta - 1)) / (1 + beta theta) < 1   for theta > 0, 0 < beta <= 1

(the paper's (beta + theta' - beta theta') / (beta + theta'), theta' = 1 / theta, p. 9).
`reference/uniqueness.py` applies A.3 to each traded good on its own, at its theta_g (P01) and
beta_g, the smallest value-added share 1 - sum_k a_kg over countries (the radius falls with
beta), and takes the largest: `spectral_bound(theta, value_added_share=value_added_share(inp))`.
`v1_mode(bound)` is `"tolerance"` for a bound at most 1 (parts i and ii.b), else `"report"`.

Why this case, and what it leaves out: (1) goods are coupled through the Leontief input-output
table and PIGL demand, while A.3's only intermediate is the tradeables aggregate; a joint A over
goods needs elasticity bounds that depend on the coefficients and on income, a new derivation
rather than the paper's; (2) value added is a normalized CES over labour and a fixed factor, A.3
has labour alone, so beta is read as the value-added share (both factors immobile across
provinces in P2); (3) local goods have no trade interaction and are left out; (4) quasi-symmetry
needs symmetric leg costs and tariffs that split into importer and exporter factors, while P2's
tariff wedge is per (importer, exporter, good) and least-cost legs (S05) need not be symmetric
in both directions; without it A.3's three-equation form has a radius above 1 (footnote 10,
p. 7), which by itself does not imply multiplicity. The bound is therefore a per-good check
consistent with the paper, not a proof that P2's equilibrium is unique; V1 itself is the evidence
that the run and the reference reach the same point.

Agglomeration (A5) and migration have no mapping yet: A.2 (economic geography with spatial
spillovers, pp. 3-6) has rho(A) <= 1 only without spillovers, and A.6 (forward-looking migration,
pp. 12-15) is unique when alpha + beta <= 0 but has no trade costs. `spectral_bound` raises for a
nonzero agglomeration or a migration elasticity until the phase that adds one derives its A.

At the P2 defaults (P01 class values 12.55, 12.55, 12.55, 2.62, 5.88, 12.55, 5.88; the fixture's
value-added shares, 0.46 to 0.73) the bound is 0.932 (chemicals, refined fuels and metals: theta
12.55, beta 0.46); with labour the only input (beta = 1) it is 12.55 / 13.55 = 0.926. V1 runs in
tolerance mode. Re-evaluate on the pack's coefficients in Task 20.

## P2 gate (21.1)

- V0: zero invariant violations; determinism, 1 vs 8 threads, replay.
- V1: first the uniqueness check (Equilibrium nesting) at the run's theta and the pack's
  value-added shares. Where the bound holds (at most 1, tolerance mode) V1 applies the 1%
  tolerances above, full pack, within 1,200 ticks. Otherwise (report mode) both models start from
  the data state, the run frictionless for 1,200 ticks and the reference solve from
  `model.data_state`, and the test reports which equilibrium each reaches (index-normalized
  prices, wages and flows of each, and their largest gaps) instead of a pass or fail at 1%.
  Comparisons with the perfect-foresight references (Caliendo-Dvorkin-Parro, Kleinman-Liu-Redding)
  are sign and rank only: agents with heuristic expectations cannot match their transition paths.
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

- Same-tick chain and putty-clay (ADR 0020, `p2_properties.rs`): one vintage equals fixed
  Leontief in every state hash over 120 ticks (`putty_clay_one_vintage_equals_fixed_leontief`); a
  50% cut of services labour with empty local stocks cuts construction output in the same tick
  (`local_shortage_reaches_downstream_same_tick`); intermediate users draw all of a short local
  good before households, and Priority fills intermediate orders first at origins
  (`local_chain_respects_intermediate_priority`); cutting coal output leaves downstream output
  unchanged in that tick and lowers it once stocks run out (`traded_shortage_still_lags`); cycles
  among local goods (`local_chain_terminates_on_cycles`); S11 Proportional gives one fill ratio
  (`proportional_rationing_switch`); `fill` and `split` in `local.rs`.
- Unit per equation: Leontief binding input, normalized CES and the closed-form L* against
  numerical maximization, wage step and clip, rigidity on nominal cuts
  (`wage_rigidity_applies_to_nominal_cuts`), frictionless wage root, income split, rounding of payments. PIGL (`demand.rs`): pop
  demand against 64-node Gauss-Hermite quadrature of individual demand over the lognormal
  (1e-10), adding-up (1e-14), inversion reproducing base shares (1e-12), Engel curves across
  fixture pops (food share falls, services share rises), clamping far from base (money x 1e-3
  and x 1e3), homogeneity, unreachable goods and categories, Gini to sigma.
- Property (proptest): money and goods conservation through a full tick on random small
  worlds; employment bounds; homogeneity (scaling all nominal values leaves real outcomes
  identical; pi^e is a log difference and stays); pi^e follows the world price index
  (`expected_inflation_follows_the_world_index`); clip counts (`world_counts_clip_bindings`).
- Unreachable destination at world level (Review Focus 1, `fixture::unreachable_island`:
  machinery has no route to or from the island, and of the island's sectors only light
  manufacturing uses it): over 240 ticks in both modes the island's q_d of machinery is the only
  infinite buyer price and the world index leaves it out, island pops buy none of it and spend
  the rest of their money, its one user orders none, never restocks and ends producing nothing,
  no price, wage or flow turns NaN and the invariants hold
  (`unreachable_destination_buys_nothing_in_the_world`); the reference solves the same variant
  with a finite residual (`test_unreachable_destination_solves`). V1 holds on the variant (final
  fix wave: price 3.0e-5, wage 2.5e-5, flow units 5.8e-5, value 7.5e-5 at 1,200 ticks) except at
  the island's light manufacturing, whose price is not determined: with no output and no orders
  any price that leaves its weight below the market floor solves its market.
- Python (pytest, hypothesis): ICIO aggregation preserves totals; downscaling preserves national
  VA; imputation flags; GE reference residuals; reference versus balanced Sinkhorn on small
  worlds; PPML recovers a known elasticity on synthetic gravity data; the uniqueness check
  reproduces the paper's worked examples (footnotes 3 and 6, Section 3 with Figure 1, Online
  Appendix A.3: `test_bound_reproduces_paper_examples`), is below 1 at the P2 defaults
  (`test_trade_only_is_unique`) and picks the V1 mode (`test_v1_mode`); the reference's PIGL
  reproduces Rust's on the export's 100 `pigl_check` draws (1e-12) and its inversion the base
  shares.
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
- Stability on the full pack: on the fixture under origin clearing, PIGL demand and the same-tick
  chain small deviations decay by 0.9964 (frictionless) and 0.9968 (default) per tick at the P2 defaults (kappa1 0.05, kappa2
  0, expectation gain 0.006; trade SPEC, Stability region and P03a, P03b, P45). The slowest
  mode is the world price level, which expected inflation turns into a damped oscillation
  (trade SPEC, Nominal mode). Re-measure on the pack in Task 20.
- The run from the data state passes through a deep coal-led shortage before it converges (the
  equilibrium's coal output is about twice base output and intermediate demand is inelastic in
  units); V1 holds on the fixture at 1,200 ticks.
- PIGL aggregation across pops: the inversion matches the 2000 shares for one lognormal at the
  country's mean income per person; the country's pops differ in mean income (education,
  province), so the aggregate data-state shares differ from Omega by up to 0.0018 on the fixture
  (food 0.118 for 0.120). Matching the aggregate exactly would need a per-country solve over
  its pops.
- Same-tick chain in cycles: two local goods that each take most of the other's output converge
  slowly (geometrically, at a loop gain near 1), and at the bound the chain clears the rest by
  cutting the users that produce no local good rather than all users pro rata; on the fixture it
  never reaches the bound. Measure on the pack (Task 19) and revisit the bound or the solve there.
- Frictionless clearing wage: each labour target is capped at L^s, so where one enterprise's
  target reaches L^s, capped demand equals L^s on a whole interval of low wages; the search
  then finds no sign change and the wage is indeterminate below the true root. Searching on
  uncapped targets (capping only the allocation) would follow the closed form. No fixture
  province is affected.
