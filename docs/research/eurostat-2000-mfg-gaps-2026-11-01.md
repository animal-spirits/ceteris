# Filling the Eurostat 2000 manufacturing gaps (ADR 0275)

Date: 2026-11-01. For the 50-goods pack (ADR 0271), manufacturing province spread by RAS (ADR 0275).
Scope: DK, CZ, HU, NO, BG, SI, and the partial FI and PT. The SFI steel item is out of scope.

## 1. What the gap actually is

Measured on the pinned file `eurostat_sbs_r_nuts03.tsv.gz` (sha256 9df6be21..., on the Mac in
`data/cache/`). Cells are NUTS 2 regions x 23 NACE Rev 1.1 manufacturing divisions (DA15 to DN37)
with a value, as a share of all cells. Wages (V13320) and persons employed (V16110) have the same
pattern; the table has no value added.

| Country | NUTS 2 (2006) | 2000 | Nearest full year | Pack provinces (Natural Earth) |
| --- | --- | --- | --- | --- |
| DK | 1 (DK00) | 0 | 2001: 1.00, but one region | 5 regions (2007) |
| SI | 1 (SI00) | 0 | none, 1995-2007 | 12 statistical regions (+181 municipalities) |
| BG | 6 | 0 | none, 1995-2007 | 28 oblasts |
| HU | 7 | 0 | 2001: 0.96, 2003: 0.98 | 20 counties (+23 urban counties) |
| NO | 7 | 0 | 2003: 1.00 (employment), 0.84 (wages) | 19 counties + Svalbard |
| CZ | 9 | 0 | 2004: 0.78, 2002: 0.70 | 14 kraje |
| FI | 5 | 0.20 (1 region) | 2005: 0.88, 2002: 0.69 | 18 maakunta |
| PT | 7 | 0.25 (2 regions) | 2002: 0.79 | 18 districts + 2 autonomous regions |

Three findings change the question:

1. **DK and SI cannot get a spread from this table in any year.** Under NUTS 2006 each is one
   NUTS 2 region, so the table only repeats national totals. This is not a 2000 gap.
2. **The row totals are already there.** DOSE V2.14 has 2000 manufacturing GVA for every province
   of DK (5), CZ (14), NO (19), BG (28), SI (12), PT (20) and FI (5 old provinces). HU is the one
   hole: all 20 counties are empty in 2000 and filled from 2001. So what is missing is the split of
   each province's manufacturing across the 19 goods, not where manufacturing is.
3. **NUTS 2 is coarser than the provinces even where Eurostat is complete.** Every gap country has
   more provinces than NUTS 2 regions (BG 28 vs 6, HU 20 vs 7, NO 19 vs 7, CZ 14 vs 9). Joined by
   overlap, each NUTS 2 region gives one goods mix to all its provinces. A source at NUTS 3 or
   national-region level is better than Eurostat would have been.

## 2. Candidate sources

National totals already come from ICIO, so national-only sources (UNIDO INDSTAT, OECD STAN, SSB
table 03325) add nothing to the spread. They are left out below. "Licence checked" means I read
the provider's page today; the others are from memory and need a check before use.

### Option 0: nearest-year Eurostat (no new source)

The same pinned file in 2001 to 2004. It needs no new licence, no new fetch entry and no new
parser: only a per-country year in the config.

- HU 2001 (0.96), NO 2003 (1.00 employment), CZ 2004 (0.78), FI 2005 (0.88), PT 2002 (0.79).
- Cost: the mix is one to five years late. Manufacturing mix by region moves slowly, but CZ and
  HU had large FDI auto plants opening 2000 to 2004 (C29), so the late year overstates C29 in some
  regions. RAS keeps the 2000 national totals and the DOSE row totals in any case.
- Cells still missing are mostly confidentiality suppressions; they start from the DOSE row share
  as ADR 0275 already says for a seedless good.
- Does nothing for DK, SI or BG.

### Denmark

