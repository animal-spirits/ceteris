# 0224. Religious change: affiliation and religiosity

Status: proposed
Date: 2026-10-28
Design sections: 6.1, 6.2, 6.4, 10.5, 16; ADRs 0032, 0055, 0058, 0189, 0207
Phase: P4 (initial shares), P6 (2.0-alpha) dynamics
Amends: ADR 0058

## Context

Pew's study of 2010-2020 finds that most change in the global religious mix came from age
structure and fertility, with switching concentrated in a few regions; Hout, Greeley and Wilde
(2001) attribute most of the US shift from mainline to conservative Protestantism to fertility.
European secularization runs largely through "fuzzy fidelity": affiliation persists while practice
fades, and children then stop affiliating (Voas 2009). Voas and Crockett (2005) find two religious
parents pass on religiosity about half the time and one parent about a quarter; Voas and Chaves
(2016) show US decline is cohort-driven. ADR 0058 adds a conversion hazard and salience but no
transmission side and no religiosity, and its data source (Pew) has unaudited terms. The CIA
Factbook gives national totals without age structure.

## Options

1. Affiliation shares plus a religiosity intensity per pop.
2. Affiliation shares only.

## Decision

Option 1 (owner, 2026-10-28).

- **State.** Affiliation share vector (as 6.1) and religiosity in [0, 1] per pop and age band.
- **Composition.** In P4, pops' religion shares ride along with group-specific fertility, mortality
  and migration; no other change until P6 (ADR 0207).
- **Transmission.** Children take a parental affiliation with retention rising in parental
  religiosity and religious homogamy; the remainder go to no affiliation. Children's religiosity
  forms from parental religiosity and the cohort's secular value score (0055).
- **Adult switching.** ADR 0058's hazard, unchanged, behind switch S20.
- **Secularization.** Religiosity drifts with the 0055 value scores; affiliation is dropped at a
  hazard rising as religiosity falls.
- **State levers.** State religion, religious education, restrictions on minority worship
  (calibrated on the Pew Government Restrictions Index if its terms allow), coerced conversion
  (a forced-assimilation act under ADR 0225 and 0226).
- **Uses.** Religiosity enters desired fertility (6.2), party platforms (10.5), organization
  recruitment (0189) and salience (0058).
- **Parameters.** About six register entries: retention intercept and slopes on religiosity and
  homogamy, the drop hazard, and religiosity drift.

## Migration

- P4: initialize religion shares by age and sex from the Wittgenstein Centre religion estimates
  (CC BY 4.0) instead of national Factbook totals; Factbook stays as a cross-check.
- P6: dynamics, yearly.
- Validation: Wittgenstein Centre and Pew 2010-2020 change by country, European Social Survey
  attendance by cohort, Latinobarometro for Protestant growth. Pew terms are audited before any
  Pew table enters a pack (ADR 0005).

## Tests

- With dynamics off, shares inside pops are constant and aggregate change equals the composition
  effect.
- A closed population with zero switching and full retention keeps its affiliation shares.
- Simulated 2010-2020 change by region within tolerance of Pew and Wittgenstein estimates.

## Consequences

Demographic, generational and conversion channels are separate and each can be switched off,
so a run can say how much of a religious shift came from births and how much from switching.
