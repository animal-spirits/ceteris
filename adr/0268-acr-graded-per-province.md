# 0268. The ACR criterion graded per province

Status: proposed
Date: 2026-10-31
Design sections: 15.2, 21 (pre-P3 benchmark); amends ADR 0265 (the ACR criterion with
input-output linkages); ADRs 0207, 0257

## Context

ADR 0265 grades ADR 0207's ACR criterion with the input-output formula applied to each country,
d ln W_c = -Omega_c' (I - A_c')^-1 diag(1/theta) d ln lambda_c (`benchmark::acr_io`). On the
scratch pack `k263/pack` the median model / ACR ratio passes (1.058) and the correlation across
countries misses (0.856 against 0.9), with no wrong signs (C7 groundwork,
`specs/pre-p3/C7-LITERATURE.md`, section 2).

The formula is exact in hats for one integrated location (Costinot and Rodriguez-Clare 2014,
NBER WP 18896, section 3.4, assumptions behind eqs. (24) to (28)). A model country is many
provinces with internal trade costs and immobile pops. Ramondo, Rodriguez-Clare and
Saborio-Rodriguez (2016, NBER WP 18532, section 2.6) show that with domestic frictions the
country-level formula holds only if regions are symmetric: each region's real wage follows its
own trade share, w_m / P_m = psi_mm^(-1/theta) (their fn. 11), and country gains aggregate the
regional changes (their Proposition 4, eq. (18)). Applied to the country's aggregate lambda the
formula errs by an amount that grows with the dispersion of province openness inside the
country, which lowers a cross-country correlation and leaves the median near 1. Large, unequal
countries (BRA, CHN, IND, RUS, USA) should carry the largest residuals.

The owner ruled on the groundwork's option B (2026-10-31, "B a a a", ruling 1): compute the
input-output formula per province and income-weight it up to countries, same band and
threshold, the country forms still reported.

## Options

- **A. Grade the simulation against the model's exact counterfactual** (`ceteris-reference`
  solved under the shock, Dekle, Eaton and Kortum 2008). Strictest tier 1 reading; needs the
  reference to take shocked trade costs.
- **B. Province-level ACR aggregated to countries** (Ramondo et al. 2016). The literature's
  answer to domestic frictions; a change to what the criterion measures, hence this ADR (ADR
  0257, tier 4).
- **C. Diagnose first** (regress the country residual on province-share dispersion, A_c column
  sums, late baseline drift).

## Decision

Option B (owner, 2026-10-31), with income weights on both sides (point 2).

1. **The graded prediction.** For province p in country c and the base and shocked runs,

   ```
   d ln W_p = -Omega_c' (I - A_c')^-1 diag(1/theta) d ln lambda_p
   d ln W_c^ACR = sum_{p in c} s_p d ln W_p,     s_p = I_p / I_c in the base run
   ```

   lambda_pg is province p's home share of its absorption of traded good g by value at origin
   prices over the flow window: what p buys from itself over what p buys from every origin,
   domestic and foreign. d ln lambda is 0 for local goods, which pass input cost changes on
   through the inverse, as in `acr_io`. Omega_c and A_c are the country's, as in ADR 0265:
   the pack carries final-demand and cost shares per country, and the province coefficients
   are built from the country's cost shares at the province's base prices (ADR 0020, owner
   2026-10-25), so A_p = A_c at the calibrated prices. I_p is the province's pop income. A
   province that buys g at home in one run only makes its d ln W_p NaN, and the country's with
   it, as in ADR 0265; a province with no absorption of g in either run adds nothing for g.
   The function is `benchmark::acr_io_province`.

2. **The model side on the same weights** (owner, 2026-10-31: the choice closer to the
   literature; country welfare as aggregate real income, as in Dhingra et al. 2017 and the ACR
   country measure). The model's welfare is reduced the same way:
   d ln W_c = sum_p s_p (d ln I_p - Omega_c' d ln q_p), q_pg the province's buyer price. The
   current reduction (`benchmark::welfare`: d ln I_c less Omega_c' times the d ln of the
   population-weighted mean price) differs from it at first order by
   sum_g Omega_cg sum_p (s_p - pi_p) d ln q_pg, pi_p the population share: the covariance
   between income per head and price changes inside the country, which is the geography this
   ADR grades. Comparing an income-weighted prediction with a population-weighted outcome would
   leave that term in the residual. The current reduction stays reported. A province with no
   buyer price for a good with Omega_cg > 0 makes its country's model welfare NaN (owner ruling
   2026-10-31, "Go with your recommendations"): the criterion grades only countries where both
   sides are finite, and the report counts the rest.

3. **The criterion is unchanged otherwise.** Under every crossing tau x 1.1, every ICIO country
   with |ACR| above 1e-4 has the ACR sign, the median model / ACR ratio is in [0.8, 1.25] and
   the correlation across countries is at least 0.9, all on the province-graded pair of point 1
   and 2.

4. **Reported beside it.** The country-level `acr_io` (ADR 0265) and plain `acr` (ACR 2012), and
   for each country the gap `acr_io - acr_io_province`, which measures how far the
   one-location formula is from the province aggregate.

## Consequences

- The flow window records each province's absorption of traded goods by value (what it buys
  from every origin), beside the home purchases it records now; `benchmark::Outcome` carries
  province income, buyer prices and home shares. The flow window is scratch, not state, so no
  golden hash moves.
- The aggregation is a first-order reduction, not the exact one. Ramondo et al. (2016) aggregate
  with a power mean in their mobility elasticity because their workers move; with immobile pops
  and real income as the outcome, income weights are the first-order change of the country's
  real income. The formula per province stays first order for the reasons ADR 0265 records
  (Leontief intermediates, data Omega and A).
- When migration moves pops between provinces (DESIGN 6.5, dispersion P16), the own share stops being
  sufficient for a province's welfare (Redding 2016) and this criterion needs revisiting.
- Under ADR 0257 tier 4, what the changed test no longer checks: that the one-location formula
  predicts a country's welfare. That is reported (the gap of point 4), not graded.
- C7 asserts the criterion at B8 on the final pack. Until then `benchmark_cases_are_reported`
  prints all three forms and whether the province one is in the band, and asserts nothing.
- PLAN (pre-P3, ADR 0207 criteria) and the econ SPEC (Calibration, Benchmark) name the
  province-graded form.
