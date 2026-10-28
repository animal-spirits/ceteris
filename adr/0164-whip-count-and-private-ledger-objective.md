# 0164. Whip count and private ledger objective

Status: proposed
Date: 2026-10-28
Design sections: 10.1, 10.6, 10.7, 23.4; ADRs 0041, 0129, 0130
Phase: P6 (2.0-alpha)

## Context

Frostpunk 2 shows how each faction will vote before a council vote and lets the player trade
promises to flip them; Suzerain builds a presidency on such bargains. Tropico gives the ruler a
private Swiss account scored apart from the island's welfare. Both fit Ceteris once politics
moves in 2.0: the legislative process of 10.6 already produces votes, and corruption (10.1) and
profit shifting (0041) already move private wealth offshore. Survey of other games' mechanics (2026-10-28): https://claude.ai/artifact/LcDMgZuczpSefSVeadiZWT

## Options

1. Leave both to the existing panels and objectives.
2. A whip-count view with recorded promises, and a private-ledger objective preset.

## Decision

Option 2.

- Whip count: for a bill in 10.6, a projected count by party and interest group with intervals
  from the ensemble, and for each group what would move it (its payoff terms from the policy
  model). Promises are recorded edits that commit the government to a later action; breaking one
  lowers that group's trust (10.7). The coalition-partner seat (0130) uses the same view.
- Private ledger: an objective preset that scores the ruling coalition's offshore wealth (0041,
  10.1) next to veil-of-ignorance welfare (0129). The post-mortem shows both.

## Tests

- The whip count never changes a state hash; only promise edits do.
- With no promises, a bill's vote matches the same tick in a branch without the view.

## Consequences

Legislation becomes a negotiation with visible stakes, and a kleptocratic run can be scored
honestly against the welfare it costs. Cost about 4-6 part-time weeks after P6.
