# 0186. Careers, pools and generated successors

Status: proposed
Date: 2026-10-28
Design sections: 6, 10.3, 10.4, 10.8, 14.4, 23.5; ADRs 0066, 0143, 0146, 0147, 0183
Phase: P6 (2.0-alpha)

## Context

After 2000 every new office holder is generated (ADR 0183). Drawing them from a random table
would put composition by hand; drawing them from the population lets the model's selection
decide who rises.

## Options

1. Generate a successor at the moment an office falls vacant.
2. Pools fed from pop cells, with offices filled by actions and hazards.

## Decision

Option 2.

- **Office graph** per state from the constitution (Comparative Constitutions Project, V-Dem):
  head of state, head of government, portfolios, central bank, party leaderships, service
  chiefs, HQ commanders (ADR 0147), governors.
- **Pools**: party ranks, legislature, civil service, officer corps (ADR 0146), business
  owners, academia, protest leaders (10.8).
- **Moves**: an appointment is an action of the appointer, scored by the loyalty and
  competence trade-off; elections pick party leaders; officers rise by merit or patronage;
  exits follow scandal, health, purge and defeat hazards.
- **Lazy materialisation**: a person is created when they first enter a pool, with a
  biography backdated from their pop cell (birth year, region, schooling, lived history).
- **No quotas**: composition by sex, region and group emerges from the cells feeding each pool
  and from the model's selection, checked against WhoGov and IPU shares.
- **Names**: Wikidata given-name and family-name frequencies by birth region, with Fluent
  grammars for order and form (ADR 0143 tooling). A name matching a living notable person in
  Wikidata is redrawn.
- **Determinism**: character streams keyed by (entity id, purpose), per 14.4, so branches keep
  the same people until their histories differ.

## Tests

- Women's share of cabinets by region and decade within tolerance of WhoGov 2000-2016.
- No generated name matches a living notable person in the shipped Wikidata extract.

## Consequences

Who governs in 2035 follows from who was born, schooled and selected, so elite composition is
a model output. Cost about 4-6 part-time weeks.
