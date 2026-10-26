# 0031. Structural transformation and informality dynamics

Status: proposed
Date: 2026-10-24
Design sections: 6.3, 7.5, 7.6, 9.6, 15.4
Phase: P2 (sector catch-up), P4 (labour-status transitions)

## Context

Developing economies dominate 2000-2100 population and growth. Two regularities shape their
paths. Manufacturing labour productivity converges unconditionally across countries while
aggregate productivity does not (Rodrik 2013), and many countries reach peak manufacturing
employment at lower incomes than early industrializers ("premature deindustrialization", Rodrik
2016). Informality is large, persistent and tied to regulation, enforcement and firm
productivity (Ulyssea 2018; La Porta and Shleifer 2014). The design has a single catch-up rate
per province and sector driven by common determinants (7.5) and labour-status switching without
a formalization mechanism.

## Options

1. Status quo: common catch-up determinants, exogenous informality shares.
2. Sector-specific catch-up speeds (manufacturing faster than services and agriculture) plus
   formalization transitions that depend on fiscal capacity, regulation cost and firm
   productivity.
3. A full dual-economy model per state.

## Decision

Option 2.

- Catch-up rate lambda in 7.5 gains a sector-class factor (P60): manufacturing, tradable
  services, non-tradable services, agriculture. Priors from the unconditional-convergence
  literature.
- Informal and formal employment and firms: transitions between labour statuses (6.3) and
  between informal and formal firm cells depend on the cost of formality (taxes, regulation)
  against its benefits (credit access, contract enforcement), scaled by enforcement capacity
  from 9.6 (P61). Informal cells have lower productivity and no access to bank credit.
- Validation: sector employment and value-added shares by country (Economic Transformation
  Database, GGDC; licence to audit), ILO informality rates, and the hump shape of manufacturing
  employment in income.

## Migration

- P2: the sector-class factor in the productivity catch-up step.
- P4: labour-status and formalization transitions.
- Register P60, P61.

## Tests

- With all class factors equal to 1, golden hashes equal the status quo.
- Simulated manufacturing productivity shows unconditional convergence while aggregate
  productivity does not, on the 2000-2024 cross-section.
- Raising enforcement capacity in a toy state raises formal employment with the sign in Ulyssea
  (2018).

## Consequences

Development paths gain the two most robust structural-change regularities, and fiscal capacity
reaches growth through formalization.
