# 0118. Contrastive explanations and history's percentile

Status: proposed
Date: 2026-10-28
Design sections: 14.5, 14.6, 20.2; ADRs 0024, 0052
Phase: P3 (contrastive), P9 (percentile strip)

## Context

Ensemble lanes (ADR 0024) hold many versions of the same world. Two questions they can answer
are not in the design: why did an event not happen in this lane when it did in others, and how
probable was what actually happened. Brainstorm:
https://claude.ai/artifact/N9zi5kznSaqvjg9ztw4WMT (ideas 4 and 6).

## Options

1. Status quo: fan charts and event-probability tables (20.2).
2. Contrastive "why not" queries across lanes, and a strip showing where observed history falls
in the ensemble.

## Decision

Option 2.

- Why not: for an event E absent in lane k, select the lanes where E occurred that are nearest
  to k before E's date (distance on a fixed set of national series), and diff k against them:
  shock-tape draws, prices, decisions. Report the shocks that differ most, using the fork-diff
  alignment of the narrative notes.
- History's percentile: treat observed 2000-2024 series as one more lane. A strip under the
  timeline shows the quarterly probability integral transform of reality in the ensemble per
  selected series, and flags quarters below the 2.5th or above the 97.5th percentile. It is a
  view of the scorecard's PIT data (ADR 0052), not a new score.

## Migration

- P3: contrastive query on ensembles. P9: percentile strip once the hindcast exists.

## Tests

- In a constructed ensemble where one shock decides an event, why-not names that shock first.
- The percentile strip reads the same PIT values the scorecard reports.

## Consequences

Users can ask about non-events, which is most of history, and see how lucky or unlucky the real
world was under the model.
