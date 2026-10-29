# 0050. Research and release pack profiles, and sources to audit

Status: proposed; amended per ADR 0113 (2026-10-29)
Date: 2026-10-24
Design sections: 16, 16.1, 2.5, 15.4, 22
Phase: pipeline, now

## Context

ADR 0005 and `python/ceteris/packs/sources.toml` already record each source's licence,
commercial use, share-alike terms and release policy, and `check_sources.py` blocks shipping
anything that does not allow commercial use. Two things are missing. First, one pack serves two
purposes that the licences pull apart: research validation, where the best data are often
non-commercial, and a public or commercial release. Second, the gap analysis of 2026-10-24 adds
sources the manifest does not yet list, several of them non-commercial:

- GLORIA multi-regional input-output tables (164 regions, 120 sectors): the best check on the
  imputed input-output structure of the ~115 economies outside OECD ICIO, but CC BY-NC-SA 4.0.
- Jorda-Schularick-Taylor Macrohistory: CC BY-NC-SA 4.0 (the manifest's macro-nc-datasets entry
  may already cover it; confirm).
- ACLED: requires a commercial licence for for-profit use and forbids using it to train systems
  that substitute for it, for any purpose.
- IHME Global Burden of Disease: non-commercial; WHO Global Health Estimates are the
  alternative (ADR 0033).
- Global Terrorism Database: non-commercial.
- renewables.ninja capacity factors: non-commercial (ADR 0034).
- V-Dem v16 (March 2026) supersedes v15; CC BY-SA 4.0, commercial use allowed with share-alike
  on redistributed derivatives.

## Options

1. One pack, excluding every non-commercial source.
2. Two pack profiles built by the same pipeline: research (may read closed-source-only and
   non-commercial sources for calibration and validation, never distributed) and release (only
   sources whose licence allows the planned distribution).
3. Separate pipelines.

## Decision

Option 2.

- `check_sources.py` gains `--profile research|release`. In the release profile, any source
  with `commercial_use = false` or `closed_source_only = true` fails the build if the pack
  reads it; in the research profile it is allowed with `use` in {validation, complement,
  fallback}.
- The pack manifest records its profile; the scorecard records which profile produced it, and
  results that need research-only data are labelled as such.
- Imputation for non-ICIO economies stays on releasable sources (ICIO, BACI, FIGARO, own
  imputation); GLORIA validates it in the research profile, and the imputation error by economy
  is published with the scorecard.
- ACLED is excluded in both profiles because of its machine-learning clause; UCDP GED (CC BY
  4.0) remains the conflict backbone.

Sources to audit and add, each with its provider page checked (the gap analysis flagged several
licences as unverified): GLORIA, ACLED (to record the exclusion), IHME GBD, WHO GHE, GTD,
renewables.ninja, V-Dem v16, WCDE v3, MESMER and its parameter sets, AR6 sea-level projections,
WRI Aqueduct, FAOSTAT, AMIS, IDMC, CShapes 2.0, DESTA, ICOW, Romelli CBIE, Garriga CBI, Global
Sanctions Database, USGS Mineral Commodity Summaries, Smithsonian Global Volcanism Program,
Baumeister oil shock series, Cruces-Trebesch haircuts, Horn-Reinhart-Trebesch China lending,
IMF Investment and Capital Stock Dataset and PIMA, Missing Profits of Nations, OECD AMNE,
FIGARO, EXIOBASE (newer versions may restrict commercial use), ARDECO, Economic Transformation
Database, Ember, IRENA statistics, OECD Pensions at a Glance, Kummu et al. 2025 subnational GDP
(Zenodo; supersedes or complements kummu-gdp), ALFRED vintages, OECD Economic Outlook and IMF
WEO vintages, ITU indicators, van Donkelaar PM2.5, published AI exposure indices. Global Energy
Monitor's statement of CC BY 4.0 conflicts with a non-commercial checkbox on its download form;
get written confirmation before relying on it for release.

Second scan (2026-10-24, beyond the economic core). More sources with restrictive or unclear
terms, several underpinning existing or proposed layers:

- Ethnic Power Relations: EPR v1.1 and GeoEPR 2011 on Harvard Dataverse are CC0 (possibly a
  platform default) and cover the 2000 initialization; EPR Core 2021 and GeoEPR 2021 state no
  licence, so all rights are reserved by default, and they carry group status after 2005 (10.8,
  11.6). Request written permission now: confirm the legacy CC0, and allow the 2021 versions
  fetched from ICR with province aggregates in public packs.
- Wittgenstein Centre: the Zenodo WIC3.005 release (V15, September 2026) is CC BY 4.0 and holds
  education-specific fertility, survival, education shares and sex ratios at birth for
  2020-2100, which covers most of ADR 0032. The Data Explorer v3 states no licence (IIASA:
  "Open Access. Pending specific license."). Ask for attainment progression ratios and
  education mortality differentials under CC BY if they are not in the files, and for the
  Explorer's terms.
- WHO data portal: CC BY-NC-SA 3.0 IGO, non-commercial (ADRs 0033, 0061).
- World Values Survey and European Values Study: non-profit use only, no redistribution (ADR
  0055).
- Gallup World Poll and World Happiness Report data: no reuse without written permission.
- Afrobarometer geocoded data: a non-commercial pledge.
- Seshat: CC BY-NC-SA on the official downloads page.
- UNODC homicide publications: no commercial use; check the data portal's terms (ADR 0060).
- World Christian Database: subscription only; excluded.
- TeleGeography's submarine cable map: restrictive terms; derive topology from open sources
  (ADR 0075).
