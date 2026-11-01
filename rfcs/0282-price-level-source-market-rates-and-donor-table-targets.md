# RFC 0282. Price level source, market rates and donor table targets (amends ADR 0098)

Status: proposed
Date: 2026-11-01
Design sections: 16, 16.1; econ SPEC (Pack additions, Economies outside ICIO, Local base price, Open questions)
Research: none separate; the measurements are the C1 and C1c pack builds quoted below.

## Summary

ADR 0098 seeds local-good prices from ICP price levels, rescales ICIO blocks to 1999-2001
exchange rates and RAS-balances donor tables. Building it found that its price level series is
misread and carries unit errors, that the rate rescale breaks at currency changeovers, and that
its RAS row targets need final demand, which the ADR does not give. This RFC fixes the source of
p_c and the screen on it (D1), drops the rescale (D2), sets the RAS row targets (D3) and asks
what the targets do where they come out negative (D6), plus two follow-ups of the p_c build:
which goods weight the run's world price index (D4) and which WDI series ADR 0098's real GDP
check reads (D5). The PWT income screen of C1 (`PWT_BELOW_WDI`, Venezuela) is a data screen of
the same pipeline stage: like the pack's other screens (WDI tariff rates at or above 100% count as
missing, `MAX_RATE`) it is not a parameter of Section 17, which registers what data does not pin,
so it takes no code.

## Decisions

### D1. Where does each country's price level p_c come from?

Found (C1, 2026-11-01): WDI PA.NUS.PPPC.RF is already the ratio of the PPP conversion factor to
the market rate, so ADR 0098's PPPC.RF / FCRF is a units error, and the series is not based on
the US (the US is 122.1 in 2000). Its 2000 values carry currency-unit errors (Turkey 3,712 times
the US, Azerbaijan 10.6, East Timor 0.003, the Palestinian territories 0.004, DR Congo 0.008), and
33 of the economies it shares with PWT 11 differ from PWT's pl_gdpo by more than 1.5 times. PWT 11
pl_gdpo over the US's covers 182 economies with one such value (Bermuda, 1,059).

Options:

- A. WDI PA.NUS.PPPC.RF over the US's, screened against PWT: keeps ADR 0098's source; the
  screen has to name a tolerance against PWT.
- B. PWT 11 pl_gdpo over the US's in 2000 first, WDI PA.NUS.PPPC.RF over the US's (same year)
  where PWT has none or its value fails the screen, the imputation donor's level last, the source
  flagged; an outlier screen on both.
- C. Balassa-Samuelson: p_c from a regression of price levels on income, for every country.

Recommendation: B, because PWT's levels are the cleaner series of the two and are already the
income source of the donor ranking, WDI fills its gaps, and C smooths away real variation.

Decision: B, Antonio, 2026-11-01.
Row: ADR 0282

