# RFC 0282 D8 and D9: handoff for the pre-P3 audit session

Antonio ruled both on 2026-11-01 (23:04 UTC). Evidence: research/pwt-wdi-price-levels-2026-11-01.md.
Each item needs a row (`cargo run -p ceteris-rows -- next 2`), a spec edit and a build, the
same way as D1 to D7.

## D8. What does ADR 0098's real GDP check gate?

Found: the check's ratio is exactly p_WDI / p_PWT, because the pack's GDP is WDI's. The sources
part over method, not data. WDI carries ICP 2011 back with GDP deflators; PWT interpolates
through ICP 1996 and 2005. Where PWT's 2000 value is a benchmark (`i_cig` = Benchmark, 31 ICIO
economies, mostly Eurostat-OECD), the median gap is 3.2%. It is 12.7% for the 37 interpolated
economies and 50.7% for the 12 extrapolated ones.

Options: A gate the 31 benchmark economies at 10%; B reported only; C WDI levels off-benchmark;
D check at 2011.

Decision: A, Antonio, 2026-11-01.

Build:
- Gate the economies whose PWT `i_cig` for 2000 is Benchmark at |ratio - 1| <= 0.10.
- Report the other 48 with their `i_cig`.
- Compare totals, not per-person values: GDP over p_c against NY.GDP.MKTP.PP.CD (fetch it and
  add it to sources.toml and validation.toml). SP.POP.TOTL counts all of Cyprus, which puts its
  ratio at 0.708 when the true value is 0.967.
- Register the 10% tolerance in Section 17 (`next-code P`). Justification: it is the measured
  agreement of two published vintages of the same benchmark PPPs, 31 of 31 within it with
  Cyprus corrected, and the largest gap is the UK's 1.093.
- Expected result: 31 of 31 pass; 23 of 31 are within 5%.

## D9. Which WDI series is D1's fallback?

Found: `wdi_PA.NUS.PPPC.RF.zip` comes from "WDI Database Archives" (DS57, 2025-10-29). Its US
row is not 1 (122.1 in 2000, 99 in 2011, 5 in 2024). Over the US's, it agrees with current WDI's
implied level within 5% for only 82 of 193 economies. D1's rejected "unit errors" come from it:
Afghanistan 0.025 against 0.215, Greenland 0.079 against 0.642, Nauru 0.056 against 0.454,
Curacao 0.059 against 0.724.

Options: A current WDI's implied level; B keep the archive.

Decision: A, Antonio, 2026-11-01.

Build:
- The WDI level becomes NY.GDP.MKTP.CD / (NY.GDP.PCAP.PP.CD x SP.POP.TOTL) over the US's, same
  year rule. Population cancels in this ratio, so Cyprus is not affected here.
- Drop `PA.NUS.PPPC.RF` from read/wdi.rs, fetch.toml and sources.toml.
- Rerun the Penn-effect screen. Expect fewer donor-filled economies, and the 17 WDI-sourced
  levels to move.
- The C1 finding "WDI 2000 values carry currency-unit errors (Turkey 3,712 ...)" becomes a
  finding about the archive file.
