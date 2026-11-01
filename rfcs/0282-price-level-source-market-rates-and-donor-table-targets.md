# RFC 0282. Price level source, market rates and donor table targets (amends ADR 0098)

Status: proposed
Date: 2026-11-01
Design sections: 12.2, 16, 16.1; econ SPEC (Pack additions, Economies outside ICIO, Local base price, Open questions)
Research: none separate; the measurements are the C1 and C1c pack builds quoted below.

## Summary

ADR 0098 seeds local-good prices from ICP price levels, rescales ICIO blocks to 1999-2001
exchange rates and RAS-balances donor tables. Building it found that its price level series is
misread and carries unit errors, that the rate rescale breaks at currency changeovers, and that
its RAS row targets need final demand, which the ADR does not give. This RFC fixes the source of
p_c and the screen on it (D1), drops the rescale (D2), sets the RAS row targets (D3) and asks
what the targets do where they come out negative (D6), plus two follow-ups of the p_c build:
which goods weight the run's world price index (D4) and which WDI series ADR 0098's real GDP
check reads (D5), and, from that check, which PWT price level D1 reads (D7). All seven are ruled;
D6 supersedes D3, and D7 amends D1's series. The PWT income screen of C1 (`PWT_BELOW_WDI`, Venezuela) is a data screen of
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

Decision: A, Antonio, 2026-11-01; superseded by D6 (A, Antonio, 2026-11-01), whose row
targets replace these.
Row: none (D6 is ADR 0286).

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

Decision: A, Antonio, 2026-11-01.
Row: ADR 0284

As built: no code change. `EconWorld::world_price_index` already weights buyer prices by base
consumption over all goods, local goods at p_c.

### D5. Which WDI series does ADR 0098's real GDP check read?

ADR 0098 tests the pack's PPP real GDP per person against WDI NY.GDP.PCAP.PP.KD within 5%. That
series is in constant international dollars of the current ICP base year, not of 2000.

Options:

- A. NY.GDP.PCAP.PP.CD, current international dollars: the 2000 value is in 2000 prices, as the
  pack's VA over p_c is.
- B. Keep PP.KD and deflate it to 2000.

Recommendation: A, because it compares like with like without a deflator.

Decision: A, Antonio, 2026-11-01.
Row: ADR 0285

As built (C1d, 2026-11-01): a test, not a build step (`econ_build.rs`
`real_gdp_per_person_against_wdi_ppp`; the series is pinned in `validation.toml`, source
`wdi-ppp`, CC BY 4.0 in the API metadata). Over the 79 ICIO economies with WDI population (Taiwan
has none), the pack's GDP per person over p_c is within 5% of PP.CD for 32. The ratio has median
0.954 and runs from 0.216 (Nigeria) and 0.342 (Angola) to 1.41 (Belarus) and 1.85 (DR Congo); the
OLS slope of its log on ln income is 0.0487, the correlation 0.259, so the misses are not a
Penn-effect tilt. The largest are oil exporters (Saudi Arabia 0.432, Brunei 0.576, Kazakhstan
0.586, the Emirates 0.780): PWT's pl_gdpo is the output-side level, which carries export prices,
while PP.CD is expenditure-side. ADR 0098's 5% therefore fails for 47 of 79; the test pins the
measured values and does not gate on 5%, and the choice of what to do about it is open.

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

Decision: A, Antonio, 2026-11-01.
Row: ADR 0286

