## 16. Data Sources and Pipeline

The 2000 pack is built from pinned sources; each is listed with its licence, evidence and release policy in the source manifest (crates/ceteris-pack/config/sources.toml, 98 sources). Non-commercial sources may be used because each user downloads them; sources that bar redistribution are fetched, never bundled. ADR 0005 lists the substitutions made in the P0 audit.

| Dataset | Content | Role | Licence | Bundle or fetch |
| --- | --- | --- | --- | --- |
| geoBoundaries | Admin boundaries | County layer, county-border splits | Per file; 28% of units share-alike | Fetch |
| Natural Earth | Coastlines, rivers | Map | Public domain | Bundle |
| Natural Earth admin-1 | First-level regions | Provinces | Public domain | Bundle |
| ETOPO 2022 | Elevation | Terrain, ruggedness | CC0 1.0 | Bundle aggregates |
| OpenStreetMap | Roads, rail, ports, pipelines | Transport graph | ODbL (share-alike) | Bundle under ODbL |
| WorldPop / GHSL | Gridded population, built-up area | Pops, urban share | CC BY 4.0 | Bundle aggregates |
| FAO GAEZ v4 | Crop suitability, yields | Land productivity | Possibly non-commercial | Fetch |
| OECD ICIO | Input-output, 45 industries, 1995-2022 (2025 edition), 80+ economies | Production, IO structure | OECD open terms | Fetch |
| CEPII BACI | Bilateral trade, product level | Trade | Open licence | Fetch |
| Subnational output (DOSE), gridded GDP | Regional output by sector | Province economies | CC BY 4.0 (likely) | Bundle aggregates |
| World Bank WDI | Macro, social indicators; GDP per person at PPP; current account balance (BN.CAB.XOKA.CD, 2000 to 2024) | Initialization, targets; fallback local base prices p_c (ADR 0289); trade-imbalance transfers (ADRs 0100, 0319, 0300) | CC BY 4.0 | Bundle extracts |
| Penn World Table 11 | GDP per person, labour share, price level of domestic absorption (pl_da) | Donor ranking, labour shares, local base prices p_c (ADRs 0282, 0317) | CC BY 4.0 | Bundle |
| UN World Population Prospects | Fertility, mortality, age structure | Demography | CC BY 3.0 IGO | Bundle extracts |
| UN international migrant stock; bilateral flow estimates | Migration | Migration | Varies | Fetch |
| Barro-Lee, Wittgenstein Centre | Education attainment | Pops | Varies | Fetch |
| ILOSTAT | Labour status | Pops | Open terms | Fetch |
| National financial accounts; IMF and BIS series | Balance sheets, rates, debt | Finance | Varies | Fetch |
| Exchange-rate regime classification (Ilzetzki-Reinhart-Rogoff) | De facto regimes | Monetary module | Free for research | Fetch |
| Government revenue and spending datasets | Taxes, spending | Fiscal | Varies | Fetch |
| V-Dem, V-Party | Institutions, parties | Politics | CC BY-SA 4.0 | Bundle extracts |
| ParlGov | Parties, elections, cabinets | Parties | Open (share-alike) | Fetch |
| Ethnic Power Relations / GeoEPR | Ethnic groups, status, areas | Identity, conflict | Open with attribution | Fetch |
| UCDP (including GED) | Conflict events, georeferenced | Conflict targets | CC BY 4.0 | Bundle extracts |
| Correlates of War; military spending series | Capabilities, alliances, spending | War module | Free with citation | Fetch |
| Global Energy Monitor trackers | Power plants, mines, pipelines | Energy assets | CC BY 4.0 | Bundle |
| ERA5 reanalysis | Weather 2000-2024 | Replay history (ADR 0241) | Copernicus licence (attribution) | Fetch, bundle aggregates |
| FaIR | Climate emulator (software) | Climate module | Apache 2.0 | Dependency |
| Disaster databases (e.g. EM-DAT) | Disaster events | Validation only | Restricted | User fetches; never bundled |

### 16.1 Pipeline

The `ceteris-pack` crate builds the pack reproducibly (`ceteris pack build`, ADR 0113): download with checksums, harmonize to the H3 grid and province map, impute with documented rules (for example, downscaling national ICIO output to provinces with subnational sector shares and night lights; imputing ICIO coefficients for the roughly 115 economies outside OECD ICIO from regional averages adjusted by income), reconcile to national totals, and emit versioned Parquet tables plus a manifest recording every source, version, licence and imputation flag. Fetch-only sources are downloaded on the user's machine at build time. The start year is a pipeline parameter (1995-2022).

Values are US dollars at the start year's market exchange rates; ADR 0098's rescale to 1999-2001 rates is dropped (ADR 0283). Each country's local goods start at its price level p_c (ADR 0282): PWT 11 pl_da, the price level of domestic absorption (ADR 0317), over the US's, else the level WDI implies, GDP at market rates over GDP per person at PPP times population (ADR 0289), over the US's of the same year, else the imputation donor's, each kept only where its residual from the Penn effect (ln p on ln GDP per person, fitted on the PWT levels) lies within Tukey's far-out fences of the fit's residuals (3 interquartile ranges beyond the quartiles), the source recorded per country. ADR 0098's real GDP check compares the pack's GDP over p_c with WDI's GDP at PPP, current international dollars of the start year (ADR 0285), as totals, so that no population enters (ADR 0290). With the pack's GDP equal to WDI's it measures the two sources' price levels against each other, so it is enforced only where PWT's 2000 price level is an ICP or Eurostat-OECD benchmark (PWT `i_cig`), at 10% (P160), and reported with `i_cig` for the rest (ADR 0290; ADR 0317 had reported it for all).

A donor's input-output table is balanced to its economy's own totals by RAS (ADR 0286): the use table [Z | F], intermediate use by good and final demand, to row totals of domestic use X + M - E by good (output, BACI imports and exports; local goods are not traded, so their use is their output), column totals of intermediate inputs X - VA by good and final demand VA + M - E, to 1e-10 in at most 200 iterations. An economy with negative domestic use of a good, or without BACI trade, keeps the donor's table, flagged; the imputed economies' value added is reported against the ICIO rest-of-world block.

