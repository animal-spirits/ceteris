# 0058. Religion switching and identity salience

Status: proposed
Date: 2026-10-24
Design sections: 6.4, 10.5, 10.7, 11.2, 11.6
Phase: P4 (switching), P6 (salience)

## Context

Religious change since 2000 is not only intergenerational transmission. Pentecostal and
charismatic Christianity grew fast in sub-Saharan Africa and Latin America through conversion,
especially among recent urban migrants where public services are weak; Western Europe
secularized; fundamentalist mobilization rose in places with insecurity and repression.
Supply-side "religious market" explanations (Iannaccone) are one contested account. Identity
salience, how much national, ethnic or religious identity matters to political choices, moves
with external threat, discrimination and elite cueing, and mediates rally effects, secession
support, irredentism and war mobilization.

## Options

1. Status quo: Bisin-Verdier transmission only; identities fixed in weight.
2. A conversion hazard between religion categories, and an identity-salience vector per pop
   updated by events.

## Decision

Option 2.

- Conversion hazard per (province, group), raised by recent rural-to-urban migration, low
  public-service coverage and network exposure, lowered by secular values (0055) (P99). Behind
  switch S20 as one lane, since the religious-market model is contested; with S20 off only
  transmission runs.
- Identity salience per pop over national, ethnic, religious and supranational identities;
  raised by war, attacks, visible discrimination and state media cueing, decaying otherwise
  (P100). Salience weights the corresponding dimensions of the policy space (10.5), secession
  support and war support.

## Migration

- P4 (switching) and P6 (salience).
- Data: Pew religious composition estimates (terms to audit), national censuses; World
  Christian Database is subscription-only and is excluded from packs.

## Tests

- With S20 off and salience constant, results equal the status quo.
- Simulated religious composition 2000-2020 by region within tolerance, including Pentecostal
  growth where data exist.

## Consequences

Religious and national identity become dynamic, which secession, war support and populism
(0046) depend on.
