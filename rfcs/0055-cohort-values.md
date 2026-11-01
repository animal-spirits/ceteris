# 0055. Cohort values: secular and emancipative orientations

Status: proposed
Date: 2026-10-24
Design sections: 6.1, 6.4, 10.7, 10.8, 12.2
Phase: P4 (state), P6 (political uses)

## Context

Fertility below replacement, female labour supply, tolerance of minorities, demand for
democracy, religiosity and willingness to protest all co-move with slow changes in values. The
World Values Survey shows national positions on two axes (traditional to secular-rational,
survival to self-expression) moving over decades while cultural zones persist (Inglehart and
Welzel 2005; Welzel 2013). The change is driven mostly by cohort replacement: people form
values in their formative years under the existential security they experienced, then change
little. The design transmits discrete religion and language (Bisin and Verdier 2001) and runs
opinion dynamics (10.7), but has no continuous value state that economic security can move.

## Options

1. Status quo: values implicit in party attachment and ideology.
2. Two continuous value axes per pop and age band, formed in the formative years and drifting
   slowly afterwards, with reversals after large shocks.
3. Per-individual value agents.

## Decision

Option 2, behind switch S19.

- Each pop's age bands carry a secular and an emancipative value score. A cohort entering
  adulthood forms its scores from existential security at ages 15 to 25: income level and
  volatility, conflict exposure and child mortality in its province (P93, the
  security-to-values elasticity, uncertain and ensembled because the causal direction of
  modernization theory is contested).
- Adult drift is slow, with reversals after war, hyperinflation or pandemic. Parents'
  socialization (Bisin-Verdier) pulls children toward family values; schooling and urbanization
  pull toward the national mean.
- Values enter: desired fertility (6.2), the female labour participation norm (0057),
  religiosity and switching (0058), demand for democracy and tolerance in the policy space
  (10.5), and protest thresholds (10.8). Opinion dynamics operate on top, not instead.

## Migration

- P4: two floats per pop and age band, updated annually; P6 adds the political uses.
- Data: World Values Survey and European Values Study for offline calibration only. Their terms
  allow non-profit use and forbid redistribution, so only derived parameters enter the pack,
  and written permission is needed before any commercial release (ADR 0050).

## Tests

- With S19 off, golden hashes are unchanged.
- Simulated national positions 2005-2022 move in the direction of the WVS cultural map while
  zones persist (no convergence).

## Consequences

Long-run fertility, gender and democratic trends get a measured, slow-moving driver, and cohort
replacement becomes a visible force in 2050-2100 runs.
