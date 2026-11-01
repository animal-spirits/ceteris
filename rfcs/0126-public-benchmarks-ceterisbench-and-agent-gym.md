# 0126. Public benchmarks: CeterisBench and an agent gym

Status: proposed; amended by ADR 0239 (seats and scoring to 2.0-alpha)
Date: 2026-10-28
Design sections: 12.5, 12.6, 14.4, 15.5, 23.4; ADR 0065
Phase: P9 (CeterisBench), 2.0-alpha (agent gym, ADR 0239)

## Context

The simulator knows the true effect of every intervention, which real data never does. That
makes it a benchmark for causal-inference methods, and, with Steward seats, for AI agents
acting over long horizons. Brainstorm: https://claude.ai/artifact/N9zi5kznSaqvjg9ztw4WMT (ideas
23 and 24).

## Options

1. Status quo.
2. CeterisBench: released corpora of planted natural experiments with known effects.
3. Option 2 plus an agent gym: seats exposed as a benchmark environment, scored against the
   Tier 1a baseline.

## Decision

Option 3.

- CeterisBench: generate natural experiments inside the model (tariff shocks, port closures,
  migration-cost changes), release the reported series with fog of statistics (ADR 0065) and
  the true paired effects under CRN. Tasks: estimate the effect with difference-in-differences,
  synthetic control, shift-share or ML estimators; score bias, interval coverage and RMSE. Each
  release is versioned with its pack and manifest hashes.
- Agent gym: a seat (trade ministry first, central bank after P5) runs for ten simulated years
  under anonymized history (12.5). The agent receives the seat's observations and returns
  actions from the Steward action space (12.6). Score is the paired objective difference
  against the Tier 1a baseline under CRN, with a bootstrap interval (23.4). Episodes are
  deterministic given the agent's actions, so any score can be replayed.

## Migration

- P9: CeterisBench from hindcast-quality runs. P10: agent gym with Steward mode.

## Tests

- Released true effects match a re-run from the published manifests bit for bit.
- An agent that replays the Tier 1a policy scores zero within Monte Carlo error.

## Consequences

The project gains research users who never play it. Benchmarks need versioning discipline: a
model update is a new benchmark version, never a silent change.