As built (C1d, 2026-11-01). Domestic use is X_k + M_k - E_k: output, because ICIO's uses count
every source economy, plus BACI imports less BACI exports; local goods have no BACI trade, so
their use is their output. Of the 160 imputed economies, 87 are balanced (Somaliland takes
Somalia's balanced table, so 88 rows carry `io_basis` `ras`), 48 keep the donor's table for a
negative domestic use and 15 for no BACI trade (47 and 14 in C1c's count, which left out the
WorldPop populations), 9 have no VA. A parent is balanced before its territories take their share.
RAS takes 10 to 56 passes (median 19; Guam 56, the Northern Marianas 55); the worst row residual
is 1.0e-10 (Wallis and Futuna), the worst column residual 4.9e-16. Only `econ_countries.arrow`
(final demand shares and the new `io_basis` column) and `econ_io.arrow` move: 8,320 coefficients
in 88 economies, mean change 0.019. The largest: Niue chemicals into food 0.054 to 0.493, Libya
utilities into chemicals 0.093 to 0.506, Guam and the Northern Marianas mining into construction
0.360 to 0.733; Iraq's final demand share of construction 0.085 to 0.471 and of services 0.459 to
0.107. ROW check: the imputed economies' VA is 1,014,073 million USD against ICIO ROW's 1,019,231
(0.9949), by good 0.674 (machinery) to 1.330 (coal and other mining).

### D7. Which PWT price level does D1 read?

Raised by the D5 measurement: 32 of 79 within 5%, the oil exporters worst, read then as pl_gdpo
carrying export prices.

Options as first put: A pl_gdpe, the expenditure-side GDP price level; B pl_con; C pl_gdpo, as
built; D the price level of non-traded construction; E WDI PA.NUS.PPPC.RF first; F a
Balassa-Samuelson fit; G widen the check.

First decision: A, Antonio, 2026-11-01 (20:34Z). Void: PWT 11 has no pl_gdpe. Its price levels
are pl_con, pl_da, pl_gdpo, pl_c, pl_i, pl_g, pl_x, pl_m, pl_n and pl_k.

Found before the second ask (C1e, offline, the D1 screen and the D5 check rerun on each series):
the pack's `gdp` of the 79 ICIO economies equals WDI NY.GDP.MKTP.CD 2000 (to 1e-9 for all 79), so
the D5 ratio is the PP.CD-implied price level (WDI GDP per person over PP.CD) over p_c. Its misses
are differences between PWT's and WDI's PPPs, not export prices: the premise of the first ask was
wrong. WDI's own PA.NUS.PPPC.RF over the US's is within 5% of the PP.CD-implied level for only 52
of the 79. Within 5% by series: pl_da 34, GDP over cgdpe (derived) 34, pl_gdpo 32, pl_con 29.

Options as re-asked:

- A. PWT 11 pl_da, the price level of domestic absorption (C + I + G), over the US's in 2000, in
  place of pl_gdpo; ADR 0098's 5% check reported, not enforced.
- B. Keep pl_gdpo (D7 moot).
- C. GDP at market rates over PWT cgdpe, a derived expenditure-side level.
- D. The PP.CD-implied level first: circular, since it is what the check compares against.

Recommendation: A, because domestic absorption is the expenditure side without net exports, the
closest PWT series to what households, firms and the state pay for local goods, and it does as well
as any series on the check.

Decision: A, Antonio, 2026-11-01 (20:45Z).
Row: ADR 0287

Why the 5% is not enforced: with the pack's GDP equal to WDI's, the check measures PWT's price
level against the one implied by WDI's PPP GDP, two ICP vintages of the same quantity. ADR 0098
gives no derivation of a tolerance between them, and WDI's own price level series misses its
PP.CD-implied level by more than 5% for 27 of 79, so 5% is not a bound any source meets.

As built (C1e, 2026-11-01). The PWT reader takes pl_da; nothing else changes. PWT gives 184
levels (as before). Level fences 0.0250 to 8.84 times the US; the Penn effect over 180 economies
is ln p = -2.6571 + 0.2453 ln y (R2 0.611), residual fences -1.198 to 1.188. Rejected on PWT:
Somalia only (1.04, residual 1.233); Bermuda (1.80) and the Turks and Caicos (1.21) now pass.
Rejected on WDI, among economies without PWT: Afghanistan, Andorra, Cuba, Greenland, New
Caledonia, Nauru, French Polynesia, East Timor, the US Virgin Islands and now Kosovo (0.149).
Coverage of the 242 pack countries: PWT 181, WDI 15, donor 46 (61 flagged); Bermuda moves from
WDI to PWT, the Turks and Caicos from the donor to PWT, Kosovo from WDI to the donor. p_c runs from
0.095 (Somalia, WDI) to 1.80 (Bermuda), GDP-weighted mean 0.911 (0.897 with pl_gdpo); 226 of 242
move, median ratio to the pl_gdpo build 1.012. Largest moves: Kosovo 0.149 to 0.422 (donor),
the Turks and Caicos 0.588 to 1.209, Bermuda 0.956 to 1.800, Lesotho 0.240 to 0.341, Congo 0.669
to 0.929, Nigeria 0.807 to 1.059, Norway 0.904 to 1.128, Qatar 0.610 to 0.760, Lebanon 1.124 to
0.917. Only `econ_countries.arrow` moves, and in it only `price_level`, `price_level_source` and
`price_level_imputed`. The D5 check: 34 of 79 within 5% (50 within 10%), median 0.950, from 0.165
(Nigeria) and 0.364 (Angola) to 1.26 (Belarus) and 1.88 (DR Congo); slope 0.0509, correlation
0.260; pinned as measured.

## Spec changes

- 12.2: the inflation expectations read is that of a world price index over all goods, local
  goods at p_c (ADR 0284).
- 16: the PWT row in the source table names the price level, pl_da (ADRs 0282, 0287).
- 16.1: price levels from PWT, WDI and the donor with the far-out screen (ADR 0282); values at
  2000 market rates (ADR 0283); donor tables balanced by RAS to domestic use (ADR 0286); ADR
  0098's real GDP check reads NY.GDP.PCAP.PP.CD (ADR 0285); PWT pl_da, and the check
  reported, not enforced (ADR 0287).
- econ SPEC (module spec): `econ_countries.arrow` price level columns, Local base price, Open
  questions (ADRs 0282 and 0283); Economies outside ICIO, the RAS balance and the
  `io_basis` column (ADR 0286); the real GDP check (ADR 0285);
  PWT pl_da in Price levels and the check reported only (ADR 0287).