- **Statistics Denmark regional accounts** (StatBank, gross value added and employment by
  province and industry). Eurostat's metadata says the series starts in 2000, covers 5 regions and
  11 provinces, and is published at the "10a3" grouping, computed from 117 industries. The 10a3
  grouping has manufacturing as one item, so it gives row totals (which DOSE has) but no goods mix.
- **RAS (register-based labour force statistics)**, employment by industry and area. Statistics
  Denmark says RAS was revised back to November 2008 only; earlier years exist on a different
  basis. A 2000 table by municipality and detailed industry may exist in older StatBank series;
  not confirmed.
- Licence: Statistics Denmark allows free reuse with source attribution; not rechecked today
  (their terms page was down).
- Verdict: no 2000 goods mix found. DK is 5 provinces; leave it on DOSE rows plus plant seeds.

### Slovenia

- **SURS SiStat** has regional accounts by the 12 statistical regions (NUTS 3) from 2000, at
  sector level (manufacturing not split by division). Enterprise statistics by region and SKD 2002
  division may exist for 2000 to 2007; I could not confirm a table.
- Licence: not confirmed today.
- Verdict: a small share of world manufacturing VA. DOSE rows plus seeds should cover the heavy
  goods (Jesenice and Ravne steel, Anhovo cement, Kidricevo aluminium), if the seed sources
  list them; not checked.

### Bulgaria

- **NSI** publishes employees, wages and enterprise counts by district (oblast), and "annual
  regional data" with section D (manufacturing) in the NACE.BG 2003 series. No division-level
  2000 table found online. 2000 to 2002 numbers exist in the printed "Regions, districts and
  municipalities" yearbooks (PDF), which would need hand extraction.
- Licence: NSI allows reuse with attribution; not rechecked today.
- Verdict: DOSE has all 28 oblasts. Plant seeds should cover Kremikovtsi steel, the Neftochim
  Burgas refinery and the Pirdop copper smelter, most of Bulgaria's traded heavy goods (whether
  the seed sources list them is not checked).

### Hungary

- Nearest year: Eurostat 2001 (0.96 of cells). Best fit of the gap countries.
- Row totals: DOSE 2000 is empty. Use DOSE 2001 scaled to the ICIO 2000 national total, or
  Eurostat `nama_10r_3gva` (NUTS 3 GVA by A*10, which has manufacturing C separately, from 2000).
  The second needs a new fetch entry; Eurostat's licence is already recorded as CC BY 4.0.
- KSH county yearbooks for 2000 have industry by county; not checked.

### Norway

- **SSB** county tables for 1999 to 2001 (static tables on ssb.no, "Industri og bergverk" by
  fylke) give employment, wages, production value and value added by county, but only for
  industry and mining as a whole. Table 03325 has every division for 1999 to 2008 but nationally.
  No county x division table for 2000 found.
- Licence checked: SSB content is CC BY 4.0 (ssb.no/en/diverse/lisens), commercial use allowed.
- Verdict: Eurostat 2003 (full employment coverage) for the mix; SSB 2000 county value added
  could replace DOSE as row totals but DOSE already has the 19 counties.

### Czech Republic

- Nearest year: Eurostat 2004 (0.78) or 2002 (0.70).
- **CZSO regional yearbooks** (one per kraj; I read the 2002 Moravian-Silesian edition's notes;
  whether an edition carries 2000 data is not confirmed) have industry by OKEC division
  (= NACE Rev 1.1) with employees, sales and wages, enterprise method, at kraj level (14, matching
  the provinces exactly). These are PDFs and Excel files in the CZSO archive; 14 yearbooks to
  extract. Value added is not in them.
- Licence: CZSO terms allow use with source citation; not rechecked today.
- Verdict: better geography and the right year, at about a day's extraction. Worth it only if the
  2004 Eurostat mix looks wrong after the auto plants are checked.

### Finland

- **Statistics Finland table 12bd** ("Income and production by area, 2000-2024", regional
  accounts). Gross value added at basic prices for 2000, by 19 maakunta (and 69 sub-regions), in
  11 manufacturing groups: 10-12, 13-15, 16, 17-18, 19-22, 23, 24-25, 26-27, 28, 29-30, 31-33
  (TOL 2008 = NACE Rev 2). It has value added, not wages, and the right year and geography.
