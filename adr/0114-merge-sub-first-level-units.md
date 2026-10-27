# 0114. Merge Natural Earth units below a country's first level

Status: proposed
Date: 2026-10-27
Design sections: 5.2, 22.2 (province-splitting rule); amends ADR 0006

## Context

Natural Earth admin-1 mixes administrative levels. The United Kingdom has 232 units (unitary
authorities, metropolitan and London boroughs) against Brazil's 27 states and Germany's 16;
Slovenia has 193 municipalities and Malta 68 local councils. ADR 0006 only splits large
regions and left merging small units for P4. The owner asked for the fix with the fewest data
problems and the most common sense.

Counts below are from the Natural Earth 10m admin-1 file (2026-10-27).

## Options

1. Merge units back up to the country's real first-level division, from an explicit
   per-country list.
2. Merge neighbouring units until each reaches a population or area floor.
3. Replace the base set for some regions (for example NUTS 2 in Europe).
4. Leave the set as it is until P4.

Option 2 invents units nobody recognises and needs gridded population to draw them. Option 3
has no worldwide equivalent, and Eurostat restricts commercial reuse of non-EU data (ADR 0005).

## Decision

Option 1. Provinces are a country's first-level division as of 2000, or its standard
statistical first level where the administrative one sits below Natural Earth's units. A merged
province is a union of whole Natural Earth units; ADR 0006's area split then runs on the result.

| Country | Units | Provinces | Rule |
|---|---|---|---|
| United Kingdom | 232 | 12 | England by `region` (Greater London is London); Scotland, Wales and Northern Ireland by `geonunit` |
| Slovenia | 193 | 12 | statistical regions, `region` |
| Latvia | 119 | 5 | planning regions, `region` (the 2009 municipalities postdate 2000) |
| North Macedonia | 84 | 8 | statistical regions, `region`; Greater Skopje joins Skopje; 15 units without one are listed below |
| Italy | 110 | 20 | regions, `region` |
| France (metropolitan) | 96 | 22 | regions of 2000, listed below; the five overseas departments stay single provinces |
| Philippines | 118 | 81 | the 16 units of the National Capital Region form one province; the other 22 cities join their province |
| Hungary | 43 | 20 | the 23 urban counties join their county |
| Kosovo (in Yugoslavia) | 30 | 7 | UNMIK districts, `region` |
| Ireland | 34 | 26 | counties: Dublin's four authorities join, as do North and South Tipperary; the four cities join their county |
| 35 countries and territories under 1,000 km² | 349 | 35 | one province each (Malta, Seychelles, Maldives, Palau, Liechtenstein, Barbados, Singapore, Bahrain and others) |

"Join their province" means the province the city shares the most border with; the build
reports each such assignment.

North Macedonia, units with no `region`: Skopje to Skopje; Strumitsa, Gevgelija and Bosilovo to
Southeastern; Jegunovce, Tearce, Tetovo and Vrapciste to Polog; Staro Nagoricane to
Northeastern; Oslomej, Drugovo and Plasnica to Southwestern; Mogila to Pelagonia; Rosoman to
Vardar; Zrnovci to Eastern.

France, 2016 regions (`region`) split back to 2000 regions by department number:

- Grand Est: Alsace 67, 68; Lorraine 54, 55, 57, 88; Champagne-Ardenne 08, 10, 51, 52
- Hauts-de-France: Nord-Pas-de-Calais 59, 62; Picardie 02, 60, 80
- Normandie: Haute-Normandie 27, 76; Basse-Normandie 14, 50, 61
- Nouvelle-Aquitaine: Aquitaine 24, 33, 40, 47, 64; Limousin 19, 23, 87; Poitou-Charentes
  16, 17, 79, 86
- Occitanie: Midi-Pyrenees 09, 12, 31, 32, 46, 65, 81, 82; Languedoc-Roussillon 11, 30, 34,
  48, 66
- Auvergne-Rhone-Alpes: Auvergne 03, 15, 43, 63; Rhone-Alpes 01, 07, 26, 38, 42, 69, 73, 74
- Bourgogne-Franche-Comte: Bourgogne 21, 58, 71, 89; Franche-Comte 25, 39, 70, 90
- Unchanged: Bretagne, Pays de la Loire, Centre, Ile-de-France, Provence-Alpes-Cote d'Azur,
  Corse

Kept as they are, because the units are the country's real first level: Uganda (112
districts), Azerbaijan (78), Moldova (40), Turkey, Thailand, Vietnam, Bahamas, Montenegro,
Serbia and the rest.

- Configuration: the list lives in a pack configuration file, keyed by country, as a mapping
  from Natural Earth `adm1_code` (or a rule over `region`, `geonunit` or `type_en`) to a merge
  group. Natural Earth's `region` is not used anywhere unlisted: in the United States it holds
  census regions.
- IDs: a merged province is `ABC-M` followed by a two-digit group number, in the list's order.
  Its parts after an area split keep the `.k` suffix.
- Data: every per-unit input (DOSE matches, raster aggregates, county nesting) is summed or
  area-weighted over members; nothing is split or imputed.
- Build: in `ceteris-pack` as migration Task 3f (ADR 0113), after the geo-pack parity check
  and before the single golden re-pin. No change to the Python pipeline.

## Consequences

- About 1,150 fewer provinces (4,796 to roughly 3,650), most of them in the European
  development slice; province IDs, the province goldens and the dev-slice province count change
  once, at the re-pin.
- Merges also remove post-2000 boundary vintages in Latvia, Slovenia, North Macedonia and
  Italy, whose Natural Earth units date from later reforms.
- Size spread between countries remains (Uganda, Azerbaijan); a population-based rule is still
  open for P4.
- Single-province island states lose inter-island transport inside the country.
- To check in Task 3f: DOSE matches for the merged countries, since DOSE regions are matched by
  name and some may sit at the merged level and some below it.