As built. Levels are each economy's value of the year nearest 2000 within five over the US's of
the same year (PWT has 2000 for all but Curacao and Sint Maarten, 2005; South Sudan's first year,
2008, is outside the window), so PWT gives 184 levels. The outlier screen is Tukey's (1977)
far-out fences, 3 interquartile ranges beyond the quartiles (`price_level::FAR_OUT`, quartiles by
linear interpolation), applied to the residuals of the Penn effect: ln p = a + b ln y, y WDI GDP
per person at market rates (the donor ranking's income), fitted by OLS on the PWT levels within
the far-out fences of their own logs (0.0262 to 8.32 times the US, which leaves out Bermuda's
1,059). The fit over 179 economies is ln p = -2.7317 + 0.2517 ln y (R2 0.610), its residual
fences -1.198 to 1.169. A PWT or WDI value is kept when its residual lies within them; an economy
with no WDI income (Anguilla, Montserrat, Taiwan, the British Virgin Islands) is held to the
level fences. No threshold is set by hand: 3 IQR is Tukey's standard far-out multiple and every
fence follows from the PWT data.

Why the residual and not the level: the level fences alone keep 23 of the 25 WDI values of
economies without PWT, among them the US Virgin Islands at 0.117 (on the dollar, income 30,062
USD), Andorra 0.080, Greenland 0.079, French Polynesia 0.029 and New Caledonia 0.027, unit errors
that sit inside them because the poorest PWT economies are near 0.12 (Tajikistan). Tukey's inner
level fences (1.5 IQR, 0.090 to 2.42) still keep the US Virgin Islands. The residual screen
rejects on PWT Bermuda, Somalia (1.09, residual 1.319) and the Turks and Caicos (2.50, 1.178),
and on WDI Afghanistan (0.025), Andorra, Cuba (0.056), Greenland, New Caledonia, Nauru (0.056),
French Polynesia, East Timor (0.003) and the US Virgin Islands; Turkey, Azerbaijan, the
Palestinian territories and DR Congo have PWT values that pass. A country of several economies
takes the mean of their levels and incomes. The donor is the imputation donor of ADR 0014 (for a
territory inside a parent, the parent's donor or the parent), whose own level passes the same
rules. Local goods take the GDP price level, as ADR 0098 says. Coverage of the C1c build: of the
242 pack countries, 179 take PWT, 17 WDI and 46 the donor (`price_level_source`;
`price_level_imputed` on the 63 that are not PWT's); p_c runs from 0.095 (Somalia, WDI, its PWT
1.09 rejected) to 1.42 (Japan), GDP-weighted mean 0.897; only `econ_countries.arrow` moves. The
screen's form, a residual fence rather than a level fence, was set in the build, not in the
ruling; it uses the Penn effect only to reject values, never to set one (option C).

### D2. Rescale ICIO blocks to 1999-2001 exchange rates?

Found (C1): the 1999-2001 mean of USD per unit of local currency over 2000's has median 1.005
over 205 economies (Germany 1.041, Japan 0.944), not the 10 to 20% ADR 0098 expects, and breaks
at changeovers and high inflation: Greece 109.8 (euro from 2001), El Salvador 3.59 (dollar from
2001), Ecuador 0.667 (dollar in 2000); the Atlas factor has the same breaks (Greece 337).

Options:

- A. Drop the rescale: values stay at 2000 market rates.
- B. Rescale with screens for changeovers and high inflation.

Recommendation: A, because the correction it was for is half a percent at the median and the
screens would remove most of what remains.

Decision: A, Antonio, 2026-11-01.
Row: ADR 0283

### D3. Which row totals does the RAS of a donor table meet?

Found (C1): the column totals of the donor's intermediate table follow from VA alone (output is
VA times the donor's output per VA), so only the rows bind, and they need final demand, which
ADR 0098 does not give. The rest-of-world constraint needs the ICIO ROW block (C1 thought the
reader did not keep it; `read::icio::aggregate` keeps it under the code ROW).

Options:

- A. Rows X_k + M_k - E_k - F_k: BACI 2000 imports M and exports E by good, F the donor's final
  demand shares of VA + M - E; the ICIO ROW block a reported check, not a constraint; tolerance
  1e-10, at most 200 iterations.
- B. Columns only (no row targets).

Recommendation: A.

Decision: A, Antonio, 2026-11-01.
Row: held for D6 (the rule cannot be built as it stands).

As measured (C1c, 2026-11-01, 133 imputed economies with WDI GDP or a donor level and BACI
trade): 122 have at least one negative row target, 321 rows in all (construction 70, food 66,
light manufacturing 37, crude oil, gas and refined fuels 34), and RAS cannot reach a negative
total from a non-negative seed. No positive target meets an empty seed row. The two causes: the
donor's final demand shares do not fit the economy's own VA mix (construction is mostly gross
fixed capital formation, and the donor's share of it exceeds the construction output that WDI's
industry share leaves), and BACI exports exceed output plus imports (57 rows in 47 economies,
such as Algeria's crude oil, 23,325 against 21,680 plus 95 million USD, or Aruba's refined fuels
with no output of its own). 14 imputed economies are not in BACI (Puerto Rico, Kosovo, the Faroes,
Jersey and others). The ROW check: the imputed economies' VA sums to 1,012,330 million USD against
ICIO ROW's 1,019,231 (0.68% below; by good from 0.67 of ROW for machinery to 1.33 for coal and
other mining).

### D4. Which goods weight the run's world price index?

The run's world price index (`EconWorld::world_price_index`, the inflation that expectations
read, ADR 0019) weights buyer prices by base consumption over all goods; with p_c, local goods
enter at p_c.

Options:

- A. All goods, local goods included: the index is a world CPI over all consumption.
- B. Traded goods only, as the calibration's normalization (ADR 0098).

Recommendation: A, because expected inflation should follow the prices households pay, most of
which are local services; the calibration's normalization over traded goods is a choice of
numeraire, not of the inflation measure.

Decision:
Row:

### D5. Which WDI series does ADR 0098's real GDP check read?

ADR 0098 tests the pack's PPP real GDP per person against WDI NY.GDP.PCAP.PP.KD within 5%. That
series is in constant international dollars of the current ICP base year, not of 2000.

Options:

- A. NY.GDP.PCAP.PP.CD, current international dollars: the 2000 value is in 2000 prices, as the
  pack's VA over p_c is.
- B. Keep PP.KD and deflate it to 2000.

Recommendation: A, because it compares like with like without a deflator.

Decision:
Row:

### D6. What do the RAS targets do where they are negative?

Raised by the D3 measurement above.

Options:

- A. Balance final demand with the inputs: RAS the use table [Z | F] of each imputed economy to
  rows X_k + M_k - E_k (domestic use of each good) and columns X_g - VA_g and total final demand
  VA + M - E, the donor's tables as the seed; an economy with any X_k + M_k - E_k < 0 (47) or
  outside BACI (14) keeps the donor's table, flagged. Final demand shares become the economy's.
- B. KRAS (Lenzen, Gallego and Wood 2009): balance to all targets with reliability weights, so
  conflicting targets (exports above output) are met as closely as their uncertainties allow;
  covers every economy but needs a reliability per source.
- C. No RAS: every imputed economy keeps the donor's coefficients (ADR 0014 as built), and the
  ROW check is reported.

Recommendation: A, because it is the standard use-table balance (Miller and Blair 2009, ch. 7)
and needs no new parameters; B can follow if the 47 flagged economies matter.

Decision:
Row:

## Spec changes

- 16: the PWT row in the source table names the GDP price level (ADR 0282).
- 16.1: price levels from PWT, WDI and the donor with the far-out screen (ADR 0282); values at
  2000 market rates (ADR 0283).
- econ SPEC (module spec): `econ_countries.arrow` price level columns, Local base price, Open
  questions (ADRs 0282 and 0283; D3 to D6 open).
