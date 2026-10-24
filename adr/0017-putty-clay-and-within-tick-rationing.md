# 0017. Putty-clay intermediates and within-tick rationing

Status: proposed
Date: 2026-10-24
Design sections: 7.2, 7.4, 8.2, 17.1 (P38), 17.2 (S09, S11), 19.4

## Context

Leontief intermediates fixed at 2000 ICIO coefficients are right for months and wrong for
decades: an open-ended run would carry the 2000 input-output structure to 2150. Separately,
shortages propagated one input-output layer per month (19.4), a timing artifact rather than a
model of supply chains.

## Options

(1) Fixed Leontief. (2) CES intermediates with one elasticity at all horizons. (3) Putty-clay:
coefficients chosen per capacity vintage at investment time, Leontief once installed.

## Decision

Option 3, with option 1 kept as S09. Rationing follows Pichler et al. (2022) within the tick:
priority by default, proportional and mixed as S11.

## Migration

- Enterprise state holds a capacity-weighted coefficient vector aggregated over vintages; input
  purchases read it instead of the national ICIO row.
- P2 has one vintage (2000 ICIO), so the vintage structure leaves P2 output unchanged; the
  coefficient choice lands with investment (7.4).
- Replace one-layer-per-tick shortage propagation with the within-tick rationing pass, using
  inventories as the buffer.

## Tests

- With one vintage, golden hashes equal S09 (fixed Leontief) bit for bit.
- In a toy supply chain, an upstream shortage reaches downstream output within the tick, and
  the priority order is respected.

## Consequences

The rationing change alters P2 output; the vintage change does not until investment exists. P38
enters the register.
