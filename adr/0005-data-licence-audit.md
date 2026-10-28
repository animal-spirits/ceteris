# 0005. Data licence audit and closed-source fallbacks

Status: accepted; closed-source fallback superseded 2026-10-28 (owner: open source and non-commercial, each user downloads the data; DESIGN 22.1)
Date: 2026-10-23
Design sections: 2.5 (open by construction), 5.2, 5.4, 13.4, 16, 22, 22.1 (licence)

## Context

Section 16 marks its licence column as working assumptions to confirm in a P0 audit.
The audit (checked 2026-10-23 against provider terms pages and repository records;
research, not legal advice) found that several planned sources cannot be redistributed
or used commercially, which an MIT/Apache release would allow downstream:

- Non-commercial or no redistribution: FAO GAEZ v4, EM-DAT, WTO tariff data, UNCTAD
  TRAINS, IMF data, BIS statistics, UNU-WIDER GRD, Correlates of War, SIPRI, Global Data
  Lab, IPUMS International, GADM, EarthStat.
- No licence stated: Ilzetzki-Reinhart-Rogoff, WID.world, EPR 2021, Powell-Thyne,
  ATOP 5.1, VIEWS, Teti's Global Tariff Database.
- Not uniformly permissive: geoBoundaries gbOpen sets a licence per file, and many
  first-level files are ODbL or CC BY-SA.

The owner decided on 2026-10-23: replace restricted sources with redistributable ones
where they exist; where none is adequate, keep the restricted source for now and keep
the project closed source.

## Options

1. Drop every restricted source now and accept the gaps.
2. Keep restricted sources only where no adequate open source exists, mark them, and
   stay closed source until they are replaced or cleared.
3. Keep all originally planned sources.

## Decision

Option 2. `python/ceteris/packs/sources.toml` is the source manifest: every source with
its licence, evidence URL, use and public-release policy. `check_sources.py` validates
it in CI. Sources usable only while nothing is distributed carry
`closed_source_only = true`; setting `distribution = "open"` fails the check until each
is replaced or cleared.

Substitutions that change Section 16:

| Role | Planned | Now |
|---|---|---|
| Province boundaries | geoBoundaries | Natural Earth admin-1 (public domain), FAO GAUL 2024 and OCHA COD-AB as complements |
| Crop suitability, yields | GAEZ v4 | ISIMIP3a GGCMI (CC0), MapSPAM 2000 (CC BY), GDHY; GAEZ kept for suitability classes |
| Disaster events | EM-DAT | USGS ComCat, NCEI hazards, IBTrACS, SPEIbase; EM-DAT kept for flood and impact validation |
| Macro series | IMF | World Bank WDI, GEM, Fiscal Space, IDS; UNCTADstat; OECD |
| Tax structure | Government revenue datasets | OECD Global Revenue Statistics plus WDI |
| Income distribution | WID | World Bank PIP, SWIID; WID kept for wealth |
| Commodity prices | (unspecified) | UNCTADstat, USGS DS-140, EIA; Pink Sheet kept for vendor benchmarks |
| Crisis dates | Laeven-Valencia | World Bank GFDD to 2017, ESRB; Laeven-Valencia kept after 2017 |
| Education history | Barro-Lee, Wittgenstein | WIC back-projection on Zenodo (CC BY), Barro-Lee as cross-check |
| Ethnic status, settlement | EPR / GeoEPR | EPR v1.1 and GeoEPR 2011 (CC0) for 2000; EPR 2021 kept for later years |
| Religion | (national data) | CIA World Factbook archives (public domain) |
| Military spending | military spending series | Global Military Spending Dataset (CC0) |
| Alliances | Correlates of War | ATOP 3.0 (CC0) and DCAD; ATOP 5.1 and COW kept after 2003 |
| Coups | coup datasets | Cline Center Coup d'Etat Project (CC BY) |

Share-alike sources (OpenStreetMap, SPEIbase, V-Dem) ship as separate artifacts under
their own licences, as Section 5.6 already requires for OpenStreetMap.

## Consequences

- Sixteen sources are closed-source-only. The biggest gaps before an open release:
  - bilateral HS6 tariffs (WTO, TRAINS): no open source exists. Requesting a licence
    for Teti's Global Tariff Database is the most promising route;
  - policy rates and bilateral bank claims (IMF, BIS);
  - subnational education and labour (Global Data Lab);
  - ethnic political status after 2005 (EPR 2021) and alliances after 2003 (ATOP 5.1).
- Non-commercial terms bind even closed-source use: if Ceteris Paribus is used
  commercially while closed, the non-commercial sources (GAEZ, EM-DAT, Global Flood
  Database, WTO, TRAINS, IMF, COW, Global Data Lab, Laeven-Valencia) are not usable.
  The manifest's `commercial_use` field lists them.
- Section 22.1's "open source" decision is suspended, not reversed; ADR 0003 (PyPI
  name) matters only when the project opens.
- Year-2000 boundary vintages: no permissive source has them; the pipeline reconstructs
  2000 borders from current boundaries.
- Licences change (ERA5's terms change on 2026-10-28). Each entry carries a `checked`
  date; re-audit before any release.
