# 0277. Air freight rate from 2000 mode shares

Status: accepted (owner, 2026-10-31: mode shares now; build now, value at the next rebuild; gateway sum, route choice off)
Date: 2026-10-31
Design sections: 5.3, 8.3 (two-level kernel), 17.1 (P04d), 21 (pre-P3, P9); amends ADR 0269
(P04d row) and ADR 0022 (direct estimation of P04); ADRs 0017, 0092, 0247, 0257, 0264, 0271

## Context

ADR 0269 borrowed P04d, air cost per tonne-km, as the BTS 2000 domestic air carrier revenue
per ton-mile over truck: 5.37 times road, 0.537 USD per tonne-km, range 3.87 to 5.37 (BTS 1990
to 2004). Its P9 plan is a fit to mode-specific moments (Allen and Arkolakis 2014). The B8
gravity re-estimate (ADR 0022 diagnostic) could not identify air: on the least-cost envelope
only 3 country pairs with a positive flow use an air leg, fewer than the guard's 50.

### Why gravity cannot see air, and where the model can

The gravity sample (`ceteris_calibrate::costs::country_costs`) prices a country pair by the
**cheapest** chain of access, crossing and access legs over all gateway pairs. Each pair and good
then goes all by air or not at all, so flows say almost nothing about air's cost. In the data,
28% of US import value came by air (Hummels and Schaur 2013, Table 1, 1991 to 2005). Mode
mixing inside HS6-exporter cells holds 75% of that value.

The trade kernel does not take that minimum. It **sums** over gateway pairs (trade SPEC,
Two-level kernel: K_od = sum over a, b of K_oa K_ab K_bd, with K = tau^-theta_g). That sum is
the entropic route choice of Allen and Arkolakis (2022) at the level of gateways, with the trade
elasticity as its dispersion. The model therefore already gives every pair and good a smooth air
share: the part of X_od dispatched over crossings whose least-cost path flies. That share is the
mode-share moment. ADR 0017's full route choice over edges is not needed, so it stays off. It is
measured as infeasible on this graph: a spectral radius of 2.96 to 6.04 at P44's prior, and
160 to 227 s per good for the gateway solves (ADR 0017, Deferral).

### Two checks on the BTS value

- **Fees counted twice.** BTS revenue per ton-mile is an average that includes terminal costs.
  The model charges those separately, as P04m airport fees at both ends (2 x 60 USD/t).
- **Air against sea.** On a 10,000 km route the model's air cost is 2 x 60 + 0.537 x 1.05 x
  10,000 = 5,759 USD/t. Its sea cost is 2 x 8 + 0.0121 x 10,000 = 137 USD/t, so air is 42 times
  dearer. Hummels and Schaur (2013, Table 1) give a median air to ocean ratio per kg of 6.46
  within product and exporter, and 27 at the 90th percentile. Ocean charges in the data include
  liner and terminal costs that the model's sea legs price lower, so the true gap is above 6.46.
  The 42 is still well above the 90th percentile.

### Data, circa 2000

| Source | Coverage | What it gives | Licence |
|---|---|---|---|
| US Census, US Imports of Merchandise (`imp_detl` layout), 2000 | US imports by HS10, country of origin and district of unlading | `AIR_VAL`, `AIR_WGT`, `VES_VAL`, `VES_WGT`, and the rest (land) as total minus air minus vessel; `AIR_CHA`, `VES_CHA` import charges | US federal work, public domain (17 U.S.C. 105). Schott's (2008) Stata conversion asks for a citation |
| Eurostat Comext, extra-EU trade by mode of transport, 1999 onwards | EU15 imports by partner, product and mode | Value and weight by mode (sea, air, road, rail, other) | Eurostat reuse policy, attribution (CC BY 4.0) |
| ECLAC International Transport Database (BTI), 2000 | Imports of 11 Latin American countries by origin, product and mode | Value, weight, freight plus insurance by mode | ECLAC terms, to read before `sources.toml`; held as the third importer set |

HS products map to the pack's goods through the OECD BTiGE key the pack already uses (ADR 0050
amendment).

## Options

- **A. Fit P04d to 2000 import charges** (slope of charges per kg on distance). Not picked.
- **B. Keep BTS 5.37.** Not picked.
- **C. Mode shares now** (owner pick). Fit P04d so that the model's air value shares match
  Census and Comext 2000 by importer, origin and good.
- **D. Airline yields (ICAO, IATA).** Not picked: paid tables with restrictive terms, and the
  yields are still averages with fees inside.

## Decision

C (owner, 2026-10-31; the gateway-sum reading confirmed the same day). Route choice over edges (ADR 0017) stays off. The shares come from the
kernel's gateway-pair sum, as described above.