- Weakness: 19-22 merges C19, C20, C21 and C22, and 24-25 merges basic metals with fabricated
  metal. Seeds cover C19 (Porvoo, Naantali) and C24 (Raahe, Tornio, Harjavalta, Kokkola);
  Eurostat 2005 divisions can split the rest inside each group.
- Licence checked: Statistics Finland open data is CC BY 4.0 (since January 2018).
- Verdict: the best single replacement in this list.

### Portugal

- Nearest year: Eurostat 2002 (0.79 of cells, 7 NUTS 2).
- **Quadros de Pessoal** (Ministry of Labour, GEP/GEE) counts establishments and employees by
  district and CAE division every year since the 1980s. The published 2000 volume has tables by
  district (matching the 18 mainland provinces); microdata are restricted. Licence of the
  published tables not checked.
- INE regional accounts by NUTS 3 give sector-level GVA only.
- Verdict: Eurostat 2002 for now; Quadros de Pessoal if district detail is wanted later.

### Cross-country options checked and set aside

- **ARDECO (JRC) / LAMASUS** NUTS 3 series 1980-2021 (LAMASUS on Zenodo, CC BY 4.0). Six NACE
  sectors with industry B-E as one, so no goods mix.
- **Eurostat `sbs_r_nuts06_r2`** (NACE Rev 2, 2008 onwards). Covers every gap country and maps
  cleanly to ICIO's ISIC Rev 4 industries (separate C20 and C21), but it is eight years late and
  NUTS 2 only. Not fetched: ec.europa.eu is blocked from both the cloud and the Mac VM today.
- **IPUMS International 2001 census samples** (HU, PT, possibly CZ and SI): industry and region in
  the microdata, but the terms forbid redistribution and need each user to register; bundling
  aggregates in the pack would need a licence ruling.

## 3. How the sources match the 19 manufacturing goods

Every NACE Rev 1.1 source (Eurostat sbs, CZSO, SSB, Quadros de Pessoal) has the same blind spots
against ICIO's ISIC Rev 4 goods, whatever the country:

- DG24 is C20 + C21 (pharmaceuticals are group 24.4).
- DJ27 is C24A + C24B (ferrous 27.1-27.3 vs non-ferrous 27.4); plant seeds split these.
- DM35 is C301 (35.1 ships) + C302T309.
- DE22 includes publishing (22.1), which is a service (J58) in Rev 4.
- DL30, 32 and 33 all fall in C26 (33.1 medical goes to C32, inside C31T33).
- DN37 (recycling) is E38 in Rev 4, not manufacturing.

The pack already has `ISIC31_ISIC4.txt`, so the mapping is one concordance for all sources.
Statistics Finland 12bd is Rev 2 based, so its groups map to ICIO without a concordance but
are coarser.

## 4. Recommendation

1. Nearest-year Eurostat for HU (2001), NO (2003), CZ (2004), PT (2002), recorded per country
   with the year in the row flag of ADR 0275. No new source.
2. Statistics Finland 12bd for FI (2000, value added, 19 maakunta), with Eurostat 2005 splitting
   the merged groups. One new source, CC BY 4.0.
3. DK, SI and BG stay on DOSE rows plus plant seeds, which ADR 0275 already allows for a country
   with no statistics. Together they are a small share of world manufacturing VA.
4. HU row totals: DOSE 2001 scaled to the ICIO 2000 national total.

ADR 0275 names "Eurostat regional SBS ... for 2000". Using other years and Statistics Finland is
a deviation, so it needs a short RFC (next free id from `ceteris-rows next`) before the fetch
code. That RFC would also record that DK and SI were never coverable at NUTS 2.

## 5. What I could not do today

- Eurostat, the national statistics offices and UNIDO are all blocked by the network allowlist,
  both from the cloud container and the Mac VM, so no new tables were downloaded. The coverage
  numbers come from the copy already pinned on the Mac.
- Licences for Statistics Denmark, SURS, NSI, CZSO, KSH and GEP are from memory, not rechecked.
