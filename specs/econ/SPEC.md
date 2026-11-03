# Econ SPEC

Status: approved (owner, 2026-10-23); P2 gate passed (owner, 2026-10-26, p2-econ c9b6e11). V1 from the
perturbed steady state 9.086e-3 (the run from the shocked data state is an `xfail(strict)` stress test);
H2 distance -0.744 (s.e. 0.015), H2 per class 1.68, 3.53 and 9.08, all in their bands (whole provinces,
ADR 0251; -0.753, 1.75, 3.72 and 9.52 on the split pack); 100 years without
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
| Input-output coefficients a_kg as quantities (7.2) | ICIO coefficients are cost shares at base delivered prices: coef_e,k = a_kg p_o / q_k(base), q_k(base) the base delivered unit price of k at the enterprise's province (`ProvinceData::base_price`: the country's price level p_c for local goods (ADR 0098), sourcing at prices 1 for traded goods, tariffs and iceberg included; coefficient 0 where unreachable), under both S09 settings (owner decision, 2026-10-25; ADR 0020 mapping) | At base prices every enterprise's input cost share equals the ICIO share, a_VA = 1 - sum_k a_kg > 0, and the national IO value structure is exact; read as quantities, delivered prices above 1 made inputs cost more than output for 11.5% of traded VA at prices 1 |
| Within-tick rationing, inventories as the buffer (8.2, 19.4, ADR 0020) | Local goods (utilities, construction, services): a same-tick chain at E2 per province (`local.rs`, Production below): a shortage of a local good cuts its users' output in the tick it happens. Traded goods keep their stocks and transit lags. Departures from the migration brief's chain, each needed for exact Leontief use: (1) a producer's own use of its good comes out of its output first, since cutting the producer pro rata with its users leaves the ratio of output to need unchanged and with empty stocks drives the pair to zero; (2) each user covers its residue from its own stock (the ledger holds stocks per enterprise), so the stock that counts for a good is sum min(b, n), and output is split so that each user's residue fits its stock; (3) one fill per local good, every user of the good at the smallest fill of its local inputs, solved exactly as the limit of the fill iteration (ADR 0093, Amendment), with no pass bound and no clearing step, so input use always equals coef Y exactly | ADR 0020 Mapping (owner, 2026-10-24): local goods have no transit, so their production, input use and rationing run as one pass within the tick |
| VA over K, effective L by education, land or resources T (7.2) | Normalized CES over one fixed factor F (capital, land and resources together) and effective labour | No investment in P2, so K and T never move separately; one fixed factor keeps the reference model small |
| Pops keyed by province, ethnic group, education, labour status (6.1) | Province x education (3 levels); labour status as employed and unemployed counts inside each pop | Ethnicity and status transitions are P4 |
| Wage posting and matching function (8.7) | Wage moves with excess labour demand, with downward rigidity; employment rationed pro rata | Matching and occupational mobility are P4 |
| Explicit fiat money through banks (A9) | Numeraire-only mode (A9 alternative); every account may run a negative balance at an interest-free clearing facility | No banks before P5 |
| Final demand split into households, government, investment | One final demander per pop, with country shares from all ICIO final demand | No state or investment in P2 |
| Observed 2000 trade imbalances | Exogenous transfers D_c through a world transfer account (ADR 0100 as amended by ADRs 0295 to 0300, Transfers below): pops spend their income plus their share of D_c; balanced trade is the switch's Off setting | Endogenous current accounts need finance (P5) |
| Agglomeration in productivity (A5, P09) | Off by default | Ablation rule: on only once it improves a test |
| Buyers face posted prices (8.6, 7.3) | Traded goods are bought at the buyer price q_d = Gamma_d P_d of the buyer's province, the CES index times a selection constant per cell fixed at the calibration point (trade SPEC, Sourcing; ADR 0200, owner ruling 2026-10-30), in household demand, input orders and the value-added price | Destination sourcing gives no single posted price at a destination; with selection on trade links (Eaton-Kortum) every source delivers at the index, and Gamma keeps q_d at the calibrated point where 1 / alpha_d put it before ADR 0200 (owner decision, 2026-10-24) |
| Inventory coverage at the market (7.3) | Enterprises order inputs up to an inventory position (stock plus goods on order) of (1 + cover + L) months of capacity use, cover = 1, L the expected months in transit (owner decision, 2026-10-24); an input that caps the enterprise's labour is ordered on its notional output instead (effective demand, owner decision 2026-10-30T22:43Z, ADR 0266) | Goods paid at dispatch and delivered after transit must be counted, and the target must hold them: a two-month position cannot hold a month in transit |

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
| `econ_goods.arrow` | 10 | name, traded flag, weight per value w_g (t per unit), inventory coverage prior; optional storable flag, tariff group (0 primary, 1 manufactured) and PIGL category (0 A, 1 M, 2 S) | BACI HS92 2000 (tonnes and values, OECD BTiGE HS to CPA key) |
| `econ_countries.arrow` | 242 | ICIO economy or imputation donor, VA basis and income source, IO basis (`io_basis`: `icio`, `ras` or `donor`, ADR 0286; Donor table balance, below), national VA by good (10), final demand shares (10) and government's (`gov_g`, GGFC and NPISH) and investment's (`inv_g`, GFCF) parts of them (10 each, ADRs 0099, 0302: each good's inventory change shared over the three in proportion to their purchases, households the rest), labour share, unemployment rate, natural rate u* (mean unemployment rate 1995 to 2005, ADR 0089), the labour share of each good (`labour_share_g`, 10, with `labour_share_pooled` and `labour_share_clipped`) and the wealth shares of the bottom 50%, middle 40% and top 10% (`wealth_0` to `wealth_2`, with `wealth_source` `wid`, `region` or `world` and `wealth_clipped`; Factor shares, below; ADRs 0307, 0308), where SEA and WID are in the fetch cache, participation rate, working-age share, education shares (3), GDP 2000, Gini 2000 and its source (`gini_source`: `swiid`, `wdi` or `donor`), coal share of good 2, price level p_c (`price_level`, the base price of local goods, ADR 0098, with `price_level_source` `pwt`, `wdi` or `donor` and `price_level_imputed` where it is not PWT's; Price levels, below); current account 2000 (`ca_2000`, USD, 0 where missing, with `ca_missing`), the transfer share D_c / W of 2000 (`transfer`) and of each year 2000 to 2024 (`transfer_2000` to `transfer_2024`; Transfers, below); an imputed flag per series | ICIO, PWT 11 (labsh), ILOSTAT (modelled estimates: 2000 rates, and 1995 to 2005 for u*), WIC reconstruction (Barro-Lee cross-check), WDI; Gini (ADR 0098): SWIID disposable-income Gini (CC0) nearest year 1995 to 2005, else WDI SI.POV.GINI (income and consumption surveys mixed, flagged imputed), else the donor's (flagged imputed); WIID is non-commercial; price level (ADRs 0282, 0289, 0294): PWT 11 pl_da, else the level WDI's NY.GDP.MKTP.CD, NY.GDP.PCAP.PP.CD and SP.POP.TOTL imply (CC BY 4.0), else the donor's; current account (ADRs 0100, 0296, 0300): WDI BN.CAB.XOKA.CD (CC BY 4.0), GDP of each year WDI NY.GDP.MKTP.CD; labour shares by good: WIOD SEA 2016 LAB and VA of 2000 (CC BY 4.0); wealth shares: WID shweal992j p0p50, p50p90, p90p100, nearest year 1995 to 2005 (terms to confirm, `closed_source_only`) |
| `econ_io.arrow` | countries x 10 x 10 | input coefficient a_kg (units of k per unit of g) | ICIO 2000, aggregated; imputed economies the donor's, RAS-balanced where `io_basis` is `ras` (BACI 2000 trade by good) |
| `econ_provinces.arrow` | 4,594 | population 2000, working-age population, VA by good (10, million USD per year), with an imputed flag and the proxy level per good | WorldPop 2000 aggregated through the H3 cells, scaled to WDI national totals, else UN WPP 2024 totals (Western Sahara: WDI has no row, WorldPop's cells give 811 people against WPP's 271,943; ADR 0098); VA downscaled as below |
| `econ_pops.arrow` | 4,594 x 3 | province, education level, population, working-age population, labour force, imputed flag | provinces split by their country's education shares; labour force at the country's participation rate of the working-age population |
| `econ_tariffs.arrow` | importer x exporter x 2 groups | applied tariff, primary and manufactured; agreement and imputed flags | WDI weighted applied rates by importer (TM.TAX.TCOM.WM.AR.ZS, TM.TAX.MANF.WM.AR.ZS); zero inside agreements and customs unions (CEPII gravity, WB DTA), as below |
| `gateways.arrow` | about 600 | node, country, kind (port, airport, land crossing), mass (share of the country's trade throughput, ADR 0092) | P1 graph, BACI 2000 trade by country |
| `econ_home_shares.arrow` | countries x 7 traded goods | country, good, home_share (ICIO 2000 domestic-origin use over total use, intermediate plus final demand without DPABR), absorption (total use, million USD), icio (an ICIO economy; outside ICIO the share and absorption are NaN, as is the share of a good an ICIO economy does not use) | ICIO 2000 (ADR 0092) |

**Goods count.** The rows of `econ_goods.arrow` are the world's goods (`Goods`, trade SPEC):
every per-good column, the IO table's width and every goods-indexed array follow its row count.
Traded goods must come first. A table without the storable, tariff group and category columns
must hold the P2 goods (the 2000 pack, format 3), whose values those are; a table of other goods
must carry them. The inputs digest covers the goods table only where it differs from P2's, so
the P2 digests and golden hashes do not move.

**Coverage.** `TradeParams::coverage` (P02, trade SPEC) is the authoritative value: the model, the
snapshot digest and the export (`goods.arrow`, which the GE reference reads) all use it. The
`econ_goods.arrow` column is the pack's copy of the default, never read by Rust; it must equal
`TradeParams::default().coverage`, and a change to one is made to both.

**Factor shares (ADRs 0307, 0308; `econ::factors`).** Labour shares:
s_L,c,g = s_L,c r_g,c / (sum_h VA_c,h r_h,c / VA_c), s_L,c the PWT share, VA_c,h the table's
national VA by good, r_g,c = (LAB_g / VA_g) / (LAB / VA) from SEA 2000 summed over the country's
economies (WIOD's one mining industry B counts in full for goods 1 and 2); a good SEA does not
give takes the mean r of the 5 SEA economies nearest in log GDP per person (PWT), else 1
(`labour_share_pooled`). A share outside [0.05, 0.95] is clipped and the rest rescaled until the
VA-weighted mean is s_L,c again (`labour_share_clipped`; `labour_share_va_weighted_mean_equals_pwt`,
1e-12). Wealth shares: WID's three shares, a negative one at 0 and the three rescaled to sum to 1
(`wealth_clipped`); a country without WID takes the population-weighted mean of the WID
economies of its World Bank region (`region`), else of all (`world`)
(`missing_source_falls_back_with_flag`). WID is keyed by ISO alpha-2, mapped by Natural Earth's
ISO_A2_EH.

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
always against the donors' in the same source, never the region median when an income exists.
PWT income is set aside where it falls more than 3 times below WDI's at the US ratio of the two
(PWT_c / WDI_c < (PWT_USA / WDI_USA) / 3, `PWT_BELOW_WDI`; ADR 0098): PWT is at PPP and WDI at
market rates, so PWT above WDI is the Balassa-Samuelson norm (124 economies differ by more than 3
times that way), while PWT a third of the US ratio below WDI would need a price level above 3
times the US's (Japan's, the highest in 2000, is 1.4). Only Venezuela falls there (PWT 490 against
WDI 4,776 USD per person in 2000, 0.068 of the US ratio; the next lowest, Barbados, is 0.55); it
ranks and scales by WDI income, flagged `income_source` `wdi_not_pwt`. With manufacturing missing, industry goes over goods 1 to 8. With
agriculture, industry or services missing, the known shares keep their goods and the goods they
do not cover take WDI GDP times the ICIO median VA over GDP less the known shares (at least zero),
by the donor's proportions; with no share, GDP times that median. Where WDI has oil or natural gas
rents, crude oil and gas takes GDP times the rents times the ICIO ratio of crude oil and gas
extraction VA (B06, B09 output less their inputs, without refining; ADR 0281, proposed) to rents,
out of its group (all of industry where industry less manufacturing is too small) and at most the
largest ICIO share of that extraction VA in industry VA of it (ADR 0249), with the ICIO
world crude oil and gas extraction (B06, B09, without refining) inputs per unit of output,
weighted by extraction output, in place of the donor's (ADRs 0256, 0270); with no GDP, population (WDI, else UN WPP,
else the WorldPop national total) times the donor's VA per person scaled by income. Regions the World Bank
does not give come from an override table: Western Sahara Middle East and North Africa, Northern
Cyprus Europe and Central Asia, Somaliland Sub-Saharan Africa, and Anguilla and Montserrat Latin
America and Caribbean (dependencies the World Bank does not classify, whose sovereign's region,
Europe and Central Asia, gave them a European donor; ADR 0098). A territory inside its
parent's WDI totals (Somaliland, Aland, Northern Cyprus) takes its population share of the
parent's VA out of the parent's. Uninhabited territories keep zero VA. Each row carries the
basis (`icio`, `wdi`, `wdi_gdp`, `donor`, `parent`, `none`) and the income source.

