# 0104. Sector labour shares and the distribution of capital income

Status: proposed
Date: 2026-10-26
Design sections: 6.1, 7.2, 8.7, 9.1; econ SPEC Income; amends ADR 0014

## Context

- **One labour share per country.** Every sector uses the national PWT labour share s_L.
  Sector shares differ widely and stably: mining, oil and gas and real estate near 0.2 to 0.3,
  most services near 0.6 to 0.7, agriculture depending on self-employment. A single share
  overstates the wage bill of resource sectors and understates their fixed-factor rents, so
  the income of resource exporters and the transmission of commodity shocks to workers are
  wrong.
- **Profits paid equally per person, nationally.** Each pop gets its population share of the
  country's profit pool (owner decision 2026-10-25). Capital income is the most unequal income
  and is partly foreign-owned (FDI, resource concessions), so this compresses inequality and
  keeps all rents at home.

## Options

1. Keep both.
2. Sector relative labour shares from WIOD SEA rescaled to the PWT national value; dividends by
   wealth decile and a fixed foreign-owned share.
3. Country-by-sector shares from national accounts and a full ownership network (P5, ADR
   0041).

## Decision

Option 2. Option 3 follows with 0041 and P5.

- **Labour shares.** s_L,c,g = s_L,c * r_g,c / (sum_h VA_c,h r_h,c / VA_c), with r the sector
  relative labour share from WIOD SEA 2000 (or EU KLEMS) for the country, pooled by income
  group where the country is missing. The VA-weighted mean equals PWT exactly.
- **Domestic dividends.** The profit pool is paid by wealth share: pops' income lognormals are
  mapped to WID wealth shares by decile (top 10%, middle 40%, bottom 50%, national), so a pop's
  dividend share is the integral of the wealth share over its income distribution. Countries
  without WID wealth series use their region's.
- **Foreign ownership.** A fixed share f_c,g of each country's profit pool in mining, oil and
  gas and manufacturing is paid abroad, distributed by IMF CDIS inward FDI positions by source
  country (2009, the first CDIS year, as the closest to 2000); f from CDIS stock over the
  sector's capital stock, capped at 0.8. The recipients pay it to their pools.

## Migration

- Pack: `econ_countries.arrow` labour share becomes a vector of 10; new tables of WID wealth
  shares and CDIS positions.
- Econ: `s_L` per enterprise from the sector table; the income step routes dividends by wealth
  share and the foreign share through a cross-border income account (the transfer account of
  ADR 0100).

## Tests

- National VA-weighted labour share equals PWT within 1e-12.
- Sum of dividends paid equals the profit pool each tick (ledger exact).
- Model top-10% income share at t = 0 against WID within 5 points for ICIO economies.

## Consequences

- Rents and their shocks concentrate where they are: resource-sector owners, top deciles and
  foreign investors.
- t = 0 wages per worker change by sector mix; A absorbs it at calibration.
- CDIS is post-2000; its use for 2000 is an approximation recorded in the pack.
