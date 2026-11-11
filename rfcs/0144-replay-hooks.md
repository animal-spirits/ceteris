# 0144. Replay hooks

Status: proposed; amended by ADR 0240 (game and flavour items to 2.0+)
Date: 2026-10-28
Design sections: 20, 23.7; ADRs 0049, 0078, 0118, 0124, 0128, 0132
Phase: P3 (ghost line, run names, postcards), 1.x (codex, front page, seed of the week)

## Context

The owner asked what keeps each run unique and players coming back. Achievements and unlocks
fit poorly with a simulation whose goal is counterfactual study. Proposal table "Reasons to come
back"

## Options

1. Achievements and unlocks.
2. Views built on the run log and ensembles, none of which touch state.

## Decision

Option 2.

- Rare-event codex: a cross-run collection of what happened in the player's worlds, each with its
  rarity from ensemble frequency ("Iberian Union, 2031: seen in 1 of your 40 runs"). Replaces
  achievements. Builds on ADR 0078 legends.
- New Year front page: each 1 January, a front page of the year's chronicle in the seat's press
  style (broadsheet, tabloid or state paper, by the media layer of 23.7 and ADR 0140).
- Ghost of real history: a dashed line of what actually happened up to 2024 on every chart, and
  a map toggle, so each divergence shows. Builds on ADR 0118.
- Run names and covers: a deterministic title from the largest events ("The Long Peg,
  2000-2031") and an atlas cover (ADR 0124) in the run library.
- Postcards: export any map view at a date with a caption and the run hash; anyone can open the
  same moment (ADR 0128).
- Seed of the week: a shared start, seat, objective and seed; compare chronicles with other
  players' runs of the same seed through the timeline library (ADR 0132).

## Tests

- None of the views changes a state hash.
- A run's title and cover are identical across machines for the same run hash and app version.

## Consequences

Each run leaves an artefact worth keeping or sharing, and repetition is rewarded by discovery
instead of unlock lists. Cost about 6-8 part-time weeks in total, mostly small views over data
the run already writes.
