# 0006. Province set at 2000 borders

Status: accepted; amended by ADR 0009 (splits follow county borders where counties exist)
Date: 2026-10-23
Design sections: 5.2, 22.2 (province-splitting rule)

## Context

Section 5.2 asks for first-level regions, frozen at 2000 borders, with very large or very
populous regions split so none dominates its country, for about 4,000 provinces. ADR 0005
replaced geoBoundaries with Natural Earth admin-1 (public domain), which has current borders,
4,596 units at mixed administrative levels and no second-level boundaries to split along.
Gridded population arrives in P4.

## Options

1. Split along second-level boundaries from another source (GADM is excluded; geoBoundaries
   is licensed per file).
2. Split by area only, along straight boundaries drawn through the region's own H3 cells.
3. Do not split until population data exists.

## Decision

Option 2.

- 2000 borders: `borders_2000.toml` remaps successor states to their 2000 country (South
  Sudan to Sudan; Serbia, Montenegro to Yugoslavia; Curacao, Sint Maarten and the BES islands
  to the Netherlands Antilles; Crimea and Sevastopol to Ukraine) and sets the de jure sovereign
  where it differs (Kosovo under Yugoslavia per UNSC 1244; Timor-Leste under UNTAET).
  Everything else keeps Natural Earth's de facto default. Antarctica is excluded.
- Splitting: a province larger than 250,000 km² is cut into ceil(area / 250,000) parts. Its
  resolution-4 H3 cell centres are split by recursive coordinate bisection into groups of
  equal size, in a plane scaled by cos(latitude); the group centroids seed Voronoi regions
  clipped to the province. Provinces across the antimeridian are unwrapped first. Part k of
  `ABC-123` is `ABC-123.k`.

## Consequences

- 4,796 provinces (93 split into 295 parts); the largest is 402,000 km² because some parts
  include water or islands without cell centres.
- Splits are deterministic and depend only on geometry, so IDs are stable across builds.
- Mixed administrative levels remain (the United Kingdom has 232 units, Germany 16); merging
  small units and population-based splitting are revisited in P4.
