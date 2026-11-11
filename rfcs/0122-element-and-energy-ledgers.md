# 0122. Element and embodied-energy ledgers

Status: proposed
Date: 2026-10-28
Design sections: 4.6, 7.7, 8.3, 20.1
Phase: P8 (extended profile until it passes ablation)

## Context

Money, goods and people are conserved (4.6). Physical flows are not tracked, so the model
cannot say where the world's copper, lithium or phosphorus goes, or what a sanction does to
material supply. Brainstorm (idea 11).

## Options

1. Status quo.
2. Coefficient ledgers: each good carries per-unit content of a few elements and carriers, and
   the ledgers follow production and trade.

## Decision

Option 2.

- Carriers: carbon, copper, lithium, nickel, phosphorus, embodied primary energy. Coefficients
  per good and sector from a multi-regional material-flow source (EXIOBASE or similar; licence
  to check under ADR 0005).
- Ledgers are computed from goods flows; they add no behaviour. Mass balance per element is
  asserted every tick: extraction plus recycling equals use plus stock change plus waste.
- Views: a world Sankey per carrier, and map modes for extraction, embodied imports and stocks.

## Migration

- P8, with energy and emissions (7.7). Extended profile until it passes ablation (15.7).

## Tests

- Element balances close every tick to the ledger tolerance.
- Embodied-carbon totals match the emissions module (7.7) within the coefficient error.

## Consequences

Physical accounting becomes a second conservation check and a lens for critical-mineral
questions (ADR 0045). The coefficients are static per good, which the UI states.
