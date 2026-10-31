# 0270. Extraction column for crude oil and gas set by rents

Status: accepted (owner, 2026-10-31)
Date: 2026-10-31
Design sections: 16.1, 22.2; amends ADR 0256

## Context

ADR 0256 gives every non-ICIO economy whose good 1 is set by WDI rents the ICIO world column of
good 1. When it was written good 1 was extraction only (B06, B09), with a_VA 0.666. ADR 0092
then moved refining (C19) into good 1, and on the P3 branch the same world column is now a
mix of extraction and refining, with a_VA 0.418. `real_oil_exporters_outside_icio_have_their_oil_and_gas`
(bound a_VA > 0.5) fails.

ICIO 2000 (SML, 2025 edition), world totals:

| Column | Output (bn USD) | a_11 (good 1 into good 1) | Other inputs | a_VA |
|---|---|---|---|---|
| Extraction (B06, B09) | 934 | 0.119 | 0.216 | 0.666 |
| Refining (C19) | 1,107 | 0.602 | 0.189 | 0.209 |
| Merged good 1 | 2,041 | 0.381 | 0.201 | 0.418 |

The merged column is set by refiners: world refining output is 1.19 times extraction output,
weighted toward the USA (1.39), China (2.21), India (3.95), Italy (3.80) and Brazil (5.18).
The high-rent ICIO economies refine little of their crude: Saudi Arabia 0.12, Norway 0.08,
Nigeria 0.02, Angola 0.08, Kazakhstan 0.10. Their own merged good-1 a_VA is 0.81, 0.82, 0.72,
0.65 and 0.39; the median of the five, 0.72, is near the extraction column, not the merged one.

The rents path sets good 1 VA from WDI oil and gas rents, which measure extraction (price less
extraction cost, times production). Pairing extraction VA with the merged column implies each
rent economy refines its crude at the world ratio: output becomes VA / 0.418 = 2.39 VA instead
of VA / 0.666 = 1.50 VA (1.59 times), own use of good 1 0.91 VA instead of 0.18 VA (5 times),
and purchases of other goods 0.48 VA instead of 0.32 VA (1.5 times).

## Options

1. Rederive the bound: keep the merged world column, lower the test bound to suit 0.418.
2. Extraction column: the world column of B06 and B09 only, its rows aggregated to the ten goods
   as usual (C19 sales to extraction fall in row 1). a_VA stays 0.666.
3. A per-economy blend of the extraction and refining columns by own refinery runs. Needs a new
   source (EIA international refinery throughput) for some 40 economies.

## Decision

Option 2. It matches what the rents measure and what high-rent ICIO economies show, needs no
new data or parameter, and is the column the crude good takes when good 1 splits in P8 (ADRs
0028, 0095), so the split does not move it again. Refining in rent economies stays where the
donor's split and WDI manufacturing put it; good 1 is one good, so their refined fuel use is met
by good 1 trade. Derived from data, no register entry (DESIGN 17).

`aggregate` keeps an extraction input column per economy (the B06 and B09 columns, as it keeps
B05 for `coal_share`); `world_oil_gas_column` weights it by extraction output. The test bound
stays a_VA > 0.5.

## Consequences

- Against the P3 branch as it stands, `econ_io` moves for the rent economies outside ICIO (the
  OPEC members in ROW, Sudan, the Caspian and Gulf of Guinea producers), and so do the
  calibration and the `p2-pack` goldens. Against main, nothing moves: a_VA is 0.666 on both.
- Option 1 would have needed no rebuild, but would add about 0.6 VA of good 1 own use per unit of
  rent VA in exactly the economies where good 1 is most of GDP.
- Refining VA in a rent economy whose donor refines is not in good 1 there. Small: C19 a_VA is
  0.209, and rent economies refine little.
