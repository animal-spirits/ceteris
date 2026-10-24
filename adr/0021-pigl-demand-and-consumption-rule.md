# 0021. PIGL demand and a household consumption rule

Status: accepted (owner, 2026-10-24, PR 1); implemented on p2-econ (MIGRATION Task 7)
Date: 2026-10-24
Design sections: 8.6, 17.1 (P06, P07, P37), 17.2 (S03), 19.4

## Context

Non-homothetic CES (Comin, Lashkari and Mestieri 2021) has no closed form: each pop needs a
root-find for its utility index and quadrature over its lognormal income, which contradicts the
closed-form demand directive of 19.4. The design also had no rule splitting income between
spending and saving, though MPCs are a target and this is the main channel through which fiscal
and monetary policy reach demand.

## Options

(1) Non-homothetic CES with Newton and Gauss-Hermite quadrature. (2) PIGL in the multi-sector
form of Alder, Boppart and Muller (2022), which aggregates exactly through E\[y^(1-epsilon)\],
closed form under lognormal income. (3) Stone-Geary.

## Decision

Option 2 by default, option 1 as S03. Total spending follows the buffer-stock rule of 8.6
(P37).

## Migration

- Replace the category-demand kernel with PIGL shares computed from each pop's income moments
  (mu, sigma^2).
- At initialization, back out the PIGL category weights from 2000 budget shares by exact
  inversion, so they are data-determined. P06 and P07 become the PIGL price and Engel
  parameters, with the same literature anchors.
- Add the consumption rule with m = 1 and lambda = 0 in P2 (spend current income); its
  parameters switch on in P5.

## Tests

- A pop's aggregate demand equals Monte Carlo integration over its lognormal income to 1e-6.
- Adding-up: category spending sums to total spending exactly.
- Engel curves across the 2000 cross-section: the food share falls and the services share rises
  with income.

## Mapping to P2 as built

P2 used homothetic CES over the 10 goods (`ceteris-econ` demand.rs), an approved spec deviation
that this ADR reverses. MIGRATION Task 7 implements it as follows (controller ruling,
2026-10-24, which replaces the earlier agent default of PIGL over the 10 goods).

- Form: Alder, Boppart and Mueller (2022), PIGL as Special Case I of their eq. (12) with price
  functions (19) and (21) and shares (22), transcribed with equation and page numbers in the
  econ SPEC, Final demand. Its price parameters are three (sigma_B, phi, gamma) and its Engel
  parameter one (eps), so P06 and P07 cannot carry them: P46 (sigma_B, phi, gamma, with the
  Dtilde weights theta) and P47 (eps) are added. P06 stays the goods substitution within a
  category; P07 stays the S03 row.
- Categories: the paper's three, A (goods 0, 3), M (goods 1, 2, 4, 5, 6), S (goods 7, 8, 9),
  with a CES over goods within each at P06, weights from the 2000 within-category shares.
  Crude oil, coal and utilities have no clean counterpart in the paper.
- Parameter sources: theta (0.634, 0.082, 0.284), omega_A = 0, sigma_B 0.26, gamma 0.71, eps
  0.71 and phi 0 from the pooled PIGL column (Tables 3 and A3, column 3). phi = 0 is the
  continuous limit of the Dtilde aggregator, linear in prices.
- Per country, exact inversion at prices 1 and base spending per person: z = Omega_A /
  theta_A sets the country's scale lambda = nu / kappa, omega_M and omega_S follow from the
  category shares; a negative weight stops the load, never clipped.
- Each pop's income is lognormal: mean its money per person, sigma = sqrt(2) Phi^(-1)((G + 1)
  / 2) from its country's 2000 Gini (`Countries.gini`; pack column in PLAN Task 15, WDI
  SI.POV.GINI or WIID). Pop demand is the share equation integrated in closed form through
  E[y^(1 - eps)]; tests use 64-node Gauss-Hermite quadrature rather than Monte Carlo.
- Spend-all (m = 1, lambda = 0) is P2's current rule, so the consumption rule adds no behaviour
  yet.
- The GE reference mirrors PIGL demand per pop, with income by effective labour share and
  rebates by population as the Rust world pays them.

## Consequences

Demand stays closed form and cheap. Structural change runs through PIGL's Engel parameter
rather than per-category income elasticities.
