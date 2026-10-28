# 0132. Stakes, memory and community play

Status: proposed
Date: 2026-10-28
Design sections: 14.4, 14.5, 12.4, 23.4, 23.5, 23.8; ADRs 0065, 0078
Phase: 1.3 (cost screen, detective mode, relay, library); 2.0 (thirteen days, ghosts, alternate
lives)

## Context

The game notes cover ironman, post-mortems and failure as content. Seven additions use
determinism and paired counterfactuals for stakes and community play. Brainstorm:
https://claude.ai/artifact/N9zi5kznSaqvjg9ztw4WMT (ideas 37 and 42 to 47).

## Options

1. Status quo.
2. Seven features, each a view, a pacing mode or a use of session files.

## Decision

Option 2.

- Cost screen: the post-mortem reports deaths, displaced people and poverty-years attributable
  to the player, as paired differences against the AI baseline under CRN (14.4, 14.5), with
  intervals and with lives saved counted as well.
- Thirteen days: rare crises (nuclear standoffs, bank runs, coups) switch to real-time pacing
  on the daily sub-tick (23.5), with wall-clock deadlines on event cards (23.8).
- Ghost rivals: an imitation-learned policy (12.4) trained on a player's past session files
  plays as an AI leader in later games.
- Alternate lives: the legends browser (ADR 0078) shows a character's careers in sibling
  branches, aligned by the entity-ID rule of 14.4.
- Detective mode: a world in 2030 with reported statistics (ADR 0065); the player infers the
  hidden fork edit (what, where, when) and is scored on accuracy.
- Century relay: a campaign passed between players a decade at a time; each handoff is verified
  by state hash and recorded in the chronicle.
- Timeline library: a curated index of community branches with chronicles, tags and parents,
  shared by run hash (ADR 0128).

## Migration

- 1.3: cost screen, detective mode, relay, library. 2.0: thirteen days, ghosts, alternate
  lives.

## Tests

- The cost screen of a player who replays the AI baseline reports zero within Monte Carlo
  error.
- A relay handoff with a modified session file fails hash verification.

## Consequences

The game shows the human cost of decisions honestly and gains formats that need no servers.
