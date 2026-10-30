# 0256. World technology for crude oil and gas set by rents

Status: proposed
Date: 2026-10-30
Design sections: 16.1, 22.2; amends ADR 0249

## Context

ADR 0249 sets the crude oil and gas VA of economies outside ICIO from WDI rents, but its input
coefficients, and so its value-added share a_VA, still come from the donor. A donor with next to
no crude sector gives a technology no oil producer has: Sudan takes Senegal's a_VA of 0.072, so
Unity province sells about Sudan's whole pop money each tick; COG, GNQ and GAB take South
Africa's 0.184, TKM, AZE and UZB Ukraine's 0.34. A fresh build of main fails V1 on the split pack
(seed 20, Tirane, 1.879e-2) and Sudan drifts at seed 21.

## Options

1. Keep the donor's column (as built).
2. The median column of a named set of high-rent ICIO economies (NOR, SAU, BRN, KAZ, AGO).
3. The ICIO world column: inputs per unit of crude oil and gas output summed over all ICIO
   economies, weighted by their crude oil and gas output.

## Decision

Option 3. It needs no list or threshold, it is the technology of the bulk of 2000 world output,
and the economies that set it are the large producers option 2 would name. Where ADR 0249's
rents path sets good 1 (GDP and a rent), its input column is the world column and its output is
VA / (1 - the column's sum), so the table still closes. Other goods and economies without a rent
keep the donor's column. Derived from data, no register entry (DESIGN 17).

## Consequences

- `econ_io` moves for every non-ICIO economy with a rent, and so do the calibration and the
  `p2-pack` goldens; `econ_countries` and `econ_provinces` hold VA and do not move. World column
  a_VA is 0.666 on 2000 data; Sudan's a_VA goes from 0.072 to it, COG, GNQ and GAB from 0.184,
  TKM, AZE and UZB from 0.340.
- Sudan no longer drifts in V1 (seeds 21, 22: price gap 1.02 before, below 1e-4 after). Unity
  (province 3474) does not oscillate on a pre-0249 pack or on this one.
