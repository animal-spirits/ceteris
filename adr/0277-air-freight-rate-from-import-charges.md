# 0277. Air freight rate from 2000 import charges

Status: proposed
Date: 2026-10-31
Design sections: 5.3, 17.1 (P04d, P04m), 21 (pre-P3, P9); amends ADR 0269 (P04d row);
ADRs 0017, 0022, 0247, 0257, 0264, 0271

## Context

ADR 0269 borrowed P04d, air cost per tonne-km, as the BTS 2000 domestic air carrier revenue
per ton-mile over truck: 5.37 times road, 0.537 USD per tonne-km, range 3.87 to 5.37 (BTS 1990
to 2004). The B8 gravity re-estimate (ADR 0022 diagnostic) could not identify air: 3 country
pairs with a positive flow use an air leg on the least-cost envelope, fewer than the 50 the
guard asks for. More BACI rows will not fix this. Country-pair flows see only least-cost legs,
and with 7 traded goods at one weight per value each, a pair-good is all air or no air. In the
data, 28% of US import value came by air (Hummels and Schaur 2013, Table 1, 1991 to 2005), with
mode mixing inside HS6-exporter cells holding 75% of import value. The model's envelope cannot
show that mix, so gravity will not identify air in any phase while route choice (ADR 0017,
built but off) stays off.

The BTS value is also the wrong concept for P04d on three counts:

- **Fees counted twice.** BTS revenue per ton-mile is average revenue, which includes terminal
  costs. The model charges those separately: an air leg pays the P04m airport fee at both ends
  (2 x 60 USD/t). At a US domestic haul of about 1,500 to 2,000 km, the fees alone are 0.06 to
  0.08 USD per tonne-km of the 0.506.
- **Haul length.** Domestic hauls are short. International air freight in 2000 averaged 5,400
  km (ICAO via BTS Freight Transportation, Table A-7: 101,560 million tonne-km over 18.80
  million tonnes). Per-km rates fall with length.
- **Cargo mix.** The BTS row is cargo and mail, carried mostly by integrators and in passenger
  bellies on domestic routes.

The ratio against sea also fails a sourced check. On a 10,000 km route the model's air cost per
tonne is 2 x 60 + 0.537 x 1.05 x 10,000 = 5,759 USD. Its sea cost is 2 x 8 + 0.0121 x 10,000 =
137 USD. That is 42 times. Hummels and Schaur (2013, Table 1) give a median air to ocean ratio
of 6.46 per kg within product and exporter, and 27 at the 90th percentile. The model's gap is
6.5 times the data median and 1.6 times its 90th percentile. Air is overpriced, sea is
underpriced, or both. ADR 0269's ocean research puts sea inside its range (0.05 to 0.20 of
road), so air is the first suspect.

### Data that price air directly

| Source | Coverage circa 2000 | What it gives | Licence |
|---|---|---|---|
| US Census, US Imports of Merchandise (`imp_detl` layout), 2000 | All US imports by HS10, country of origin and district of unlading | `AIR_VAL`, `AIR_WGT`, `AIR_CHA` (customs value, shipping weight, import charges: freight, insurance and other charges from foreign port to US port), the same for vessel | US federal work, public domain; Schott's (2008) Stata conversion asks for a citation |
| ECLAC International Transport Database (BTI), from BADECEL customs records, 2000 (1997 to 2000 held) | Imports of 11 Latin American countries (AR, BO, BR, CL, CO, EC, MX, PY, PE, UY, VE) by origin, product and mode | Value, weight, and freight plus insurance (CIF minus FOB) by mode. VE has no CIF; BO and CL have gaps | ECLAC publication; terms to read before `sources.toml` |
| Eurostat Comext, extra-EU trade by mode of transport, 1999 onwards | EU15 imports and exports by partner, product and mode | Value and weight by mode; no charges | Eurostat reuse policy (attribution) |
| BTS NTS Table 3-21 | US domestic air, 1990 to 2004 | Average revenue per ton-mile | Public domain (already in the register, ADR 0269) |

The first two price air legs in the form the model uses. S05 prices a leg as a fixed part
(fees) plus a rate times length, and ADR 0247 fitted sea bulk the same way: a fixed fee and a
slope from voyage rates. Charges per kilogram regressed on distance give the slope, which is
P04d times circuity, and an intercept to compare with the airport fees. Hummels (GTAP working
paper 1162, Table 3) reports the distance elasticity of US air freight rates at 0.459 (s.e.
0.003). A linear-plus-fixed form gives that elasticity when the fixed part is about 1.2 times
the per-km part at the mean distance, so the form fits. Comext has no prices. It and the Census value shares are the
mode-share moments ADR 0269 kept for P9, and they need route choice on.