**Donor table balance** (`econ::icio::impute`, `econ::ras::balance`; ADR 0098 as amended by ADR
0286, RFC 0282 D6, which supersedes D3). The donor's coefficients are balanced to the economy's
own totals by RAS (Stone 1961; Miller and Blair 2009, ch. 7), for every imputed economy with VA
basis `wdi`, `wdi_gdp` or `donor` and BACI 2000 trade. Seed: the 10 x 11 use table [Z | F], with
Z_kg = a_kg X_g, the donor's coefficients (with the world crude oil and gas column where rents
set good 1) at the economy's output X, and F_k = f_k (VA + M - E), the donor's final demand shares
f times the economy's final demand. M_k and E_k are BACI 2000 imports and exports of good k
(`read::baci::trade_by_good`: HS lines to goods by the BTiGE key as for the goods' weights,
million USD, the economies a pack country combines summed), VA, M and E without a subscript their
sums over goods. Row targets, domestic use of each good: u_k = X_k + M_k - E_k. Output enters
because ICIO's uses count every source economy (`CountryIo::io` and the final demand shares sum
the sellers of all economies), so a good's use at home is what the economy makes less what it
sells abroad plus what it buys abroad. Local goods (7 to 9) have no BACI trade and are not traded
in the model, so their use is their output. BACI's FOB values enter as they are; the margins
between them and ICIO's basic prices are not corrected. Column targets: the intermediate inputs
of each good, X_g - VA_g (the seed's own column sums, as VA_g = X_g (1 - sum_k a_kg)), and final
demand VA + M - E; rows and columns both total sum_k (X_k + M_k - E_k). Tolerance 1e-10 relative
on every row and column total, at most 200 passes of a row then a column scaling (ADR 0098). The
balance moves uses only: output and VA stay, so VA + inputs = output holds as before. The
economy's coefficients become a_kg = Z_kg / X_g (a good without output keeps the donor's column)
and its final demand shares F_k / sum_k F_k (`io_basis` `ras`). The donor's table is kept
(`io_basis` `donor`) where a target is negative, u_k < 0 for some good (exports above output plus
imports: RAS cannot reach a negative total from a non-negative seed), where the economy has no
BACI trade, and for VA basis `none`; a territory inside its parent (`parent`) takes its parent's
table and IO basis (the parent is balanced on its whole, before the territory takes its share). A positive target on a row the seed leaves empty, or a balance that does not
converge, stops the build. Rest of world (reported, not a constraint): the imputed economies' VA
by good against ICIO's ROW block (`read::icio::aggregate` keeps it as ROW), in the build summary.
Built: 87 economies balanced (88 rows `ras` with Somaliland), 48 keep the donor's table for a
negative domestic use, 15 for no BACI trade, 9 have no VA; 10 to 56 passes (median 19), worst
residual 1.0e-10 on rows and 4.9e-16 on columns; imputed VA 1,014,073 million USD against ROW's
1,019,231 (0.9949; by good 0.674 to 1.330).
Tests (`tests/ras.rs`, `tests/icio.rs`): `ras_balances_a_rectangular_table`,
`ras_balances_the_donor_use_table_to_domestic_use`,
`negative_domestic_use_keeps_the_donor_table`, `no_trade_keeps_the_donor_table`,
`territory_inside_takes_its_parents_balanced_table`; `tests/read_trade.rs`
`trade_by_good_sums_exports_and_imports_by_the_good_of_each_line`; on the built tables
`econ_build.rs` `donor_tables_are_balanced_to_domestic_use`.

**Price levels** (`econ::price_level`; ADR 0098 as amended by ADRs 0282, 0289 and 0294, RFC 0282
D1, D7 and D9). A country's price level p_c, the base price of its local goods, is PWT 11 pl_da (the
price level of domestic absorption, C + I + G) over the US's; else the level WDI implies
(`wdi_implied`), NY.GDP.MKTP.CD over NY.GDP.PCAP.PP.CD times SP.POP.TOTL (GDP at market rates over
GDP at PPP), over the US's; else the level
of the country's imputation donor (`donor`) by the same two rules. Each series is read at the
year nearest 2000 within five and divided by the US's of that year (PWT: Curacao and Sint Maarten
2005, South Sudan outside). A country of several economies takes the mean of their levels and
incomes. Screen (`Screen`): Tukey's far-out fences, 3 interquartile ranges beyond the quartiles
(`FAR_OUT`, quartiles by linear interpolation), first on the logs of the 184 PWT levels (0.0250
to 8.84 times the US), then on the residuals of the Penn effect ln p = a + b ln y, y WDI GDP per
person at market rates, fitted by OLS on the PWT levels inside the first fences: over 180
economies ln p = -2.6571 + 0.2453 ln y (R2 0.611), residual fences -1.198 to 1.188. A PWT or
WDI value is kept when its residual lies inside them, or, with no WDI income (Anguilla,
Montserrat, Taiwan, the British Virgin Islands), its level inside the level fences. Rejected:
PWT Somalia (1.04) only; no WDI level is rejected. WDI has no GDP at PPP for Cuba, New Caledonia,
French Polynesia and Kosovo, which take the donor's. (Before ADR 0289 the fallback was the WDI
Database Archives file of PA.NUS.PPPC.RF, whose US row is not constant and whose 2000 values the
screen rejected for Afghanistan 0.025, Greenland 0.079, Nauru 0.056, East Timor 0.003, the US
Virgin Islands 0.117 and others.) Built: 181 pack countries from PWT, 21 from WDI, 40 from the
donor; p_c from 0.127 (Tajikistan) to 1.80 (Bermuda), Somalia 0.827 (WDI). Values stay at 2000
market rates: ADR 0098's rescale of ICIO blocks to 1999-2001 rates is dropped (ADR 0283). Tests
(`tests/price_level.rs`): `pwt_reader_keeps_the_domestic_absorption_price_level`,
`pwt_reader_keeps_the_price_basis_of_2000`, `wdi_level_is_gdp_over_gdp_at_ppp`,
`levels_are_relative_to_the_united_states_in_the_same_year`,
`fences_are_tukeys_far_out_fences_on_the_logs`,
`the_screen_fits_the_penn_effect_and_fences_its_residuals`, `pwt_first_then_wdi_then_the_donor`,
`values_the_screen_rejects_fall_to_the_next_source`,
`a_country_of_several_economies_takes_their_mean`, `no_level_and_no_donor_is_an_error`; on the
built tables `econ_build.rs` `price_levels_come_from_pwt_then_wdi_then_the_donor`.

Real GDP check (ADR 0098, Tests; ADRs 0285, 0290 and 0294, RFC 0282 D5, D7 and D8). For each
ICIO economy WDI reports, the pack's real GDP, `gdp` over p_c, against WDI NY.GDP.MKTP.PP.CD 2000
(current international dollars, so of 2000 prices as the pack's values are; validation only,
`validation.toml`). Totals, not per person: WDI's SP.POP.TOTL counts all of Cyprus while its GDP
covers the south, which put Cyprus at 0.708 per person (0.967 on totals). The pack's `gdp` equals
WDI's, so the ratio is WDI's price level over PWT's. Enforced where PWT's `i_cig` of 2000 is
Benchmark (31 economies, mostly Eurostat-OECD): within 10% (P160, `BENCHMARK_TOLERANCE`, the
measured agreement of two vintages of the same benchmark PPPs). Measured: 31 of 31 within 10%, 23
within 5%, the largest gap the UK's 1.093 (Portugal 0.912, Norway 0.915, Greece 0.930). The
other 48 (37 Interpolated, the US among them, and 11 Extrapolated) are reported with `i_cig` and
not gated: PWT interpolates between ICP 1996 and 2005 there or extrapolates back from 2005, WDI
backcasts from ICP 2011. Over
all 79: 35 within 5%, 51 within 10%, median 0.955 (0.165 Nigeria to 1.88 DR Congo), slope of the
log ratio on ln income 0.0523, correlation 0.267, pinned (`econ_build.rs`
`real_gdp_against_wdi_ppp`).

**Transfers** (`econ::transfers`; ADR 0100 as amended by ADRs 0296, 0298 and 0300, RFC 0287).
A pack country's current account CA_c is the sum of BN.CAB.XOKA.CD over its WDI economies
(`parts`) in the year; a country none of whose economies reports it is 0 and flagged
(`ca_missing`, 2000 only). Its transfer share is minus its current account less its GDP share of
the world discrepancy, over world GDP:

