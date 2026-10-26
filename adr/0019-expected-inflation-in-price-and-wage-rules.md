# 0019. Expected inflation in price and wage rules

Status: accepted (owner, 2026-10-24, PR 1); implemented on p2-econ (MIGRATION Task 6)
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

## Mapping to P2 as built

- Price rule: `ceteris-trade` market.rs `reprice` gains pi^e; wage rule: `ceteris-econ`
  labour.rs `wage_step` gains pi^e, and downward rigidity applies when the nominal step is
  negative.
- pi^e is the adaptive expectation of the world numeraire price index (P2 has one currency area,
  S04 adaptive only). Its gain is a new register entry.
- The stability region in (kappa1, kappa2) is derived for the linearized origin market of ADR
  0016 and checked on the fixture; it replaces the measured growth factors behind the 2026-10-24
  owner decision (kappa1 0.02, kappa2 0). Outcome: at kappa1 0.02 every P45 gain in the prior
  broke V1, so the owner set kappa1 0.05 and gain 0.006, kappa2 0 unchanged (2026-10-24,
  MIGRATION Task 6).
- P03c (step clip) is already registered; runs count every tick it binds, per market, in the run
  export.
- Register clash: rev 180 assigns P37 to the consumption rule and P38 to the putty-clay
  elasticity, which P2's spec used for kappa_w and delta_w. P2's entries move to the next free
  IDs (P37 to P41 become P39 to P43), in code doc comments and specs alike.

## Consequences

Inflation persistence now comes partly from expectations, which the 12.2 heuristics and survey
targets discipline.
