# 0123. Reflexive forecasters and alternative monetary regimes

Status: proposed
Date: 2026-10-28
Design sections: 9.2, 9.13, 12.2, 23.1; ADR 0079
Phase: P5 onward, extended profile

## Context

Expectations are formed by rules (12.2). Real forecasts are made with models and move markets,
which can make them self-fulfilling or self-defeating. Monetary regimes are limited to the 2000
set (9.2), though users ask about the gold standard, currency boards, demurrage or a global
unit. Brainstorm (ideas 12 and 13).

## Options

1. Status quo.
2. Reflexive forecasters: in-world forecasters run a frozen surrogate of the model, and some
   agents act on their forecasts.
3. Alternative regimes as switchable monetary modules.

## Decision

Options 2 and 3, both in the extended profile.

- Reflexive forecasters: a few in-world institutions publish forecasts from the surrogate of
  ADR 0079, frozen per run and keyed by a hash, so determinism holds. Investors and central
  banks put a weight on them in the expectation rule of 9.13. With the weight at zero the model
  is unchanged.
- Monetary modules: gold standard, currency boards, demurrage money, a supranational unit
  (Keynes's bancor). Each implements the regime interface of 9.2 and nests the default regime
  when its parameters vanish.

## Migration

- P5: the regime interface; modules after P5. Reflexive forecasters after the P9 surrogates.

## Tests

- Forecaster weight zero reproduces the base run bit for bit.
- Each regime module at its nesting limit reproduces the default regime.

## Consequences

Goodhart's law and reflexivity become mechanisms to study. Both stay extended until they pass
ablation.
