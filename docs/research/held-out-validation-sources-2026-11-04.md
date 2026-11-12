# Held-out validation sources (2026-11-04)

Purpose: data the calibration never reads, used to show the model works rather than fits.
Closed or non-commercial (NC) licences are fine here: distribution is "closed", users fetch
their own copies, and a scorer only publishes aggregate scores.

Grounding: `crates/ceteris-pack/config/sources.toml` (113 entries, 12 already `use =
"validation"`), DESIGN 15.3 to 15.7, ADR 0107 (holdout split; file status still "proposed"),
ADR 0241 (only 1 Jan 2000 data in a run), pre-P3 PLAN C6/C7, `specs/pre-p3/C7-LITERATURE.md`.
Licences of rows marked * are from memory or the provider's general terms and are unverified.

## 1. What calibration reads today (must not be scored against itself)

Pack readers on main: ICIO (IO structure, home shares that fit nu per class, ADR 0092), BACI
and CEPII Gravity (PPML theta, borders), DTA/tariffs, PWT (capital, TFP, labsh, price levels),
WDI, DOSE, Kummu GDP, US CBP, METI, Eurostat SBS, MapSPAM, GEM, USGS, EIA SEDS. On
`pre-p3/audit`: WIOD SEA labour shares (ADR 0307), AAMNE foreign shares (0309), SWIID Gini plus
US DFA 2000Q1 top-10% wealth (proxy, RFC 0303). Yotov (2016) is an estimator reproduction test,
not a model test.

Corollary: C7 "openness within 1.5 of ICIO" passes by construction (C7-LITERATURE section 3);
it is a fit check, not validation.

## 2. Inventory by domain

Status: **H** held out (never read by `pack build` or `ceteris-calibrate`), **C** calibration,
**R** report-only cross-check (shares upstream with a C source, so weak as a test).
When: **T0** 2000 cross-section, scorable now; **S** 2011-2018 selection; **Z** 2019-2024 sealed.

