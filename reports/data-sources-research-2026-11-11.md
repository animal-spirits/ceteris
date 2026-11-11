# Public and permissive data sources: gaps, upgrades, licence-clean replacements (2026-11-11)

Scope: candidates not already in `crates/ceteris-pack/config/sources.toml`, ranked by value to the
model. Constraint: 1.0 is free on Steam, 2.0 is paid, so a source must allow redistribution in a
packaged pack (including commercial), or be fetched on the user's machine (research profile, ADR
0050). Licences below were read on the provider's own page on 2026-11-11 unless marked
**unverified**. NC, SA, ND and unclear terms are flagged. This is a study: nothing here changes
the manifest; each adoption needs its own issue and a `sources.toml` entry.

Licence key: **OK** redistributable in a paid pack with attribution. **OK+** OK with an extra
clause to decide on. **Fetch** user-fetch only (research profile). **?** not verified.

Already covered and not re-recommended: everything in `sources.toml` (ICIO, BACI, Gravity, PWT,
WIOD SEA, WDI, WPP, WIC, ILOSTAT, DOSE, Kummu, GEM, MapSPAM, ISIMIP, ERA5, UCDP, V-Dem, etc.),
plus leads already recorded in RFC 0050 and the 2026-10-28 gap audit (WFP market prices, OxCGRT,
EUREGIO, US CFS, KNOMAD, AidData, Aqueduct, UNAIDS). Those are referred to, not re-researched.

## Top findings

1. **EIA International Energy Statistics is public domain** and covers country energy production
   and consumption by fuel from 1980. It can replace the NC IEA balances as a calibration-grade
   source, not just a held-out check. Ember (CC BY 4.0) adds electricity generation by fuel.
2. **ADB MRIO is CC BY 3.0 IGO with commercial use explicitly allowed** and has a 2000 table. It
   is the only licence-clean, independently assembled MRIO found with a T0 year besides ICIO and
   WIOD, so it can take over part of EXIOBASE's held-out role without the no-leakage burden.
3. **FAOSTAT is not in the manifest yet** and is the largest gap: crop and livestock production,
   food balances, producer prices and land use from 1961, CC BY 4.0 with one FAO clause (no use
   "in conjunction with the promotion of a commercial enterprise") that needs a one-time decision.

## Ranked list

| # | Source | Part | Licence | Feeds |
| --- | --- | --- | --- | --- |
| 1 | EIA International Energy Statistics | 3 | OK (public domain) | spec 07 energy rows; 15 calibration |
| 2 | FAOSTAT | 1 | OK+ | spec 07 agriculture, 08 food trade and prices |
| 3 | ADB MRIO | 3 | OK | spec 15 held-out IO validation |
| 4 | Malaria Atlas Project surfaces | 1 | OK | spec 06 / P4 disease kernel (ADR 0167) |
| 5 | Human Mortality Database | 1, 2 | OK for HMD outputs; Fetch preferred | spec 06 / P4 mortality validation |
| 6 | PRIMAP-hist v2.6 | 1, 3 | OK | spec 13 climate (FaIR emissions) |
| 7 | ICP 2011/2017/2021 basic headings | 1 | OK | spec 08 price levels, validation |
| 8 | Ember yearly electricity | 1, 3 | OK | spec 07 electricity; P8 |
| 9 | GLC_FCS30D land cover 1985-2022 | 3 | OK | spec 05 land cover |
| 10 | EMERGING MRIO v2 | 2 | OK | spec 15 IO validation, 2015+ windows |
| 11 | EDGAR (non-CO2 gases, gridded) | 1 | OK, except IEA-EDGAR CO2 (NC-ND) | spec 13 climate |
| 12 | OECD Regional Database (TL2/TL3) | 3 | OK? | spec 15 subnational validation |
| 13 | USDA FAS PSD | 1 | OK (public domain) | spec 08 storage (ADR 0091) |
| 14 | GLORIA MRIO | 3 | ? | spec 15 IO validation |

## Part 1. Data we do not cover yet

