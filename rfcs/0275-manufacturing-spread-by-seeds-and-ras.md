# 0275. Manufacturing province spread from plant and census seeds, fitted by RAS

Status: accepted
Date: 2026-10-31
Design sections: 5.2, 7.1, 22 (pack build); amends ADRs 0014 and 0271 (Downscaling)

## Context

ADR 0271 gives each industry its group's downscaling proxy, so all 19 manufacturing goods share
one province spread within a country. That spread is DOSE manufacturing GVA where a DOSE region
matches a province; DOSE V2.14 has 2000 manufacturing figures for 44 countries, 86.9% of world
manufacturing VA in ICIO 2000. Elsewhere it is gridded GDP, then population. Shipyards, refineries
and pharmaceuticals would then sit in the same proportions in every province.

Only traded goods need spread data: calibration stage 3 solves local output from local demand, so
a local good's downscaled VA is overwritten.

## Options

1. Plant seeds for heavy industry and national industry statistics for the US, Japan and the EU,
   fitted by RAS.
2. Plant seeds only, fitted by RAS.
3. The shared spread of ADR 0271 until a finer source is found.

## Decision

Option 1 (owner, 2026-10-31).

- **Fit.** Per country, a provinces x 19 goods matrix is scaled biproportionally (RAS): column
  totals are the ICIO national VA by good; row totals are DOSE manufacturing VA where DOSE matches
  a province, scaled to national manufacturing VA. A good without a seed starts from the DOSE row
  share, so it takes each province's residual; a country with no seeds gets today's spread exactly.
  Without DOSE there are no row totals and each good is normalised alone.
- **Plant seeds.** Capacity in service in 2000 times one VA per unit of capacity per country:
  steel plants for C24A (GEM Global Steel Plant Tracker), cement plants for C23, refineries for
  C19, aluminium and copper smelters for C24B. Sources are fixed when their licences are checked.
- **National statistics.** Province by two-digit industry VA, payroll or employment for 2000:
  US County Business Patterns (states), Japan Census of Manufactures (prefectures), Eurostat
  regional SBS (NUTS 2, NACE Rev 1.1, joined to provinces by overlap). Each maps to ICIO industries
  by a published concordance.
- **Other traded goods.** A02 takes forest cover 2000; A03 population within 50 km of the coast;
  B05 GEM coal mines; B07 MRDS precious, base and ferrous sites (and uranium); B08 MRDS industrial
  sites; A01 and B06 as now.
- **Flags.** Each row records its level (plant, census, DOSE, gridded GDP, population) and is
  imputed where a fallback placed it, as in ADR 0014.
- Other countries' statistics (China, Korea, Canada, Mexico, Brazil, India) come later, one ADR
  each.

## Consequences

- With the US (26.2% of world manufacturing VA), Japan (19.1%) and the EU, about 65 to 70% of
  manufacturing is placed from statistics, if Eurostat's 2000 regional coverage holds.
- Each new source needs a `sources.toml` entry and a licence check before the pack uses it.
- About two owner-weeks within the P4 task of ADR 0271.
- RAS margins are tested to 1e-12, and a seedless country reproduces the shared spread exactly.
