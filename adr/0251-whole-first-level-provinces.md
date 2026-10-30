# 0251. Whole first-level provinces

Status: accepted (owner, 2026-10-30); amends ADR 0006 and ADR 0009 (drops the area split)
Date: 2026-10-30
Design sections: 2, 5.2, 5.3, 22.2, ADR 0006, ADR 0009

## Context

DESIGN 5.2 split every Natural Earth admin-1 region over 250,000 km² into
ceil(area / 250,000) parts: along county borders where geoBoundaries has enough counties
(82 regions, ADR 0009), by Voronoi cells otherwise (11 regions, ADR 0006). The 93 regions
became 295 parts, for 4,796 provinces from 4,594 regions.

On the app's map the owner saw Minas Gerais and Rio Grande do Sul cut in pieces. The cut
lines follow no administrative border anyone knows: county groups chosen to balance area,
or straight Voronoi edges. The economic data is first-level (DOSE, census tables), so the
parts of a split region only share its values by a weight; the split added provinces without
adding information. ADR 0115 and ADR 0190 already give sub-province territory its own
mechanism (cell labels, in-run redraws), so the pack does not need fixed area splits for it.

## Options

1. Keep the split.
2. Keep it but raise the threshold (for example to 1,000,000 km²), splitting only the
   largest regions (Sakha, Western Australia, Amazonas, the Canadian territories).
3. Drop the split: one province per admin-1 region at 2000 borders.

## Decision

Option 3 (owner, 2026-10-30, on the card "Undo splits in the pack").

- The pack build keeps every admin-1 region whole. `split_area_km2` leaves `pack.toml`, and
  the county and Voronoi split code leaves `ceteris-pack`. Province ids are Natural Earth
  `adm1_code` with no `.k` suffix.
- Counties stay a display layer clipped to the province they overlap most (ADR 0009). The
  county-misfit statistic now covers every assigned county, since no county is cut by a
  split any more.
- The DOSE matching in `econ::downscale` still groups provinces by base region; with whole
  provinces each group has one member.

## Consequences

- 4,594 provinces. The largest provinces return to their admin-1 size (the largest is
  3,053,172 km², against 402,129 with the split). Each has one transport anchor, so
  trade inside such a region is not priced; domestic trade between provinces still is.
- Every province index changes, so the pack tables, the econ tables, the calibration, the P2
  gate and every golden hash that reads the pack move together in one rebuild
  (`### BEHAVIOUR` entry in the CHANGELOG).
- The 22.2 default "province-splitting rule" is resolved as no split. Population-based
  splitting or merging can return as a new ADR, through ADR 0190's redraws if it is in-run.
- Province counts in DESIGN 1, 5.2, 5.3, 6.5, 19.2, 21 and 22.1 and in the geo, econ and trade
  specs read 4,594. The rebuilt graph has 40,187 nodes and 133,907 edges (40,534 and
  134,810); gateways stay 600 (286 ports, 171 airports, 143 land crossings).
- P2 gate on the rebuilt pack (main with ADRs 0249, 0256 and 0263; 41 passes): H2 distance
  -0.745, theta by class 1.66, 3.50, 9.17, 100 years without violation, 1 vs 8 threads
  identical. V1 from the perturbed steady state misses in Albania (flows 2.7e-2 at seed 20),
  as main's split pack does (3.1e-2), so the miss is not this ADR's. The pack steady-state test
  misses on Pitcairn's one pop (money 1.49 times its 2e-6 tolerance at tick 10): its data is
  unchanged, the rebuilt network moves its import prices 1-4% and so its calibrated point, and
  the tolerance is relative to a balance of a third of a tick's income.
- Tick median 42 ms against 46 ms on the split pack (M4 Pro).
- The gravity fit (`ceteris gravity --pack`) gives no estimates on either pack: on the split
  pack the P04 stop rule fires (air ratio -0.70), on the whole pack the mode costs do not
  converge in 50 steps. `gravity_2000.toml` is not written and nothing reads it yet, so this
  ADR does not change trade parameters.