**FAOSTAT** (production, trade, food balances, producer prices, land use). 1961 to date, ~245
countries and territories, ~200 crop and livestock items. Licence: FAO Statistical Database Terms
of Use apply CC BY 4.0 to all datasets "unless specified otherwise in their metadata", plus:
"Datasets shall not be used for or in conjunction with the promotion of a commercial enterprise",
no implied endorsement, and third-party datasets "may not be redistributed or reused without the
consent of the original data provider" (https://www.fao.org/contact-us/terms/db-terms-of-use).
Same clause already flagged on `fao-ffpi`. Feeds: agriculture output and yields at country level
(anchors MapSPAM shares), food balances for final demand, producer prices for relative price
checks; an ADR already says "to add to the manifest". Recommendation: add as primary; decide once
whether selling a game counts as "promotion of a commercial enterprise" (likely not, but it is
Antonio's call), and record the decision on `fao-ffpi` too.

**Malaria Atlas Project** (PfPR, Pf/Pv incidence and mortality rasters). 2000 to ~2022, global
endemic zone, 5 km. Licence: "Our maps are available to all users under the Creative Commons
Attribution 3.0 Unported License"; survey and routine case data are not MAP-owned but carry "no
restrictions", cite originals (https://malariaatlas.org/?p=641). Feeds: P4 B2 disease kernel
initial prevalence and calibration (ADR 0167 already lists MAP). Recommendation: add as primary
for the malaria compartment; it is the only endemic-disease surface found that is cleanly
redistributable.

**Human Mortality Database**. Period and cohort life tables, deaths and exposures, ~40-45
mostly high-income countries, many back to the 19th century, through ~2022. Licence: HMD
constructed series are CC BY 4.0; input data "remain under each provider's distribution
license" and "should not be used for commercial gain or re-published" without the owners'
permission; HMD asks users not to pass copies on and to point to the site instead
(https://www.mortality.org/Data/UserAgreement). Feeds: P4 mortality validation (observed, not
modelled, life tables) and age-specific death rates for the pop-on golden. Recommendation: use
the CC BY outputs only, fetched at build time rather than shipped raw; ship only derived
parameters.

**PRIMAP-hist v2.6**. Kyoto gases by IPCC 2006 sector, 1750-2023, nearly all UNFCCC parties.
Licence: "Climate Resource makes this data available CC BY 4.0 licence"
(https://zenodo.org/records/13752654). Excludes international aviation and shipping; LULUCF only
in the no-rounding file. Feeds: spec 13 climate, FaIR emission inputs and the 2000 emission
intensity per sector. Recommendation: add as primary for country emissions; it also replaces the
NC-ND IEA-EDGAR CO2 series (Part 3).

**ICP basic-heading PPPs** (2011 revised, 2017, 2021). 176-179 economies, 44 expenditure
headings; annual PPPs for 6 major headings. Licence: CC BY 4.0
(https://datacatalog.worldbank.org/search/dataset/0038406/international-comparison-program-2017).
No 2000 round at heading level. Feeds: spec 08 relative price levels by category (the gap audit's
top data item); validation in the S window, PWT stays the T0 source. Recommendation: add as
validation for category price structure from 2011.

**Ember yearly electricity data**. Generation by fuel, capacity, demand, emissions; 200+
geographies, 2000 to date. Licence: "All content is released under a Creative Commons
Attribution Licence (CC-BY-4.0)" (https://ember-energy.org/data/yearly-electricity-data/). Ember
compiles EIA, Eurostat, UN and national data; the CC BY statement is Ember's own. Feeds:
electricity sector mix and P8 energy rows. Recommendation: add as primary for power generation by
fuel; keep EIA as the cross-check.

**EDGAR** (gridded and national emissions, air pollutants). Licence: EU-owned material CC BY 4.0;
IEA-EDGAR CO2 is CC BY-NC-ND 4.0 (https://edgar.jrc.ec.europa.eu/dataset_ghg70). Feeds: spatial
emissions and non-CO2 gases. Recommendation: complement only; never ingest the IEA-EDGAR CO2
files into a release pack.

**USDA FAS PSD Online**. Production, supply, distribution and ending stocks for major grains,
oilseeds, cotton, sugar, livestock by country, 1960 to date. Licence: US federal work, public
domain (catalog.data.gov "Production, Supply, and Distribution Database"; not read on a USDA
licence page, **verify**). Feeds: competitive storage at origin markets (ADR 0091), initial
stocks. Recommendation: add as primary for initial stocks; already named in one ADR but absent
from the manifest.

Leads from earlier audits, still unadded (licences already checked there): WFP market food prices
(CC BY IGO), OxCGRT (CC BY 4.0), EUREGIO within-country trade (CC BY 4.0), KNOMAD bilateral
remittances, AidData Chinese development finance, WRI Aqueduct. Recommendation: open one issue
per item when its model component is scheduled.

## Part 2. Better data than what we use

This part is thin: the current manifest already uses the strongest open sources for most rows.

- **HMD over WPP for mortality checks**: WPP is modelled for every country; HMD is observed for
  ~40. Use HMD as held-out truth where it exists, WPP elsewhere.
- **EMERGING MRIO v2.2** (Huo, Wang, Guan et al.): 245 economies, 133 sectors, years 2015, 2018,
  2021, 2023, with sectoral CO2; CC BY 4.0 (https://zenodo.org/records/19461860). No 2000 table,
  so not a T0 source; it is a better post-2015 IO check than EXIOBASE for small economies in the
  S and Z windows. Recommendation: add as validation for 2015+ windows.
- **Kummu et al. 2025 subnational GDP** supersedes `kummu-gdp` (already recorded in RFC 0050);
  the licence is still listed as unverified there. Recommendation: verify and swap.
- **Ember over WDI electricity shares** for generation by fuel (finer, more recent, CC BY 4.0).

## Part 3. Licence-clean replacements for unusable data

| Unusable source | Problem | Replacement | Licence | Gap left |
| --- | --- | --- | --- | --- |
| IEA World Energy Balances | CC BY-NC-SA 4.0 IGO | EIA International Energy Statistics; Ember for power | public domain; CC BY 4.0 | no full balance (transformation losses, sectoral final use less detailed) |
| EXIOBASE 3.9 | validation-only, no leakage | ADB MRIO 2000 and 2007+; WIOD 2016 WIOT 2000 (already R); EMERGING for 2015+ | CC BY 3.0 IGO; CC BY 4.0 | 163-product detail and environmental extensions |
| IEA-EDGAR CO2 | CC BY-NC-ND 4.0 | PRIMAP-hist v2.6 | CC BY 4.0 | gridded CO2 (use EDGAR non-CO2 grids as spatial proxy) |
| ESA CCI Land Cover (rejected) | ESA CCI terms | GLC_FCS30D, 30 m, 1985-2022 | CC BY 4.0 | none for 2000 |
| WHO GHE, IHME GBD | NC | MAP (malaria), HMD (mortality); UN IGME child mortality | CC BY 3.0; CC BY 4.0; IGME **?** | cause-of-death shares for non-HMD countries |
| World Bank Pink Sheet | third-party content | EIA spot prices, USGS DS140 (have), FAOSTAT producer prices | public domain; CC BY 4.0+ | metals spot series |
| Growth in Regions (Gennaioli) | no licence | OECD Regional Database TL2/TL3 | OECD data terms, **verify** | non-OECD regions; shares upstream with DOSE, so weak as held-out |
| IMF IFS/WEO | commercial reuse needs permission | WDI and WB GEM (have) | CC BY 4.0 | WDI series sourced from IFS may carry IMF terms; check per series |
| EPR / GeoEPR | CC BY-NC-SA 4.0 | none verified | n/a | see below |

Details:

**EIA International Energy Statistics**. Primary energy production and consumption by fuel,
electricity, reserves, ~220 countries, 1980 to date. Licence: "U.S. government publications are
in the public domain and are not subject to copyright protection. You may use and/or distribute
any of our data, files, databases..." with acknowledgment
(https://www.eia.gov/about/copyrights_reuse.php). Caveat: the page also says contributed or
licensed materials "may be protected"; it does not exempt international statistics, so keep the
acknowledgment and record that caveat. Feeds: spec 07 energy rows (coal, oil, gas production and
use per country), replacing IEA as calibration input and leaving IEA held-out. Recommendation:
add as primary; highest value per unit of work.

**ADB MRIO**. Licence: "CC BY 3.0 IGO", shareable and adaptable "for both commercial and
non-commercial purposes at no cost", citation "Contains information from [database name], © ADB
[year]"; excludes material ADB has no rights to (https://data.adb.org/terms-use-data). Coverage:
2000 and 2007 onward, 62-73 economies, 35 sectors (**verify** exact grid on the download page).
Feeds: held-out IO checks (foreign input shares, coefficient rank correlation) with commercial
rights, so it can sit in the release validation suite. Built partly on WIOD, so independence from
ICIO is partial. Recommendation: add as validation; lets the paid build keep an MRIO check if
EXIOBASE terms ever tighten.

**GLC_FCS30D**. Licence: CC BY 4.0 (https://zenodo.org/records/8239305). 30 m, 1985-2022.
Feeds: land cover classes for the province raster (spec 05), replacing the rejected ESA CCI LC.
Recommendation: add as primary if a land-cover layer beyond Hansen tree cover is needed; resample
at build time (large files).

**GLORIA**. 164 regions, 120 sectors, 1990 onward. No licence statement found on the IELab or
University of Sydney pages reached (https://isa.sydney.edu.au/ielab/). RFC 0054 already excludes
it from release packs. Recommendation: keep research-only until IELab states terms in writing.

**OECD**: `sources.toml` already records "OECD statistics" as CC BY 4.0 while `oecd-icio` and
`oecd-amne` still say `LicenseRef-OECD-Terms`. OECD moved to an open-access model in July 2024
(press release; licence text not readable from here). Recommendation: confirm whether ICIO and
AMNE now fall under CC BY 4.0; if so, this also closes the open check in
`reports/oecd-icio-licence-2026-11-08.md`.

**EPR / GeoEPR**: no licence-clean substitute verified this session. Country-level
fractionalization is covered by HIEF (CC0, have). Spatial ethnic settlement leads (GREG, SIDE,
CREG) were not checked. Recommendation: keep EPR user-fetch per the 2026-10-28 decision; open a
separate issue if a release-pack ethnic layer is needed.

## Not verified here

GLORIA terms; USDA PSD licence page; UN IGME terms; ARDECO (JRC catalogue page rejected the
request); OECD ICIO/AMNE under the 2024 open model; ADB MRIO exact grid; GREG, SIDE, CREG.

## Next steps

One issue per adopted source (manifest entry, fetch entry, `pack check-sources` profile). Suggested
order: EIA international, FAOSTAT (after the clause decision), MAP, ADB MRIO, PRIMAP-hist, HMD.