### Moment

For importer d in {US, the EU15 members}, origin country o and traded good g:

```
S_odg(r) = sum over gateway pairs (a, b) with an air crossing of K_oa K_ab(r) K_bd   /   K_odg(r)
```

with r = P04d / P04a, and air crossings the gateway pairs whose least-cost crossing path has
positive air km (path quantity q[AIR] > 0). K_ab is the kernel's crossing factor at the
calibrated nu and tariffs, using the gateway masses (ADR 0092) if they are built when the fit
runs. Province weights are the pack's value-added weights, as in `country_costs`.

The data share s_odg is air value over total value, after mapping HS products to goods.
Estimator: fractional-response quasi-likelihood (Papke and Wooldridge 1996), Bernoulli, with
weights equal to the cell's import value, so that

```
r_hat = argmax sum w_odg [ s_odg ln S_odg(r) + (1 - s_odg) ln (1 - S_odg(r)) ]
```

It is one-dimensional, solved by Newton on the analytic derivative of S with respect to r. A
finite-difference check is part of the tests. Standard errors are clustered by country pair. The
gravity sample's cheapest-chain costs stay as ADR 0022's diagnostic and are not used here.

- **Value:** r_hat on Census 2000 plus Comext hindcast to 2000 (Amendment 2026-11-01). **Range for P9:** the minimum to the maximum of
  r_hat on each importer set alone (Census, Comext, BTI once its licence is read), on 1999 and
  2001, and BTS 1990 to 2004 (ADR 0269).
- **Ordering** (DESIGN 5.3): r_hat must exceed 1, so that air is dearer than road. If it does
  not, the fit stops and goes to the owner. It is not clamped.
- **Held fixed:** road, rail and sea (ADR 0269), theta_g, nu, P43 time costs, P04m fees and
  speeds. Sea's share is reported against the vessel share as a diagnostic, not fitted.

### Diagnostics in the C7 report (tier 2, ADR 0257)

- Fit by theta class: data and model air share in bins of distance and weight per value. One
  dispersion, theta_g, sets both the trade and the mode margins. A systematic gradient in the
  residuals is the evidence for a separate mode dispersion, which belongs to P9's joint refit
  with ADR 0264's option F.
- Air tonne-km and tonnes against ICAO 2000: 101,560 million tonne-km and 18.80 million tonnes
  international, a mean haul of 5,400 km (BTS Freight Transportation, Table A-7).
- The model's air to sea cost ratio per kg at 10,000 km, against Hummels and Schaur's 6.46 and
  27.

### Build (owner pick: now; the value lands at the next goldens re-bless)

1. `sources.toml`: Census 2000 imports (public domain, via Schott's annual files, with a check
   that the mode fields came through the conversion) and Comext by mode (Eurostat). BTI goes in
   only after its terms are read.
2. Fetch on the owner's Mac, as with ADR 0271's data. The container cannot reach Dropbox or
   Comext's bulk service.
3. `ceteris-calibrate`: `air_share` beside `gravity.rs`. It computes per-gateway-pair kernel
   weights from the same costs as `network_from_graph`. Tests:
   - on a synthetic two-country graph with known r, the fit recovers r to 1e-9;
   - S matches a brute-force sum over gateway pairs to 1e-12;
   - the derivative of S matches a finite difference;
   - S rises as r falls.
4. `ceteris calibrate` writes r_hat, its s.e. and the diagnostics. The register, the geo SPEC
   and DESIGN 17.1 take the value at the next rebuild that re-blesses the goldens: C7 if the fit
   is ready, otherwise the 50-goods pack (ADR 0271).

## Consequences

- The value is unknown until the fit runs. The sea check says P04d should fall from 5.37, so
  more light, long-haul trade routes through airports, and nu (ADR 0092) and the steady state
  are recalibrated in the same rebuild.
- Goldens that price legs move at that rebuild: `tests/golden/p1-geo.txt` and the econ goldens.
  The commit and the CHANGELOG carry `### BEHAVIOUR: P04d air rate fitted to 2000 mode shares
  (ADR 0277)`.
- No new mechanism: P04d is a value in the existing kernel. ADR 0017 stays deferred, and the
  ablation test (15.7) is not triggered.
- No silent tuning (DESIGN 21.2, rule 5). The value comes from the fit, not from a test.
- With 7 goods (10 in the pack) at one weight per value each, within-good dispersion in value
  per kg is absent. The fit then loads it onto r, and the 50 goods of ADR 0271 reduce that.
  The fit is rerun on the 50-goods pack.
