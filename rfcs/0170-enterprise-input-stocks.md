# 0170. Enterprise input stocks and inventory investment

Status: proposed
Date: 2026-10-28
Design sections: 4.6, 7.3, 8.2, 15.5; ADRs 0091, 0099
Phase: P5 (may start in P3)

## Context

Stocks sit only with merchants at the province market (8.2) and in competitive storage for
commodities (0091). Enterprises buy inputs as they use them, and ADR 0099 drops ICIO's
change-in-inventories column. Inventory adjustment explains a large part of the 2008-09 collapse
in trade (Alessandria, Kaboski and Midrigan 2010), and input stockpiling amplified the 2021-22
shortages. Both are 15.5 cases. The rationing model the design cites (Pichler et al. 2022)
already gives each industry an input inventory with a target in days of use.
Gap audit (2026-10-28)

## Options

1. Status quo: merchant inventories only.
2. Enterprise input stocks with a target cover, restocked by orders.

## Decision

Option 2.

- Each enterprise holds a stock of each input with a target cover in months of planned use,
  set per sector from the ICIO inventory column and published inventory-to-sales ratios. Orders
  equal planned use plus a partial correction of the gap to target (register entry, the
  correction speed).
- Shortfalls are met from the stock before output is rationed, so rationing (8.2) binds later
  and harder.
- Inventory investment is part of final demand again, closing the gap 0099 left. The goods
  invariant of 4.6 counts enterprise stocks.

## Migration

- P5 (or P3 if the supply-chain experiments need it first): a stock array per enterprise and
  input class; the order rule in the production step.
- Pack: ICIO change in inventories and sector target covers.

## Tests

- With target cover zero, results equal the status quo.
- A toy demand drop of 10% produces a larger percentage fall in upstream orders, with the
  amplification of Alessandria, Kaboski and Midrigan.
- Goods conservation holds with stocks every tick.

## Consequences

The bullwhip in supply chains becomes endogenous, which matters for the 2008-09 and 2021-22
cases. Memory grows by one stock per enterprise and input class. Register: 1 entry.