```
s_c = -(CA_c - GDP_c sum_c' CA_c' / W) / W,   W = sum_c GDP_c
```

so the shares sum to zero over the pack's countries (up to rounding) and a country with D_c > 0
runs a deficit. For 2000 (`transfer`, the fixed-share mode) GDP_c is the pack's `gdp` column (ADR
0298); for each year y of the path (`transfer_y`, 2000 to 2024, ADR 0300) GDP_c is the sum of
NY.GDP.MKTP.CD over the country's economies in year y and W the sum over the countries that report
it; a country with no GDP that year takes no part of the discrepancy. A pack without these columns
(built before ADR 0100) loads with no transfers.

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
network (system T0 of the trade SPEC; the caller triggers it in P2, through `Sim::intervene` so a
seek replays it, and shocks arrive in P3).

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
| Sector, productivity A, base VA0, base employment L0, labour share s_L | enterprise (province, sector) | `f64` | | calibration table; in the data state A = 1 / p_o (1 for traded goods, 1 / p_c for local goods, ADR 0098); s_L its country's for its good (`Countries::s_l_good`, ADR 0104), else national |
| Employment L | enterprise | `f64` | effective labour units | base L0 |
| Output, sales, profit last tick | enterprise | `f64` | | |
| Input coefficients coef_k (putty-clay, ADR 0020; static in P2) | enterprise x 10 goods | `f64` | units per unit of output | one vintage, the 2000 column `EntData::column`: calibrated a_kg p_o / q_ik (calibration tables), else the country's ICIO column at base delivered prices, a_kg p_o / q_k(base), p_o the base price of the output (1 for traded goods, p_c for local goods, ADR 0098) |
| VA coefficient a_VA | enterprise | `f64` | real VA per unit of output | calibrated 1 - sum_k a_kg, else 1 - sum_k coef_k q_k(base) / p_o (1 - sum_k a_kg) |
| Input stock | enterprise x 10 goods | `f64`, held in the ledger | units | calibrated (1 + COVER) months of use, else one month of base input use |
| Wage w | province | `f64` | numeraire per effective unit | base w0 = sum_g s_L VA0_ig / ((1 - u*_i) L^s_i) |
| Wage clip ticks: ticks at which the wage step clip bound | province | `u32` | ticks | zero |
| Effective labour supply L^s | province | `f64` | effective units | sum_k e_k LF_k |
| Accounts | pops, enterprises, markets, gateways, transit (per traded good), rebate and profit pool (one each per country), mint, world transfer account (only where S22 is on and some transfer share is nonzero, ADR 0100) | ledger | money in minor units (1 minor unit = 1e-4 numeraire): `i64` for pops and enterprises, `i128` for markets, gateways, rebate and profit pools and the mint (ADR 0106); goods `f64` | |

**Initial state.** A world whose inputs carry the calibration tables (`EconInputs::calibrate`,
`EconInputs::from_pack` when the pack holds them) starts from the calibrated steady state of ADR
0089 (Calibration, stage 5): its posted prices, last D, Q and sales, merchant and input stocks,
the pipeline, each pop's money (dividends and rebates; wages come at E2) and last income, each
enterprise's last output, sales and profit; wages w0, employment L0, unemployment u*, pi^e = 0
and buyer prices refreshed at the posted prices. Otherwise it starts from the data state: prices
1 for traded goods and p_c for local goods (ADR 0098, below), the same base point, input stocks at one month of base use, merchant stocks at coverage times
the origin's own base output of the good (s_g Y0 at every active traded cell, Y0 = A VA0 / a_VA
of the province's enterprise of that good, the base output that sizes the input stocks; local
goods hold none, as before), nothing in flight and one month of base dividends minted to each
pop. The stock ignores storage loss: with competitive storage on, the steady state holds
coverage times sales, Y / (1 + delta s) (Calibration, stage 5), so the data state starts above it
by the factor 1 + delta s; with storage off (the default, delta 0) the two agree.

**Load guard and interventions** (ADRs 0291 to 0293). A calibrated start runs only under the structural
parameters its tables were built under (`structural_digest`: the defaults with the calibrated
border friction, `EconInputs::trade_params`); other structural parameters panic at
`EconWorld::new`, and `ceteris run --data-state` starts the calibrated inputs from their data
state instead (it needs a calibrated pack; an uncalibrated pack starts there anyway). A run under
other structural parameters starts from the baseline steady state as an ex-post intervention:
load under the calibration's parameters, then `EconWorld::set_params` or `set_trade_params`
between ticks (`Sim::intervene` records it for seek). An edit leaves the dynamic state as it
is and rebuilds the kernel and the layout under the new parameters, so a structural edit moves
the world off its steady state, an edit of the PIGL parameters too, since the calibrated weights
were inverted at the defaults. `set_network` is the same kind of edit. Snapshots carry both
parameter sets and the digest of the network, and a restore returns to them, so a seek across
an edit replays it (format 3).

