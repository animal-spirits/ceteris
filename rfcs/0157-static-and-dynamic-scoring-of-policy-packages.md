# 0157. Static and dynamic scoring of policy packages

Status: proposed
Date: 2026-10-28
Design sections: 8.5, 9.6, 14.2, 14.4, 14.5, 23.7; ADR 0079
Phase: P5 (fiscal instruments); tariff packages from P3

## Context

Budget games (American Public Media's Budget Hero, the CRFB Debt Fixer) give the player policy
cards with official costs; Rule the Waves shows every trade-off live while a ship is designed
within a budget. Real fiscal practice separates the conventional score (cost at current
quantities) from the dynamic score (after behaviour and general equilibrium respond). The gap
between them is what Ceteris exists to show. Survey of other games' mechanics (2026-10-28)

## Options

1. Edit levers one at a time, as now.
2. A package designer with a live static score and an on-demand dynamic score.

## Decision

Option 2, in lab and extended profiles.

- A package is a named set of edits to taxes, tariffs, transfers and spending lines (14.2).
- While editing, the static score updates live: revenue and spending change at current
  quantities and prices, and incidence per pop group (skill group, income decile, province), with
  no behavioural response. This is arithmetic on the current state, not a run.
- "Score dynamically" runs the package against the parent branch under CRN (14.4) and reports
  paired effects with intervals (14.5): revenue, debt, output, prices and incidence.
- The report puts static and dynamic side by side and attributes the difference to channels
  (0049 channel shutdown).
- Surrogate sliders (0079) may preview the dynamic score while editing, labelled with their
  error.
- A package saves as an ordinary edit list, so it can be shared, rebased or cherry-picked (0116).

## Tests

- A package of zero edits has a static score of zero and reproduces the parent bit for bit.
- The static score of a single tax-rate change equals rate change times the current base.

## Consequences

The most common real policy question (what will this cost, and who pays) gets a direct tool, and
the distance between accounting and equilibrium becomes visible. Cost about 6-8 part-time weeks
on top of P5.
