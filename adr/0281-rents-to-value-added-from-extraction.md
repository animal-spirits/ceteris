# 0281. Rents-to-VA factor from extraction value added

Status: proposed
Date: 2026-10-31
Design sections: 16.1, 22.2; amends ADR 0270 (and ADR 0249's rents path)

## Context

ADR 0249 sets crude oil and gas VA of a non-ICIO economy with WDI oil or gas rents to GDP times
the rents times k, k the ICIO oil and gas VA over rents summed over the ICIO economies with a rent
(`oil_gas_va_over_rents`), out of its group and at most the largest ICIO share of crude oil and
gas in the VA of goods 1 to 8 (`oil_gas_most_of_industry`). ADR 0270 then gave that VA the world
extraction column (B06, B09; a_VA 0.666), because the rents measure extraction. Both factors still
read good 1's VA, which holds refining (C19) since ADR 0092 (whole-branch review, pre-P3 B9).

ICIO 2000 (SML, 2025 edition), the ICIO economies with GDP and a rent:

| Sum | Good 1 VA (with C19) | Extraction VA (B06, B09) | Ratio |
|---|---|---|---|
| Over rents (k) | 1.430 | 0.991 | 1.443 |

World totals: extraction VA 621.6 bn USD (0.666 of extraction output 934.0 bn, ADR 0270's
table), good 1 VA 852.7 bn (0.418 of 2,041 bn). Among the high-rent ICIO economies extraction is
0.72 (Russia) to 0.99 (Norway, Nigeria, Angola) of good 1's VA. The cap moves from 0.7554 to
0.7231 (Brunei in both).

So the rents path takes 1.443 times the extraction VA the rents measure, and produces it with a
column whose a_VA is extraction's: the imputed economies get the refining VA of the ICIO rent
economies (Russia's above all) on top of their crude, at extraction's output per unit of VA.

## Options

1. **Extraction VA in both factors** (recommended): k = sum of (extraction output less extraction
   inputs) over rents; the cap is extraction VA over the VA of goods 1 to 8. Both ICIO totals are
   already aggregated (`CountryIo::extraction`, `extraction_output`, ADR 0270).
2. **Keep good 1's VA** and record it in ADR 0270: the imputed rent economies hold some refining
   in good 1 at the world extraction column. No rebuild.

## Decision

Option 1, pending the owner (recommended by the B9 review triage). The rents, the column and the
VA factor then measure the same activity, extraction; refining in a rent economy stays where its
donor's split and WDI manufacturing put it, as ADR 0270 says. Derived from data, no new
parameter, no register entry (DESIGN 17).

`icio::extraction_va` is a country's B06 and B09 output less their inputs, as `CountryIo::va` is
for a good; `oil_gas_va_over_rents` and `oil_gas_most_of_industry` use it.

Tests: `oil_gas_va_over_rents_takes_extraction_value_added` and
`oil_gas_most_of_industry_takes_extraction_value_added` (synthetic ICIO with refining in good 1);
`real_oil_exporters_outside_icio_have_their_oil_and_gas` bounds k by the extraction column's a_VA
below (a rent, price less unit cost times output, is at most the output's value) and by the same
sum over good 1's VA above (refining VA is not negative), in place of 0.5 to 3.

## Consequences

- `econ_countries` and `econ_provinces` (VA columns) move for the rent economies outside ICIO;
  `econ_io`, the other econ tables and the P1 tables do not. The calibration follows. The fixture
  is synthetic and does not move.
- The installed pack stays as it is until the owner rules; the measurements below are on a
  scratch copy.
