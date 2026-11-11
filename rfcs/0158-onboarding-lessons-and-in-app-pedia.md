# 0158. Onboarding: lessons and in-app pedia

Status: proposed
Date: 2026-10-28
Design sections: 14, 20.2; ADRs 0049, 0078, 0127, 0141
Phase: P3 (pedia, first lessons), extended through 1.0 as mechanisms land

## Context

The specs have no onboarding. Victoria 3 teaches through Lessons, short guided sequences that open
when the player first meets a mechanic, inside the player's own game. Crusader Kings III links
every game term inside a tooltip to its concept entry, and Civilization keeps an encyclopedia one
click away. The parameter wiki of 0127 is a static site outside the app. Survey of other games' mechanics (2026-10-28)

## Options

1. A front-loaded tutorial scenario.
2. Contextual lessons built as small experiments on the user's own world, plus an in-app pedia
   generated from the specs.

## Decision

Option 2.

- Lessons trigger on the first occurrence of a detector event in the user's runs (first price
  spike, first rationing, first default, first capital-flow reversal). Each forks the current
  state, applies one edit (no export ban, half the tariff), runs a short ensemble under CRN and
  walks the user through the diff with the explainer and branch manager. The user can dismiss or
  replay any lesson.
- A lesson is a text file (Fluent, localizable as in 0141) plus an edit list and a horizon. It
  never changes the user's branch; the fork is kept only if the user asks.
- Pedia: terms in tooltips link to concept pages generated from specs/ and adr/ at build time:
  the equation, current parameter values with sources and register ids, and the deciding ADRs.
  The parameter pages of 0127 are the same pages.

## Tests

- Dismissing a lesson leaves the user's branch hash unchanged.
- Every pedia page names a source file and commit; the build fails on a dangling term link.

## Consequences

New users learn the workbench by using it on their own question. Lessons are cheap to add as
mechanisms land. Cost about 4-6 part-time weeks for the frame and the first ten lessons.
