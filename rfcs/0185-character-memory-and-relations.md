# 0185. Character memory and relations

Status: proposed
Date: 2026-10-28
Design sections: 10.5, 10.8, 12.2, 23.5; ADRs 0066, 0078, 0132, 0139, 0183
Phase: P6 (2.0-alpha)

## Context

The Dwarf Fortress comparison named leaders with memory as the largest gap. Memory must act
through existing parameters, so it adds texture without new verbs.

## Options

1. No memory; traits are fixed at creation.
2. A small typed memory per person that changes only existing planner parameters.

## Decision

Option 2.

- **Lived experience.** The Malmendier-Nagel declining-weight kernel of ADR 0139 over
  inflation, unemployment, war and crisis years, kept per person, back-filled from real
  history for anyone alive in 2000.
- **Episodes.** The 16 most salient events the person took part in (coup survived, election
  lost, war commanded, purge, exile), each an event id, counterpart, valence and tick. Salience
  decays; a new episode replaces the weakest.
- **Relations.** Typed edges (patron, protégé, ally, rival, betrayed by, purged by), created
  only by logged events.
- **Effects, and no others.** Expectations and hawkishness through the kernel; coalition
  bargaining costs (10.5) between people with a grudge; the chance of joining a coup against
  someone who purged you; risk aversion after a lost war or near-miss crisis.
- **Views.** Dossiers and the legends browser (ADR 0078); alternate lives (ADR 0132).
- About 200 bytes per person; about 150,000 people over a 50-year run.

## Tests

- Memory weights at zero reproduce ADR 0184's nesting hash.
- Episodes and relations are deterministic from the event log.

## Consequences

Leaders carry their histories into choices, and the chronicle gets characters with reasons.
Cost about 3-4 part-time weeks.
