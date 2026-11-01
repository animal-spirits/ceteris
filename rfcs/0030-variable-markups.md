# 0030. Variable markups within cells

Status: proposed
Date: 2026-10-24
Design sections: 7.2, 8.2, 8.8, 15.7, 17
Phase: P2, behind a switch; kept only if it passes ablation

## Context

ADR 0026 gives each cell a Pareto distribution of firm productivity with CES varieties, which
implies constant markups. Variable markups explain incomplete pass-through of cost and
exchange-rate shocks and pricing-to-market: large exporters absorb exchange-rate moves in their
markups (Amiti, Itskhoki and Konings 2019), and Kimball demand reproduces the small and gradual
pass-through in micro price data (Klenow and Willis 2016). The aggregate markup trend (De
Loecker, Eeckhout and Unger 2020) moves the labour share. Section 8.8's exchange-rate
pass-through target and 15.7's micro price facts depend on this.

## Options

1. Constant CES markups (status quo after 0026).
2. Kimball demand within cells: the markup falls with relative price, so pass-through is
   incomplete and larger firms have higher markups.
3. Atkeson-Burstein oligopoly in each cell: markups depend on market shares; needs explicit top
   firms.

## Decision

Option 2 behind switch S14 (CES or Kimball), with one superelasticity parameter per sector
class (P59). With Pareto productivity, Kimball demand keeps closed-form cell aggregates up to a
one-dimensional integral per cell, computed by fixed quadrature. It is kept only if it improves
holdout skill on pass-through and price-setting moments (15.7 ablation rule); otherwise it
moves to the extended profile.

## Migration

- ceteris-econ: the cell's price index and markup become functions of the cell's relative
  price, evaluated with fixed quadrature nodes (deterministic).
- Validation targets: exchange-rate pass-through into import prices by invoicing currency;
  markup dispersion by firm size as published aggregates (OECD MultiProd style); the published
  aggregate markup series as a pattern only, since its firm-level source (Compustat) cannot be
  bundled.

## Tests

- With S14 = CES, golden hashes equal 0026's path.
- Pass-through of a toy exchange-rate shock falls with the superelasticity, matching the
  Kimball closed form at small shocks.

## Consequences

One parameter class and a quadrature per cell. Pass-through and labour-share dynamics gain a
mechanism; if ablation rejects it, nothing else depends on it.
