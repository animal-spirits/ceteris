# 0205. Construction capacity as a priced bottleneck, with priority

Status: proposed
Date: 2026-10-28
Design sections: 7.1, 7.3, 8.2, 8.7, 17; ADRs 0020, 0042, 0075, 0086, 0105, 0122, 0174, 0204
Phase: P5

## Context

ADRs 0075 and 0086 limit repair and reconstruction by "construction capacity and crews" (P128,
P141) without defining it. Victoria 3 makes construction a sector that turns goods into points;
HOI4 uses civilian factories. Ceteris already has construction as an ICIO sector with its own
labour, inputs and province price.

Design note: https://claude.ai/artifact/XmuftWfuBvehjp3ou7SYYd

## Options

1. A separate construction-points pool per state.
2. The construction sector's priced output and labour are the capacity.

## Decision

Option 2 (owner, 28 Oct 2026).

- Structures and dwellings draw mainly on the construction sector, whose output is local: high
  frictions in the trade kernel, with imports of construction services only up to P141 (0086).
  Machinery and ICT draw on their own sectors, which trade normally.
- When orders exceed what the sector can deliver, the stock-flow price rule (8.2) raises the
  construction price and within-tick rationing (0020) slows project stages (0204).
- Construction labour is a skill-group market (0105), so a boom raises construction wages and pulls
  temporary migrants (0174). Construction capacity itself grows through 0204's pipeline.
- **Priority** when rationing binds: reconstruction backlog (0086) and infrastructure repair (0075)
  first, then state and SOE projects, then private, each by willingness to pay within its tier.
  The tiers are a rationing priority in the sense of 0020's S11, with proportional as the
  alternative setting.
- Cement and steel in structures enter 0122's ledgers when that ADR lands.

## Migration

- P5, in `ceteris-econ`, with 0204. No new pack data beyond the ICIO construction sector.

## Tests

- With priority off (proportional), results equal plain 0020 rationing.
- A toy stimulus at full construction capacity raises construction prices and delays completions;
  with slack capacity it does neither.
- A reconstruction backlog delays a private project in the same province, not the reverse.
- China 2009 (via 0171's lever) and Spain and Ireland 2007-2012 construction paths within wide
  bands (P9).

## Consequences

Stimulus can hit a capacity wall, reconstruction competes with new building, and construction costs
are outcomes rather than constants. No construction points exist.