- Commercial-safe additions: World Bank Harmonized Learning Outcomes and Human Capital Index
  (CC BY 4.0), AidData GCDF 3.0 (ODC-By, with OSM-derived features under ODbL), GHSL (free and
  open; confirm), UCDP (CC BY 4.0), V-Dem (CC BY-SA 4.0), Global Forest Watch (CC BY 4.0).
- To audit: Archigos, REIGN, ITU indicators, Hanson-Sigman capacity, Regional Authority Index,
  PRIndex, IPI peacekeeping, Voeten UN voting, GSMA mobile money, Freedom on the Net, NCD-RisC,
  CDC WONDER (public domain), UNICEF-WHO-World Bank malnutrition estimates, SAGE or MIRCA crop
  calendars, FAO fisheries statistics, Sentinel-5P (Copernicus), Transboundary Freshwater
  Dispute Database.

## Migration

- Pipeline: the profile flag in `check_sources.py` and the pack build; a CI job builds the
  manifest check for both profiles.
- Manifest entries for the sources above, each with `checked` set on the day the provider page
  is read.

## Tests

- A fixture manifest with a non-commercial source fails the release check and passes the
  research check.
- The release pack's provenance lists no source with `commercial_use = false`.

## Consequences

Research quality and release legality stop competing. The Steam release plan inherits a
mechanical guarantee instead of a manual review.

## Amendment (owner, 2026-10-25)

The release profile failed on four sources that P2 added. The owner's rule: replace each with a source as good
or better whose licence allows commercial use on a confident reading; where none exists, enforce the release
profile only when `distribution` is "open"; run closed for now (ADR 0005). All four were resolved, so the release
profile stays enforced in every distribution and `check_sources.py` keeps its checks.

- HS to industry bridge: WITS HS 1988/92 to ISIC Rev 3 (no licence stated) chained with the UNSD ISIC Rev 3.1 to
  Rev 4 table (UN copyright) is replaced by the OECD BTiGE conversion key (2025; OECD terms allow any use,
  including commercial, with credit), `oecd-btige-key`. It maps each HS 1988/92 line to one CPA 2.1 activity
  (ISIC Rev 4 at division level, split as ICIO 2025 splits industries), so the equal split over linked divisions
  and the principal-link rule are gone. Every 2000 BACI line is covered (the old route left 0.6% of trade out).
  The pipeline adapts the key in two places, which the OECD terms permit (`goods.read_hs_cpa`): non-monetary
  gold (HS 7108; the key puts unwrought gold in metal ores, B072) is read as basic metals (C24B), so good 2
  stays physical bulk; printed books and recorded media (J58, J59_60) are read as printing and reproduction
  (C18), light manufacturing. Electricity (D35), recycling (E38) and the service and art codes (H53, M71, M74,
  R90, R91) stay with the local goods: 23.1 billion USD, 0.35% of 2000 BACI trade, outside the traded goods.
  Weights per value, new over old: agriculture 1.05, crude oil and gas 1.00, coal and other mining 0.96, food
  0.96, light manufacturing 1.55 (it no longer receives a share of the repair divisions), chemicals, refined
  fuels and metals 0.87, machinery, electronics and vehicles 0.82; coal over machinery is 221 (was 188).
- Trade elasticities: each CPA activity takes the ISIC Rev 3 division of Caliendo and Parro's Table A.1
  (`theta.REV3_OF_CPA`). Theta per good becomes 9.11, 13.53, 13.53, 2.62, 9.15, 10.57, 5.19 (was 6.73, 12.64 and
  5.33 for goods 4 to 6). The ADR 0023 rule draws three classes: 2.62 (food), 5.19 (machinery, electronics and
  vehicles) and 10.57 (the other five goods), instead of 2.62, 5.88 and 12.55. Before the gold and books
  adaptations it drew four (2.62, 5.19, 9.94, 13.53); the owner adopted the key's classes on 2026-10-25, and
  `TradeParams::default()` moves to them in a separate P2 task (done, ADR 0023).
- Income where WDI and PWT have none: UNSD National Accounts (UN copyright) is replaced for the Cook Islands by
  ADB Key Indicators (`adb-kidb`, free for commercial use with attribution; 5,091 against 6,307 USD per person in
  2000) and for North Korea by the Maddison Project 2023 (`maddison-2023`, CC BY 4.0). Maddison values are 2011
  international dollars, so North Korea is ranked and scaled against its donors' Maddison values, never against
  current-USD incomes, as PWT is used. North Korea's value added falls from 10.5 to 6.8 billion USD and its donor
  moves from Viet Nam to Cambodia; no other economy changes.
- Validation-only files: a source read only by tests or diagnostics is not a pack source. Such files live in
  `python/ceteris/packs/validation.toml` (same schema as `fetch.toml`), which `fetch` downloads and verifies but
  the pack build neither fetches nor lists in its provenance, and which `pack_sources()` does not read.
  `check_sources.py` requires each to name a source with `use = "validation"`. The Yotov et al. (2016) sample
  (`yotov-2016`, WTO all rights reserved) moved there; `test_gravity_reproduces_yotov_2016` is unchanged.

## Amendment (ADR 0113, 2026-10-29)

The check moved to Rust with the same rules: `check_sources.py --profile` is `ceteris pack
check-sources --profile research|release`, which CI runs for the research profile. The manifest,
`fetch.toml` and `validation.toml` moved from `python/ceteris/packs/` to
`crates/ceteris-pack/config/` with their schemas unchanged; `pack_sources()` is
`ceteris_pack::sources::pack_sources`, and `test_gravity_reproduces_yotov_2016` is
`gravity_reproduces_yotov_2016` in `crates/ceteris-calibrate/tests/gravity.rs`.
