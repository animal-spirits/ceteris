# 0115. Territory as partitions of a fixed cell grid

Status: proposed
Date: 2026-10-28
Design sections: 5.1, 5.2, 11.5, 11.6, 11.8, 14, 23; amends ADRs 0006, 0009 and 0114

## Context

Provinces are fixed for a run, and territory moves only in whole provinces (ADR 0006). The
game needs finer moves: front lines, occupation, partition, peace terms that trade a few
districts. Most territorial changes since 2000 are partial (Donbas 2014, Syria and Iraq 2014 to
2017, Yemen from 2015, Karabakh 2020).

Data exists at three grains. Cells: WorldPop population, Kummu gridded GDP, MapSPAM crop area,
ETOPO terrain, with night lights, GHSL and GeoEPR planned. First-level regions: DOSE output by
sector, EIA by US state. Countries: input-output tables and the national series. Counties
(ADR 0009) add shapes and names but no statistics of their own; theirs are sampled from cells.
No source gives pops by education or labour status, or capital, below the country.

Exploration, with diagrams: https://claude.ai/artifact/XcrCGoq3WgM1p9FPSPgMsC

## Options

The choice is two grains: how finely territory can move, and how finely the economy stores
state.

1. Nested (today): countries are unions of provinces; the economy lives on provinces.
2. Shares: territory moves by cell; the economy stays on provinces and reads the share each
   country holds.
3. Overlay: territory moves by cell; the economy lives on provinces x countries, so a border
   through a province splits its market.
4. Counties as provinces: 53,084 markets, about ten times the 19.2 budget, all state imputed.
5. State per cell: an economy with no data at its grain.

Option 2 gets long partitions wrong (one market under one government for years). Options 4 and
5 need data that does not exist.

## Decision

Options 2 and 3, by speed of change.

- Grid: H3 resolution 6 (about 36 km², roughly 4 million land cells), fixed for a run. Static
  cell fields live here. Resolution 6 is the county sampling grid of ADR 0009, so every cell
  already has a province and a county.
- Partitions: a label per cell. Provinces (ADRs 0006 and 0114), countries, control, admin
  regions, counties and ethnic areas are partitions. Territory changes rewrite labels, never
  geometry.
- Operators: aggregate sums a value from a finer partition to a coarser one, exactly.
  Disaggregate splits a value over a finer partition in proportion to a cell weight field, as
  the pack already does to place national totals in provinces.
- Grain of state: every state variable names its partition. Markets, pops, enterprises and
  capital live on the economic partition, the overlay of provinces and countries: one unit per
  (province, country) pair that shares cells. While countries nest in provinces, as at the
  start, the overlay equals the province set.
- Control (option 2): battles and occupation write the controller label at any tick. Per
  province, the economy reads the weight each side holds and scales taxation, output,
  conscription and grievance by it (11.5). Markets never change on control.
- Sovereignty (option 3): only political events write the owner label: treaty terms (11.8),
  secession and breakaway states declaring (10.8, 11.6), annexation, and interventions (14).
  There is no timer: occupation never splits a market however long it lasts.
- On a sovereignty change: recompute the overlay, then for each unit that changed split stocks
  by cell weights or merge them by summing, then rebuild that unit's market and transport
  edges. Weights per kind of stock: pops by population, and by GeoEPR ethnic area for the
  ethnic dimension; capital by gridded GDP; farms by crop area. Night lights and built-up area
  replace or join these when fetched.
- Slivers: an overlay unit under a few percent of its province's population (a register
  parameter) folds into the province's largest unit of the same country for the economy, and
  keeps its labels for politics and the map.
- Unit IDs: an overlay unit is `<province ID>/<country code>`; the unit holding most of the
  province keeps the bare province ID.
- Transfer units: peace terms and map lines use counties where they exist and clusters of
  cells elsewhere. Counties hold no state.
- Admin reforms (France 2016) are an admin-region relabel for politics and the map, not an
  economic event.
- Data licence: the owner accepts non-commercial use (2026-10-28): Ceteris is open source and
  every user fetches their own data, so GeoEPR (CC BY-NC-SA) may weight splits. Its derived
  tables carry CC BY-NC-SA and attribution, and stay apart from V-Dem-derived tables (BY-SA).
- Build: the pack keeps the cell-to-province and cell-to-county maps at resolution 6 and adds
  one weight field per kind of stock, in `ceteris-pack` with migration Task 3f (ADR 0113).
  Control and the overlay are built with politics and war in P6 and P7.

## Consequences

- No change to the simulation until P6. The pack gains three cell weight fields now, GeoEPR
  later.
- The economic unit count changes during a run, only at sovereignty events; arrays resize and
  snapshots carry the label arrays (about 8 MB per label at resolution 6, as u16).
- Splits are the one point where stocks are imputed below the data grain. A split and reunion
  returns exact totals but not the earlier distribution within the province.
- ADR 0009's counties become display and peace-term units; their share-alike licence touches
  only the map.
- Validation of partial control rests on UCDP GED event locations and hand-coded cases; no
  open global dataset of territorial control is known.
- Open: whether control should also cut transport edges at the front, and the sliver
  threshold's value.
