# 0249. Partial WDI sector shares and oil and gas from rents

Status: accepted (owner, 2026-10-30)
Date: 2026-10-30
Design sections: 16.1, 22.2; amends ADR 0014

## Context

ADR 0014 sets the VA of economies outside ICIO from WDI GDP and sector shares, split within each
group by the donor's proportions. Two faults showed in the pre-P3 class-3 diagnosis:

- With one of agriculture, industry or services missing, VA was GDP times the shares there are,
  so the missing groups' VA vanished. Nine economies: KWT (VA/GDP 0.004, no industry or
  services), BMU 0.12, TUV 0.23, ERI 0.32, KEN 0.44, LBY 0.71, PNG 0.73 (no services), MDG 0.79
  (no industry), MAC 0.92 (no agriculture).
- The donor's proportions place crude oil and gas wherever the donor has it. LBY (donor MLT)
  had 0.08 bn USD against about 14 bn of 2000 crude output, DZA (EGY) 5.3 against 20, VEN (PER)
  3.3 in crude oil and gas with 13.6 in coal and mining, KWT 0.04 against 21.

## Options

Level with a share missing: (1) GDP times the shares there are (as built); (2) the ICIO median
VA over GDP, already used with no share, the missing groups taking what the known shares leave.

Crude oil and gas outside ICIO: (1) the donor's split (as built); (2) donors among ICIO fuel
exporters only; (3) WDI oil and gas rents times the ICIO ratio of crude oil and gas VA to rents.

## Decision

- Level: option 2. The known shares keep their goods; the goods they do not cover take GDP times
  the ICIO median VA over GDP less the known shares, at least zero, by the donor's proportions.
  Manufacturing without industry covers goods 3 to 6. All three shares present: unchanged.
- Crude oil and gas: option 3 (owner, 2026-10-30). Where WDI has an oil or natural gas rent
  (NY.GDP.PETR.RT.ZS, NY.GDP.NGAS.RT.ZS; a missing one counts as zero), good 1 takes GDP times
  the rents times k, out of the group that holds it and at most c of it; the rest of the group
  keeps the donor's proportions. k is the sum of ICIO crude oil and gas VA over the sum of GDP
  times rents of the ICIO economies with a rent (0.99 on 2000 data). c is the largest share of
  crude oil and gas in the VA of goods 1 to 8 among the ICIO economies, so electricity and
  construction, local goods, keep a producer (TKM's rents exceed all of its industry). k and c
  are derived from data, not tuned, so no register entry (DESIGN 17).
- Where the rents' VA exceeds c of industry less manufacturing and WDI gives manufacturing
  apart, industry counts as one group (goods 1 to 8): WDI 2000 then counts hydrocarbons in
  manufacturing (DZA 45% of GDP in manufacturing, 8% in the rest of industry). Without this
  and c, DZA, ECU, IRN and TKM had no electricity or construction and the calibration found
  local demand without a producer.
- A good the donor lacks gets output equal to its VA (the donor's input column is zero).

## Consequences

- Country rows move for the nine partial economies and every non-ICIO economy with a rent;
  `econ_countries`, `econ_io` (output), `econ_provinces` and the calibration move, and so do the
  `p2-pack` goldens. Class 3 (fuels) cross-border share moves by about +0.015 (diagnosis).
- Two WDI series join the fetch list; their licence follows the WDI source entry.
- The basis column is unchanged (`wdi`, `wdi_gdp`); whether good 1 came from rents is read from
  the rents series, not stored.
