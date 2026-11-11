# 0133. Other worlds: Earth variants, importers, LLM-drafted packs, nested zoom

Status: proposed
Date: 2026-10-28
Design sections: 2.5 (principle 2), 23.2, 23.3; ADRs 0114, 0115
Phase: 2.x (variants, importers, zoom); 3.x (LLM-drafted packs)

## Context

Synthetic worlds (23.3) generate geography from noise. Four cheaper or more targeted routes to
other worlds reuse the consistency solver (23.2) and the cell partitions of ADR 0115.
Brainstorm (ideas 51, 56, 57 and 58).

## Options

1. Status quo: real 2000 packs and synthetic worlds.
2. Four authoring routes, all extended profile, all through the solver.

## Decision

Option 2.

- Earth variants: real Earth with one geological or climatic edit (Ice Age sea level, a green
  Sahara, no Isthmus of Panama), solved to a consistent economy. Everything not edited stays
  real data.
- Importers: maps from fiction or other games (a strategy-game save, a fantasy atlas) become
  terrain, settlements and borders; the solver infers an economy, and every value is tagged by
  how it was imputed for the data-quality map mode.
- LLM-drafted packs: for settings with only text sources (the Ottoman Empire in 1850), an LLM
  drafts pack values from cited sources; each value is tagged "drafted from source X" and
  frozen when the pack is built, so runs stay deterministic.
- Nested zoom: one country at municipal or county grain inside the world at province grain, as
  a finer partition of the same cells (ADR 0115), with exact aggregation across the seam.

## Migration

- After the solver (1.4): variants and importers. 2.x: nested zoom. 3.x: LLM-drafted packs.

## Tests

- A null variant returns the real pack bit for bit.
- Aggregating a nested-zoom country to province grain reproduces the province pack's totals.

## Consequences

Users get other worlds without the full generator, and every imputed number stays labelled.
