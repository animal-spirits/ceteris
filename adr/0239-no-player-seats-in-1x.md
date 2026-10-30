# 0239. No player seats in 1.x: game mechanics start at 2.0

Status: accepted (owner, 2026-10-29)
Date: 2026-10-29
Design sections: 2.1, 12.6, 21, 23.4, 23.9; ADRs 0126, 0129, 0132, 0136, 0160, 0161, 0162, 0206, 0210, 0237
Phase: 2.0-alpha onward

## Context

The owner stated (2026-10-29) that 1.0 is a pure simulator and that the game is 2.0 and later.
ADR 0136 (accepted) still places P10 as "1.3 Steward for economic levers", DESIGN 23.9 puts
objectives, scoring and Ironman at 1.3, and ADR 0237 puts the Steward and office seats at 1.3.
Several proposed ADRs hang game features on that 1.3 Steward.

## Options

1. Keep 1.3 Steward as the first playable minor.
2. Move every seat and every player-scoring feature to 2.0 or later; 1.x minors stay simulator,
   lab, editor and view work.

## Decision

Option 2. This amends ADR 0136's phase order: P10 becomes "2.0-alpha onward: Steward for
economic levers, then full Steward and Tier 1b in 2.x". The rule that seats are minors still
holds for seats added after 2.0.

Moved from 1.x to 2.0-alpha or later:

- Steward mode (12.6) and the office seats of 0237.
- Objectives, score against the AI baseline and Ironman (23.4).
- Ambitions, Pareto histograms, ascension and policy paths (0161).
- Commitment devices as a player verb (0162); binding a rule's lever stays in P5.
- Veil-of-ignorance scoring (0129) and the 1.3 items of 0132 that score or stake a player.
- The commodity trader and corridor planner (0160), the planned-economy seat (0237) and seat
  placement verbs (0206).
- The agent gym (0126), which exposes a seat; CeterisBench stays at P9.

Stays in 1.x: policy rules and the tape (0210) as the only policy in every state, god-mode edits
of any lever (14.2), event cards, the chronicle and other views, the policy catalog as edits
(23.7), the consistency solver and editors (1.4), and climate (1.1).

Order inside 2.0 follows 0198: the democrat's negotiating verbs ship no later than the
autocrat's levers. The Steward for economic levers lands with 2.0-alpha.

## Migration

- DESIGN 2.1 drops "(later)" for "(2.0)" on Steward; 21 and 23.9 replace 1.3 with 2.0-alpha
  for the moved rows; the 23.9 lead sentence names the 1.4 solver as the first 23.9 addition (climate, 1.1, is
  outside 23.9).
- Phase lines of 0126, 0129, 0132, 0160, 0161, 0162, 0206 and 0237, and the two "1.3" mentions in
  0210, read 2.0-alpha (applied 2026-10-29).

## Tests

- A 1.x build exposes no seat: the app and the CLI offer observer, god mode and batch only.

## Consequences

Every 1.x minor is simulator, lab, editor or view work, so a 1.x result never depends on a
player. The first playable release becomes 2.0-alpha, after P6 starts.
