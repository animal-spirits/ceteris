# RFC 0283. Manufacturing census seeds: nearest year for Eurostat gaps, Statistics Finland for Finland

Status: accepted
Date: 2026-11-01
Design sections: 22 (pack build); amends ADR 0275
Research: [Filling the Eurostat 2000 manufacturing gaps](/mnt/project-files/research/eurostat-2000-mfg-gaps-2026-11-01.md)

## Summary

ADR 0275 seeds the EU manufacturing province spread from Eurostat regional SBS
(`sbs_r_nuts03`, NUTS 2, NACE Rev 1.1) for 2000. Measured on the pinned file, 2000 has no
manufacturing cells for DK, CZ, HU, NO, BG and SI, and 0.20 and 0.25 of cells for FI and PT.
DK and SI are one NUTS 2 region each under NUTS 2006, so the table gives no spread for them in
any year; BG is absent 1995 to 2007. Later years of the same file are near complete: HU 2001
(0.96 of cells), NO 2003 (1.00 for persons employed), CZ 2004 (0.78), PT 2002 (0.79), FI 2005
(0.88). Statistics Finland table 12bd has 2000 gross value added by 19 maakunta in 11
manufacturing groups (TOL 2008), under CC BY 4.0. DOSE V2.14 has 2000 manufacturing rows for
every gap country except HU, which has them from 2001.

## Decisions

### D1. Which plan fills the Eurostat 2000 manufacturing gaps?

Options:

- A. Nearest year + Finland 12bd: HU 2001, NO 2003, CZ 2004, PT 2002 from the pinned file; FI
  from Statistics Finland 2000; DK, SI, BG on DOSE plus plant seeds.
- B. Nearest year only: as A, but FI uses Eurostat 2005 too. No new source.
- C. Also extract national yearbooks: adds CZSO kraj yearbooks and NSI Bulgaria PDFs by hand
  for the 2000 mix, about 2 to 3 more days.
- D. Keep ADR 0275 as is: gap countries fall back to DOSE rows plus plant seeds, with no
  industry split.

Recommendation: A, because it needs no new licence except one CC BY 4.0 table, and FI gets the
right year and finer regions; DK, SI and BG are a small share of world manufacturing.

Decision: A, Antonio, 2026-11-01.
Row: ADR 0287

The row means:

- **Seed year per country.** Eurostat regional SBS seeds use 2000 where the country has it,
  else HU 2001, NO 2003, CZ 2004, PT 2002. The row flag of ADR 0275 records the year used.
  Missing cells start from the DOSE row share, as ADR 0275 does for a seedless good.
- **Finland.** Seeds are Statistics Finland table 12bd, B1GPH (gross value added at basic
  prices), 2000, current prices, by maakunta, joined to provinces by overlap. Its merged groups
  (19-22, 24-25, 26-27, 29-30) are split inside each maakunta by the Eurostat 2005 division
  shares of the NUTS 2 region holding it; plant seeds for C19 and C24 enter as in ADR 0275.
- **DK, SI, BG.** No census seeds: DOSE rows plus plant seeds (ADR 0275's seedless country).
- **HU row totals.** DOSE 2001 manufacturing rows, scaled to the ICIO 2000 national
  manufacturing VA.
- **Concordance.** NACE Rev 1.1 divisions map to ICIO industries through the pack's ISIC Rev 3.1
  to Rev 4 table; 12bd's TOL 2008 groups map directly.
- Statistics Finland needs a `sources.toml` entry and a licence check before the pack uses it
  (ADRs 0005, 0050).

## Spec changes

None in `spec/` text: ADR 0275 is a pack-build rule recorded only in its row. `rows.toml` gains
ADR 0287 (section 22) and the appendix is re-rendered.
