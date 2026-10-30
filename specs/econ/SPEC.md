# Econ SPEC

Status: approved (owner, 2026-10-23); P2 gate passed (owner, 2026-10-26, p2-econ c9b6e11). V1 from the
perturbed steady state 9.085e-3 (the run from the shocked data state is an `xfail(strict)` stress test);
H2 distance -0.753 (s.e. 0.016), H2 per class 1.75, 3.72 and 9.52, all in their bands; 100 years without
violation; 1 vs 8 threads identical; tick about 34 ms and `source_dispatch` 7.9 ms against the re-budgeted
35 ms and 8 ms. Numbers and commands: P2 gate, Measured
Phase: P2
Design sections: 3.2, 3.3, 3.4, 4.2, 4.4, 4.6, 6.1, 7.1, 7.2, 7.3, 8.6, 8.7, 9.1, 15.3 (V0, V1), 16, 17, 18 (H1, H2), 19.2, 19.4, 20, 21.1 (P2); ADRs 0012, 0014 to 0016, 0018 to 0023, 0089, 0112 and 0264

## Purpose

`ceteris-econ` owns the minimal economic core of P2: static pops, one enterprise per
(province, sector), the province labour market, final demand, income distribution and a single
numeraire money. It assembles the P2 world (`EconWorld`) and its tick schedule from
`ceteris-core`, `ceteris-geo` and `ceteris-trade` (`specs/trade/SPEC.md`). This spec also
covers the P2 additions to the pack pipeline (2000 economic data, gateways), the GE reference
model in `ceteris-reference`, the P2 gate, and what the app shows. Demography,
investment, finance, the state and politics are later phases.

## Deviations from DESIGN.md (approved with the spec)

| Design | P2 | Why |
|---|---|---|
| Enterprises set output from demand forecasts and inventory coverage (7.3) | Price-taking enterprises move employment toward the profit-maximizing level at expected (current) prices; inventory coverage lives with the province market (trade SPEC) | Province markets absorb all output at the posted price, so firms face prices, not demand curves; this rule nests the GE optimum exactly. The forecast rule can return as a switch when firms get pricing power |
| PIGL over five categories: food, energy, housing, manufactures, services (8.6) | PIGL over the three categories the form was estimated on, A (goods 0, 3), M (goods 1, 2, 4, 5, 6), S (goods 7, 8, 9), with a CES over goods within each (ADR 0021, controller ruling 2026-10-24) | Alder, Boppart and Mueller (2022) estimate agriculture, manufacturing and services only; crude oil, coal and utilities have no clean counterpart there and are placed by the nearest category |
| Intermediates Leontief plus a CES energy nest (7.2) | All intermediates Leontief | The energy nest comes with energy and emissions (P8) |
| Putty-clay intermediates: coefficients per capacity vintage, chosen at investment by a CES at P38, Leontief once installed (7.2, ADR 0020) | Each enterprise runs on its own coefficient vector (`EntData.coef`, `Layout.coef`), one vintage: its country's 2000 ICIO column as cost shares at base delivered prices (row above), filled at load (`EconInputs::set_coefficients`). S09 `Technology::FixedLeontief` reads that base column (`EconInputs::base_column`) instead. With one vintage both settings read the same numbers and run bit for bit alike, state hashes included: the switch enters the static digest only once some enterprise's coefficients differ from its country's (`putty_clay_one_vintage_equals_fixed_leontief`) | No investment in P2, so no vintage beyond the first; the vintage choice and P38 arrive with investment (7.4) |
| Input-output coefficients a_kg as quantities (7.2) | ICIO coefficients are cost shares at base delivered prices: coef_e,k = a_kg / q_k(base), q_k(base) the base delivered unit price of k at the enterprise's province (`ProvinceData::base_price`: 1 for local goods, sourcing at prices 1 for traded goods, tariffs and iceberg included; coefficient 0 where unreachable), under both S09 settings (owner decision, 2026-10-25; ADR 0020 mapping) | At base prices every enterprise's input cost share equals the ICIO share, a_VA = 1 - sum_k a_kg > 0, and the national IO value structure is exact; read as quantities, delivered prices above 1 made inputs cost more than output for 11.5% of traded VA at prices 1 |
| Within-tick rationing, inventories as the buffer (8.2, 19.4, ADR 0020) | Local goods (utilities, construction, services): a same-tick chain at E2 per province (`local.rs`, Production below): a shortage of a local good cuts its users' output in the tick it happens. Traded goods keep their stocks and transit lags. Departures from the migration brief's chain, each needed for exact Leontief use: (1) a producer's own use of its good comes out of its output first, since cutting the producer pro rata with its users leaves the ratio of output to need unchanged and with empty stocks drives the pair to zero; (2) each user covers its residue from its own stock (the ledger holds stocks per enterprise), so the stock that counts for a good is sum min(b, n), and output is split so that each user's residue fits its stock; (3) one fill per local good, every user of the good at the smallest fill of its local inputs, solved exactly as the limit of the fill iteration (ADR 0093, Amendment), with no pass bound and no clearing step, so input use always equals coef Y exactly | ADR 0020 Mapping (owner, 2026-10-24): local goods have no transit, so their production, input use and rationing run as one pass within the tick |
| VA over K, effective L by education, land or resources T (7.2) | Normalized CES over one fixed factor F (capital, land and resources together) and effective labour | No investment in P2, so K and T never move separately; one fixed factor keeps the reference model small |
| Pops keyed by province, ethnic group, education, labour status (6.1) | Province x education (3 levels); labour status as employed and unemployed counts inside each pop | Ethnicity and status transitions are P4 |
| Wage posting and matching function (8.7) | Wage moves with excess labour demand, with downward rigidity; employment rationed pro rata | Matching and occupational mobility are P4 |
| Explicit fiat money through banks (A9) | Numeraire-only mode (A9 alternative); every account may run a negative balance at an interest-free clearing facility | No banks before P5 |
| Final demand split into households, government, investment | One final demander per pop, with country shares from all ICIO final demand | No state or investment in P2 |
| Observed 2000 trade imbalances | Balanced trade: pops spend their income each month | Deficits need finance (P5) |
| Agglomeration in productivity (A5, P09) | Off by default | Ablation rule: on only once it improves a test |
| Buyers face posted prices (8.6, 7.3) | Traded goods are bought at the buyer price q_d = Gamma_d P_d of the buyer's province, the CES index times a selection constant per cell fixed at the calibration point (trade SPEC, Sourcing; ADR 0200, owner ruling 2026-10-30), in household demand, input orders and the value-added price | Destination sourcing gives no single posted price at a destination; with selection on trade links (Eaton-Kortum) every source delivers at the index, and Gamma keeps q_d at the calibrated point where 1 / alpha_d put it before ADR 0200 (owner decision, 2026-10-24) |
| Inventory coverage at the market (7.3) | Enterprises order inputs up to an inventory position (stock plus goods on order) of (1 + cover + L) months of capacity use, cover = 1, L the expected months in transit (owner decision, 2026-10-24) | Goods paid at dispatch and delivered after transit must be counted, and the target must hold them: a two-month position cannot hold a month in transit |

## Goods (energy-forward, owner choice)

Aggregated from the ICIO 2025 edition industries. If the edition splits coal (B05) from oil and
gas (B06), use the split; otherwise split B05_06 by EIA 2000 production values at Pink Sheet
prices, per country.

| ID | Good | ICIO industries | Traded between provinces |
|---|---|---|---|
| 0 | Agriculture | A01_02, A03 | yes |
| 1 | Crude oil, gas and refined fuels | B06 (or share of B05_06), B09, C19 (ADR 0092) | yes |
| 2 | Coal and other mining | B05 (or share of B05_06), B07_08 | yes |
| 3 | Food | C10T12 | yes |
| 4 | Light manufacturing | C13T15, C16, C17_18, C31T33 | yes |
| 5 | Chemicals and metals | C20, C21, C22, C23, C24 (all splits), C25 | yes |
| 6 | Machinery, electronics and vehicles | C26, C27, C28, C29, C30 (all splits) | yes |
| 7 | Electricity and utilities | D, E | no (P2) |
| 8 | Construction | F | no (P2) |
| 9 | Services | G to T | no (P2) |

## Pack additions (pack format 3)

Built by the pipeline after the P1 tables; same rules as P1 (pinned inputs in `fetch.toml`,
licences in `sources.toml`, byte-identical rebuilds, `tests/golden/p2-pack.txt`).

