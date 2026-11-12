# Why PWT and WDI disagree on 2000 price levels

Date: 2026-11-01. Feeds: ADR 0098's real GDP check (ADR 0285), RFC 0282 D1 and D7.
Data: PWT 11.0 (`data/cache/pwt110.xlsx`) and WDI (`data/cache/wdi_*.zip`, release
2026-10-08) from Antonio's Mac; ICIO economies are the 80 with `va_basis = icio` in the
installed 2000 pack (79 with WDI population; Taiwan has none). Scripts are in this thread's
scratchpad and are not in the repository.

## Summary

1. **The check measures source disagreement, nothing in the pack.** The pack's GDP is WDI's
   GDP, so pack real GDP over WDI PP.CD is exactly WDI's implied price level over PWT's. No
   change to the pack can move it except changing p_c's source.
2. **The two sources use different benchmarks for 2000.** For economies outside the
   Eurostat-OECD programme, WDI extrapolates 2000 backward from the ICP 2011 benchmark with
   relative GDP deflators (World Bank data help desk, article 665452). PWT 11 interpolates
   between the ICP benchmarks it keeps (1996 where a country took part, 2005, 2011) or
   extrapolates from 2005. WDI no longer uses ICP 2005 or 1996 at all.
3. **Where both have a 2000 benchmark, they agree.** PWT flags how each value was made
   (`i_cig`). The 31 ICIO economies with an ICP or Eurostat-OECD benchmark in 2000 have a median
   gap of 3.2%. The 37 interpolated ones have a median gap of 12.7%, and the 12 extrapolated
   ones 50.7%. At 2011, the benchmark both sources share, `pl_da` agrees with WDI within 5% for
   69 of 79. At 2005, which only PWT uses, it agrees for just 27.
4. **Neither source is clearly better off-benchmark.** Inklaar and Rao (2017) find that ICP
   2005 overstated price levels in poor countries, which favours WDI's 2011 anchor. But an
   11-year deflator backcast drifts too, and on a Penn-effect fit WDI's level is closer to its
   own line for only 18 of the 45 misses.
5. **Two artefacts on our side.** WDI's "internal" mismatch (51 of 79) comes from the
   `PA.NUS.PPPC.RF` file, which is a corrupt archive series. Cyprus's miss comes from a
   population denominator.

So no reconciliation makes the 5% check pass for all 79 without making it circular. A check
limited to the 31 shared-benchmark economies can be enforced: all 31 are within 10% once
Cyprus is corrected, and 23 are within 5%.

## 1. The check is a price-level ratio

ADR 0285 compares GDP per person over p_c with NY.GDP.PCAP.PP.CD. With the pack's GDP equal to
WDI's NY.GDP.MKTP.CD (C1 checked this), the ratio is

    (GDP_usd / p_PWT) / (GDP_usd / p_WDI) = p_WDI / p_PWT,

with p_WDI = (MKTP.CD / pop) / PP.CD over the US's. Every number below is that ratio.

## 2. How each source makes a 2000 price level

**WDI.** ICP PPPs exist for the 2011, 2017 and 2021 rounds, with 2012-2016 and 2018-2020
interpolated. Before 2011, the PPP is the 2011 PPP moved by the country's GDP deflator over the
US's (linked deflator since May 2020). For the European and other OECD economies, WDI uses the
Eurostat-OECD annual PPPs where they exist. Source: World Bank data help desk article 665452,
"How do you extrapolate the PPP conversion factors?".

**PWT.** PWT keeps every ICP benchmark a country took part in and makes the years in between
by interpolation that respects national-accounts growth, extrapolating past the first or last
benchmark (Feenstra, Inklaar and Timmer 2015; PWT user guide). `i_cig` records the result per
country and year: benchmark, interpolated or extrapolated.

ICIO economies in 2000 by PWT's method:

| `i_cig` in 2000 | Economies | Within 5% | Median gap |
| --- | --- | --- | --- |
| Benchmark (mostly Eurostat-OECD) | 31 | 22 | 3.2% |
| Interpolated (between ICP 1996 and 2005) | 37 | 11 | 12.7% |
| Extrapolated (back from ICP 2005) | 12 | 1 | 50.7% |

Agreement of `pl_da` with WDI by year, ICIO economies:

| Year | Benchmark for | Within 5% |
| --- | --- | --- |
| 2000 | Eurostat-OECD only | 34 of 79 |
| 2005 | PWT only (ICP 2005) | 27 of 79 |
| 2011 | both (ICP 2011) | 69 of 79 |
| 2017 | both (ICP 2017) | 55 of 79 |
| 2021 | both (ICP 2021) | 53 of 79 |

The agreement peaks at 2011 and falls away from it. At 2017 and 2021, the remaining gaps come
from PWT's absorption level (`pl_da`, C + I + G) differing from ICP's GDP level by net exports,
and from revisions. The output-side `pl_gdpo` agrees for only 43 and 36 there.

A deflator backcast from WDI's own 2011 level, using PWT's deflators, reproduces WDI's 2000
level within 5% for 37 of 79, against 16 from 2017 and 11 from 2021. That fits WDI anchoring
on 2011. The remaining spread is the difference between WDI's and PWT's deflators.

## 3. Nigeria and the oil exporters

PWT's `pl_da` over the US's and WDI's implied level:

| Nigeria | 1996 | 1999 | 2000 | 2005 | 2011 |
| --- | --- | --- | --- | --- | --- |
| PWT | 1.63 (benchmark) | 1.32 | 1.06 | 0.45 (benchmark) | 0.53 (benchmark) |
| WDI | 0.56 | 0.16 | 0.17 | 0.26 | 0.37 |

Three things stack up for Nigeria:

- PWT's 2000 sits on a line between an ICP 1996 benchmark at 1.63 times the US and ICP 2005 at
  0.45. A 1.63 price level for an economy at about 470 USD per person (PWT's 1996 GDP at its market
  rate; 550 USD in 2000) is not plausible. It comes from
  the 1996 African comparison, converted at PWT's market rate (76 naira per dollar, against an
  official 22 until the 1999 unification).
- WDI drops 1996 and 2005 and backcasts from 2011. WDI's 1993-1998 levels use the official rate
  and jump at the 1999 unification (0.60 to 0.16), but 2000 is after it.
- Even at 2011, PWT's absorption level is 1.43 times WDI's GDP level. Nigeria's net exports make
  the two aggregates differ.

On the Penn-effect fit (ln p on ln income, 174 economies), PWT's Nigeria residual is 1.18,
which sits exactly on the D1 screen's far-out fence (1.18). WDI's residual is -0.35.

Saudi Arabia (2.48), Angola (2.75), Brunei (1.87) and the Emirates (1.31) are all
"extrapolated" in PWT: they have no ICP benchmark before 2005, so PWT carries the 2005 level
back with its deflators. Their 2005 values are already 1.2 to 1.8 times WDI's backcast, which
fits Inklaar and Rao's finding on ICP 2005. DR Congo (0.53) and Belarus (0.79) go the other way,
and both had very high inflation, which is where deflator backcasts are least reliable.

## 4. Artefacts on our side

**The WDI fallback file is corrupt.** `wdi_PA.NUS.PPPC.RF.zip` comes from "WDI Database
Archives" (database 57, updated 2025-10-29), not from current WDI. Its US row is not 1: it runs
121.9 (1990), 122.1 (2000), 99 (2011), 57 (2017), 25 (2021), 5 (2024). A ratio-to-US series
cannot do that. Turkey 2000 is 453,396. Over the US's, it matches current WDI's implied level
within 5% for only 82 of 193 economies, and that is the "51 of 79" internal mismatch. The
"unit errors" D1's screen rejected are this file's:

| Economy | Archive over US | Current WDI implied |
| --- | --- | --- |
| Afghanistan | 0.025 | 0.215 |
| Greenland | 0.079 | 0.642 |
| Nauru | 0.056 | 0.454 |
| Curacao | 0.059 | 0.724 |
| San Marino | 0.344 | 0.700 |
| Puerto Rico | 0.494 | 0.706 |

The current-WDI level (MKTP.CD / PP.CD, which is GDP over GDP at PPP and consistent by
construction) gives plausible values for all of them. D1's WDI fallback (17 economies) and its
donor fallback for screened-out economies both read the archive file.

**Cyprus is a population artefact.** Cyprus's ratio is 0.71 to 0.73 in every benchmark year,
1999 to 2011. SP.POP.TOTL (0.948 million) counts the whole island, while WDI's GDP and,
evidently, the PP.CD per-capita denominator cover the government-controlled area (PWT 0.694
million). 0.694 / 0.948 = 0.732. With the denominator matched, Cyprus's ratio is 0.967. The fix
in the check is to compare totals (NY.GDP.MKTP.PP.CD, not yet fetched) rather than per-person
values.

## 5. Can the check be enforced?

| Variant | Within 5% | Within 10% | Circular? |
| --- | --- | --- | --- |
| All 79, `pl_da` (as built after D7) | 34 | 50 | no |
| All 79, Cyprus corrected | 35 | 51 | no |
| 31 shared-benchmark economies, Cyprus corrected | 23 | 31 | no |
| 2011 instead of 2000, all 79 | 69 | | no, but it tests the source, not the 2000 pack |
| p_c from WDI where PWT has no 2000 benchmark | 79 by construction for 48 of them | | yes, for 48 |

On the shared-benchmark set, the remaining 5-10% gaps (Portugal 0.91, Norway 0.92, Greece
0.93, Turkey 0.94, Romania, Switzerland and Ireland 0.95, the UK 1.09) are two vintages of the
same Eurostat-OECD PPPs plus WDI's re-linked national accounts. I have not traced each one, so
that is an inference.

Recommendation:

- **Enforce the check on the 31 economies where PWT's 2000 value is a benchmark, at 10%, and
  report the other 48 with their `i_cig` flag.** 10% is a tolerance set by hand. It goes in the
  Section 17 register with this justification: it is the measured agreement of two published
  vintages of the same benchmark PPPs. The check then catches unit and wiring errors where the
  comparison means something, and it stops gating on a methodological split that neither
  source resolves.
- **Replace the archive `PA.NUS.PPPC.RF` with current WDI's implied level** (MKTP.CD over
  PP.CD times population, over the US's) as D1's WDI fallback. This is an amendment of ADR
  0282, which needs an RFC item.
- **Compare totals, not per-person values**, in the check, which fixes Cyprus.
- Keep `pl_da` (D7 A) as p_c. Nothing here favours WDI's level for the disagreeing economies
  strongly enough to override D1's choice of PWT.

## Sources

- Feenstra, R. C., Inklaar, R. and Timmer, M. P. (2015). The Next Generation of the Penn World
  Table. American Economic Review 105(10), 3150-3182.
- Inklaar, R. and Rao, D. S. P. (2017). Cross-Country Income Levels over Time: Did the
  Developing World Suddenly Become Much Richer? AEJ: Macroeconomics 9(1), 265-290.
  https://doi.org/10.1257/mac.20150155
- Deaton, A. and Aten, B. (2017). Trying to Understand the PPPs in ICP 2011: Why Are the
  Results So Different? AEJ: Macroeconomics 9(1), 243-264. Cited from memory, not re-read.
- World Bank data help desk, "How do you extrapolate the PPP conversion factors?" (article
  665452): https://datahelpdesk.worldbank.org/knowledgebase/articles/665452-how-do-you-extrapolate-the-ppp-conversion-factors
- WDI metadata in the downloaded zips (PP.CD sources; Nigeria's dual-rate note; the
  PA.NUS.PPPC.RF file's "WDI Database Archives" header).
