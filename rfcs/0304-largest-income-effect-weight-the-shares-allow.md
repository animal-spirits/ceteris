# RFC 0304. The largest income-effect weight the household shares allow (amends ADR 0094)

Status: accepted
Date: 2026-11-04
Design sections: 8.6; econ SPEC (Final demand, Inversion)
Research: none (diagnosis of the C6 calibration on the rebuilt pack, below)

## Summary

The PIGL inversion of ADR 0094 fixes omega_A = 0 (the pooled estimate of Alder, Boppart and
Mueller 2022) and backs out the income-effect weight z = Omega_A / theta_A, then omega_M and
omega_S, from each country's household shares. That needs Omega_c >= z theta_c for M and S. Since
ADR 0099 and RFC 0301 invert households' part of final demand only, three countries of the
rebuilt 2000 pack fail it: Nigeria (ICIO), Ghana (Nigeria's table as donor) and Chad (RAS), with
household manufactures 2.0% to 5.6% against food 44% to 54% (M / A 0.046 to 0.10, the bound
theta_M / theta_A = 0.129). The calibration panics before pass 1.

Three general routes were compared (2026-11-04, owner's request):

- Data. ICP 2005 (World Bank, CC BY 4.0, all 142 economies it covers) shows Nigeria's ICIO
  household column is an artifact (telecoms 12% of household spending in 2000), but on honest
  bounds (only clothing and vehicles certain goods) Chad's value lies inside ICP's range and stays
  infeasible, and ICP's purchaser-price food shares exceed theta_A in 8 countries, so taking ICP's
  split breaks them.
- PIGL re-estimated on the 2000 cross-section of 233 countries: with ABM's parameters common
  preferences fit worse than the mean (R2 < 0); theta free goes to theta_M = 0 (25 countries
  infeasible); theta, eps and gamma free degenerate to theta = (1, 0, 0) with eps 0.28 (R2 0.43).
- Non-homothetic CES (Comin, Lashkari and Mestieri 2021, S03): best common fit (R2 0.53), always
  feasible, but with an income elasticity of food at its bound of 0 and services below
  manufactures, and a Newton solve and quadrature per pop.

The diagnosis common to all three: ABM's theta is estimated on rich-country histories (US, UK,
Canada, Australia since 1900), and poor budgets of 2000 lie outside omega_A = 0 on either price
basis. This RFC keeps theta, eps and gamma and relaxes omega_A = 0 by one rule for every country:
the income-effect weight is the largest at which every omega is nonnegative.

## Decisions

### D1. How does the inversion treat household shares outside omega_A = 0?

Options:

- 1. Largest feasible income-effect weight, one rule for every country: z = min_c Omega_c /
  theta_c at prices 1 (A on a tie), the binding category's omega 0 and the others (Omega_c - z
  theta_c) / (1 - z); over pops, the binding category is A where omega_A = 0 fits, else M or S by
  the sign of the M aggregate's miss, its aggregate giving lambda in closed form and the split of
  the other two meeting theirs. Identical to ADR 0094 wherever omega_A = 0 fits; never fails
  (z < 1 unless Omega = theta). Food enters B(P) only where it must.
- 2. Non-homothetic CES for households (S03 as default): always feasible, best common fit, slower
  ticks and no closed-form aggregation.
- 3. Correct Nigeria's household column (and Ghana through it) from ICP 2005; Chad still needs 1
  or 2.

Recommendation: 1, because it keeps the paper's parameters and the closed form, changes nothing
where the paper's normalization fits, and covers any shares, including food above theta_A.

Decision: 1, Antonio, 2026-11-04.
Row: ADR 0313

## Spec changes

- 8.6: the income-effect weight is the largest the household shares allow; omega_A = 0 where it
  fits (ADR 0313).
- Econ SPEC, Final demand, Inversion: the binding category over pops; the closed form z = min_c
  Omega_c / theta_c.

## Tests

- `inversion_outside_omega_a_zero_binds_another_category` (econ `demand.rs`): low services,
  Ghana's shares and food above theta_A; the binding category's omega 0, omega_A > 0, the shares
  met in the closed form (1e-12) and over four pops at their own prices and incomes (1e-11).
- `inversion_outside_omega_a_zero_binds_another_category` (reference `tests/reference.rs`): the
  same rows; the joint inversion of one pop at prices 1 equals the closed form (1e-12) and meets
  the shares (1e-12).
- Existing inversion tests unchanged: every fixture country keeps omega_A = 0.

## Consequences

- C6 calibrates on the rebuilt pack; Ghana, Nigeria and Chad bind on M with omega_A > 0, so their
  rich pops keep buying some food in B(P). No other country's weights change, so no table
  columns, format or digest change.
- Nigeria's ICIO household column stays as ICIO reports it; an ICP-based correction of artifacts
  is left to a later data RFC.