## Options

- **A. Fit the air rate to 2000 import charges, Census first, BTI as the second importer
  set.** Per air cell (HS6, origin, district of unlading or importer): charges per kg = F_k +
  s L + i v, with L the great-circle km between the origin's mass-weighted pack airports and the
  destination's, v the value per kg (the insurance and ad valorem part of charges), F_k an HS6
  effect, and kg weights. P04d = s / 1.05 (P04h circuity). The kg-weighted mean of F_k is
  reported against 2 x P04m (airport) and not applied. Census 2000 sets the value. BTI 2000,
  and Census 1999 and 2001, set the P9 range, together with BTS 1990 to 2004 for the upper end.
  Public domain data, the estimating form is S05's, and 0247 is the precedent.
- **B. Keep BTS 5.37 and record the sea-ratio miss.** No new data. The 42-to-6.5 gap stays,
  and the value stays a domestic average with fees counted twice.
- **C. Mode-share moments now (Allen and Arkolakis 2014: Census and Comext value shares by
  mode).** The estimator the literature uses, but it needs route choice on (ADR 0017, off),
  which changes every leg and is P9's joint refit (ADR 0269, ADR 0264 option F).
- **D. Airline yields per tonne-km (ICAO Survey of International Air Transport Fares and Rates,
  IATA WATS).** International route-group averages, closer in haul length than BTS. They are
  still average revenue with fees inside, and the tables are paid publications with
  restrictive terms.

## Decision

Proposed: A. The owner's ruling and its date go here.

| ID | Mode | Value | Range for P9 | Source |
|---|---|---|---|---|
| P04d | Air | s_2000 / 1.05 from Census 2000 air charges, as a ratio to road | min to max over Census 1999 to 2001, BTI 2000 and BTS 1990 to 2004 | This ADR, fit A |

- Ordering check (DESIGN 5.3): the fitted ratio must exceed 1 (air dearer than road). If it
  does not, the fit stops and goes to the owner. It is not clamped.
- The PPML diagnostic of ADR 0022 stays. Two quantity diagnostics join the C7 report as tier 2
  rows (ADR 0257): the model's air share of international trade value, against 28% of US
  imports (Hummels and Schaur 2013), and air tonne-km, against 101,560 million (ICAO 2000).
  Neither is fitted. The named mechanism is route choice with within-good weight dispersion
  (ADR 0017, P44), with P9's joint refit and ADR 0271's 50 goods.
- P9 keeps ADR 0269's plan: mode-share moments with route choice on replace this fit if they
  identify air within the range.

### Build

1. `sources.toml`: Census US Imports of Merchandise 2000 (public domain, via Schott's annual
   files, with a check that the mode fields came through the conversion) and ECLAC BTI 2000
   (licence read first).
2. Fetch on the owner's Mac (the container cannot reach Dropbox or the ECLAC query service), as
   with the 50-goods pack (ADR 0271).
3. `ceteris-calibrate`: `air_rate` beside `gravity.rs`. Tests: synthetic cells with a known F,
   s and i recover them to 1e-9. Weights and fixed effects use the existing `hdfe`.
4. `ceteris calibrate` writes the estimate, its pair-clustered s.e., the intercept against
   P04m and the two diagnostics. The register and the geo SPEC take the value.

## Consequences

- The sea check gives the direction, not the value. Taken literally (air at 6.46 times the
  model's 137 USD/t sea cost at 10,000 km), it would put air at about 0.073 USD per tonne-km,
  below road. But ocean charges in the data include container terminal and liner costs that the
  model's sea legs price lower, so the check overstates the cut. Expect P04d well below 5.37,
  and possibly near the ordering check. The fit sets the value. If P04d falls, more pair-goods
  route by air (light goods, long hauls), and nu (ADR 0092) and the steady state are
  recalibrated.
- Goldens that price legs move: `tests/golden/p1-geo.txt` and the econ goldens. The commit and
  the CHANGELOG carry `### BEHAVIOUR: P04d air rate fitted to 2000 import charges (ADR 0277)`.
- No silent tuning (DESIGN 21.2, rule 5). The value comes from the fit, not from a test.
- Census charges run from the foreign port to the US port, so destination handling and the
  access legs are outside them. The intercept is a check on P04m, not an estimate of it.
- The register class of P04d becomes "Calibrated (import charges, ADR 0277); mode shares in P9".
