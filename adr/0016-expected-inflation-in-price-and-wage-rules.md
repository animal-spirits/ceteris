# 0016. Expected inflation in price and wage rules

Status: proposed
Date: 2026-10-24
Design sections: 8.2, 8.7, 12.2, 15.3 (V1)

## Context

The price rule (8.2) and the wage rule (8.7) responded only to real excess demand. With
explicit money (A9), nothing anchors the nominal side: the long-run Phillips curve is not
vertical, a permanent change in money growth has permanent real effects, and P5's monetary
policy would act only through demand.

## Options

(1) Keep the rules real and rely on the central bank. (2) Add expected inflation from the
heuristic expectations of 12.2, as Poledna et al. (2023) do for firm prices.

## Decision

Option 2 in both rules, with unit pass-through. Unit pass-through is a homogeneity restriction,
not a parameter.

## Migration

- Price rule: change in ln p = pi^e + kappa1 (D - Q)/Q - kappa2 (S - S\*)/S\*.
- Wage rule: wage growth = pi^e + f(vacancies / unemployed); downward rigidity applies to
  nominal wages.
- In P2, with numeraire money, pi^e is the adaptive expectation of the numeraire price index.
  It stays near zero, but the rules are homogeneous now and P5 needs no rewrite.
- P2 spec: derive the stability region in (kappa1, kappa2) for the linearized market (the rule
  is a proportional-integral controller on the inventory gap), register the step clip, and
  count every tick it binds.

## Tests

- Homogeneity: multiplying every nominal stock and price by lambda at a tick leaves the real
  path unchanged to 1e-9.
- V1 neutrality (from P5): a permanent change in money growth leaves real allocations unchanged
  after the transition.
- Stability: kappa pairs inside and outside the derived region behave as predicted on the
  linearized toy market.

## Consequences

Inflation persistence now comes partly from expectations, which the 12.2 heuristics and survey
targets discipline.