| Domain | Source | Licence | Status | When | Compared against |
| --- | --- | --- | --- | --- | --- |
| IO / production | Eora26 or full MRIO 2000* | free academic NC, registration | H | T0 | foreign input shares by country and class; IO coefficient rank correlation (non-ICIO construction, 190 countries, covers the non-ICIO economies the model imputes) |
| IO / production | WIOD 2013/2016 NIOT, WIOT 2000 | CC BY 4.0 | R | T0 | same as Eora, 40 economies; shares national SUTs with ICIO |
| IO / production | EXIOBASE 3.9+* | NC | H | T0 | sector output by country, energy and metals rows |
| Energy | IEA World Energy Balances (free tier)* | CC BY-NC-SA 4.0 IGO | H | T0, S, Z | coal, oil, gas production and final use by country (EIA is C for US states and prices only) |
| Energy | Energy Institute Statistical Review* | free, no reuse licence | H | T0, S, Z | same, second opinion |
| Trade | IMF DOTS (in `imf-data`) | IMF terms, closed | H | T0, S, Z | bilateral goods totals; partly independent of Comtrade/BACI reporting |
| Trade | WTO-OECD BaTiS services* | terms unclear | H | T0 | services trade shares (goods are C via BACI and ICIO) |
| Trade | literature: Head-Mayer 2014 distance -0.9, border 1.55 | n/a | H | T0 | V2 stylized fact on simulated flows |
| Wealth | WID wealth shares (`wid`) | no licence, closed | H | T0, S, Z | top-10% and top-1% wealth share, every country except the US (US DFA is the proxy's only fit point, ADR 0311) |
| Income | LIS Key Figures / LISSY* | registration, NC research | H | T0, S | income deciles and Gini for ~50 economies, disposable vs market |
| Income | WID pre-tax income shares | no licence, closed | H | T0, S, Z | top-10% income share (C6 already reports it) |
| Prices | World Bank Pink Sheet (`pink-sheet`) | third-party, closed | H | S, Z | commodity price volatility and co-movement (15.4); T0 relative prices report-only |
| Prices | IMF PCPS / IFS CPI (`imf-data`) | IMF terms, closed | H | S, Z | inflation by country; WB GEM CPI is C for t0 levels |
| Prices | FAO FFPI (`fao-ffpi`) | CC BY + clause | H | S, Z | food price spikes |
| Prices | Eurostat-OECD PPP 1999/2002* | Eurostat terms (non-EU data NC) | R | T0 | price levels; PWT price levels are C |
| Subnational | Gennaioli et al. 2013 regional GDP* | no licence stated | H | T0 | province income, log RMSE and rank correlation (DOSE and Kummu are C) |
| Subnational | GDL SHDI (`gdl-shdi`) | closed | H | T0, S | subnational income index, education |
| Subnational | VIIRS lights (`viirs-vnl`) | CC BY | H | S, Z | province growth after 2012 |
| Agriculture | EarthStat (`earthstat`, rejected for build) | NC | R | T0 | crop area and yields; anchored to FAOSTAT like MapSPAM |
| Finance | BIS LBS (`bis-stats`) | closed | H | S, Z | cross-border bank claims (if not used to initialise, 15.7) |
| Finance | Laeven-Valencia (`laeven-valencia`), WB GFDD, ESRB | closed / CC BY | H | S, Z | banking-crisis counts per decade |
| Macro paths | IMF WEO vintages (`imf-data`) | closed | H | S, Z | benchmark forecasts (15.7), scored at matched horizons |
| Hazards | EM-DAT, Global Flood DB (`em-dat`, `global-flood-db`) | NC | H | S, Z | disaster counts and damages per decade |
| Conflict | ACLED* , ViEWS (`views`) | NC terms / no licence | H | S, Z | event hazards by region; ViEWS as forecast benchmark |
| Politics | Polity5*, Freedom House*, COW (`cow`), BMR, WTH | NC / CC0 | H | S, Z | regime-transition hazards (V-Dem is C) |

Best first-tier picks (independent upstream, NC, cover the current econ core): **Eora 2000,
WID wealth (non-US), IEA balances 2000, Gennaioli regional GDP, LIS income**. The dynamic ones
(Pink Sheet, IFS, WEO, BIS, crises, hazards) wait for P3+ paths.

## 3. Validation protocol

1. **Register.** Each H source carries `use = "validation"` (or a `holdout = true` flag) in
   sources.toml; `pack check-sources` fails if a `pack build` or `ceteris-calibrate` reader opens
   one. Only a scorer (P9's analysis crate; for now a `ceteris-calibrate` report test) reads them.
2. **Pre-register.** Before the first run against a source, commit the metric, tolerance and
   target file hash (ADR 0107 sealed mechanic). No tolerance or parameter changes after seeing a
   score; a miss goes to Antonio with the table, as C7 Step 3 already says.
3. **Metrics and starting tolerances** (T0, static 2000 equilibrium):
   - shares (foreign input, energy production, top-10% wealth): per-country ratio model/data in
     [1/1.5, 1.5] for at least 70% of countries, and the cross-country distribution reported
     (median, 10th and 90th percentile), as C7-LITERATURE asks for openness;
   - levels with wide spread (province income, sector output): Spearman >= 0.7, log RMSE reported;
   - signs and rank of welfare responses (already in C7 via ACR).
   S/Z periods: CRPS, PIT and 90% coverage (15.4), banded counts per decade for events.
4. **When.**
   - C6/C7 (pre-P3 gate): T0 checks on Eora, WID wealth, IEA, Gennaioli, report-only.
   - Each phase gate from P3: rerun T0, plus S scores once paths exist; ablation uses S only.
   - Sealed Z: once per gate, recorded in the changelog, never used to choose (0107).
   - Leave-one-region-out (15.7) at P9.
5. **Leakage rules.** A source moves from H to C only by an RFC naming its replacement held-out
   source. A source that shares upstream with a C source (BACI and Comtrade, MapSPAM and
   FAOSTAT, TiVA and ICIO) is R, never cited as an independent pass.

## 4. Decision

Antonio, 2026-11-04: report-only at C6/C7; no RFC, register or check-sources guard until P9.

## 5. Earlier open choices

- Report-only or blocking at C7 for the T0 held-out checks (recommended: report-only now,
  blocking from P9 once tolerances are pre-registered).
- Whether to put the register and the check-sources guard in an RFC now (next free RFC 0304) or
  keep this list in project files until P9.
- Licence checks for the * rows before fetching (Eora, IEA, LIS, ACLED, Gennaioli, Polity5).