| Table | Rows | Content | Sources |
|---|---|---|---|
| `econ_goods.arrow` | 10 | name, traded flag, weight per value w_g (t per unit), inventory coverage prior | BACI HS92 2000 (tonnes and values, OECD BTiGE HS to CPA key) |
| `econ_countries.arrow` | 242 | ICIO economy or imputation donor, VA basis and income source, national VA by good (10), final demand shares (10), labour share, unemployment rate, natural rate u* (mean unemployment rate 1995 to 2005, ADR 0089), participation rate, working-age share, education shares (3), GDP 2000, Gini 2000, coal share of good 2; an imputed flag per series | ICIO, PWT 11 (labsh), ILOSTAT (modelled estimates: 2000 rates, and 1995 to 2005 for u*), WIC reconstruction (Barro-Lee cross-check), WDI; Gini: WDI SI.POV.GINI nearest year 1995 to 2005, else SWIID (disposable-income Gini, CC0; WIID is non-commercial) |
| `econ_io.arrow` | countries x 10 x 10 | input coefficient a_kg (units of k per unit of g) | ICIO 2000, aggregated |
| `econ_provinces.arrow` | 4,796 | population 2000, working-age population, VA by good (10, million USD per year), with an imputed flag and the proxy level per good | WorldPop 2000 aggregated through the H3 cells, scaled to WDI national totals; VA downscaled as below |
| `econ_pops.arrow` | 4,796 x 3 | province, education level, population, working-age population, labour force, imputed flag | provinces split by their country's education shares; labour force at the country's participation rate of the working-age population |
| `econ_tariffs.arrow` | importer x exporter x 2 groups | applied tariff, primary and manufactured; agreement and imputed flags | WDI weighted applied rates by importer (TM.TAX.TCOM.WM.AR.ZS, TM.TAX.MANF.WM.AR.ZS); zero inside agreements and customs unions (CEPII gravity, WB DTA), as below |
| `gateways.arrow` | about 600 | node, country, kind (port, airport, land crossing), mass (share of the country's trade throughput, ADR 0092) | P1 graph, BACI 2000 trade by country |
| `econ_home_shares.arrow` | countries x 7 traded goods | country, good, home_share (ICIO 2000 domestic-origin use over total use, intermediate plus final demand without DPABR), absorption (total use, million USD), icio (an ICIO economy; outside ICIO the share and absorption are NaN, as is the share of a good an ICIO economy does not use) | ICIO 2000 (ADR 0092) |

**Coverage.** `TradeParams::coverage` (P02, trade SPEC) is the authoritative value: the model, the
snapshot digest and the export (`goods.arrow`, which the GE reference reads) all use it. The
`econ_goods.arrow` column is the pack's copy of the default, never read by Rust; it must equal
`TradeParams::default().coverage`, and a change to one is made to both.

**Value added convention.** National VA by good is output less intermediate inputs, summed over
all sellers: ICIO value added at basic prices plus its taxes less subsidies on products. Hence
`1 - sum_k a_kg` is the VA share of output in every column of `econ_io.arrow`, ICIO and imputed
alike, and VA + inputs = output holds exactly (the identity the GE reference model relies on).

**Downscaling VA to provinces (traded goods only).** National VA by good from ICIO, then
province shares from the best proxy available: agriculture by MapSPAM 2000 physical area of all
crops (cropland; SPAM2000 has no value of production and SPAM2005's licence is non-commercial);
oil and gas in the United States by EIA 2000 state production (SEDS, crude oil and marketed gas
by heat content, federal offshore to the nearest coastal provinces by GEM fields), elsewhere by
GEM extraction assets, flagged imputed in a country where assets with no start or discovery year
carry more than half of its GEM weight; coal and other mining by GEM coal mines and P1 mineral
sites; manufacturing goods by DOSE manufacturing GVA where a DOSE region matches a province by
ISO 3166-2 code or exact name within its country (never through GADM geometry), else by Kummu
gridded GDP, which serves the manufacturing chain only. Each fallback falls back to population.
Local goods get a base point only: national VA split by population (their equilibrium output
follows local demand). Provinces with fewer than 1,000 working-age people get no VA; theirs is
reallocated within the country by the same proxy, so every enterprise has workers to hire. Every row carries an imputation flag.

**Economies outside ICIO.** IO coefficients and final demand shares from the ICIO economy
nearest in (region, log GDP per capita); national VA by good from WDI sector shares
(agriculture, industry, manufacturing, services) split within each group by the donor's
proportions. Agent default under 22.2, ADR 0014. Donor income is PWT GDP per person, else WDI
(ADB Key Indicators where WDI has none) GDP per person, else Maddison Project GDP per person,
always against the donors' in the same source, never the region median when an income exists. With manufacturing missing, industry goes over goods 1 to 8. With
agriculture, industry or services missing, the known shares keep their goods and the goods they
do not cover take WDI GDP times the ICIO median VA over GDP less the known shares (at least zero),
by the donor's proportions; with no share, GDP times that median. Where WDI has oil or natural gas
rents, crude oil and gas takes GDP times the rents times the ICIO ratio of crude oil and gas VA to
rents, out of its group (all of industry where industry less manufacturing is too small) and at
most the largest ICIO share of crude oil and gas in industry VA of it (ADR 0249), with the ICIO
world crude oil and gas inputs per unit of output, weighted by output, in place of the donor's
(ADR 0256); with no GDP, population (WorldPop national
total if WDI has none) times the donor's VA per person scaled by income. A territory inside its
parent's WDI totals (Somaliland, Aland, Northern Cyprus) takes its population share of the
parent's VA out of the parent's. Uninhabited territories keep zero VA. Each row carries the
basis (`icio`, `wdi`, `wdi_gdp`, `donor`, `parent`, `none`) and the income source.

**Gateways.** Per country: k gateways, the k-medoids on great-circle distance of its
candidates: its listed ports and airports and its land crossing points, the endpoints of graph
edges (road, rail) whose two ends lie in different 2000 countries, merged within 50 km of
midpoint per country pair. k is proportional to the square root of 2000 trade (BACI exports
plus imports), at least one and at most the candidate count, scaled to about 600 in total; each
gateway's kind is its candidate's. Agent default under 22.2, set by benchmark (open questions).
Each gateway's mass (ADR 0092) is its share of its country's 2000 trade throughput; the pack's
sources carry no throughput per gateway (Natural Earth ports and OurAirports airports list no
tonnage, value or passengers, and land crossings no traffic), so every gateway of a country with
k gateways has mass 1/k (`gateway_mass_sums_to_one`, 1e-12). A pack built before the column is
refused at load with an error naming it.

**Tariffs.** The importer's WDI weighted mean applied rate of each group (primary goods 0 to 3,
manufactures 4 to 6), the year nearest 2000 within five years (a year at 100% or more is a data
error and counts as missing), applies to every exporter outside its agreements. Zero inside a goods free trade agreement or customs union in force on 1 January
2000: CEPII gravity pairs flagged (fta_wto, rta_type CU or FTA, with or without an EIA) in both
1999 and 2000, or World Bank DTA pairs listed for 2000 and earlier, or for 2000 under an
agreement that entered into force on 1 January 2000. Partial scope and services-only agreements
keep the rate. A dependency trades free of tariffs with its sovereign. An importer without a WDI
rate takes its sovereign's, else its donor's (same region, nearest log income), flagged imputed.

**Loading (`EconInputs::from_pack`).** The economy's provinces are the pack provinces with value
added and a labour force; the others (below the working-age floor, or uninhabited) stay in the
transport graph only, and `ProvinceData::pack` maps each economy province to its pack province.
Enterprises are the (province, good) cells with VA > 0, VA0 the pack VA per month (USD); a pack
province with VA but no labour force, or a country with enterprises but no pops, stops the load
(`PackError`). Leg costs
are least-cost paths on the graph, run in parallel from every province and gateway, each keeping
only its own country's provinces and gateways (from a province) or other countries' gateways
(from a gateway): domestic, access and crossing legs, all gateway pairs of different countries.
Where a theta class has a bulk freight rate (P154, P155, ADR 0247), the same runs on a copy of
the graph repriced at that rate give the bulk legs (`Network.bulk`: same endpoints, the bulk
paths' cost per tonne and hours), which the class's goods use; the export's `legs` table carries
them as `bulk_cost_per_t` and `bulk_hours` (NaN without bulk legs; export format 12).
`network_from_graph` is that step alone, and `EconWorld::set_network` installs a rebuilt
network (system T0 of the trade SPEC; the caller triggers it in P2, shocks arrive in P3).

**Calibration tables** (not built by the pack pipeline; written by
`ceteris calibrate --pack DIR`, Calibration below), keyed by pack province:
`econ_calib.arrow` per enterprise (A, VA0, L0, a_VA, own price, output, sales, profit, coef and
input stock per good; the summary in its `calibration` metadata), `econ_calib_cells.arrow` per
(province, good) (posted and buyer prices, the selection constant Gamma of ADR 0200, lead, Q, D by
buyer class, sales, merchant stock, and the own-price elasticity E and supply elasticity eps of
the market's price loop, ADR 0263, trade SPEC, Per-market gain),
`econ_calib_provinces.arrow` (w0, employment, income), `econ_calib_pops.arrow` per (province,
education) (money at the start of a tick, income, wage), `econ_calib_pipeline.arrow` (goods in
flight by buyer, good and ticks ahead) and `econ_calib_countries.arrow` (u*, rebate, PIGL
weights). Every table carries `calibration_format` in its schema metadata. Installed into the
pack (copied next to the econ tables; their sha256 then joins `tests/golden/p2-pack.txt`), they are
read by `EconInputs::from_pack` (`EconInputs::calibrate`, `calibration.rs`) into A, VA0, L0,
a_VA, the coefficients (both S09 settings), w0, the PIGL weights, E and eps per cell
(`EconInputs::market_elasticity`, in the inputs digest) and the initial state (State
components): only at `calibration_format` 6 (5 since ADR 0200 added `gamma`, 6 since ADR 0263
added E and eps) and only when the
`static_digest` in the `calibration` metadata is that of the uncalibrated inputs under default
parameters (`EconInputs::from_pack_data`, which `ceteris export` writes), so tables of another pack
build, another format or other coefficients stop the load with a `PackError`.

## State components

| Component | Entity | Type | Units | Initialized from |
|---|---|---|---|---|
| Population, working-age, labour force | pop (province, education) | `f64` | persons | econ pack; static in P2 |
| Employed | pop | `f64` | persons | (1 - u*) x labour force (natural rate, ADR 0089) |
| Ownership share | pop | `f64` | share of its country's profit pool | population share N_k / N_c (national ownership) |
| Income last tick | pop | `f64` | numeraire | |
| Sector, productivity A, base VA0, base employment L0, labour share s_L | enterprise (province, sector) | `f64` | | calibration table |
| Employment L | enterprise | `f64` | effective labour units | base L0 |
| Output, sales, profit last tick | enterprise | `f64` | | |
| Input coefficients coef_k (putty-clay, ADR 0020; static in P2) | enterprise x 10 goods | `f64` | units per unit of output | one vintage, the 2000 column `EntData::column`: calibrated a_kg p_o / q_ik (calibration tables), else the country's ICIO column over base delivered prices, a_kg / q_k(base) |
| VA coefficient a_VA | enterprise | `f64` | real VA per unit of output | calibrated 1 - sum_k a_kg, else 1 - sum_k coef_k q_k(base) |
| Input stock | enterprise x 10 goods | `f64`, held in the ledger | units | calibrated (1 + COVER) months of use, else one month of base input use |
| Wage w | province | `f64` | numeraire per effective unit | base w0 = sum_g s_L VA0_ig / ((1 - u*_i) L^s_i) |
| Wage clip ticks: ticks at which the wage step clip bound | province | `u32` | ticks | zero |
| Effective labour supply L^s | province | `f64` | effective units | sum_k e_k LF_k |
| Accounts | pops, enterprises, markets, gateways, transit (per traded good), rebate and profit pool (one each per country), mint | ledger | money in minor units (1 minor unit = 1e-4 numeraire): `i64` for pops and enterprises, `i128` for markets, gateways, rebate and profit pools and the mint (ADR 0106); goods `f64` | |

**Initial state.** A world whose inputs carry the calibration tables (`EconInputs::calibrate`,
`EconInputs::from_pack` when the pack holds them) starts from the calibrated steady state of ADR
0089 (Calibration, stage 5): its posted prices, last D, Q and sales, merchant and input stocks,
the pipeline, each pop's money (dividends and rebates; wages come at E2) and last income, each
enterprise's last output, sales and profit; wages w0, employment L0, unemployment u*, pi^e = 0
and buyer prices refreshed at the posted prices. Otherwise it starts from the data state: prices
1, the same base point, input stocks at one month of base use, merchant stocks at coverage times
initial demand, nothing in flight and one month of base dividends minted to each pop.

Enterprises exist for every (province, traded good) with VA > 0 and every (province, local
good). About 4,796 x 10 rows at most, sorted by province (CSR offsets, 19.4). Every pop and
enterprise belongs to one province; per-province work is a contiguous slice.

## Equations

**Production.** For enterprise e = (i, g) with input coefficients coef_k (`Layout.coef`: its
putty-clay coefficients, one vintage in P2, or its 2000 column under S09 fixed Leontief; both the
same numbers in P2) and VA coefficient a_VA (`EntData::a_va`: 1 - sum_k a_kg, the cost shares at
the calibrated prices, for a calibrated world; 1 - sum_k coef_k q_k(base) for the base column):

```
Y  = min( VA / a_VA,  min_k M_k / coef_k )                       (Leontief)
VA = A * VA0 * [ (1 - s_L) + s_L (L/L0)^rho ]^(1/rho)              (normalized CES, F fixed at F0)
rho = (sigma_FL - 1) / sigma_FL                                    (P08)
```

sigma_FL = 1 uses the Cobb-Douglas limit VA = A VA0 (L/L0)^s_L. Traded inputs M_k come from the
enterprise's input stock, bought at earlier ticks and delivered after transit; a shortfall caps Y
in the tick the stock runs out, so a traded shortage reaches its users with their stocks and
transit as the lag. Units are delivered units (ADR 0200): an origin's output leaves in shipped units
at coef_k per unit, and on route r each shipped unit arrives as s_r delivered units (trade SPEC,
Sourcing), so the origin's best producers, which make what d buys from it, use 1 / s_r of the
cell's average inputs per delivered unit. The production equations do not change; the dispatch
books the difference as the selection term. Local inputs (goods 7 to 9) come from this tick's local
output first and the enterprise's stock second, in the same-tick chain (ADR 0020, `local.rs`), run
per province:

```
cap_e = min(VA / a_VA, min over traded k of M_k / coef_k)     every enterprise of the province
one fill F_k in [0, 1] per local good k (ADR 0093, Amendment):
    Y_e  = cap_e min over local k used by e (its own good left out) of F_k   (cap_e if none)
    R_k  = (1 - coef_kk) Y_p                                    p the producer of k: own use first
    U_k  = sum_e max(coef_ek Y_e - b_ek, 0)                     b_ek = e's stock of k
    k fits when U_k <= R_k
F = the limit of the fill iteration: from F = 1, passes over k = 7, 8, 9, each setting
    F_k = largest x <= F_k at which k fits, the other fills as they stand
solved exactly: each step is the root of a piecewise-linear U_k on its piece; a fill below the
smallest normal f64 is 0; on its pieces a pass, and a cycle of up to three passes, is an affine
map of the fills, and the passes jump to its fixed point where repeating it stays on the pieces,
or ahead to the end of the last whole cycle on them, so that the next pass crosses a kink;
passes stop when one changes nothing
bounded work (ADR 0093, second amendment): a jump needs a resolvable fall (above 4,096 eps f per
cycle), follows at most 64 cycles one at a time, then 2^k cycles at once (k <= 64) checked at
those points, the shortages strictly, and where the passes leave the pieces between two checks,
bisects the cycles between them (2^i cycles added for i = k - 2 down to 0 where the cycle from
there stays on the pieces) and takes one whole cycle more; all jumps share 4,096 units of work
per province and tick;
a pass whose falls are all below the margin, or the 256th pass, moves the goods resolvably to
the fixed point of the last pass's map (or, at a stop, of the last two or three passes' cycle
within 9.3e-10 of f), inside [0, f] and confirmed by the cycle's passes; the last pass's fixed
point beyond 9.3e-10 of f only where it lies on that pass's pieces (every step's piece holds
there, as the jumps check it, and every good the pass leaves fits to 1e-12 of its flows), so
that the passes run on them from f to it to first order (a threshold on one fill holding at f
and g holds along a monotone fall; the passes follow g + M^k (f - g), the segment only along
one near-unit direction, and comparisons between two fills are not kept by a fall but hold
trivially at ties); where it is refused, the goods whose fixed point lies within 9.3e-10 of
f still move there, if the pass confirms it, and only then is a stop's cycle of two or three
passes tried; else stops (after a
rounding pass, every good fitting to 1e-12 of its flows) or reports the stall and goes on; a
fall below the margin is no fall whatever its source, so a real loop of gain within about
9e-13 of 1 is not followed (gain 1 - 8e-13 stays at [0.4999999999992, 0.9999999999984, 1])
pass cap: no call runs more than 1,024 passes, a jump's confirming passes included: at 1,021
(the cap less a jump's up to 3) the fills are kept where every good fits to 1e-12 of its flows,
else the goods that do not are set to 0 one at a time in good order; the stall is reported
degenerate limits (U = R at the limit): rounding of U - R decides the fit near the limit; the
fills land within 1.81e-12 of it on the translated collapses (tests cap 1e-11; the per-case
eps (U + R) / |slope| has no bound where a good's U - R is about 0 along the ray); a last pass
that sits on pieces reaching only just below the limit has a fixed point far below it, off its
pieces, which is not taken (seed 13067305529229119429 translated to 0.3 ended at
[0.267, 0, 0] against t when it was;
`translated_collapse_stop_refuses_an_off_piece_fixed_point`)
draws: with output enough, every user draws n_ek = coef_ek Y_e from output; short, each draws
max(n_ek - b_ek, 0) plus the rest of R_k pro rata to min(b_ek, n_ek), its stock the remainder
```

Y never rises above cap. Every good fits at the fills as evaluated in floating point (each step
lowers its root to the largest f64 at which it does: 8 units in the last place one at a time,
then a bisection of the bit patterns of [0, x], at most 64 evaluations, which gives the same
fill since the evaluated fit is monotone in it; ADR 0093, amendment of 2026-10-30), except where
the passes stop at what f64 resolves: there a good may be short by rounding, at most about
9e-13 of its flows, which the split shares pro rata within the Leontief invariant's allowance
for local draws (Invariants). That holds by construction: each good fit
at its own step in the last pass, its R fell later in the pass by at most the relative fall of
its producer's binding fill (below the margin), and other falls only lower its U. No clearing
step is needed. The draws are
paid at the posted price at E2, count in the market's demand and sales, and come before
household demand under every S11 setting (the chain runs before E3); at T3 the rest of local output is shared between enterprise orders (stock refills) and households by
S11, and what is left perishes. Use equals coef Y exactly. On the fixture, from the data state over
1,200 ticks, the chain never binds in the default or the frictionless run; with services feeding
services at 0.3 and construction and services each other at 0.2 it binds in 415 of 3,000 province
ticks over 120 ticks, each within the passes its kinks allow, without a stall
(`local_chain_terminates_on_cycles`). Where every jump applies (a resolvable fall per cycle, on
pieces that repeat within 3 passes) the passes are at most 2 + 3 + 2K: one from F = 1, which
cuts only the goods short there; at most two per kink the fall crosses (the pass across it, and
one more where that pass straddles it so that its map does not repeat), since each jump from a
cycle on its pieces ends at the limit or after the last whole cycle before the pieces change;
and at the limit up to 3 confirming passes and the pass that changes nothing. K counts the
users' stock thresholds inside (0, 1) (b < coef cap; each crossed at most once, as needs only
fall), each local good beyond the first that starts to fall, and each pair of local goods some
user takes both of (a switch of the binding fill, taken once per pair: an assumption, not
derived; a pair can switch more than once, and the allowance of two passes per kink has held
those seen). The most is 6 passes,
province 5 at tick 79, against a bound of 21: utilities and construction, inputs of each other
with no stocks but residues from earlier collapses, fall at a loop gain of 0.7 to 0.9 per pass
to 0 across the residue thresholds. Before a jump ahead found the last whole cycle (it ended at
the doubling's last check), it could stop short of a kink and cost passes no kink accounts for:
11 passes there at 13410ce, 7 for 4 on a fall of gain 0.995 to a kink 1,500 cycles on
(`a_jump_ahead_ends_after_the_last_whole_cycle_on_the_pieces`; neither the fixture bound nor
the property below tells that defect apart). Where a jump does not apply, the fills report the
passes outside the accounting (`Fills::uncovered`): a pass after which no jump moved and one did
not apply (map undefined or singular, fall within rounding, budget spent); two for a pass that
cuts a good by rounding alone while another falls resolvably (it changes the pieces where no
kink lies, and ends the previous pass's jump); a pass a stop moves on from; at the stall rule
every pass since the last move, and every pass after a stall. Passes <= 2 + 3 + 2K + uncovered
on random small provinces of five families (the first below, higher coefficients, no stocks, up
to 10 enterprises, scarce stocks; `chain_passes_within_their_kinks`), and on all of 3,000,000
probed. Of 1,000,000 (200,000 per family), 22 exceed 2 + 3 + 2K
(`chain_passes_beyond_their_kinks_are_uncovered`; the 21 of the jump fix plus one that stalls):
10 cut a good by rounding every other pass (it sits at its limit with U = R along the rest of
the fall: 88 passes against 25, with 39 such cuts), 11 end at the stall rule (pieces repeating
over 4 or more passes, or binding fills switching every pass; one stalls and runs 792 passes),
and 1 lands a step's root on a kink every other pass (map undefined: 14 against 11). None
exceeds it where every jump applies. About 11% of the provinces have an uncovered pass; those
without are at least 1 pass inside the bound. On 1,000,000 random small
provinces with coefficients up to 0.6, before that change, all but 129 settled within 12 passes,
the most took 257 and none more than 0.3 ms (on 200,000 after it, all but 1, at 257); over five
other families of 1,000,000 (higher coefficients, no stocks, up to 10 enterprises, scarce stocks,
near-balanced markets) 7 provinces stall, collapses crawling towards the smallest normal f64,
and take up to the pass cap of 1,024 that bounds every call. In 200,000 provinces of the calibrated form (markets balanced at F = 1, one capacity
cut) the fills match the plain iteration to 1.4e-10 within 2 passes, the iteration itself
drifting by rounding over thousands of passes (its error, not the fills'). Each tick the enterprise orders up to an inventory position (stock plus goods on order, the paid shipments in its pipeline):

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
buyer price q_k (ADR 0200), spend = order_k q_k, so the units ordered are the units that arrive. At
a steady state inputs do not bind, Y_cap = Y, local stocks stay at their target and their orders are
zero, so the steady state (and the GE reference) is that of the lagged rule.

**Labour target.** With value-added price p_VA = (p_ig - sum_k coef_k q_ik) / a_VA, the own
(origin or province market) price at current prices and the buyer prices q_ik of last tick's
E3 (the buyer price of traded inputs, the posted price of local ones), the profit-maximizing
employment solves p_VA dVA/dL = w_i, in closed form:

```
q  = w L0 / (p_VA A VA0 s_L)
L* = L0 * ( (q^(rho/(1-rho)) - s_L) / (1 - s_L) )^(-1/rho)
```

L* = 0 if p_VA <= 0. When the bracket is not positive the target is a corner: zero if rho < 0,
the province's labour supply if rho > 0. Cobb-Douglas: L* = L0 q^(1/(s_L - 1)). A unit test
checks the closed form against numerical maximization. Employment moves toward the target:
L_t = L_{t-1} + eta (L* - L_{t-1}) (P41). If sum_g L_t exceeds L^s_i, all are scaled down pro
rata.

**Wages.** Excess labour demand x_i = (sum_g L*_ig - L^s_i) / L^s_i, read against the natural
rate u*_i (below), expected inflation pi^e (trade SPEC, Origin market; ADR 0019):

```
dlnw = pi^e + clip( kappa_w (x_i + u*_i), -c_max, c_max ),  times (1 - delta_w) when negative    (P39, P40)
```

Unit pass-through of pi^e is a homogeneity restriction, not a parameter; downward rigidity
applies to nominal cuts, so a real cut that is a nominal rise is not damped. pi^e tracks the
world price index over buyer prices with base consumption weights; cells whose buyer price is
infinite (no reachable origin) are left out of the index (owner ruling, MIGRATION Task 6), so a
change in reachability moves its weights. c_max is P03c; it bounds the excess-demand term only,
so expected inflation passes through unclipped (ADR 0106), and the rigidity applies after the clip
to the whole step when it is a nominal cut. Every tick the clip binds is counted per province
(`clip_ticks` in `run_wages`).

Frictionless mode (V1): w_i solves sum_g L*_ig(w) = (1 - u*_i) L^s_i exactly on the uncapped
targets (monotone in w; safeguarded Newton in ln w; ADR 0093), so the wage follows the closed
form where one target alone reaches L^s; only the allocation caps (sum L_ig <= L^s), and eta = 1,
so frictionless unemployment is u*. Where demand never meets supply (every p_VA <= 0, say), the
wage moves at most a doubling or halving per tick
(`frictionless_wage_matches_closed_form_when_one_target_reaches_supply`). The unemployment
rate u_i = 1 - sum_g L_ig / L^s_i applies equally to each education level.

Known defect (owner decision pending): E1 hires labour it cannot equip. Frictionless, a target
valued at the posted price of an origin with no output can take a province's whole labour, paid
from debt, until a wage payment overflows the ledger (ignored test
`frictionless_labour_spiral_overflows_the_ledger`). Proposed fix: cap each labour target at the
labour its inputs allow.

**Natural rate (ADR 0089; supersedes the full-employment owner choice).** L^s stays the full
effective labour force, and the wage rule reads excess demand against the country's natural rate
u*_i, its mean ILOSTAT modelled unemployment rate 1995 to 2005 (`econ_countries.u_star`), so the
2000 cycle is not frozen in:

```
dlnw = pi^e + clip( kappa_w (x_i + u*_i), -c_max, c_max ),  times (1 - delta_w) when negative
```

At the calibrated state x_i = -u*_i and the wage step is pi^e = 0, so steady-state unemployment
is u*, not zero; employment may exceed (1 - u*) L^s in a boom, up to L^s. Frictionless mode
solves sum_g L*_ig(w) = (1 - u*_i) L^s_i. The base wage uses u*: w0_i = sum_g s_L VA0_ig / ((1 -
u*_i) L^s_i), and the data state starts at unemployment u* (a generated world, which has no 1995
to 2005 window, takes its 2000 rate as u*). P4 matching replaces u* with matching frictions.

**Income (national profit ownership, owner 2026-10-25).** Labour income stays local; profits
and rebates are national, paid per person. Pop k of country c in province i receives, per tick:

```
income_k = w_i * e_k * employed_k                  (wages, local)
         + N_k / N_c * sum_(e in c) profit_e       (profits, national)
         + N_k / N_c * rebate_c                    (tariffs, national)
```

employed_k = LF_k L_i / L^s_i is set by E1 (LF_k the pop's labour force, L_i = sum_g L_ig the
province's employment, L^s_i its labour supply), so sum_(k in i) e_k employed_k = L_i exactly and
the wage term is the pop's share of the province's wage bill w_i L_i. N_k the pop's population,
N_c its country's pops' population, profit_e = sales - wages - purchases of enterprise e
plus the losses it carries, paid only when positive (limited liability, owner 2026-10-26: a
loss stays with the enterprise as retained losses and later cash flow repays it before any
payout, so enterprises pay no negative dividend), and in the profit pool each province market's
signed cash flow (trade SPEC), so the pool is sum over the country's enterprises and markets.
Markets keep the signed rule (ADR 0112, Markets). Retained losses beyond M months of base sales
are written off against the country's pops (loss write-off, ADR 0112, below). Shares are fixed
at load. At a steady state profit_e = (1 - s_L) VA0_e > 0, no loss is carried, nothing is
written off, and market cash flow is 0. A province without traded output so
earns dividends and buys local goods (it had only its rebate share under provincial ownership,
Task 18e). The base income of a province, rebates aside (`world::base_income`,
`io.Inputs.base_income`), is w0 (1 - u*) L^s + N_i / N_c sum_(e in c) (1 - s_L) VA0_e, N_i the
population of its pops. A pack country with enterprises but no pops stops the load (`PackError`):
nobody would own its profits.

*Rounding (E4).* Every enterprise pays its balance when positive, and every province market its
signed cash flow, in minor units, into its country's profit pool account; an enterprise with a
negative balance pays nothing and keeps it. Each country then pays its pool, one i128 sum per tick,
and its rebate account to its pops in pop ID order by cumulative population shares rounded half
to even: part_j = round(P S_j / S) - round(P S_(j-1) / S), S_j the population of the first j
pops, the last pop taking P - round(P S_(n-1) / S). The parts add up to the pool exactly, so the
pool, the rebate account and every market end the tick at zero, every enterprise at its
retained losses or zero, and money is conserved exactly; each part is within one minor unit of
N_k / N_c of the pool (`national_profit_pool_conserves_money_exactly`,
`limited_liability_keeps_losses_and_repays_them_first`). Wages use the same cumulative-share
rule (`split`) over the province's pops by employed effective labour.

*Loss write-off (E4, ADR 0112).* After the distribution, enterprise e's retained loss beyond
T_e = M S_e (M P148, 3 months; S_e = VA0_e / a_VA_e its base monthly sales, the calibrated
steady-state sales value) is written off: per country the excesses E_e = max(0, -balance_e -
T_e) are credited to their enterprises and debited from the country's pops in proportion to
their positive money, never taking a pop below zero. If the pops hold less than the sum of the
excesses, what they hold is written off, shared over the enterprises by their excess, and the
rest is carried to the next tick. Both sides are one exact i64 split per country per tick by the
largest remainder (floor of each exact share; the leftover units one each to the largest
remainders, ties to the first), routed through the country's pool account; flow `WRITE_OFF`,
which is not income. The cap is nominal, so homogeneity of degree zero holds only while nothing
is written off. S_e = VA0_e / a_VA_e is the steady-state sales only on a calibrated world, where A
is solved so that output Y = A VA0 / a_VA matches it; on an uncalibrated world (the fixture without
calibration tables, `--fixture` runs) base sales are A VA0 / a_VA, so the cap is a factor 1 / A off,
which is harmless while A is near 1 and is not corrected. An enterprise that loses money every month becomes a perpetual levy on its
country's pops once its cap is full.

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
dispersion s = s_w, its country's within-pop dispersion (ADR 0094): s_w^2 = max(0, s_nat^2 -
Var_k ln m_k), s_nat = sqrt(2) Phi^-1((G + 1) / 2) = 2 erfinv(G) from the country's 2000 Gini
and the variance population-weighted over its pops' base incomes per person m_k, rebates aside
(w0 (1 - u*) times the pop's effective labour, plus its population share of the country's
profit pool, over its population; `world::pop_base_income`). The national Gini contains the
spread between pops, so it is not counted twice. A country whose between-pop variance alone
exceeds s_nat^2 gets s_w = 0 and is listed in the calibration summary (`within_sigma_zero`;
none on the fixture). The weights carry s_w (`PiglWeights::sigma`, `pigl_sigma` in the
calibration's country table). With
E[e^(1 - eps)] = m^(1 - eps) exp(-eps (1 - eps) s^2 / 2), the pop's category shares are, in
closed form,

```
s_c = sB_c + lambda (Dt / B)^gamma (sD_c - sB_c) (m / B)^-eps exp(-eps (1 - eps) s^2 / 2)
```

Shares below 0 (a pop far poorer than its country's base) are set to 0 and the rest rescaled to
sum to 1 (Review Focus 4). Spending on good g is s_c(g) times its within-category share times
the money balance: spending equals money.

*Inversion (per country, over its pops; ADR 0094, Amendment).* Parameters theta, sigma_B, phi,
gamma, eps and omega_A = 0 from the pooled PIGL column of the paper (Tables 3 and A3, column 3,
pp. 24 and 48). The unknowns lambda, omega_M = t, omega_S = 1 - t and alpha (summing to 1 within
each category over the goods with demand) solve, for every good,

```
sum_k E_k s_kg(q_k, m_k, s_w) = Omega_g sum_k E_k
```

over the country's pops k, each at its own buyer prices q_k and income per person m_k, weighted
by its spending E_k (unclamped shares). Rounds from alpha_g = Omega_g / Omega_c(g): at the
current alpha, lambda follows from the food aggregate in closed form (sB_A = 0) for each t, and
t solves the M aggregate by a bracketed secant (Illinois) on [0, 1]; then alpha_g is rescaled by
Omega_g over its aggregate share and renormalized within its category (the contraction of Berry,
Levinsohn and Pakes 1995). The rounds stop when every aggregate share is within 1e-14 of Omega
relative (or, below 1e-12, has stopped improving for 20 rounds); more than 1,000 is an error. No
sign change of the M aggregate on [0, 1] or a negative lambda is a negative weight and stops the
load (never clipped). At load without a calibration the pops are at prices 1 with E_k their base
income and m_k their base income per person (`world::household_preferences`); with one pop, or
pops alike, the solve is the closed form of one lognormal: z = Omega_A / theta_A, omega_c =
(Omega_c - z theta_c) / (1 - z), lambda = z m_0^eps exp(eps (1 - eps) s^2 / 2), alpha_g =
Omega_g / Omega_c(g) (`invert_pigl`). The simulator (`demand::invert_pigl_pops`) and the GE
reference (`demand::invert_pops`) implement it independently.

Spending on traded goods is sourced over origins (trade SPEC) and paid at dispatch for what is
filled; rationed purchases leave money unspent until the next tick.

**Money.** The mint issues each pop's initial money at tick 0 (Initial state: its calibrated
balance, or one month of base dividends in the data state). Markets pay out their signed cash flow
every tick (the merchant rule, trade SPEC, Market cash flow), so their balances return to zero.
Enterprises pay out only positive balances: under limited liability (Income) a loss stays as a
negative balance, repaid before any payout and written off beyond M months of base sales (ADR
0112), so enterprise balances need not return to zero. Any account may be negative within or
across ticks (clearing facility, no interest). Every payment is a ledger transfer rounded half to
even to minor units. Prices and wages scale freely: the dynamics are homogeneous of degree zero in
nominal values while no write-off fires, since the cap of the write-off is nominal (ADR 0112), so
below the cap the price level is set by initial money and comparisons use a world price index.

## Calibration (`ceteris calibrate p2`)

The 2000 data state is the model's steady state (ADR 0089, owner 2026-10-25; it replaces the
joint inversion of productivity, whose nominal targets fold and whose real targets have no root,
PLAN Tasks 18b to 18d). Values are held fixed and prices solved first:

1. Rust loads the pack, builds the kernel, and exports kernel legs, enterprises, pops and
   parameters as Arrow. theta per class, the border friction P05 and the mode cost ratios P04 are
   estimated before this, directly by PPML structural gravity on 2000 BACI with least-cost (S05)
   costs between countries (ADR 0022, PLAN Task 18; the goods of a bulk class on the kernel's
   bulk legs, ADR 0247), with provenance "data".
2. `reference/calibrate.py` (numpy, the source of truth) solves stages 1 to 5 below at the pack's
   u* and writes the calibration tables (Pack additions), which Rust reads as the initial state.
   Where the build departs from ADR 0089's text, and why, is in the ADR's "As built (Task 18e)"
   section.
3. Before the stages, `ceteris calibrate --pack` fits the border friction nu_c of each theta class
   (P05, ADR 0092; `ceteris_calibrate::border`) so that the cross-border share of the class in the
   ICIO economies, sum of (1 - home share) absorption over sum of absorption from
   `econ_home_shares.arrow`, is the model's: the value the destinations of those countries buy
   from other countries over all they buy, at origin prices. Each evaluation is stage 2 at stage
   1's spending of the data state (r = 1, no rebates, the data state's buyer prices and PIGL
   weights), held across trials, so each class's share depends on its own nu only and falls as it
   rises; the four roots are found by Brent's method per class, its bracket narrowed to 1e-9 in
   nu over [0, 10] (the upper end halved where the scaling fails there, on a scaling failure
   only), the classes sharing each evaluation and every scaling starting from the scaling at nu
   0, so the result does not depend on the order of the evaluations (19 evaluations on the
   fixture, `nu_fit_matches_cross_border_share`). The root itself is resolved only to the
   scaling's noise in the share over the share's slope: on the fixture 6.8e-9 for class 3, whose
   share moves by 2.7e-7 per unit nu, and 2e-10 or better for the others. That test checks the
   shares relative to their target (1e-6), a deviation from the PLAN's absolute 1e-6, which class
   3's flat share passes for any nu from 0 to about 4. A target above the share at nu 0, the
   largest the class reaches with nu >= 0, clamps the class at nu = 0, the corner of the
   constraint, and the fit reports it (`Fit::clamped`, the share at nu 0 against the target;
   ADR 0092, Amendment 2026-10-30); a target below the share at the bracket's upper end stops the
   calibration, naming the class (`nu_fit_is_monotone_and_bracketed`, both ends). Interfaces as built: `border::fit_border_nu(inp, target, icio, limits, progress)
   -> Result<Fit, CalibrationError>` (no `u_star`, which stage 1 at the data state does not use;
   the ICIO flags; the shares and the evaluation count with the nu) and
   `border::target_shares(pack, theta_class) -> Result<([f64; 4], Vec<bool>), String>` (also the
   ICIO flag of every pack country), in place of the PLAN's `fit_border_nu(inp, u_star, target,
   limits) -> [f64; 4]` and `target_shares(pack) -> [f64; 4]`. The stages then run at the fitted nu (crossing factors and
   arrival shares rescaled, base buyer prices recomputed), the calibration tables record it
   (`econ_calib.arrow` metadata `border_nu`, calibration format 4), and `EconInputs::calibrate`
   reads it into `EconInputs::border_nu`, from which `EconInputs::trade_params` builds the trade
   parameters a calibrated world runs under; `EconWorld::new` refuses a calibrated start under
   other structural parameters, nu included. After the stages the command prints the shares at
   the calibrated steady state, where stage 1 is no longer the data state's. The fixture keeps
   its default nu (it has no ICIO data). `ceteris calibrate --pack DIR --no-border-fit` skips the
   fit and calibrates at the default nu, for comparisons (H2 before the fit).

**Stage 1, values.** Output value V_o = VA_o / (1 - sum_k a_kg) per traded enterprise, VA the
pack's. Destination spending on traded goods E_dg is households' PIGL spending at q_d (stage 4)
plus sum_g' a_gg' V_dg' of the province's enterprises. Household income follows Income above:
the province's wages s_L VA by effective labour share, and the country's profits (1 - s_L) VA and
tariff rebate by population. One scalar r_g per traded
good rescales its V and VA so that world spending net of tariffs equals world output value (the
2000 imbalances, reported). Spending scales with the r (incomes are value added, and stage 4
re-inverts lambda at the new income), so this fixes the r only up to one common factor; each pass
rescales the traded r so that world traded value added sum_e r_g VA_e equals the data's (ADR 0106;
without it the level depended on the start and the mixing path, by up to 4e-4 from starts at
+-50% of the data state's buyer prices, `calibration_unique_from_eight_starts`). Rebates follow from
sourcing, local output from income and income from local output, so passes of stages 1, 2 and 4
repeat until rebates, r, buyer prices and the PIGL weights change by at most 1e-12 relative
(Anderson-mixed; `PASS_TOL`), so the returned state is a fixed point to the 1e-12 its checks
claim (`sinkhorn_is_unique_up_to_scale`). The mixing runs on a state in the units `change` uses:
ln q, ln r, the PIGL omega and within-good weights as they are, and the rebates and the PIGL
scales each divided by their block's largest magnitude (the rebates' from the first pass, the
scales' from the start), fixed for the whole run. In money units the rebates' residual (about
1e7) swamped the prices' (about 1) in Anderson's least squares, and on the full pack with the
B1 tables and theta 16.91 the passes wandered between 2e-4 and 6e-2 for 100 passes: the
undamped pass map is not a contraction (it locks onto a period-6 cycle in the good-1 prices of
the DR Congo's interior and that country's PIGL weights), so settling rests on the mixing, and
the mixing only saw the rebates. The passes still stop on `change` alone; the units change no
equation and no fixed point. The stopping tolerance is a solver setting, not a model
parameter, so it has no register ID. Within a pass the income and local-output fixed point is
iterated until no local output changes by more than 1e-14 relative.

**Stage 2, origin prices.** Per traded good, balanced matrix scaling on the two-level kernel:
w_o sum_d K_od / (1 + t_od) E_d / Phi_d = r_g V_o with Phi_d = sum_o K_od w_o, unique up to scale
(Sinkhorn 1967). Sweeps run in the log domain (each good's weights shifted by their largest
before each pass over the kernel), Anderson-accelerated (memory 8), to max |ln(received /
target)| <= 1e-13, within at most 100,000 sweeps (`MAX_SWEEPS`, an iteration cap, not a
tolerance; 20,000 before 2026-10-29). The cap was raised because of a slow mode: on the full pack
with ADR 0101's thetas, coal and other mining (theta 16.91) has an origin, province 3906, about 52
times its good's cheapest price, which its destinations buy almost only from. Its part of the
scaling contracts by about 0.99995 a sweep (the residual falls about 5% per 1,000 sweeps), and pass
7 needs 34,614 sweeps. Keeping Anderson mixing from restarting while the residual still falls
over its 20-sweep stall window was tried and is slower (pass 7 above 7e-13 after 15,000 sweeps,
against 2.9e-13); dropping the restarts makes pass 4 diverge. On the current pack (B2c, 2026-10-30) the
slow mode is another: Brazil's 48 origins of the same good keep 0.999 of their Sinkhorn coupling,
and the plain sweep contracts that block by 0.99908 a sweep (the Jacobian's second eigenvalue
below); the cap stays. An extrapolated step that leaves f64 (a non-finite
residual) restarts the mixing from the plain sweep it extrapolated, which a plain sweep's
overflow does not (that is an error): the fixture's scaling at border frictions 0.15, 0.6, 0.35
and 1.2 overflowed this way after 182 sweeps where plain sweeps converge. Scalings that converged
before take the same path. p_o = w_o^(-1/theta_g), each good's scale set so that its part of the world
price index (Omega_g times calibrated province VA as weights) is 1; local goods take p = 1, so
the index is 1. q_d, the arrival slots and the lead follow from sourcing: q_d = 1 / alpha_d, the
delivered unit price of the physical arrivals, at every pass (`Model::physical`), and the
selection constant of ADR 0200, Gamma_d,g = q_d / P_d at the calibrated prices (1 where no origin
is reachable), is written to `econ_calib_cells.arrow` (`gamma`), so a calibrated world's buyer
price Gamma P_d is the calibrated q_d. The export carries every world's Gamma in `provinces`
(`gamma_0` to `gamma_9`, export format 13), which the GE reference reads. The arrival slots split
each destination's spending by value over its routes' slots and divide by q_d: every unit of
spending arrives as 1 / q_d units (stage 1 values do not depend on them; the lead and the pipeline
do, `steady_state_unchanged_by_selection`).

**Stage 3, technology.** coef_kg = a_kg p_o / q_ik (the 2000 cost shares at the solved prices,
so input coefficients differ across enterprises of a country), a_VA = 1 - sum_k a_kg (cost
shares at the calibrated prices, not at base prices), p_VA = p_o, A = 1 / p_VA, Y_o = V_o / p_o.
Local goods take p = 1, and local output solves the 3 x 3 Leontief system of each province
against its local demand (households and every enterprise's a_kg V); local VA = a_VA Y is an
output of calibration, and its national gap to ICIO is reported. w0_i = sum_g s_L VA0_ig / ((1 -
u*_i) L^s_i) and L0 = s_L VA0 / w0, so with L = L0 nominal VA is VA0 and the labour first-order
condition holds. S09 FixedLeontief reads the same calibrated column. Every province produces:
under national ownership even a province without traded output earns dividends and buys local
goods.

**Stage 4, demand weights.** Per country, PIGL weights inverted jointly over its pops (ADR 0094,
Amendment; `demand::invert_pops`), each pop at its province's buyer prices q of the pass, its
income per person, and weighted by its income (rebates included), at the within-pop dispersion
s_w fixed by the inputs. Pops then spend by PIGL at those prices and incomes, so in the data
state each country's aggregate household shares are Omega
(`aggregate_data_state_shares_match_omega`, 1e-12).

**Stage 5, state at the start of a tick.** Input stocks (1 + COVER) months of use, traded and
local, so the order-up-to rule orders one month of use; the pipeline of every buyer (pops and
enterprises) holds, j ticks ahead, one month of steady-state shipments of every arrival slot j or
later; merchant stocks coverage times output (S* = coverage D with D = Q); pops hold one month
of income less this tick's wages (dividends and rebates; wages come at E2); pi^e = 0; last D, Q
and sales their steady-state values (D = Q = sales).

**Checks** (`crates/ceteris-calibrate/tests/steady_state.rs`, fixture and full pack): two starts of stage 2 reach the
same weights up to scale, to the bound the stop rule implies (`sinkhorn_is_unique_up_to_scale`).
With g = ln received - ln target over a good's origins, each solve stops at max |g| <= tau
(1e-13). Two solutions d apart in ln w have g_1 - g_2 = (I - M) d to first order (their r agree to
1e-16), M the Jacobian of -ln reach in ln w, M_oo' = sum_d (K_od / (1 + t_od) b_d / reach_o)
(K_o'd w_o' / Phi_d) with b_d = E_d / Phi_d: row-stochastic, M 1 = 1 the free scale. On the
mean-free part d = sum_k M^k (g_1 - g_2), so the spread max |d - mean d| is at most about 2 tau
/ (1 - lambda2_g), lambda2_g the second eigenvalue of M for good g (power iteration with the
constant deflated), with tau raised by delta_g, the rounding of the computed gap against the
exact one at the same stored ln w. From the gap's operations (u = eps / 2, libm exp and ln
within 1 ulp): exp(ln w - top) carries 2uL + 2u (L = max |ln w| of the good's origins in either
solution); the origin and destination sums, positive terms D additions deep (D = max(m_dom,
m_gate + m_cross + m_prov) + 2, the most legs of the good in one bucket), add (2D + 4)u; ln got
and the target ln (r v) add 4uT + u (T = max |ln (r v)|); r is common to the good's origins and
moves none against another. So delta_g <= eps (2L + 2T + D + 4.5), and the test asserts
1.5 x 2 (tau + delta_g) / (1 - lambda2_g) (1.5 a margin for the first-order and asymptotic
steps and the estimate, not a parameter). The part in L is k eps L with k = 6 / (1 - lambda2_g).
The sums' worst case dominates on the full pack (D = 858): the rounding term is about twice the
stop-rule term, and good 2's bound 1.0e-9 against an observed 2.1e-10 (2026-10-30, task B3
fill round 5). A near-closed block of origins sets lambda2_g: on the
B2c pack Brazil's coal and other mining, lambda2 0.99908, stop-rule term 2.2e-10 against an
observed 2.1e-10. The fixed 1e-10 the check used before was never derived: it fails on the B2c pack (2.1e-10)
and held on the B2a pack at 9.5e-11 (2026-10-30); the reference model at the calibrated point, with labour
supply (1 - u*) L^s, has residual at most 1e-10 and reproduces VA0, L0 and Y; local output is
positive and clears; the stage 5 state is stationary under the order rule. In Rust
(`crates/ceteris-econ/tests/p2_steady_state.rs`), 480 ticks from the calibrated state with every
friction on hold every quantity to 1e-9 on the fixture. On the full pack, 120 ticks, integer money
bounds them: real quantities to 2e-6, money to 2e-6 or 4 minor units, world aggregates to 1e-9.
On both the drift stays bounded (ADR 0089, As built): its largest value over the second half of
the run, over its tolerance, is at most twice that over ticks 5 to the half. The fixture run was
120 ticks until the calibrated nominal level was pinned (ADR 0106, As built): the fixture's
rounding drift then peaks near tick 105 (market sales 8.1e-12 relative, 0.8% of the tolerance)
and falls after, so ticks 61 to 120 read 2.01 times ticks 5 to 60; over 480 ticks every quantity
above rounding is no larger in its second half than in its first (controller ruling, 2026-10-29). The reduced residual
of the former joint inversion (`reference/reduced.py`) and its JAX twin were deleted with the
Python reference (ADR 0113).

## GE reference model (`ceteris-reference`)

Static equilibrium of the same economy with all frictions off: unknowns are ln p for every
active (province, good) cell (origin markets of traded goods, province markets of local goods)
and ln w for every province. Conditions: every origin sells its output, output = sum_d X_od /
(p_o (1 + t)) with X_od = E_d K_od w_o / Phi_d, destination spending E_d of households (PIGL at
q_d) and enterprises (a Y q_d); local goods clear locally; labour clears at the natural rate,
sum_g L*_ig = (1 - u*_i) L^s_i (`io.Inputs.employed_supply`), with p_VA at q; each pop
spends its income by PIGL (`reference/demand.py`, the mirror of `demand.rs`): its effective
labour share e_k LF_k / L^s of its province's wages w (1 - u*) L^s, and its population share of its country's
profits (national ownership, Income) and rebate (market cash flow is zero at a fixed point); the rebate is the tariff share of
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
| E4 | distribute | enterprise and market cash flow, rebates, pop money | profit pools (positive enterprise balances, signed market cash flow), pop money for next tick, then the loss write-off (ADR 0112) | finance (P2 stand-in) |
| E5 | assert | ledger, state | | accounting assertions |

System IDs 201 to 210 in this order.

Pops spend at E3 the money they hold: this tick's wages plus last tick's dividends and rebates.

Snapshots every 12 ticks (core). No RNG draws in P2; streams are reserved per system ID.

## Invariants

| Invariant | Statement | Tolerance | Checked where |
|---|---|---|---|
| Money | Sum of balances zero; every flow tagged | exact | `Ledger::check`, every tick |
| Goods | Created - destroyed = change in holdings (incl. transit). The transit part is true by construction right after T2b, which brings each transit account to the goods in flight (ADR 0200); the Selection invariant below does the real check | 1e-9 relative; transit 1e-9 relative plus 1e-12 of the good's market turnover, since markets fill it by differences of their holdings | every tick |
| Selection | What T2b creates less what it destroys, per traded good (selection at home purchases and in transit, the melt), equals `dispatch`'s selection term less its melt (ADR 0200; trade SPEC, Invariants) | 1e-9 of the good's delivered and shipped units, plus 1e-12 of its market turnover | every tick; `goods_invariant_holds_with_selection` (120 ticks of the fixture) |
| Population | Pops unchanged | exact | every tick |
| Labour | 0 <= employed <= labour force; sum L <= L^s | 1e-12 relative | every tick |
| Leontief | Inputs used = coef_k Y, traded from stock, local through the same-tick chain | 1e-12 of max(coef_k Y, available); a local good (1e-12 + 2 FIT) of max(coef_k Y, available) plus 2 (N_GOODS + 1) eps D_k, D_k the units of k drawn at the province market that tick: where the passes stop short (U - R <= FIT (U + R)) a user drawing pro rata misses (n - b)(1 - R / U) < 2 FIT of its own need, and the draws from the market account round its balance M up to 2 (N_GOODS + 1) times by eps / 2, which the user at which it runs dry absorbs, with M <= D_k / (1 - (N_GOODS + 1) eps) < 2 D_k since it ends empty (ADR 0093, amendment of 2026-10-30) | every tick; `conservation_over_120_ticks` seed of tick 43, good 7; `a_tiny_local_user_left_without_its_input_breaks_leontief` |
| Local fit | Each local good's uncovered need at most what its producer leaves, U_k <= R_k (exact where the passes end on a pass that changes nothing; to 1e-12 of the flows where they stop at what f64 resolves, ADR 0093 bounded work, seed 7778631857305059895) | 1e-12 of U_k + R_k | `fits_up_to_rounding` in the fill tests of `p2_properties.rs` |
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
| P149 | Input stock cover (COVER, `systems::COVER`): months of use an enterprise keeps beyond replacement and transit, the `cover` of the order rule and of calibration stage 5 | 1 (owner decision, 2026-10-24; not tuned) | Free |
| P148 | Loss write-off threshold M (ADR 0112): retained losses an enterprise keeps, in months of its base sales | 3 (controller ruling, 2026-10-26; not tuned) | Free |

P149 is a constant of the order rule, not an `EconParams` field, so it is outside the static digest:
changing it changes the calibrated stocks and needs a recalibration (calibration stage 5 uses the same
value).

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

At the P2 defaults (P01 class values 7.68, 16.91, 16.91, 2.91, 7.68, 2.91, 4.14, ADR 0101; the
fixture's value-added shares, 0.46 to 0.73) the bound is 0.946 (coal and other mining: theta
16.91, beta 0.66); with labour the only input (beta = 1) it is 16.91 / 17.91 = 0.944. Before ADR
0101 (10.57, 5.19, 2.62) it was 0.922 on the fixture and 0.967 on the 2000 pack (smallest
value-added shares over countries 0.040 to 0.252 for the traded goods; machinery, electronics and
vehicles: theta 5.19, beta 0.040); the pack value is recomputed when the pack is rebuilt with the
four classes. All run V1 in tolerance mode (Task 20). The restatement above was checked against the January 2024 version in Task 20
(Theorem 1 p. 6, Remarks 4 and 5 pp. 10 to 11, Online Appendix A.3 pp. 6 to 9 with Gamma, B and
the radius on p. 9, footnote 10 p. 7).

## P2 gate (21.1)

- V0: zero invariant violations; determinism, 1 vs 8 threads, replay.
- V1: first the uniqueness check (Equilibrium nesting) at the run's theta and the pack's
  value-added shares. Where the bound holds (at most 1, tolerance mode) V1 applies the 1%
  tolerances above, full pack, within 1,200 ticks. Otherwise (report mode) both models start from
  the data state, the run frictionless for 1,200 ticks and the reference solve from
  `model.data_state`, and the test reports which equilibrium each reaches (index-normalized
  prices, wages and flows of each, and their largest gaps) instead of a pass or fail at 1%.
  On the calibrated pack the reference is the calibrated root, read from the calibration tables
  (`calibrate.read_reference`, residual 5.6e-12): a solve from `model.data_state` is infeasible
  there (Newton-Krylov moved the residual from 53.6 to 51.7 in 7 steps of 35 s, Task 20).
  Comparisons with the perfect-foresight references (Caliendo-Dvorkin-Parro, Kleinman-Liu-Redding)
  are sign and rank only: agents with heuristic expectations cannot match their transition paths.
- H2: PPML of simulated country-pair trade (traded goods; the origin value of the shipped units,
  `run_flows.value`, which is what the exporter receives and excludes the tariff; steady state of the default
  run) on ln distance (CEPII population-weighted) and contiguity, with exporter and importer
  effects, excluding domestic pairs: the distance coefficient is reported against the data's -1.1
  to -0.7 and fails the test only when not finite or not negative (ADR 0264; it blocked the gate
  until then). Nothing is fitted to it: costs come from P04 and the literature time cost P43.
  With iceberg leg costs each class's coefficient is about -0.09 theta_c, against about -0.8 in
  the data; the log-distance leg cost that ADR 0022 says the data prefer is scheduled for P9.
- H2 per theta class (ADRs 0018, 0092 and 0264): PPML of each class's simulated country-pair
  trade, valued at what the importer pays (`run_flows.value` times 1 + t), on ln(1 + tariff) and
  the ln of the model's country-pair kernel net of tariffs at the calibrated point
  (`model::ln_country_kernel`, province legs weighted by the sourcing weights w_o and E_d /
  Phi_d), with exporter x good and importer x good effects over the foreign pairs a crossing
  links (`ppml::class_tariff_fit`), recovers the class's own theta_c: theta_c lies within max(2
  pair-clustered standard errors, 1% of theta_c) of the class's estimate. The regression is
  nearly an identity with the model's kernel as the control, so the s.e. stops measuring
  sampling error and the tolerance is V1's 1%. At the reference equilibrium the
  regression is exact (`kernel_controlled_tariff_regression_recovers_theta_per_class`, fixture
  with tariffs per pair, 1e-6). If a class misses, work stops and the estimates go to the owner.
  The earlier control, ln distance (CEPII population-weighted), is printed as a diagnostic: it
  leaves the costs the model puts on near agreement partners in the tariff coefficient.
- 100 years (1,200 ticks) of the default run on the full pack without violation.
- Budgets (19.2, M4 Pro): demand 10 ms, production and labour 12 ms, markets 2 ms, trade 30 ms
  per tick; criterion benchmarks in CI with the 20% regression gate.
  - Measured (Task 19 after its fix rounds, M4 Pro on AC power, full pack from its calibrated
    steady state, tick 1, criterion medians of `cargo bench -p ceteris-econ`; Task 19 budgets
    in brackets): consume (E3) 6.83 ms [10], of which the price refresh, E3's unit `source`
    and `dispatch` of each traded good (migration carry), 5.85; labour 0.73 + produce 3.38 =
    4.12 ms [12], of which the same-tick chain of local goods (migration carry, parallel by
    province) 0.54; clear_reprice 0.24 ms [2] (econ clear 1.89, reprice 0.22);
    source_dispatch_parallel, T2's warm sourcing and T2b's dispatch of the seven traded goods
    in parallel as the tick runs them, 7.90 ms [5] (41.3 ms one good after another): missed,
    each good's dispatch (about 5.3 ms over 356,080 crossings per class) is the critical path;
    the E3 carry in parallel 5.42; kernel_build 120 ms. A whole tick 33.9 to 34.1 ms
    (deliver 1.03, source 1.42, dispatch 20.6 to 20.9, distribute 0.67, end_tick 2.39); a run
    35 to 37 ms per tick with snapshots, 150 ms before Task 19; peak memory of 120 ticks with
    history 1.88 GB (1.80 GB before). State hashes, run files and goldens are unchanged, and
    identical at 1 thread and at all threads.
  - Tried for dispatch and not kept: two-lane SIMD division (`wide`), bit-identical but 0.4 ms
    slower in parallel; and a structure-of-arrays layout for the crossings, not built: reading
    every crossing and domestic entry as often as dispatch does takes at most 1.2 of its
    5.2 ms per good, and the CPU counters put 55% of its cycles as useful and 44% stalled on
    instruction processing (dependent sums, divisions), not on memory.
  - Pruning gateway crossings by contribution (Task 19b, not kept): within a country pair every
    crossing's tau is within about 1.3 times the best, and every live crossing carries at least
    1% of its pair's largest flow. Only the 10,710 crossings to countries with no economy
    province are exactly negligible (about 0.25 ms); 5 ms needs about 190,000 crossings, which
    moves Phi_d by 50% to 95%, a change of the model and not of its speed.
  - Re-budget (owner, 2026-10-26): source_dispatch 8 ms and a whole tick 35 ms on the full pack;
    trade stays inside the 30 ms of 19.2. The gateway count, which sets the crossing count and
    the access it buys, moves to the ADR 0092 rework after the gate.
- Measured on the 2000 pack (Task 20 and its fix rounds; limited liability of enterprises and the
  loss write-off of ADR 0112; `p2_acceptance.rs` and `test_p2_gate.py`; owner review pending):
  - V1, tolerance mode (bound 0.967), against the calibrated root. From the steady state
    perturbed by 10% (seed 20; prices, wages and stocks, though frictionless mode re-solves wages
    every tick, so this start tests prices and stocks): price 9.085e-3 (São Tomé and Príncipe light
    manufacturing, the period-2 cycle of the trade SPEC, Stability region), wage 9.1e-4, flow
    units 4.6e-5, value 5.0e-5 at 1,200 ticks: passes (7.8e-3, 8.4e-4, 3.2e-5, 3.5e-5 before
    limited liability).
  - V1 from the data state of the calibrated inputs (prices 1, nothing in flight) misses at 1,200
    ticks: price 0.83 (Mali services), wage 0.58, flows 1.83; outside the former debtor countries
    (Albania, Mali, Namibia, Pitcairn, Uganda) price 3.8e-2 (Burkina Faso coal), wage 2.2e-2. At
    4,800 the gap is 7.9e-2 (Mali) and 8.3e-3 outside them (the São Tomé cycle), within 1% outside
    from tick 3,600. The deep coal-led transient's enterprise losses are written off early
    (8.43e12 of 8.63e12 by tick 60; 2.1e12 in Sudan, 8.9e11 Mali, 7.7e11 Namibia, 6.9e11 the United
    States, 5.5e11 Uganda, 3.9e11 Albania), and the world price index returns from 2.38 at tick
    120 to 1.28 at 1,200 and 0.96 at 4,800. What remains is market losses, which the signed rule
    passes to pops as negative dividends (ADR 0112, Markets): up to 5,299 pops with negative money
    (tick 32), 40 at 1,200 (Albania, Mali and Uganda), 1 at 4,800 (Albania), who buy nothing
    until repaid. Before the write-off: price 0.83 and 0.33 outside at 1,200, 0.21 and 5.9e-2 at
    4,800, with 5.0e12 of retained losses repaid over about 33,000 ticks and an index of 7.1 at
    1,200; before limited liability 0.88 and 4.0e-2 at 1,200, 0.82 and 8.1e-3 at 4,800.
  - Gate decision (owner, 2026-10-26): P2's V1 is the run from the perturbed steady state, now
    that the data state is the steady state (ADR 0089). The run from the shocked data state is a
    stress test, `xfail(strict=True)` in `test_p2_gate.py`. Two pre-gate trials did not cure it
    and are parked on local branches: producers holding inventory and planning output (ADR 0112,
    second decision, `p2-r3-producer-stock`; frictionless mode pins employment at the natural
    rate), and ADR 0090's split price rule (`p2-adr0090`; its nominal anchor turns a local real
    shortage into world inflation). The anchor is redesigned after the gate.
  - H2: distance elasticity -0.753 (s.e. 0.016, pseudo-R2 0.982, 47,306 pairs): passes. Per
    class, on what the importer pays (FOB value times 1 + t; the FOB value gives 2.78, 4.70 and
    10.55): food 1.75 (s.e. 0.15), machinery 3.72 (0.47), the five-good class 9.52 (0.33), all in
    their bands (pseudo-R2 0.998, 0.993, 0.959), but below the model's 2.62, 5.19 and 10.57 (Open
    questions, H2 per class).
  - 100 years: no violation, prices and wages finite and positive; the default run stays at the
    steady state (no pop with negative money, no negative pool, no retained loss, no stockout),
    and is byte-identical with and without limited liability and the write-off.
  - 1 vs 8 threads, 120 ticks: identical hashes.
  - After the country-scaled weights (ADR 0093) and the stored reciprocals (ADR 0097),
    recalibrated (46 passes, Sinkhorn residual 8.9e-14): V1 from the perturbed steady state
    passes (price 9.085e-3, wage 9.06e-4, flow units 4.6e-5, value 5.0e-5); H2 distance -0.753
    (s.e. 0.016) passes; H2 per class passes (food 1.75, s.e. 0.15; machinery 3.72, 0.47; the
    five-good class 9.52, 0.33). The reference matches Rust's delivered prices to 2.8e-15.
    A first build that also had ADR 0092's gateway masses (1/k per country) missed H2 distance
    at -0.536 (s.e. 0.013) with V1 and H2 per class passing; the owner deferred the masses to the
    border friction fit per class (2026-10-26).
  - On the pack built and calibrated in Rust (ADR 0113, step 4f; GEOS 3.15, 46 passes, Sinkhorn
    residual 9.6e-14), the gate in Rust (`p2_gate.rs`, `p2_acceptance.rs`, release): V1 from the
    perturbed steady state passes (price 9.085e-3, São Tomé and Príncipe light manufacturing;
    wage 9.057e-4; flow units 4.631e-5; value 5.039e-5; bound 0.9671; reference residual 7.6e-12
    at the calibrated point); V1 from the data state still misses (price 0.826, wage 0.580, flow
    units 1.83, value 1.56), as the stress test expects; H2 distance -0.7531 (s.e. 0.0161,
    pseudo-R2 0.982, 47,306 pairs) passes; H2 per class passes (food 1.750, s.e. 0.148;
    machinery 3.715, 0.468; the five-good class 9.520, 0.335). 100 years: no violation, world
    price index 1.0000000000000067, no pop with negative money, no negative pool, no retained
    loss, no stockout. 1 vs 8 threads, 120 ticks: identical hashes (20.0 s and 6.3 s). Every
    number equals the Python gate's at its printed precision.
  - Commands (M4 Pro, `data/packs/2000` built and calibrated, `PACK=$PWD/data/packs/2000`,
    `EXPORT` a scratch directory):
    - `cargo run --release --bin ceteris -- export --pack $PACK --out $EXPORT`, the export the
      gate reads.
    - V1 (both starts), H2 distance and H2 per class, about 265 s:
      `CETERIS_PACK=$PACK CETERIS_PACK_EXPORT=$EXPORT cargo test -p ceteris-cli --release --test p2_gate -- --nocapture`;
      `v1_full_pack_from_the_perturbed_steady_state` gives the 9.085e-3 line,
      `v1_full_pack_from_the_data_state_stays_open` the stress test, `h2_distance_elasticity` the
      -0.753 line, `h2_theta_by_class` one line per class.
    - 100 years and 1 vs 8 threads (`hundred_years_without_violation`,
      `full_pack_one_vs_eight_threads`, with V0's determinism and replay):
      `CETERIS_PACK=$PACK cargo test -p ceteris-econ --release --test p2_acceptance`.
    - The data-state census behind the write-off figures:
      `CETERIS_PACK=$PACK cargo test -p ceteris-econ --release --test p2_acceptance -- --ignored data_state_write_off_census`.
    - Benchmark medians (tick, `source_dispatch_parallel` and the per-system rows): `cargo bench -p ceteris-econ`
      (`benches/econ.rs`, `benches/trade.rs`), M4 Pro on AC power.
- Golden: `tests/golden/p2-econ.txt`, hashes of a generated 4-country, 25-province world over
  1,200 ticks (runs in CI); `tests/golden/p2-pack.txt` for the pack tables and calibration.
- UI: below.

## App (after the native app reaches parity, ADRs 0011 and 0088)

The app is fully native (no React, webview or Tauri), with Slint panels over a wgpu map
(ADR 0088). P2's UI is built there (`app/`, specs/app/SPEC.md). Map modes: price by good
(index-normalized), output by sector, net exports by good, wage, unemployment. Province panel:
market rows per good (price, inventory, supply, demand), its enterprises (sector, employment,
output, profit), its pops (education, labour force, employed, income). Time controls: play,
pause, step, speed, driving the core in the same process. Colouring goes through the feature
texture fed from core columns.

## Outputs

`ceteris run` writes `run_prices`, `run_wages`, `run_enterprises`, `run_flows`, `run_home` and `run.json` (the state at the final tick; flows and home purchases are means over the last 12 ticks, `ticks_averaged` in `run.json`, and fewer after a snapshot restore or `seek`, since the window is scratch and not state). `--data-state` needs `--pack`. With `--history [--history-every N]` it also writes `run_history_world`, `run_history_provinces` and `run_history_markets`, one row per sampled tick (tick 0, every N-th, the final tick) for charting the dynamics; recording only reads the state, so state hashes and the default output are unchanged. Columns: `crates/ceteris-econ/src/history.rs`.

## Test plan

- Same-tick chain and putty-clay (ADR 0020, `p2_properties.rs`): one vintage equals fixed
  Leontief in every state hash over 120 ticks (`putty_clay_one_vintage_equals_fixed_leontief`); a
  50% cut of services labour with empty local stocks cuts construction output in the same tick
  (`local_shortage_reaches_downstream_same_tick`); intermediate users draw all of a short local
  good before households, and Priority fills intermediate orders first at origins
  (`local_chain_respects_intermediate_priority`); cutting coal output leaves downstream output
  unchanged in that tick and lowers it once stocks run out (`traded_shortage_still_lags`); cycles
  among local goods settle in at most 8 passes (`local_chain_terminates_on_cycles`); the exact fills equal
  the fill iteration run to convergence, with its steps found by bisection, on every province tick
  of the fixture runs and on random small provinces, at 1e-12, wherever it converges
  (`exact_fill_equals_converged_iteration`, `exact_fill_equals_converged_iteration_on_random_provinces`);
  S11 Proportional gives one fill ratio (`proportional_rationing_switch`); `split` in `local.rs`.
- Unit per equation: Leontief binding input, normalized CES and the closed-form L* against
  numerical maximization, wage step and clip, rigidity on nominal cuts
  (`wage_rigidity_applies_to_nominal_cuts`), frictionless wage root, income split, rounding of payments. PIGL (`demand.rs`): pop
  demand against 64-node Gauss-Hermite quadrature of individual demand over the lognormal
  (1e-10), adding-up (1e-14), inversion reproducing base shares (1e-12), Engel curves across
  fixture pops (food share falls, services share rises), clamping far from base (money x 1e-3
  and x 1e3), homogeneity, unreachable goods and categories, Gini to sigma, the within-pop
  dispersion (`within_sigma_subtracts_the_between_pop_variance`), the joint inversion over pops
  (`pigl_pops_inversion_reproduces_base_shares`; reference `one_pop_inversion_equals_old_at_prices_one`,
  `between_and_within_variance_add_up`, `pigl_inversion_reproduces_base_shares`).
- Property (proptest): money and goods conservation through a full tick on random small
  worlds; employment bounds; homogeneity (scaling all nominal values leaves real outcomes
  identical; pi^e is a log difference and stays; run with the loss write-off off, whose cap is
  nominal, ADR 0112); pi^e follows the world price index
  (`expected_inflation_follows_the_world_index`); clip counts (`world_counts_clip_bindings`).
- Unreachable destination at world level (Review Focus 1, `fixture::unreachable_island`:
  machinery has no route to or from the island, and of the island's sectors only light
  manufacturing uses it): over 240 ticks in both modes the island's q_d of machinery is the only
  infinite buyer price and the world index leaves it out, island pops buy none of it and spend
  the rest of their money, its one user orders none, never restocks and ends producing nothing,
  no price, wage or flow turns NaN and the invariants hold
  (`unreachable_destination_buys_nothing_in_the_world`); the reference solves the same variant
  with a finite residual (`unreachable_destination_solves`). V1 holds on the variant (final
  fix wave: price 3.0e-5, wage 2.5e-5, flow units 5.8e-5, value 7.5e-5 at 1,200 ticks) except at
  the island's light manufacturing, whose price is not determined: with no output and no orders
  any price that leaves its weight below the market floor solves its market.
- Pipeline and reference (`ceteris-pack`, `ceteris-reference`; proptest): ICIO aggregation
  preserves totals; downscaling preserves national VA; imputation flags; GE reference residuals; reference versus balanced Sinkhorn on small
  worlds; PPML recovers a known elasticity on synthetic gravity data; the uniqueness check
  reproduces the paper's worked examples (footnotes 3 and 6, Section 3 with Figure 1, Online
  Appendix A.3: `bound_reproduces_paper_examples`), is below 1 at the P2 defaults
  (`trade_only_is_unique`) and picks the V1 mode (`v1_mode`); the reference's PIGL
  reproduces Rust's on the export's 100 `pigl_check` draws (1e-12) and its inversion the base
  shares.
- History (`history.rs`, `tests/history_cli.rs`): row counts at intervals 1 and 5, last row equals the run files, same state hash with and without recording, deterministic bytes, default output byte-identical.
- Acceptance: `tests/p2_acceptance.rs` (V0, V1, 100 years, determinism) and
  `ceteris-cli/tests/p2_gate.rs` (V1 comparison, H2); full-pack parts skip without a pack, as
  in P1.
- Benchmarks: `labour`, `produce`, `consume` (criterion, full pack).

## Open questions

- ICIO 2025 edition industry list (whether B05 and B06 are split) is checked at fetch.
- Gateways (Task 17): the spec rule above replaced two parts of the first text. k-medoids run on
  great-circle distance, not path cost, since path costs need the Rust paths in the Python
  pipeline. Land crossings enter as candidates, not each as a gateway pair: the P1 graph has
  2,481 cross-border road and rail edges, 1,359 after merging within 50 km, which as gateways
  would make about 2,700 gateways and the all-pairs crossing matrix twenty times its budget. The
  built pack has 600 gateways (287 ports, 168 airports, 145 land crossings). Revisit with path
  cost medoids, or sparse land crossings joined only to their neighbour, in the ADR 0092 rework after the
  P2 gate; the 356,080 crossings per traded good, the Task 19b pruning result and the 8 ms re-budget are in
  the trade SPEC, Open questions.
- Tariffs: WDI applied rates are effectively applied rates, preferences included, so the
  rate charged outside agreements understates each importer's MFN rate; rescaling it so the
  trade-weighted mean over all partners matches the WDI rate, or the MFN series, would correct it.
- Sector-specific labour shares (P2 uses the PWT country share for every sector).
- DOSE region matching rate to Natural Earth provinces without GADM geometry; if too low,
  manufacturing falls back to gridded GDP for those countries.
- 2000 tariff detail beyond importer x two groups (WTO IDB is closed-source only, fetch).
- Stability on the full pack: on the fixture under origin clearing, PIGL demand and the same-tick
  chain small deviations decay by 0.9964 (frictionless) and 0.9968 (default) per tick at the P2 defaults (kappa1 0.05, kappa2
  0, expectation gain 0.006; trade SPEC, Stability region and P03a, P03b, P45). The slowest
  mode is the world price level, which expected inflation turns into a damped oscillation
  (trade SPEC, Nominal mode). On the pack (Task 20; trade SPEC, Stability region) the default
  mode decays by 0.99230 per tick; frictionless, the rest of the world decays by 0.99007 while São
  Tomé and Príncipe's light manufacturing (value-added share 0.0742, supply elasticity 30.4) sits
  outside the region on a period-2 cycle of about 0.7%. Whether V1's frictionless mode should
  hold for such cells (a slower kappa1, or a supply elasticity cap) is for the owner.
- The run from the data state passes through a deep coal-led shortage before it converges (the
  equilibrium's coal output is about twice base output and intermediate demand is inelastic in
  units); V1 holds on the fixture at 1,200 ticks.
- Debt overhang (Task 20): pops may hold negative money (clearing facility, no interest) and buy
  nothing while negative. Enterprises have limited liability and their losses beyond three
  months of base sales are written off against their country's pops (ADR 0112), which clears the
  data-state transient's losses early (8.43e12 of 8.63e12 by tick 60). Markets keep the signed rule,
  since limited liability inflates the fixture without bound and the write-off only bounds it
  (index 13 at tick 1,200), so a deep transient still leaves market losses as pop debts (Albania
  and Mali, P2 gate, measured). How merchants finance stock, interest and repayment terms belong
  to the owner (A9 banks arrive in P5).
- Inventory invariant (resolved, Task 20 round 3): it compared a market's stock S' = S + (Q - D)
  with the ledger's holding to 1e-9 of max(S', held, 1), but both are sums of the order of the
  turnover, so a market sold almost empty gave a false violation (frictionless at kappa1 0.2,
  province 3204's agriculture: 1.8543306161 against 1.8543306142 after selling 3.59e7 units). The
  tolerance is now 1e-9 of the turnover S + Q + D, as the ledger's goods invariant has; that grid
  cell runs its 600 ticks (frictionless 1.0000, deviation 1.1 at 600; default 1.0003).
- H2 per class (Task 20): the tariff elasticity recovered from simulated flows is below the
  model's theta for food (1.75 against 2.62) and machinery (3.72 against 5.19), and closer for
  the five-good class (9.52 against 10.57). Replacing ln distance by the model's own country-pair
  kernel without the tariff, ln sum over crossings of (v_a / V_A) K_ab (1 + t)^theta (y_b / Y_B)
  at the reference equilibrium, recovers 2.6200, 5.1900 and 10.5700 with pseudo-R2 1, so the
  flows and kernel are right and the gap is the ln distance regressor, which does not span the
  S05 costs (ADR 0022). Whether the gate should test theta on the model's costs is for the owner.
- Money headroom (ADR 0106): the mint, which holds minus the world's money (-1.15e16 minor units
  on the full pack), the pools, markets and gateways are `i128`, so the overflow the i64 ledger hit
  at a uniform scale of about 802 on prices, wages and balances (ADR 0089, Evidence) is gone
  (`mint_survives_scale_1000` runs the fixture with its mint at 1,000 times the pack's). Pops and
  enterprises stay `i64`: the largest single balance, and each payment (an `i64` entry), bounds the
  nominal scale now. On the installed 2000 pack the largest cell sales value is 7.0e10 numeraire
  per tick and the largest pop balance 1.4e10, so single sale entries bind at a uniform scale of
  about 1.3e4 and pop money at about 6.5e4 (`nominal_scale_beyond_the_ledger_fails_loudly`: on the
  fixture too a payment fails first). The currency scale is a P5 decision (core SPEC, Money scale).

## Changes for the next design-doc snapshot

Records for the refresh of `specs/DESIGN.md` and the Claude Docs design doc, which is done on the
owner's request in its own commit. Neither is edited here. DESIGN line numbers are from a re-grep at
rev 226 (Task 21); re-grep again before editing.

- Register (17.1): P39 to P43 are in DESIGN already (lines 1104 to 1108, with ADR 0019's renumbering of
  the old P37 to P41). Add:
  - P44 route-choice dispersion theta_r (ADR 0017; 41 to 182, value 111.5; unused in P2, deferred) and
    P45 expectation gain g of pi^e (ADR 0019; 0.004 to 0.65 per month, start 0.006), from the trade SPEC.
  - P46 PIGL price parameters (sigma_B, phi, gamma, with the Dt weights theta) and P47 PIGL Engel
    parameter eps (ADR 0021), from this SPEC. P06 stays the goods substitution within a PIGL category
    and P07 stays the S03 row, so DESIGN 1955 ("P06 and P07 become the PIGL price and Engel
    parameters") changes.
  - P03 splits into P03a kappa1 (start 0.05), P03b kappa2 (start 0) and P03c the step clip (0.1);
    P01 takes its class values 2.91, 4.14, 7.68 and 16.91 (ADRs 0023 and 0101; DESIGN 1061 says
    "centre 5"). P150 theta scale (ADR 0101; 1, lane {0.6, 1, 1.4} in tariff experiments).
    P03c's row says the clip bounds the gap terms and expected inflation passes through, and P151
    is the step clip c_max_storable of the storable commodities, goods 0, 1, 2, 5 (ADR 0106; 0.3 in
    log per month, oil fell about 30% in one month in late 2008).
  - P148 loss write-off threshold M, 3 months of base sales (ADR 0112, accepted), and P149 input stock
    cover, 1 month (owner decision, 2026-10-24; the `cover` of the order rule). Held or proposed, not
    built in P2: P143 kappa_n of ADR 0090 (proposed; the split price rule is parked on `p2-adr0090`),
    P147 M_F of ADR 0112's second decision (reserved, held; `p2-r3-producer-stock`) and P56 storage
    cost and convenience yield (ADRs 0029 and 0091, proposed; 0090 reads it).
  - DESIGN 2133 says P44 to P49 stay free for P2 work in progress; P44 to P47 are now taken.
- Section 21 and 22.1 (ADR 0012): the development-slice paragraph (DESIGN 1329) and the Map scope row
  (1394, "Full globe from P4") become the full globe from P2, with `dev_slice` kept in the pack for
  ad hoc fast runs and no gate depending on it. The P1 and P4 acceptance texts follow.
- Section 19.1 app stack (ADR 0088, accepted 2026-10-24; the earlier plan text named ADR 0011 option 3):
  fully native Rust, Slint panels over a wgpu map, no React, webview or Tauri; stage 1 of ADR 0011
  becomes a native spike instead of the hybrid. DESIGN 33, 1153, 1162, 1163, 1208 and 1398 carry
  "today Tauri, native later" with ADRs 0011 and 0088 together; they should name 0088 as the decision.
- Route choice deferred past P2 (ADR 0017, Deferral, owner 2026-10-24): P2 keeps least-cost legs (S05)
  with the gateway kernel, and the route-choice module stays in `ceteris-geo` and `ceteris-trade`,
  tested and off (`Network.route_taus = None`; P44 unused). DESIGN still describes route choice as the
  P2 cost model at 257 and 265 (5.4), 441 (8.3), 1242 and 1245 (19.4, "exact under route choice"),
  1337 (P2 in 21.1, "over route-choice costs"), 1788 (the ADR index, "changes P2 output: Yes"), 1856 and
  1875 (ADR 0017 Decision and Consequences). The line 1324 named by the earlier audit is the opening
  of Section 21 and mentions none. The revisit trigger is the P8 corridor network.
- Demand (8.6, ADR 0021 as built): PIGL over three categories, A (goods 0, 3), M (goods 1, 2, 4, 5, 6)
  and S (goods 7, 8, 9), the ones Alder, Boppart and Mueller (2022) estimate, with a CES over the goods of
  each at P06; DESIGN 459 names five (food, energy, housing, manufactures, services). The Gini feeding
  PIGL is WDI, else SWIID (ADR 0021).
- P2 acceptance (DESIGN 1338, "ADRs 0015 to 0022 implemented"): 0015 to 0023, 0089 and 0112, plus the
  gate decision below.
- What P2 as built adds, one line each:
  - Trade elasticity in 4 classes (2.91, 4.14, 7.68, 16.91), harmonic means of the OECD key's line
    thetas on the Caliendo-Parro replication vector, drawn by ADR 0023's rule (ADR 0101), with a
    per-class H2 prior band of two standard errors (1.02 to 4.81, -3.97 to 12.26, 5.94 to 9.42, 8.33
    to 25.50). Refined petroleum (ICIO C19) is in good 1 (ADR 0092).
  - Calibration to a data state that is the steady state, and a natural rate u* in the wage rule
    (ADR 0089), replacing full employment; the productivity A is calibrated, not estimated.
  - National profit ownership (owner, 2026-10-25; ADR 0089): the enterprises' profits go to the pops of
    their country by population, wages stay local.
  - Limited liability of enterprises and the loss write-off beyond M = 3 months of base sales
    (ADR 0112, P148); markets keep the signed rule.
  - The `source_dispatch` budget re-set to 8 ms and the whole tick to 35 ms on the full pack (owner,
    2026-10-26); DESIGN 1181 and 1845 state 5 ms for the trade system, where this SPEC budgets trade
    at 30 ms with `source_dispatch` at 8 ms; the snapshot takes this SPEC's values (controller ruling,
    2026-10-26, following the owner's re-budget). The gateway count and rule move to the ADR 0092 rework.
  - The gate decision on V1 (owner, 2026-10-26): V1 is the run from the perturbed steady state; the run
    from the shocked data state is a stress test (`xfail(strict)`), and ADR 0090's price rule is
    redesigned after the gate.
