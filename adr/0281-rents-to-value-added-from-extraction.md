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

## As built (pre-P3 B9, 2026-10-31; scratch pack, not installed)

- **Code** (198c0c8): `icio::extraction_va`; no golden moves (the installed pack is unchanged
  and the fixture is synthetic). A copy of the installed pack rebuilt `--econ-only` at HEAD
  reproduces every econ table bit for bit; at 198c0c8 only `econ_countries` (sha256 0395240b...,
  was 1ba6b89c...) and `econ_provinces` (65fca14a..., was a55e0b9b...) move, and in them only
  the VA columns (largest relative move 0.31 in good 1, 0.93 in goods 3 to 6 of an economy whose
  crude oil and gas came out of a merged industry group). Good-1 VA source shares: gem 81.4%
  (82.3%), eia 14.7% (13.9%), population 3.9% (3.8%).
- **Calibration** (with the B9 Sinkhorn pin): nu 2.60516 (class 0), 0.80014 (class 1), 0.39731
  (class 2), 0.046059 (good 1, was 0.065907) and 0 (good 2, clamped: share 0.2260 against its
  target 0.2583); 18 evaluations; 70 passes, last change 8.39e-14, Sinkhorn residual 8.70e-14,
  1,420 s, 6.0 GB. Table sha256: econ_calib 9d89cc86..., cells 4b165022..., countries
  2ea64c59..., pipeline 8baed99b..., pops 077a9e5f..., provinces 54b36027....
- **Gate** on that pack: V1 from the perturbed steady state passes for seeds 20, 21 and 22
  (price 2.08e-4, 1.58e-4, 1.29e-4, all province 2699 good 9; were 1.94e-4, 1.54e-4,
  1.25e-4). V1 from the data state still fails (0.751 at 1,200 ticks, province 3903 good 9;
  0.762 before). H2 distance -0.4554 (s.e. 0.0104; -0.4539 before), reported. H2 per class
  passes (theta within 2.1e-8 relative); by good of class 3, goods 1 and 2 recover 16.91 within
  3.8e-9; with ln distance as the control class 0 2.429, 1 3.537, 2 7.647, 3 16.326, good 1
  16.461, good 2 15.397.
- **Miss**: `pack_calibrated_state_holds_to_2e_6_and_4_minor_units` fails its boundedness check
  over its 120 ticks: the price drift of cells of province 2464 peaks at 9.8e-2 of the tolerance
  in ticks 61 to 120 against 4.2e-2 in ticks 5 to 60 (bound: twice). Run 480 ticks, it is a
  bounded slow oscillation (peaks near ticks 100 and 282, 9.8e-2 in ticks 5 to 240, 1.02e-1 in
  241 to 480) and passes; every quantity stays below 0.46 of its tolerance. On the pin-only
  recalibration of the installed pack the 120-tick test passes.