Merchant stocks of the data state (amendment 2026-10-31, B8e; owner, 2026-10-31, relayed by the
coordinator). The
data state stocked each origin at coverage times the orders one month of base demand placed there
at prices 1, where the origin weights are the kernel alone, so an origin's stock did not scale
with what it makes. A tiny economy then held months of the world's orders and sold them at tick
1: Pitcairn (province 2892, 25.6 workers) started with 5.209e8 units of goods 0 to 6 against a
pop money of 8.85e3, the world bought them at tick 1 (TRADE 5.2087e8 numeraire at price 1), and
E4 paid the market's cash flow to its pops, 58,800 times their money; Western Sahara 10,100
times, the Indian Ocean Territories 5,600, Niue 4,600, Norfolk Island 2,100 and St Helena 2,100
(B8d). North Macedonia's tick-1 sale (345 times its pops' money) ran its wages far above w0,
whose losses the write-off and the markets' signed cash flow turned into pop debt, and its eight
provinces with only local goods locked at a zero wage. Money and goods are conserved through all
of it (the stocks are created at load), so no invariant sees it. Inventories scale with the
holder's own sales (inventory-to-sales ratios), and at the steady state the two rules agree
(Calibration, stage 5: S* = coverage D with D = Q), so the data state now stocks coverage times
the origin's own base output, what it can supply. Output, not sales, since the data state has no
sales yet; base output, since the data state's prices are 1 and Y0 is its output at base
employment. A calibrated start does not change: its stocks are the calibration tables'. Test
(`a_tiny_economy_has_no_tick_one_windfall`, both modes): the fixture's island at 1e-4 of its
population and value added (60 people, Pitcairn's scale), from the data state. After tick 1 its
pops may hold at most their money before, plus one month of the island's base value added, plus
(1 + max s_g) e^(max c_g) times its base traded output at prices 1: what a month of its own
activity, and the sale of its whole stock and a month's output at the step-clipped price, can pay
them. Under the old rule they held 5,850 times their money before.

Local base price (ADR 0098). Each country's local goods (utilities, construction, services)
start at its price level p_c (`Countries::price_level`, from `econ_countries.arrow` `price_level`,
1 where the pack predates the column). Values stay those of the data: for a local enterprise p_o =
p_c, A = 1 / p_c, so real output is Y0 = A VA0 / a_VA = V / p_c and nominal value added at base
employment is VA0; coef_k = a_kg p_o / q_k and a_VA = 1 - sum_k coef_k q_k / p_o = 1 - sum_k a_kg,
so every enterprise's input cost shares are the ICIO shares at base prices. The base delivered
price of a local good (`ProvinceData::base_price`, `world::base_delivered_prices`) is p_c, the
data state's local market prices are p_c, and the PIGL weights are inverted at the data state's
prices, 1 for traded goods and p_c for local goods, so households' t = 0 shares are Omega in
value. Traded goods are untouched: their origin prices start at 1, their base delivered prices
and Gamma come from sourcing at those prices. With p_c = 1 everywhere every quantity is the one
before, bit for bit (A = 1 / 1, a_kg 1 / q, coef q / 1), and so is every golden; the pack's
p_c come from Price levels (Pack additions). The world price index of the run
(`EconWorld::world_price_index`) keeps its weights, base consumption values over all goods.
Tests (`crates/ceteris-econ/tests/p2_properties.rs`): `price_level_values_unchanged`, the fixture
with p_c = 1.3 in one country: at t = 0 only that country's local base and market prices move
(by 1.3), local units by 1 / 1.3 and local input coefficients of its local producers by 1.3,
while every enterprise's nominal VA, output value and input cost shares, every pop's money and
base income, household value shares and the national accounts (VA and final demand by country
and good) are the p_c = 1 world's to 1e-12; `price_level_one_is_the_data_state`, the same world
at p_c = 1 has the fixture's digest. The GE reference reads p_c as the base price of local goods
(`Inputs::data_state_price`), and the calibration keeps local goods at p_c (stages 2 and 3; the
block gain of ADR 0267 normalizes a household's local spending by the value of the cell's output,
p_c times its units). Tests (`crates/ceteris-calibrate/tests/price_level.rs`, same fixture):
`reference_reads_the_price_level` (data-state prices, A, a_VA and coefficients bit for bit, PIGL
weights to 1e-12), `v1_price_level_matches_reference` (the frictionless run from the data state
against the reference solved from the export, V1's 1%; measured 5.9e-6 on prices, 2.5e-6 on
wages), `calibration_keeps_local_goods_at_the_price_level` and
`calibrated_price_level_world_is_stationary` (120 ticks from the tables, every price and wage
within 1e-9; measured 3.9e-12).

Enterprises exist for every (province, traded good) with VA > 0 and every (province, local
good). About 4,594 x 10 rows at most, sorted by province (CSR offsets, 19.4). Every pop and
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
one fill F_k in [0, 1] per local good k (ADR 0093, Amendment; stock floor, amendment of
2026-10-30 (2)):
    Y_e  = cap_e min over local k used by e (its own good left out) of G_ek(F_k)   (cap_e if none)
    G_ek(F) = s_ek + (1 - s_ek) F, 1 at F = 1      s_ek = min(b_ek / (coef_ek cap_e), 1)
    R_k  = (1 - coef_kk) Y_p                                    p the producer of k: own use first
    U_k  = sum_e max(coef_ek Y_e - b_ek, 0)                     b_ek = e's stock of k
    k fits when U_k <= R_k
F_k is the share of each user's uncovered need at capacity, (coef_ek cap_e - b_ek)^+, that k's
output serves: where k binds a user, its draw from output is F_k times that need, and its stock
covers the rest. A user whose stock covers its whole need at capacity is not cut by k (s_ek = 1),
and with no output of k at all a user still runs on its stock (G_ek(0) = s_ek)
F = the limit of the fill iteration: from F = 1, passes over k = 7, 8, 9, each setting
    F_k = largest x <= F_k at which k fits, the other fills as they stand
solved exactly: each step is the root of a piecewise-linear U_k on its piece (kinks where one of
a user's levels G becomes its smallest; a user k binds adds (coef cap - b)^+ F_k); a fill below the
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
fills land within 1.08e-13 of it on the translated collapses under the stock floor, 1.72e-12 on
goods the tests loosen (1.81e-12 before it; tests cap 1e-11; the per-case eps (U + R) / |slope|
has no bound where a good's U - R is about 0 along the ray); a last pass that sits on pieces
reaching only just below the limit has a fixed point far below it, off its pieces, which is not
taken (seed 13067305529229119429 translated to 0.3 ended at [0.267, 0, 0] against t when it was,
before the stock floor; no translated collapse or random province under the floor reaches that
case, 3,000,000 and 1,000,000 probed; the hand-built regression is a user whose stock comes to
cover it between f and the fixed point, `local::tests::the_stop_refuses_a_fixed_point_past_a_cover_point`)
non-uniqueness at gain exactly 1: where the province has a direction of loop gain exactly 1 (a
line of fixed points, as where a good's producer supplies exactly what its users without stock
need along a ray), the fills are not unique: the stop may take any fixed point on the last
pass's pieces, which need not be the limit of the iteration from 1 defined above. Every good fits
there and the invariants hold, and the result is deterministic, but it can over-cut: a
translated collapse balanced at t = 0.1 by an extra user ends at [0.093, 0.013, 0.013], every
user's output at most its output at t (`neutral_good_stop_takes_a_fixed_point_on_its_line`).
Such provinces are of measure zero; the translated collapses exclude them (`translatable`)
draws: with output enough, every user draws n_ek = coef_ek Y_e from output; short, each draws
its uncovered need u_ek = coef_ek cap_e max(min over its inputs k' of (G_ek' - s_ek), 0), at most
n_ek (n_ek where b_ek = 0; G_ek' - s_ek written (1 - s_ek) F_k for k itself and
(s_ek' - s_ek) + (1 - s_ek') F_k' otherwise, so that the stock's share does not cancel to the
rounding of b_ek), plus the rest of R_k pro rata to min(b_ek, n_ek), its stock the remainder;
U_k is the sum of the u_ek, and s_ek is rounded down until coef_ek (cap_e s_ek) <= b_ek
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
services at 0.3 and construction and services each other at 0.2 it binds in 482 of 3,000 province
ticks over 120 ticks (483 before the stock floor; ADR 0266: 415 before labour was capped at what
traded inputs allow, 547 with the cap before the split order rule), each within the passes its
kinks allow, without a stall
(`local_chain_terminates_on_cycles`). Where every jump applies (a resolvable fall per cycle, on
pieces that repeat within 3 passes) the passes are at most 2 + 3 + 2K: one from F = 1, which
cuts only the goods short there; at most two per kink the fall crosses (the pass across it, and
one more where that pass straddles it so that its map does not repeat), since each jump from a
cycle on its pieces ends at the limit or after the last whole cycle before the pieces change;
and at the limit up to 3 confirming passes and the pass that changes nothing. K counts the
users' floors inside (0, 1) (0 < b < coef cap; the user's stock comes to cover it where its
smallest other level falls to its floor, once, as the fills only fall; before the stock floor,
the threshold where its own fill fell to b / (coef cap)), each local good beyond the first that
starts to fall, and each pair of local goods some
user takes both of (a switch of the binding fill, taken once per pair: an assumption, not
derived; a pair can switch more than once, and the allowance of two passes per kink has held
those seen; under the stock floor each user of both goods switches on its own line,
s_j + (1 - s_j) F_j = s_k + (1 - s_k) F_k, so a fall may cross one switch per such user, not per
pair, and the bound does not count the others). The most is 14 passes, against its own bound
of 25: province 10 at tick 76, where utilities and construction collapse to 0 across the floors
of users holding residues of earlier collapses (1e-52 to 1e-12 of their needs). The smallest
margin over the 482 binding province ticks is 11 (that tick), the smallest bound 15. Before
the stock floor the most was 4 passes. Before ADR 0266's split order rule the most was 6 passes, both
before the input cap and with it: province 5 at tick 79, against a bound of 21, where utilities
and construction, inputs of each other with no stocks but residues from earlier collapses, fall
at a loop gain of 0.7 to 0.9 per pass to 0 across the residue thresholds. Before a jump ahead found the last whole cycle (it ended at
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
to 10 enterprises, scarce stocks; `chain_passes_within_their_kinks`), and on all of 1,000,000
probed under the stock floor (3,000,000 before it). Of 1,000,000 (200,000 per family), 13 exceed
2 + 3 + 2K (`chain_passes_beyond_their_kinks_are_uncovered`): 12 end at the stall rule (pieces
repeating over 4 or more passes, or binding fills switching every pass; one stalls and runs 792
passes), and 1 lands a step's root on a kink every other pass (map undefined: 14 against 11).
Before the stock floor 22 did: the 10 more cut a good by rounding every other pass, sitting at its
limit with U = R where users' stocks covered their needs at a threshold (88 passes against 25,
with 39 such cuts); under the floor a user bound by a good draws (1 - s) F of its need, which no
stock zeroes along the fall. None exceeds it where every jump applies. About 11% of the provinces
have an uncovered pass. Under the stock floor the stall rule acts in 5 provinces of the stocked
families (200,000 each: 1 with higher coefficients, 1 with up to 10 enterprises, 3 with scarce
stocks, one of which stalls at 792 passes, as before the floor), and the first family takes at
most 14 passes; computing a user's uncovered need as coef cap G - b instead cancels to the
rounding of b where it runs mostly on its stock, and the jumps cannot follow such falls: 133 of
100,000 stalled. On 1,000,000 random small
provinces with coefficients up to 0.6, before that change, all but 129 settled within 12 passes,
the most took 257 and none more than 0.3 ms (on 200,000 after it, all but 1, at 257); over five
other families of 1,000,000 (higher coefficients, no stocks, up to 10 enterprises, scarce stocks,
near-balanced markets) 7 provinces stall, collapses crawling towards the smallest normal f64,
and take up to the pass cap of 1,024 that bounds every call. In 200,000 provinces of the calibrated form (markets balanced at F = 1, one capacity
cut) the fills match the plain iteration to 1.4e-10 within 2 passes, the iteration itself
drifting by rounding over thousands of passes (its error, not the fills'). Each tick the enterprise orders up to an inventory position (stock plus goods on order, the paid shipments in its pipeline):

```
order_k = max(0, (1 + cover + L_k) coef_k Y_plan,k - stock_k - on_order_k),    cover = 1

Y_plan,k = VA(L_notional) / a_VA   if traded input k bound the enterprise's labour at E1
         = VA(L) / a_VA            otherwise (Y_cap, capacity output at this tick's employment)
```

where L_k is the expected
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

**Effective demand for inputs (ADR 0266; owner decision 2026-10-30T22:43Z).** Once E1 caps labour
at what traded inputs allow (Labour, Input cap), Y_cap is itself cut by the scarcest input, and
planning every order on it brings back the ratchet the 2026-10-24 decision ruled out: the short
input is reordered on the output its own shortage cut. The rule follows the dual decision of a
rationed agent (Clower 1965; Barro and Grossman 1971) in Benassy's effective-demand form (Benassy
1975, 1982; Dreze 1975): on the market where it is rationed an agent expresses its demand as if
that constraint were absent, and on every other market its demand reflects the constraints it
meets. So:

- A traded input k **binds** the enterprise's labour at E1 when the input cap cut its target there
  (the target without the cap, at the wage the targets were set at, exceeds L_in) and k sets Y_in:
  M_k / coef_k <= Y_in (1 + 1e-9), the margin taking in ties to the last bits (the data state holds
  one month of every input). `systems::scarcest_inputs`; E1 keeps the set per enterprise
  (`Scratch::short`).
- "Exceeds" carries the same relative margin: the cap cut the target when L*_notional > L_in
  (1 + 1e-9) (`systems::cap_cuts`; B9; option A, owner, 2026-11-01). Frictionless and rationed,
  the wage is w_top, the marginal value product of the marginal enterprise at its cap, so that
  enterprise's target at w_top equals L_in exactly in real numbers; computed, the two come from
  different closed forms (the labour target at w_top, and VA^-1(a_VA Y_in) inside w_top), which
  agree only to a few units in the last place, so with `>` alone the split rule was decided by
  rounding (the fixture's frictionless run scaled by 1e4 decided enterprises 8 and 53 the other
  way at tick 7 and differed by 16% in employment from tick 8). Within the margin both order
  bases agree to it: notional output over capped output is VA(L*_notional) / VA(L_in) <= 1 +
  1e-9 to first order (VA rises less than proportionally in L), so the choice moves no order by
  more than 1e-9 relative, while 1e-9 is about 4.5e6 units in the last place, far above the
  closed forms' rounding. It is the tie margin of `scarcest_inputs` (`SCARCEST`), a numerical
  constant, not a parameter, and no register entry. With it the fixture's frictionless run is
  homogeneous of degree zero over 120 ticks (`homogeneity_of_degree_zero`, 1e-9).
- A binding input is ordered on notional output, VA(L_notional) / a_VA: L_notional is the
  employment E1 would have allocated on its targets without the input cap (the same eta step and
  L^s scaling from last tick's employment, at the wage the capped targets were set at: w_{t-1}
  under the wage rule, the new w_t frictionless), kept per enterprise (`Scratch::notional`).
- Every other input, local ones included, is ordered on constrained output Y_cap = VA(L) / a_VA at
  this tick's employment: complementary inputs scale with the binding shortage instead of piling
  up stocks the enterprise cannot use.

Where no cap binds both are Y_cap and the rule is the one above, so the steady state, the
reference model and the calibration are unchanged; the fixture's calibrated state and the scratch
pack's steady start are bit for bit as before (ADR 0266, As built). Ordering up to a target
position is the target-stock (order-up-to) policy (Arrow, Harris and Marschak 1951; Scarf 1960),
whose output-smoothing role at the firm is Blinder (1986) and Ramey and West (1999); the evidence
that an input shortage propagates by cutting the use of complementary inputs, not by stocking
them, is Boehm, Flaaen and Pandalai-Nayar (2019) and Carvalho et al. (2021).

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
checks the closed form against numerical maximization.

**Input cap (ADR 0266).** An enterprise does not hire labour it cannot equip: production is
fixed-coefficient (Leontief), so labour demand is bounded by the scarcest input (quantity
rationing: Clower 1965; Barro and Grossman 1971; Malinvaud 1977). At E1, after T1:

```
Y_in  = min over traded inputs k with coef_k > 0 of M_k / coef_k   (infinite if none)
L_in  = VA^-1(a_VA Y_in)      L at which VA(L) = a_VA Y_in; infinite beyond VA's asymptote
                              (rho < 0), 0 below VA(0) (rho > 0); Cobb-Douglas L0 (VA/(A VA0))^(1/s_L)
L*    = min(L*_notional, L_in)
```

