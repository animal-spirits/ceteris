# 0109. International migration under admission caps

Status: proposed
Date: 2026-10-26
Design sections: 6.5, 10.6, 15.4; amends ADR 0046 (immigration salience)

## Context

Migration (6.5) is a logit over destinations on real wages net of bilateral costs (Caliendo,
Dvorkin and Parro style). Within countries this fits well. Across borders, flows are bounded
by admission policy: the number of people who would move at observed wage gaps exceeds actual
flows by large factors (Gallup potential-migration surveys against UN DESA stocks). A pure cost
logit must either set bilateral costs implausibly high, which then mis-prices every other
response, or overstate flows. ADR 0098's price-level correction removes part of the gap; the
rest is policy.

## Options

1. Pure logit with fitted costs.
2. Admission caps per destination plus a logit allocation among applicants.
3. Endogenous immigration policy from politics.

## Decision

Option 2 now; option 3 in P6, where 0046's immigration salience sets the cap.

- Each destination country has an annual cap Q_d on permanent admissions (monthly Q_d / 12).
  Initial Q_d is the 2000-2005 mean of UN DESA International Migrant Stock net inflows (CC BY
  3.0 IGO); refugee admissions are separate and set by conflict (11.7).
- Applicants: each origin pop's logit share for destination d gives an application flow A_od.
  If sum_o A_od > Q_d, admissions are rationed proportionally by origin, with a bilateral
  preference weight for pairs with existing diaspora (stock share, Beine, Docquier and Ozden
  2011). Rejected applicants stay.
- Irregular migration: a fraction z of rejected applicants migrate anyway at an extra cost; z
  is a new register entry (prior 0 to 0.2, Calibrated against bilateral flow estimates of Abel
  and Cohen 2019).
- Bilateral costs are fitted to Abel and Cohen bilateral flows with caps on, not to stocks.
- Internal migration stays the pure logit.

## Migration

- pop module: per-destination admission account and cap; rationing step after the logit.
- Pack: UN DESA stocks and net inflows, diaspora stocks.

## Tests

- With caps set to infinity, flows equal the pure logit's.
- Admissions never exceed Q_d; rationing shares sum to one.
- 2000-2010 simulated bilateral flows against Abel and Cohen: PPML pseudo-R2 reported.

## Consequences

- International flows are bounded by policy, as observed; wage gaps translate into queue length
  and irregular flows instead of admissions.
- A policy lever exists for scenarios (cap changes) before endogenous policy arrives.
