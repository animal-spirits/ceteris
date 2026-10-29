# 0009. County display layer

Status: accepted (owner, 2026-10-23); amends ADR 0006 and ADR 0008
Date: 2026-10-23
Design sections: 5.1, 5.2, 16, 19.1 (Map UI), 20.1, ADR 0005, ADR 0008
Paths (ADR 0113, 2026-10-29): the lock is `crates/ceteris-pack/config/counties.lock.toml`; `ceteris pack record-counties` regenerates it.

## Context

The owner asked for sub-provincial units (counties, municipalities). Making them simulation
provinces would multiply markets, pops and the trade solve about tenfold against the 19.2
budget (about 50,000 units instead of 4,796), and the economic data is first-level. A
display and aggregation layer leaves the simulation unchanged: counties are drawn on the map
and carry raster statistics, and each belongs to exactly one province.

Findings from metadata and a synthetic rendering test (2026-10-23):

- geoBoundaries gbOpen has ADM2 for 180 countries (49,363 units) and ADM3 for 81. The 73
  countries without ADM2 cover 3.6% of land (largest: Greenland, Libya, South Sudan, UAE).
- Levels are not comparable across countries: ADM2 is the departement in France (96), the
  municipality in Brazil (5,570), Mexico (2,457) and Japan (1,745), and coarser than our
  provinces in Italy (20). A level has to be chosen per country.
- Licences, by unit, at the chosen levels: about 72% public domain or attribution-only; about
  28% share-alike (ODbL, CC BY-SA), including India and Russia. Usable while closed; on
  opening they ship as a separate artifact under their own licence, as OpenStreetMap does.
- Boundary vintages are 2005 to 2023; no open source has 2000 county boundaries.
- Synthetic layer of 60,543 units nested in provinces, densified to 3.3M vertices (the
  estimate for real boundaries simplified at 0.01 degrees), as 66.5 MB of GeoJSON, in headless
  Chrome on Metal at 1440x900, scale 2: 60 fps in every configuration, p99 frame 16.8 ms, no
  frame over 33 ms; recolouring all units through feature state takes 4 ms. Costs: load 2.6 s
  instead of 0.5 s, main-thread heap 300 to 470 MB instead of 70 MB.

## Options

1. Counties as simulation provinces (rejected for v1: budget and data, see above).
2. County display layer from geoBoundaries, one level per country, GeoJSON.
3. As 2, delivered as PMTiles vector tiles.
4. No county layer.

## Decision

Option 3, and oversized provinces are now split along county borders (amending ADR 0006).

- Source: geoBoundaries gbOpen simplified files (377 MB for 166 countries) at commit-pinned
  URLs. `python/ceteris/packs/counties.lock.toml` records each file's level, licence, year,
  sha256 and size; `python -m ceteris.packs.counties --record` regenerates it deliberately.
- Level per country: the coarsest of ADM2 and ADM3 with 3 to 120 units per Natural Earth
  province, else 1.5 to 3, else none. Overrides: Canada keeps census divisions (76) over 5,162
  subdivisions; Brazil takes its 5,570 municipalities, since no open level lies between them
  and the states. 166 countries, 53,492 units. Spain (next level 8,205 municipalities), Italy
  and the United Kingdom (no level below Natural Earth's) and countries without data show
  provinces only.
- Province splits: an oversized province is split by grouping its counties into parts of equal
  area (weighted recursive bisection); counties larger than half the area limit are first cut
  by Voronoi inside themselves; gaps between the two sources' borders join the part they share
  most border with. A province with fewer than two counties per part, or whose largest part
  would exceed 1.5 times the limit, keeps the Voronoi split.
- Nesting: each county goes to the province it overlaps most, matched on the current or 2000
  country, and is clipped to it.
- Statistics: county elevation and ruggedness are sampled on resolution-6 H3 cells (about
  36 km²); slivers without a cell are sampled at their anchor.
- Delivery: one PMTiles archive (`web/map.pmtiles`, built with tippecanoe) with layers
  provinces, countries and counties (from zoom 4), features numbered from 1. tippecanoe
  writes identical tiles in a varying order, so the manifest and golden file track the sha256
  of each layer's input GeoJSON rather than the archive. Pack format 2.

## Results of the implementation

- 93 oversized provinces: 82 split along counties, 11 by Voronoi (Greenland, northern Canada
  and others with few or no counties). Largest province 402,000 km², as before.
- 53,084 counties nested; 357 unassigned (mostly tiny islands). Among counties small enough to
  fit a province (at most half the limit), 3.0% of area falls outside the assigned province,
  down from 13% with Voronoi splits; the rest is the sources' border disagreement.
- Pack build: about 2.5 minutes, from 25 seconds (resolution-6 sampling and tiling).
- App: 60 fps mean, p99 frame 16.8 ms, no frame over 33 ms over a flight from world view to
  zoom 6 with county borders on (1,249 drawn), heap 128 MB.

## Consequences

- No change to the simulation, the transport graph or the 19.2 budget.
- One more fetch-time dependency with per-file licences; 28% of units block a single-licence
  open release of the layer until shipped separately.
- Granularity stays uneven between countries (municipalities in Brazil, arrondissements in
  France, none in Spain); the inspector names the level.
- Building the pack needs tippecanoe on PATH.
- Province IDs of split regions keep the `ABC-123.k` form, but parts now follow county lines,
  so part k covers different ground than in the first P1 build.
- Open: the production app serves tiles through Tauri's asset protocol, whose HTTP Range
  support PMTiles needs is unverified; development uses Vite, which supports it.

## Spike results (real data, 2026-10-23)

geoBoundaries simplified files at commit 9469f09, 166 countries, 377 MB.

- Nesting: 53,068 of 53,441 units overlap a province of their country. The 373 that do not
  are mostly tiny islands, plus Crimea, where Natural Earth's current country (Russia) differs
  from geoBoundaries' (Ukraine); matching on the 2000 country as well fixes that case.
- Misfit by count: the median unit has 0.5% of its area outside its province; 14% have more
  than 10%. These are mostly small units (median 244 km²) whose borders differ between the
  sources by a band of 0.02 km (median) to 3.7 km (p99). Clipping hides them.
- Misfit by area: 13% of county area falls outside the assigned province, and 84% of that is
  in provinces split by ADR 0006's Voronoi rule (Canada, Russia, Australia), whose cut lines
  cross large counties. Splitting oversized provinces along county boundaries instead (the
  option ADR 0006 lacked data for) removes most of it; that changes the P1 pack and goldens.
- Coverage: clipped counties cover 95% of their province's area at the median (p10 82%).
  Counties per province: median 9, p90 33, maximum 645 (Sao Paulo).
- Geometry: 2.54M vertices, 52.6 MB GeoJSON; PMTiles (tippecanoe 2.79, zoom 4 to 9) 45.9 MB
  on disk, built in 9 s. Feature ID 0 is dropped by tippecanoe: number features from 1.
- Map, headless Chrome on Metal, 1440x900 at scale 2, flight from world view to zoom 6, about
  1,250 counties rendered at the end:

  | Delivery | Load | Heap | fps | p99 frame |
  |---|---|---|---|---|
  | GeoJSON, provinces only | 0.48 s | 95 MB | 60 | 16.8 ms |
  | GeoJSON, + counties | 1.93 s | 300 MB | 60 | 16.8 ms |
  | PMTiles, provinces only | 0.22 s | 43 MB | 60 | 16.8 ms |
  | PMTiles, + counties | 0.21 s | 90 MB | 60 | 16.8 ms |

- Licences were taken from the geoBoundaries metadata API, which serves each release file's
  own metadata record.