- The register class of P04d becomes "Calibrated (2000 mode shares, ADR 0277)". The P9 plan of
  ADR 0269 becomes a joint refit of all mode ratios on the same moments.

## Amendment (owner, 2026-11-01): EU data hindcast from 2002, pooled

Eurostat publishes trade by mode of transport only from January 2002 (DS-059334). 1999 to 2001
existed only on a paid Comext DVD. The owner ruled that 2002 is hindcast to 2000 and pooled into
the value:

- **Composition.** Each EU15 importer, extra-EU origin and HS6 cell keeps its 2002 air share,
  weighted by its 2000 import value from DS-045409 (EU trade since 1988, HS6, no mode). Cells
  with 2000 trade but no 2002 mode record are dropped and counted.
- **Trend.** Per traded good, the cell shares are shifted in log-odds by the US change from 2000
  to 2002 (Schott's 2002 file), import-value weighted over the origins both years share.
- **Value:** r_hat on US 2000 plus hindcast EU15 2000, pooled, clusters by (importer, origin).
  **Range:** the US-only and EU-only fits, beside the sets listed under Decision.
- Mode 0 (not declared, about 9.5% of 2002 EU15 import value) is left out of the share's
  denominator and counted.

## Amendment (deferred, owner, 2026-11-01: keep BTS 5.37, revisit on the 50-goods pack, ADR 0271): path-consistent estimator

**Ruling.** P04d stays at ADR 0269's value, the BTS ratio of 5.37 times road. The fit is revisited
on the 50-goods pack (ADR 0271), where the path-consistent estimator below is the method to try
first. The evidence and results below are kept as the record.

On the per-good least-iceberg paths of ADR 0279 the estimator of Decision (Fisher scoring on
paths held fixed, then the paths solved again at the fitted r until the set of air crossings
repeats) does not converge. The fixed-path likelihood keeps rising past the point where the
paths change: crossings that fly at r keep their air km as r rises. Re-solved at the fitted r,
most of them stop flying, and the next fit runs r to the floor. Evidence on the 2000 pack
(Iceberg calibration, US 2000 and hindcast EU15 2000, 11,440 cells pooled):

| Set, start | Outer step 1 | Outer step 2 |
|---|---|---|
| pooled, r 8 (P04d default) | r to 8.7e-12 | |
| pooled, r 2.83 | r 7.45 | r to 2.1e-10 |
| EU, r 2.83 | r 5.48 | r to 5.7e-13 |
| US, r 3.36 | no convergence in 100 steps (r 6.09) | |

Solved at each r instead (`ceteris air-share --profile`, 25 points from 0.5 to 32), the
fixed-set quasi-likelihood has an interior maximum in every set: r 2.83 pooled and EU, 3.36 US.

**Method (built, `ceteris air-share`; not used for the value):** maximise that profile, with the paths solved at every r (geo SPEC, Air rate from
mode shares, Estimator): the grid's best point, then golden section in ln r between its
neighbours to 1e-4. Errors of ln r by a cluster bootstrap over (importer, origin), 200
replications with a fixed seed, on a 1/32-octave grid across the bracket, with a 90% percentile
interval. The clustered sandwich from the profile's curvature is reported beside it but is not
the error: the profile jumps where paths switch, so its curvature at the maximum describes one
piece only.

Results (Iceberg pack, road 0.10 USD per tonne-km; 65 path solves, about 20 min a set):

| Set | r_hat | P04d (USD/tkm) | Bootstrap se of ln r | 90% interval of r | Curvature se | Model / data air share, informative cells |
|---|---|---|---|---|---|---|
| pooled | 2.607 | 0.261 | 0.082 | 2.60 to 3.10 | 0.0075 | 0.386 / 0.281 (3,950 cells) |
| US | 3.364 | 0.336 | 0.057 | 3.00 to 3.72 | 0.0067 | 0.237 / 0.247 (1,151) |
| EU | 2.607 | 0.261 | 0.010 | 2.59 to 2.62 | 0.0048 | 0.387 / 0.347 (2,814) |

The pooled and EU maxima sit at the same r, just below a path switch, and the US maximum is the
grid point itself: the profile is piecewise, so the bootstrap interval is asymmetric. By good,
the model's air share at r_hat overshoots for machinery (good 6, 0.63 against 0.37 pooled),
undershoots for agriculture, light manufacturing and chemicals (goods 0, 4, 5), and is zero for
coal and other mining (good 2, data 0.19). That is the within-good dispersion of value per kg
noted under Consequences, which the 50 goods of ADR 0271 address.

The owner kept the BTS value instead of either estimate on this pack: with 7 goods the fit is a
level match with large offsetting errors by good.
