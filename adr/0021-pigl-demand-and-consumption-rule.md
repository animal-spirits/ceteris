# 0021. PIGL demand and a household consumption rule

Status: accepted (owner, 2026-10-24, PR 1); implementation pending on `p2-econ`
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

P2 uses homothetic CES over the 10 goods (`ceteris-econ` demand.rs), an approved spec deviation
that this ADR reverses.

- Categories are the 10 P2 goods (agent default): PIGL shares per good, weights backed out from
  each country's 2000 final demand shares by exact inversion.
- Each pop's income is lognormal: mu from its mean income, sigma^2 from its country's 2000 Gini
  (sigma = sqrt(2) Phi^(-1)((G + 1) / 2)), a new pack column with its own source row.
- P06 and P07 become the PIGL price and Engel parameters; spend-all (m = 1, lambda = 0) is P2's
  current rule, so the consumption rule adds no behaviour yet.
- The GE reference mirrors PIGL demand.

## Consequences

Demand stays closed form and cheap. Structural change runs through PIGL's Engel parameter
rather than per-category income elasticities.