M_k is the enterprise's stock of traded input k as delivered by T1, its own good g included
(owner decision 2026-10-30T22:43Z). No new parameter: the cap
uses coef, a_VA, A, VA0, L0, s_L and P08 only (`systems::input_output`,
`production::labour_for_value_added`). At a steady state it never binds: stocks hold
(1 + COVER + L) months of use (P149), so L_in > L*; the reference model and the calibration are
unchanged.

**Local inputs set no cap** (owner decision 2026-10-30T21:51Z; ADR 0266). The same-tick chain
already bounds their use within the tick, and a bound from last tick's chain fails both ways it was
built. From the enterprise's last fill (last output where it was below 1, none at 1), an enterprise
cut to no output hires no one, its need vanishes, its fill reads 1, and it rehires the whole
province the next tick, every other tick: the fixture's spiral case still overflowed the ledger,
near tick 76. From the enterprise's stock plus the province's last output, a province whose local
goods all reach zero together stays there (every cap is 0 / coef): on the scratch pack the
frictionless benchmark base locked province 3464 at zero by tick 220, its local prices rose every
tick under the Q = 0 rule (7.6e5 by tick 380), expected inflation carried them to every market, and
the ledger overflowed at tick 1,186 (world price index 6e9), a run that completes without the cap.

**The enterprise's own good caps too** (owner decision 2026-10-30T22:43Z). The cap as first built
left it out: capped on it, the fixture's default run inflated (world price index 1.1e7 at tick 100)
and overflowed the ledger before tick 150, and the reason given was that a short own good cuts the
output that refills it. That reason does not hold for a traded own good, which is bought at the
origin markets like any other input. A probe of the order rule (ADR 0266, As
built) showed the lock came from E3: orders planned on Y_cap, which the cap had cut, so the short input
was reordered on its own shortage. With the effective-demand order rule (Production, Effective
demand for inputs) the own good caps like any traded input and the default run stays at a world
price index of 1.19 over 1,200 ticks.

Employment moves toward the target: L_t = L_{t-1} + eta (L* - L_{t-1}) (P41). If sum_g L_t
exceeds L^s_i, all are scaled down pro rata.

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

Frictionless mode (V1): w_i solves sum_g min(L*_ig(w), L_in,ig) = (1 - u*_i) L^s_i on the
targets without the L^s cap but with the input cap (monotone in w; safeguarded Newton in ln w;
ADRs 0093, 0266), so the wage follows the closed form where one target alone reaches L^s; only
the allocation caps (sum L_ig <= L^s), and eta = 1. Where demand never meets supply (every p_VA
<= 0, say), the wage moves at most a doubling or halving per tick
(`frictionless_wage_matches_closed_form_when_one_target_reaches_supply`). Where every
enterprise that would hire (p_VA > 0, L_in > 0) is at its input cap at that wage, labour demand
is flat in w up to w_top = min over them of p_VA dVA/dL(L_in): every wage up to w_top hires the
sum of their caps. That happens when the caps sum to (1 - u*) L^s or less. At equality (the
fixture's data state, whose one month of stocks caps every enterprise at L0 exactly) the root is
not unique; below it no wage clears and the caps leave more than u* unemployed. Either way the
wage is min(w_{t-1}, w_top): the last wage where it is on the flat stretch, else the highest wage
that is (owner decision 2026-10-30T21:51Z; ADR 0266). Without it the search runs down the flat
stretch to its lower limit, and the data state would set wages near 1e-58 at tick 0. Where no
enterprise would hire (every p_VA <= 0 or every L_in = 0) demand is 0 at every wage and the
no-root bound above applies: the wage halves. Frictionless unemployment is u* except where input
caps leave more labour unhired (`frictionless_unemployment_is_the_natural_rate`, which also
checks the wage there; on the fixture: province 24 at tick 1, whose first tick used up its stocks
with its orders in transit, every cap 0 and the wage halved; province 20 at ticks 8 and 9, every
hiring enterprise held by its stock of good 2 and the caps summing below (1 - u*) L^s, the wage
cut to w_top at tick 8 and kept at tick 9). The unemployment rate u_i = 1 -
sum_g L_ig / L^s_i applies equally to each education level.

Before ADR 0266, frictionless E1 valued labour at the posted price of an origin with no output
and could hand it a province's whole labour, paid from debt, until a wage payment overflowed the
ledger (`frictionless_labour_spiral_is_bounded`: the same case now runs 120 ticks with the world
price index at most 1.55).

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

**Income (national profit ownership, owner 2026-10-25; dividends by wealth, ADR 0308).** Labour
income stays local; profits and rebates are national, rebates paid per person, profits by
dividend share d_k (N_k / N_c where the inputs have no wealth shares). Pop k of country c in
province i receives, per tick:

```
income_k = w_i * e_k * employed_k                  (wages, local)
         + d_k * sum_(e in c) profit_e             (profits, national)
         + N_k / N_c * rebate_c                    (tariffs, national)
```

*Dividend shares (ADR 0308; `wealth::dividend_shares`, `EconInputs::set_dividend_shares`).*
Fixed at load from the country's wealth shares W_b of the bottom 50%, middle 40% and top 10%,
with perfect rank correlation of income and wealth: the country's income distribution is the
mixture of its pops' lognormals (mean the pop's base income per person with dividends per
person, dispersion s_w those incomes imply; Pop demand), cut at its quantiles 0.5 and 0.9 by
bisection, and d_k = sum_b W_b n_kb / n_b, n_kb the pop's population in bracket b and n_b the
bracket's; with s_w = 0 the pops fill the ranks in order of income, then of ID. The shares sum
to 1 over the country's pops and are positive for every pop with people
(`dividend_shares_integrate_wealth_over_income`, against a quadrature, 1e-4 relative). Base incomes and
s_w are then those of the shares (one pass, no fixed point). The export writes d_k (`pops`
`dividend`), which the GE reference and the calibration read.

