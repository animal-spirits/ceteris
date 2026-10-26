# 0098. Price levels and source corrections at calibration

Status: proposed
Date: 2026-10-26
Design sections: 3.3, 6.5, 8.6, 15.1, 16; econ SPEC Calibration; amends ADRs 0014, 0089
Phase: P2, before the gate

## Context

ADR 0089 calibrates the 2000 data state with local goods (utilities, construction, services) at
p = 1 in every province. Values are exact, but the unit of real local output is a dollar of
services at market exchange rates everywhere. Services cost two to four times more in rich
than in poor economies (Balassa-Samuelson; ICP). The steady state is unaffected, because PIGL
weights are inverted per country, but three things read real quantities:

- PIGL is non-homothetic in real expenditure, so the Engel response to income growth starts
  from a real income that is understated two to four times in poor countries.
- Migration (6.5) compares real wages across countries. At p = 1 the real-wage gap is
  overstated by the price-level ratio, which pushes migration and its fit residuals up.
- Welfare and real-GDP outputs are not comparable across countries.

Three smaller source issues sit in the same pipeline stage:

- Values convert at 2000 market rates. The dollar was near its 1985-2024 high (euro about
  0.92), which understates non-US economies by roughly 10 to 20% relative to adjacent years.
- The Gini comes from WDI SI.POV.GINI, which mixes income and consumption surveys, so
  dispersion is not comparable across countries.
- Economies outside ICIO take the nearest donor's IO proportions (0014). Donor coefficients do
  not add up to the economy's own sector VA or trade.

## Options

1. Keep p = 1 and market rates; correct real comparisons in reporting only.
2. Seed local-good prices from ICP price levels; average exchange rates over 1999-2001; SWIID
   Gini; RAS-balance donor tables.
3. Endogenous price levels from a Balassa-Samuelson productivity gap.

## Decision

Option 2. Option 3 is what option 2 converges to once A differs by sector; option 2 gives it the
right starting point.

- **Local price levels.** Each country's local goods start at p_c = PLI_c, the ICP price level
  relative to the US (WDI PA.NUS.PPPC.RF over PA.NUS.FCRF, CC BY 4.0, 2000 or the nearest
  ICP-anchored year). Values stay fixed: Y = V / p_c, and A for local sectors absorbs the
  rest, so t = 0 values, shares and the national accounts are unchanged. The world index
  normalization of 0089 is taken over traded goods only.
- **Exchange rates.** Each country block of ICIO is rescaled by its 1999-2001 mean rate over its
  2000 rate before inversion. Within-block ratios are unchanged.
- **Gini.** SWIID (free with citation) replaces WDI as the primary source; WDI and WIID become
  the fallback for countries SWIID lacks. The pack records which source each row used.
- **Donor IO.** The donor's coefficients are RAS-balanced so that row and column totals match
  the economy's WDI sector VA and its BACI export and import totals by good. The sum over
  non-ICIO economies is constrained to the ICIO rest-of-world block.

## Migration

- Pipeline: `econ_countries.arrow` gains `price_level` and `gini_source`; the ICIO reader
  applies the rate rescale; the donor step gains a RAS pass (iterations to 1e-10, at most 200).
- Calibration: local-good base price is p_c, not 1, in `ProvinceData::base_price` and in
  coef = a p_o / q.
- PIGL inversion is unchanged in form; its inputs are real budgets at p_c.

## Tests

- t = 0 nominal values per enterprise are bit-identical with and without price levels.
- The PPP real GDP per capita from the pack matches WDI NY.GDP.PCAP.PP.KD in 2000 within 5%
  for ICIO economies.
- RAS: imputed economies hit their VA and trade totals within 1e-8, and the rest-of-world sum
  holds.

## Consequences

- Real-wage gaps that drive migration shrink by the price-level ratio, which is the largest
  single correction to migration pressure available without new mechanisms (see ADR 0109).
- The Engel path of poor countries changes: services shares rise more slowly with nominal
  income growth.
- Imputed economies lose their exact donor structure. The imputation flag stays.