employed_k = LF_k L_i / L^s_i is set by E1 (LF_k the pop's labour force, L_i = sum_g L_ig the
province's employment, L^s_i its labour supply), so sum_(k in i) e_k employed_k = L_i exactly and
the wage term is the pop's share of the province's wage bill w_i L_i. N_k the pop's population,
N_c its country's pops' population, profit_e = sales - wages - purchases of enterprise e
plus the losses it carries, paid only when positive (limited liability, owner 2026-10-26: a
loss stays with the enterprise as retained losses and later cash flow repays it before any
payout, so enterprises pay no negative dividend), and in the profit pool each province market's
signed cash flow (trade SPEC), so the pool is sum over the country's enterprises and markets.
Markets keep the signed rule (ADR 0112, Markets). Retained losses beyond M months of base sales
are written off against the country's pops (loss write-off, ADR 0112, below). Transfers D_c
(below) are paid after the rebates and are not income. Shares are fixed
at load. At a steady state profit_e = (1 - s_L) VA0_e > 0, no loss is carried, nothing is
written off, and market cash flow is 0. A province without traded output so
earns dividends and buys local goods (it had only its rebate share under provincial ownership,
Task 18e). The base income of a province, rebates aside (`world::base_income`,
`io.Inputs.base_income`), is w0 (1 - u*) L^s + N_i / N_c sum_(e in c) (1 - s_L) VA0_e, N_i the
population of its pops (with dividend shares, sum_(k in i) d_k in place of N_i / N_c). A pack
country with enterprises but no pops stops the load (`PackError`):
nobody would own its profits.

*Rounding (E4).* Every enterprise pays its balance when positive, and every province market its
signed cash flow, in minor units, into its country's profit pool account; an enterprise with a
negative balance pays nothing and keeps it. Each country then pays its pool, one i128 sum per tick,
and its rebate account to its pops in pop ID order by cumulative population shares rounded half
to even: part_j = round(P S_j / S) - round(P S_(j-1) / S), S_j the population of the first j
pops, the last pop taking P - round(P S_(n-1) / S) (the pool by cumulative dividend shares d_k
where the inputs have them, `wealth_dividends_pay_the_pool_by_share`). The parts add up to the pool exactly, so the
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
rest is carried to the next tick. Both sides are one exact split per country per tick of an
`i128` total (an aggregate, as the pool account it passes through; ADR 0106), each part within
its own weight and so `i64`, by the largest remainder (floor of each exact share; the leftover units one each to the largest
remainders, ties to the first), routed through the country's pool account; flow `WRITE_OFF`,
which is not income. The cap is nominal, so homogeneity of degree zero holds only while nothing
is written off. S_e = VA0_e / a_VA_e is the steady-state sales only on a calibrated world, where A
is solved so that output Y = A VA0 / a_VA matches it; on an uncalibrated world (the fixture without
calibration tables, `--fixture` runs) base sales are A VA0 / a_VA, so the cap is a factor 1 / A off,
which is harmless while A is near 1 and is not corrected. An enterprise that loses money every month becomes a perpetual levy on its
country's pops once its cap is full.

**Transfers (ADR 0100 as amended by ADRs 0295 to 0300; S22 `EconParams::transfers`).** At E4,
after dividends and rebates, each country receives D_c = s_c(t) Y_{t-1} from one world transfer
account (wide), Y_{t-1} = sum_k income_k of the previous tick, world nominal income (wages,
dividends and rebates; at the first tick of a data-state start, which has no previous income, the
tick's own). s_c(t) is the pack's 2000 share (`Transfers::FixedShares`, the default), or the
observed share of the tick's year (`Transfers::ObservedPath`, tick t in year 2000 + t / 12, lab
profile), which after 2024 fades as s_c(2024) 2^-((t - 299) / 120) (a 10-year half-life, ADR
0100); `Transfers::Off` pays nothing (balanced trade). Exactly: every paying country (s_c < 0)
pays round_half_even(|s_c| Y) minor units, the account splits their total over the receiving
countries in proportion to s_c ([`split`], so it pays out exactly what it took in), and each
country's amount is split over its pops in proportion to the income credited to them this tick.
The account is opened only where the transfers are on and some share is nonzero, so a world
without transfers keeps its ledger, and the switch enters the static digest and snapshots only
then (`transfers_net_to_zero_every_tick`, `off_switch_reproduces_balanced_trade`). Pops spend the
transfer with their money next tick, through the three final demanders (ADRs 0297, 0303).

**Final demand (ADR 0021).** Each pop spends its whole money balance each month (when positive;
the consumption rule of 8.6 with m = 1, lambda = 0) at buyer prices (q_d for traded goods, the
province price for local goods; a good with no reachable origin has q_d infinite and gets
nothing): PIGL across three categories, a CES over goods within each.

*Three demanders (ADR 0099 as accepted by ADRs 0301 to 0306, RFC 0301).* Where the inputs split final
demand (`Countries::gov`, `inv`), a pop's money is spent by three demanders: government buys
g_cg M and investment s_cg M of good g at fixed nominal shares (`demand::fixed_shares`; a good with
no reachable origin gets nothing and its share goes to the demander's other goods in proportion),
and households spend h_c M, h_c = 1 - sum_g (g_cg + s_cg), by PIGL at income per person h_c M /
N. g_cg and s_cg are the shares of all final demand, absorption income + D_c, so the transfer is
spent by the three; calibrated in stage 4 (`PiglWeights::gov`, `inv`), else the data's parts over
the country's final demand. The government and investment goods are consumed on arrival like the
households': in P2 they are demand only (the fiscal and capital blocks, 9.6 and 7.4, replace the
fixed shares; the accelerator of ADR 0099 waits for P4, ADR 0306). Without the split h_c = 1 exactly and nothing changes
(`households_only_keeps_the_fixture`); at the data state the three buy the final demand shares
(`three_demanders_spend_the_final_demand_shares_at_the_data_state`, 1e-10) and leave the money
stock unchanged (`money_stock_unchanged_by_three_demanders`).

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
sum to 1 (test `pigl_shares_stay_in_unit_interval_far_from_base`). Spending on good g is s_c(g) times its within-category share times
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
load (never clipped). At load without a calibration the pops are at the data state's prices (1
for traded goods, p_c for local goods, ADR 0098) with E_k their base
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
3. Before the stages, `ceteris calibrate --pack` fits the border friction nu of each friction
   group (P05, ADRs 0092, 0276; `ceteris_calibrate::border`): a group is a theta class, except
   class 3, each of whose goods (crude oil, gas and refined fuels; coal and other mining) is a
   group of its own (ADR 0276, proposed: one class value set by crude oil took mining below its
   own share and the calibration failed). It is fitted so that the cross-border share of the
   group's goods in the ICIO economies, sum of (1 - home share) absorption over sum of absorption from
   `econ_home_shares.arrow`, is the model's: the value the destinations of those countries buy
   from other countries over all they buy, at origin prices. Each evaluation is stage 2 at stage
   1's spending of the data state (r = 1, no rebates, the data state's buyer prices and PIGL
   weights), held across trials, so each group's share depends on its own nu only and falls as it
   rises; the roots (five on the 2000 pack) are found by Brent's method per group, its bracket narrowed to 1e-9 in
   nu over [0, 10] (the upper end halved where the scaling fails there, on a scaling failure
   only), the groups sharing each evaluation and every scaling starting from the scaling at nu
   0, so the result does not depend on the order of the evaluations (22 evaluations on the
   fixture, `nu_fit_matches_cross_border_share`; goods 1 and 2 at 1.2 and 0.5 come back within
   2.4e-8 and 1.4e-11; where goods 1 and 2 share a nu, classes 0 to 2 and both goods come back at
   the per-class fit's values, `per_good_fit_keeps_the_class_fit_where_goods_agree`). The root itself is resolved only to the
   scaling's noise in the share over the share's slope: on the fixture 6.8e-9 for class 3, whose
   share moves by 2.7e-7 per unit nu, and 2e-10 or better for the others. That test checks the
   shares relative to their target (1e-6), a deviation from the PLAN's absolute 1e-6, which class
   3's flat share passes for any nu from 0 to about 4. A target above the share at nu 0, the
   largest the group reaches with nu >= 0, clamps the group at nu = 0 (one good of class 3 alone,
   `per_good_fit_clamps_one_good_of_class_3`), the corner of the
   constraint, and the fit reports it (`Fit::clamped`, the share at nu 0 against the target;
   ADR 0092, Amendment 2026-10-30); a target below the share at the bracket's upper end stops the
   calibration, naming the group (`nu_fit_is_monotone_and_bracketed`). Interfaces as built: `border::fit_border_nu(inp, groups, target, icio, limits, progress)
   -> Result<Fit, CalibrationError>` (no `u_star`, which stage 1 at the data state does not use;
   the ICIO flags; the shares and the evaluation count with the nu) and
   `border::target_shares(pack, groups) -> Result<(Vec<f64>, Vec<bool>), String>` (also the
   ICIO flag of every pack country; groups from `border::friction_groups`, or
   `border::class_groups` for per-class openness), in place of the PLAN's `fit_border_nu(inp, u_star, target,
   limits) -> [f64; 4]` and `target_shares(pack) -> [f64; 4]`. The stages then run at the fitted nu (crossing factors and
   arrival shares rescaled, base buyer prices recomputed), the calibration tables record it
   (`econ_calib.arrow` metadata `border_nu`, one per good since calibration format 10), and `EconInputs::calibrate`
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
tariff rebate by population; households also receive the transfer D_c = s_c Y in
proportion to income, Y = sum_k income_k world income (Transfers, above; 0 where the export has no
`transfer` column, which it writes only when S22 is on and some share is nonzero) and spend
income + D_c by PIGL. V and VA stay at the data: ADR 0100 removes the per-good rescale r_g of ADR
0089, which assigned the 2000 imbalances to goods. World spending net of tariffs then meets world
sales V / (1 + delta s) of each good through stage 4's targets instead (ADR 0295, RFC 0287 D1): per
pass, each country's traded final demand at its data shares, F_cg = Omega_cg (Y_c + D_c) (Y_c its
households' income), is rescaled biproportionally, F'_cg = a_c b_g F_cg, so that its row is
unchanged (each country's traded absorption, sum_g F'_cg = sum_g F_cg) and each good's world net
household spending sum_c nu_cg F'_cg meets mu C_g, C_g = sales_g - sum_d nu_dg I_dg the good's
world sales less its net intermediate spending, nu the net-of-tariff share of a destination's
spending (`Model::flows`, 1 - tariff; nu_cg its households' spending-weighted mean in the
country) and mu one common factor. Within a pass the row and column totals need not agree (local
output, rebates and incomes are those of the pass), hence mu; at the fixed point the accounting
identity makes it 1 (`balance_residual`, reported with the Sinkhorn's r_g - 1). b_g is iterated
on its own, a_c following from the rows, until the column ratios agree to `RAS_TOL` (1e-14, the
local fixed point's tolerance; at most `MAX_RAS` 10,000 rounds, an iteration cap); a good with no
final demand anywhere (the fixture's crude oil) is left out, and must be balanced by its data
(else its imbalance stays in its r_g - 1 and in mu); a good with final demand whose column target
is not positive within a pass (intermediate use above sales, a transient of the passes' local
output) is held at its previous factor for that pass, and stops the calibration only at the
settled pass. The targets of stage 4 are then F'_cg / (Y_c + D_c) for
traded goods and the data's Omega_cg for local goods. The column factors beta_g = sum_c F'_cg /
sum_c F_cg are the residual of the old r_g (`column_factor` in the summary). The fixture's
traded value added is its generated value times the r_g its calibration solved before ADR 0100
(`fixture::BALANCE`, ADR 0299), so it is balanced at D_c = 0: its column factors are 1 and its
calibrated state is the one before to rounding. With V fixed the
fixed point has no free level (ADR 0106's pin of sum_e r_g VA_e is gone with r). Rebates follow from
sourcing, local output from income and income from local output, so passes of stages 1, 2 and 4
repeat until rebates, buyer prices and the PIGL weights change by at most 1e-13 relative
(Anderson-mixed; `PASS_TOL`, P159, owner, 2026-10-31; 1e-12 before), so the returned state is a fixed
point to the 1e-12 its checks claim (`sinkhorn_is_unique_up_to_scale`, and stage 5's
`stage5_state_is_stationary_under_the_stock_rules`). The stage-5 check compares Q, the output
settled at the last pass's rebates, buyer prices and PIGL weights, with D, the orders at the
settled prices from the spending those values give; the Sinkhorn of that pass fitted the orders
to the previous pass's spending, so D - Q carries the last pass change, about 3.8 times on the
B8 pack (worst cell 1.34e-12 at a last change of 3.55e-13, B8d), plus a rounding term of about
3e-13 (the sums' operation count, as in the uniqueness bound below). At 1e-12 the check's gap
may reach about 3.8e-12 + 3e-13, above its 1e-12; the tolerance must be at most (1e-12 - 3e-13)
/ 3.8 = 1.8e-13, and 1e-13 is the decade below (gap at most about 6.8e-13; 5.25e-13 measured in
B8d at pass 71). The mixing runs on a state in the units `change` uses:
ln q, the PIGL omega and within-good weights as they are, and the rebates and the PIGL
scales each divided by their block's largest magnitude (the rebates' from the first pass, the
scales' from the start), fixed for the whole run. In money units the rebates' residual (about
1e7) swamped the prices' (about 1) in Anderson's least squares, and on the full pack with the
B1 tables and theta 16.91 the passes wandered between 2e-4 and 6e-2 for 100 passes: the
undamped pass map is not a contraction (it locks onto a period-6 cycle in the good-1 prices of
the DR Congo's interior and that country's PIGL weights), so settling rests on the mixing, and
the mixing only saw the rebates. The passes still stop on `change` alone; the units change no
equation and no fixed point. The stopping tolerance is a solver setting, not a model
parameter; it is in the register (P159) since its value is derived from the stage-5 check.
Within a pass the income and local-output fixed point is
iterated until no local output changes by more than 1e-14 relative.

**Stage 2, origin prices.** Per traded good, balanced matrix scaling on the two-level kernel:
w_o sum_d K_od / (1 + t_od) E_d / Phi_d = r_g V_o with Phi_d = sum_o K_od w_o, unique up to scale
(r_g = received over V within the scaling, 1 at the fixed point since stage 4's targets balance
each good; it is no longer a rescale of output)
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
before take the same path. The sweep is the map at the shifted weights, not shifted back, and a
converged scaling is returned with each good's largest ln w at 0 (`sinkhorn_pins_the_level_of_the_weights`):
the scaling fixes ln w only up to a level per good, which the mixing and the warm start of the
next pass otherwise moved freely. On the B8 pack at the export's nu 0.3 coal and mining's level
reached 532 by pass 3, where a unit in the last place of ln w is 1.1e-13, and pass 9's residual
froze at 1.208e-13 (the same value from sweep 22,199 to the 100,000 cap, after falling from
1.15e-12 at sweep 1,000 at about 0.9987 a sweep): a rounding floor of the unpinned level, not
conditioning (pinned, pass 9 converges in 1,960 sweeps). The tolerance does not change. p_o = w_o^(-1/theta_g), each good's scale set so that its part of the world
price index (Omega_g times calibrated province VA as weights) is 1; local goods take their price
level p = p_c (ADR 0098), so the normalization runs over traded goods only and their index is 1. q_d, the arrival slots and the lead follow from sourcing: q_d = 1 / alpha_d, the
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
Local goods take p = p_c (ADR 0098; coef = a_kg p_c / q, A = 1 / p_c, Y = V / p_c), and local
output value solves the 3 x 3 Leontief system of each province
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
(`aggregate_data_state_shares_match_omega`, 1e-12). Where final demand is split (ADR 0304), stage 1's balance rescales all of it with one factor per country and good; PIGL is
inverted on the households' part of the balanced shares, each pop at its households' share h_c of
its spending, and government and investment keep their balanced parts over the country's
balanced total as their spending shares (`Inputs::final_demand_split`,
`pigl_inverted_on_hfce_only`, 1e-12).

**Stage 5, state at the start of a tick.** Input stocks (1 + COVER) months of use, traded and
local, so the order-up-to rule orders one month of use; the pipeline of every buyer (pops and
enterprises) holds, j ticks ahead, one month of steady-state shipments of every arrival slot j or
later; merchant stocks coverage times output (S* = coverage D with D = Q); pops hold one month
of income less this tick's wages, plus their transfer (dividends, rebates and D_c; wages come at
E2); pi^e = 0; last D, Q
and sales their steady-state values (D = Q = sales).

**Checks** (`crates/ceteris-calibrate/tests/steady_state.rs`, fixture and full pack; the full
pack is the export at the border friction of the installed calibration, `write::read_border_nu`
and `pack_world_is_at_the_calibrated_border_friction`, since the export holds the data state's
P05 defaults and the CLI fits its own before the steady state): two starts of stage 2 reach the
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

### Benchmark (`ceteris_calibrate::benchmark`, ADRs 0200, 0207)

A calibrated pack runs frictionless from its steady state under the shocks of
`ceteris_calibrate::shock` and is reduced to country aggregates (`Outcome`). Welfare is
d ln I_c - sum_g Omega_cg d ln q_cg, I_c pop income and q_cg the population-weighted mean of the
provinces' buyer prices; lambda_cg is the home share of absorption by value at origin prices over
the flow window. Two ACR predictions sit beside it:

- `acr`: -sum over traded g of Omega_cg / theta_g d ln lambda_cg (Arkolakis, Costinot and
  Rodriguez-Clare 2012), with no input-output linkages.
- `acr_io`: d ln W_c = -Omega_c' (I - A_c')^-1 diag(1/theta) d ln lambda_c, Costinot and
  Rodriguez-Clare (2014, Handbook of International Economics vol. 4 ch. 4, section 3.4, eq. (28)
  of NBER WP 18896 under perfect competition, delta_k = 0, where the adjusted Leontief inverse is
  the plain one; Caliendo and Parro 2015). A_c holds the country's cost shares, a_kg of input k in
  sector g (`Countries::io`, which the calibrated coefficients reproduce at the calibrated
  prices); d ln lambda is 0 for local goods, which pass input cost changes on through the
  inverse. With one good using itself at cost share a the change is 1 / (1 - a) times `acr`
  (fn. 20 there). The model has Leontief intermediates, so its welfare follows `acr_io`.

A good bought at home in one run only makes the country's ACR NaN in both forms.

`Outcome` also carries each province's pop income I_p, buyer prices q_pg and home share
lambda_pg: what p buys from itself over what p buys from every origin, domestic and foreign, by
value at origin prices over the flow window (the province's payment net of tariff,
`flows::MeanFlows::absorption`). Both sides of the graded pair are reduced from provinces with
base-run income weights s_p = I_p / I_c (ADR 0268; Ramondo, Rodriguez-Clare and
Saborio-Rodriguez 2016, section 2.6, Proposition 4), provinces with no base income left out:

- `acr_io_province`: sum_p s_p d ln W_p, d ln W_p = -Omega_c' (I - A_c')^-1 diag(1/theta)
  d ln lambda_p with the country's Omega and A. A province NaN makes its country NaN.
- `welfare_province`: sum_p s_p (d ln I_p - sum_g Omega_cg d ln q_pg). It differs from
  `welfare` at first order by -sum_g Omega_cg sum_p (s_p - pi_p) d ln q_pg, pi_p the population
  share: the covariance of income per head with price changes inside the country. A province
  with no buyer price for a good with Omega_cg > 0 makes its country NaN (`welfare` averages the
  finite prices instead); the report counts such countries with the NaN ACR ones and grades
  neither.

With one province per country the province forms are the country forms bit for bit. ADR 0207's
criterion grades `welfare_province` against `acr_io_province` (ADR 0268, amending ADR 0265):
under every crossing tau x 1.1, every ICIO country with |ACR| above 1e-4 has its sign, the median
model / ACR ratio is in [0.8, 1.25] and the correlation across countries is at least 0.9. `acr`
and `acr_io` against `welfare` stay reported, with each country's gap `acr_io -
acr_io_province`. C7 asserts the criterion at B8 on the final pack.
On the B8 pack (manifest 3c7895d1, settled baseline, drift 3.78e-8) the country forms read:
`acr_io` median 1.077 (p10 0.931, p90 1.325), correlation 0.862 and 0 wrong signs over 80;
`acr` median 2.417 (p10 1.868, p90 3.016), correlation 0.855 (before ADR 0268 and 0269).

| Test | Checks |
|---|---|
| `province_forms_weight_provinces_by_base_income` | hand-computed two-province forms to 1e-14; a zero-income province adds nothing, as does a good a province absorbs in neither run; a vanishing province home share gives NaN |
| `income_weights_differ_from_population_weights_by_the_price_covariance` | the first-order gap of point 2 of ADR 0268, remainder below (d ln q)^2 |
| `province_forms_reduce_to_each_province_country` | two countries with interleaved provinces: each reduces to its own provinces and income |
| `welfare_and_acr_match_the_hand_computed_toy` | one province: province forms equal the country forms bit for bit |
| `outcome_aggregates_the_flow_window` | province incomes add to the country's; lambda_p is home over absorption, in [0, 1] |
| `flows::tests::pairs_account_for_everything_dispatched` | province absorption adds to the country's home purchases and pairs into it, to 1e-12 |

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
1. Solved with Newton-Krylov (scipy) from the data state (ln p = 0 for traded origins, ln p_c for
local goods: the reference reads p_c as the base price of local goods, and inverts PIGL at those
prices; ADR 0098), Anderson-accelerated tatonnement as a
fallback (under PIGL the fixture needs the fallback); tolerance 1e-10 on scaled residuals. Balanced Sinkhorn on the dense kernel is the S02
full solve and reproduces the equilibrium's sourcing flows. numpy and scipy only (JAX stays
optional, 19.1).

## System order (monthly unless noted)

| System ID | Name | Reads | Writes | Position (4.4) |
|---|---|---|---|---|
| T1 | deliver (trade) | pipeline | pops' consumption, enterprise input stocks | first |
| E1 | labour | prices, last tick's buyer prices, wages, pi^e, A, traded input stocks (after T1) | L*, L, wages, wage clip counts, unemployment; for E3 the inputs that bound each enterprise's labour and its notional employment | labour market |
| E2 | produce | L, input stocks, local output (same-tick chain per province) | input use (local inputs drawn from this tick's output first, paid at the posted price), output to the province's market (Q), wages paid | production |
| E3 | demand | pop money, prices, output, pipeline, E2 local draws, E1's binding inputs and notional employment | buyer prices q and leads L, household demand and enterprise input orders on top of the local draws | consumption demand |
| T2 | source (trade) | traded spending, prices, kernel | orders at origin markets | inter-province trade |
| T3 | clear (trade) | orders, output, inventory; local demand | fills, sales, inventory; local sales of what the chain left (S11) and payments | markets |
| T2b | dispatch (trade) | fills | shipments, pipeline, payments, tariffs | inter-province trade |
| T4 | reprice (trade) | D, Q, S, pi^e, buyer prices | prices, clip counts; then pi^e from the world price index | markets |
| E4 | distribute | enterprise and market cash flow, rebates, pop money, last tick's income, the tick | profit pools (positive enterprise balances, signed market cash flow), pop money for next tick, the transfers D_c (ADR 0100), then the loss write-off (ADR 0112) | finance (P2 stand-in) |
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
| Transfers | The world transfer account ends every tick at zero: what paying countries' pops put in is exactly what receiving countries' pops take out (ADR 0100) | exact in ledger | every tick (`end_tick`); `transfers_net_to_zero_every_tick` |
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
| S22 | Trade-imbalance transfers (`EconParams::transfers`, ADR 0100): `Off`, `FixedShares` (default) or `ObservedPath` (lab profile; after 2024 a 10-year half-life, ADR 0100's value, not tuned) | `FixedShares` | Switch |

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

**Cells outside one-market J2.** The bound above is about the equilibrium; whether the run's
price rule reaches it is a second question. A market whose own loop gain kappa_cell J_cell is at
or beyond 2 (J2, trade SPEC Stability region; J at the run's eta, kappa_cell the run's gain, ADRs
0263 and 0267) does not contract on its own, so V1's tolerances do not apply to it:
`uniqueness::cells_outside_region(inp, gains)` lists those active cells (`uniqueness::Gains`, per
cell the run's gain and J), and V1 in `p2_gate.rs` leaves their prices out of the 1% comparison,
prints each with its gap, and fails when they are more than 1% of the active cells. Under the
per-market cap kappa_cell J <= 2 m with m = 0.5 (P03d), so a run of the calibrated rule has none:
the list is a check that the cap holds, not a fallback. The fixture has none even at the
uncapped kappa1 (largest kappa1 J 0.692 frictionless over 177 active cells,
`cells_outside_region_is_empty_on_the_fixture`); `v1_excludes_cells_outside_region` pushes one
fixture cell past 2 / J and checks that it alone is left out and reported.

## P2 gate (21.1)

- V0: zero invariant violations; determinism, 1 vs 8 threads, replay.
- V1: first the uniqueness check (Equilibrium nesting) at the run's theta and the pack's
  value-added shares. Where the bound holds (at most 1, tolerance mode) V1 applies the 1%
  tolerances above, full pack, within 1,200 ticks; where competitive storage is on (ADR 0091), within
  6,000, since from the data state the stored goods converge slowly and not monotonically (on the
  fixture flows are 1.5% off at 3,600 ticks, 2.1% at 4,800, 1.4e-4 at 6,000; owner,
  2026-10-31). Otherwise (report mode) both models start from
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
  the data; the log-distance cost that ADR 0022 says the data prefer is built in P4 as a
  crossing friction beside nu, not in the legs (ADR 0272 on main, replacing ADR 0264 Decision 3).
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
  - On the pack with whole provinces before ADR 0249 (ADR 0251, 2026-10-30; 4,594 provinces, 45 passes,
    Sinkhorn residual 9.9e-14): V1 from the perturbed steady state passes (price 9.086e-3,
    wage 9.067e-4, flow units 4.950e-5, value 5.390e-5, bound 0.9671, reference residual
    5.0e-12); V1 from the data state still misses (price 0.879, wage 0.604); H2 distance
    -0.7438 (s.e. 0.0151, pseudo-R2 0.982, 47,306 pairs) passes; H2 per class passes (food
    1.676, s.e. 0.171; machinery 3.533, 0.517; the five-good class 9.084, 0.332). 100 years: no
    violation, world price index 1.0000000000000124, no negative money, pool, retained loss or
    stockout. 1 vs 8 threads, 120 ticks: identical hashes (20.1 s and 6.1 s). Tick about
    53.5 ms (12 ticks from the data state, not a benchmark median).
  - On the B8 pack (pre-P3, 2026-10-31; ADR 0251's whole provinces with the rebuilt econ tables,
    4,368 provinces with value added, ADR 0276's nu per good; 68 passes, Sinkhorn residual
    9.9e-14, manifest 3c7895d1): V1 from the perturbed steady state passes at seeds 20, 21 and 22
    (price 1.939e-4, 1.536e-4, 1.245e-4, all Malawi province 2699 good 9; wage 2.027e-4,
    1.607e-4, 1.302e-4; flow units 7.85e-5, 6.457e-5, 5.601e-5; value 8.37e-5, 6.887e-5,
    5.974e-5; bound 0.9843; reference residual 1.238e-12; 0 of 37,334 cells outside J2). V1
    from the data state misses (price 1.31e3, wage 1.32e3), all of it Pitcairn (province 2892,
    local goods 7 to 9; pop money 8.9e3 to 7.8e7 by tick 60, its wage 601 to 4.3e6 by tick 240 at
    fixed employment, falling to 8.6e5 by tick 1,200). The money is the data state's merchant
    stocks (Initial state: coverage times initial demand): at prices 1 every active origin
    draws world orders by the kernel alone, so Pitcairn's market starts with 5.209e8 units of
    traded goods 0 to 6 against a pop money of 8.85e3; the world buys them at tick 1 (TRADE into
    the market, 5.2087e8 at price 1) and E4 pays the market's cash flow to its pops (DIVIDEND),
    58,800 times their money. Money and goods are conserved (the stocks are created at load,
    before the first tick), so no invariant sees it; the other small territories carry the same
    start (Western Sahara 10,100 times, Indian Ocean Territories 5,600, Niue 4,600) and keep less
    of it (B8d); without Pitcairn the worst is Uganda good
    9, 0.897 (0.83 on the old pack), then Sierra Leone 0.629, North Macedonia 0.532 (its wage at
    0: the same tick-1 sale, 345 times its pops' money, then wages far above w0 whose losses
    the write-off and the markets' signed cash flow turn into pop debt by tick 30; in its eight
    provinces with only local goods 7 to 9 the debtors buy nothing, prices stay below input
    cost, no wage clears labour, so the frictionless wage halves every tick, 1e-30 to 1e-116 of
    w0 by tick 900; B8d), Albania 0.368. Neither cap drives it: with the block limits off 1.36e3, with both caps off
    1.38e3 (France then 6.9). With the data state's merchants at coverage times own base output
    (Initial state, amendment 2026-10-31, B8e; storage off) V1 from the data state still misses
    frictionless: price 0.762 at tick 1,200 (Taiwan province 3903 good 9, wage 0.737; Paraguay
    0.723, Albania 0.403; 233 countries with a cell over 1%), 0.670 at 3,600 (Taiwan; Albania
    0.368; two countries over 1%), 0.295 at 6,000 (Albania province 62 good 8 alone; next Malawi
    1.9e-3), flow units 0.728, 0.304, 0.121. Pitcairn's gap is 1.3e-2, 2.5e-4 and 2.2e-4, and
    North Macedonia's wages are 0.984 to 1.085 of the reference at 1,200 and 0.9996 to 1.0008
    at 6,000. The default run from the data state passes V1's 1% at 1,200 (price 9.29e-3,
    Malawi; wage 9.34e-3; flow units 4.40e-3) and reaches 1.4e-7 at 3,600 and 8.2e-8 at 6,000. H2 distance -0.4539 (s.e. 0.0105, pseudo-R2 0.964), reported (ADR
    0264). H2 per class passes (theta within 2.0e-8 relative); with ln distance as the cost
    control class 0 2.453, 1 3.561, 2 7.630, 3 16.039, and in class 3 good 1 alone 16.275 (s.e.
    0.417), good 2 alone 14.443 (0.739). 100 years: no violation, index 1.0000000000000067. 1 vs
    8 threads identical (120 ticks 22.9 s and 6.9 s). ACR as above. Gravity re-estimates (`ceteris
    gravity`, not written): P01 by class 12.82, 14.45, 11.34, 22.88; P05 0.2001 (by class 0.262,
    0.117, 0.329, 0.208); P04 stops (air not identified, 3 country pairs; road and rail 8.714,
    sea 64.79 against the order water < rail < road < air), so P04 keeps its values.
  - Commands (M4 Pro, `data/packs/2000` built and calibrated, `PACK=$PWD/data/packs/2000`,
    `EXPORT` a scratch directory):
    - `cargo run --release --bin ceteris -- export --pack $PACK --out $EXPORT`, the export the
      gate reads.
    - V1 (both starts), H2 distance and H2 per class, about 265 s:
      `CETERIS_PACK=$PACK CETERIS_PACK_EXPORT=$EXPORT cargo test -p ceteris-cli --release --test p2_gate -- --nocapture`;
      `v1_full_pack_from_the_perturbed_steady_state` gives one line per seed (20, 21, 22),
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

`ceteris run` writes `run_prices`, `run_wages`, `run_enterprises`, `run_flows`, `run_home` and `run.json` (the state at the final tick; flows and home purchases are means over the last 12 ticks, `ticks_averaged` in `run.json`, and fewer after a snapshot restore or `seek`, since the window is scratch and not state). `--data-state` needs a calibrated pack. With `--history [--history-every N]` it also writes `run_history_world`, `run_history_provinces` and `run_history_markets`, one row per sampled tick (tick 0, every N-th, the final tick) for charting the dynamics; recording only reads the state, so state hashes and the default output are unchanged. Columns: `crates/ceteris-econ/src/history.rs`.

## Test plan

- Same-tick chain and putty-clay (ADR 0020, `p2_properties.rs`): one vintage equals fixed
  Leontief in every state hash over 120 ticks (`putty_clay_one_vintage_equals_fixed_leontief`); a
  50% cut of services labour with empty local stocks cuts construction output in the same tick
  (`local_shortage_reaches_downstream_same_tick`); intermediate users draw all of a short local
  good before households, and Priority fills intermediate orders first at origins
  (`local_chain_respects_intermediate_priority`); cutting coal output leaves downstream output
  unchanged in that tick and lowers it once stocks run out (`traded_shortage_still_lags`); cycles
  among local goods settle within the passes their kinks allow (at most 14, against its bound of 25;
  smallest margin 11; `local_chain_terminates_on_cycles`); a user whose stock covers its local needs runs at
  its cap when a local producer makes nothing, while the users without stock are cut to nothing
  (`a_stocked_user_is_not_cut_by_the_fill`, `local::tests::stock_is_a_floor_under_the_fill`);
  the exact fills equal
  the fill iteration run to convergence, with its steps found by bisection, on every province tick
  of the fixture runs and on random small provinces, at 1e-12, wherever it converges
  (`exact_fill_equals_converged_iteration`, `exact_fill_equals_converged_iteration_on_random_provinces`);
  S11 Proportional gives one fill ratio (`proportional_rationing_switch`); `split` in `local.rs`.
- Transfers (ADR 0100, RFC 0287): the world transfer account ends every tick at zero and every
  pop's transfer is its country's D_c split by income, over 120 ticks of the fixture with
  nonzero shares (`transfers_net_to_zero_every_tick`, `p2_properties.rs`); on inputs without
  transfers Off and FixedShares run bit for bit alike, state hashes included, and Off on inputs
  with shares leaves the dynamic state of the balanced run unchanged
  (`off_switch_reproduces_balanced_trade`); the observed path takes the year's share and fades
  after 2024 (`observed_path_fades_after_2024`); the pack's shares sum to zero with missing
  current accounts at 0 (`transfer_shares_spread_the_discrepancy_by_gdp`, pack crate); calibration
  reports the column factors and the balance residual and has no r column
  (`r_g_removed_and_residual_reported`, `steady_state.rs`); the GE reference spends D_c
  (`reference_spends_the_transfers`).
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
- Unreachable destination at world level (`fixture::unreachable_island`:
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

- ADR 0098 (built: SWIID Gini first, WPP national totals, Anguilla and Montserrat regions, the
  Venezuela income rule, `econ::ras::balance`; the price level source and the dropped rate
  rescale are ADRs 0282 and 0283, Price levels above). RFC 0282's other items:
  - RAS targets: ruled by D6 (ADR 0286, Donor table balance above), which supersedes D3's rows
    X_k + M_k - E_k - F_k (negative for 321 rows of 122 of the 133 imputed economies with BACI
    trade). KRAS (D6 B) can follow if the economies that keep the donor's table matter.
  - D4 (ADR 0284): the run's world price index keeps local goods; no code change. D5 (ADR
    0285): the real GDP check reads NY.GDP.PCAP.PP.CD (Price levels above). D7 (ADR 0294): PWT
    pl_da replaces pl_gdpo, and the check's 5% is reported, not enforced. D8 (ADR 0290): the
    check compares totals on NY.GDP.MKTP.PP.CD and is enforced at 10% on the 31 economies whose
    PWT level of 2000 is a benchmark. D9 (ADR 0289): the WDI fallback is current WDI's implied
    level, not the archive PA.NUS.PPPC.RF.

- ICIO 2025 edition industry list (whether B05 and B06 are split) is checked at fetch.
- Gateways (Task 17): the spec rule above replaced two parts of the first text. k-medoids run on
  great-circle distance, not path cost, since path costs need the Rust paths in the Python
  pipeline. Land crossings enter as candidates, not each as a gateway pair: the P1 graph has
  2,481 cross-border road and rail edges, 1,359 after merging within 50 km, which as gateways
  would make about 2,700 gateways and the all-pairs crossing matrix twenty times its budget. The
  built pack has 600 gateways (286 ports, 171 airports, 143 land crossings; ADR 0251). Revisit with path
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
- Stage 5 on the full pack (B8d; resolved in B8e, owner, 2026-10-31):
  `stage5_state_is_stationary_under_the_stock_rules` missed its 1e-12 on merchant stock and
  supply, S* = coverage D and Q = D (81 of 24,230 cells, worst 1.34e-12, province 2436 good 3;
  the first failing cell 1.12e-12). It is not rounding: Q is the settled output and D the
  orders at the settled prices from the spending the last pass's values give, which moved by up
  to the pass change (3.55e-13 at pass 68, PASS_TOL 1e-12), and D carries that change about four
  times. PASS_TOL is now 1e-13 (P159; Calibration, stage 1). The other option, a check bound
  derived from the pass tolerance, needs D's sensitivity to the pass inputs and was not taken.

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
