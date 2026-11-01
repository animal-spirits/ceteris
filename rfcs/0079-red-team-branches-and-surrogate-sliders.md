# 0079. Red-team branches and surrogate sliders

Status: proposed
Date: 2026-10-24
Design sections: 14, 20.3, ADR 0053
Phase: P3 (red-team), P9 (surrogates)

## Context

Two interaction modes from policy simulation fit the branch architecture. Matrix wargames (RAND
and others) let participants propose actions that are adjudicated and logged; here human or
analyst injections can be branch metadata, so they stay reproducible. En-ROADS shows how much
instant feedback helps exploration; full runs take seconds to minutes, but the ensemble batches
already train surrogates (0053) that answer in milliseconds.

## Options

1. Status quo: interventions through the intervention list only.
2. A red-team mode where several participants take turns injecting structured, logged actions
   into a shared branch; and surrogate sliders for fast exploration with a one-click
   confirmation run.

## Decision

Option 2.

- Red-team mode: participants own states or actors; each turn they submit actions from the
  Steward action space; actions become timed interventions on the branch, so any session
  replays exactly.
- Surrogate sliders: for a chosen outcome and a handful of policy levers, a trained surrogate
  shows the predicted outcome distribution as sliders move; a button launches the full ensemble
  to confirm, and the UI shows the surrogate's error.

## Migration

- P3 (red-team, on the branch and intervention machinery); P9 (surrogates from 0053).

## Tests

- A red-team session replays bit for bit from its branch metadata.
- Surrogate predictions on held-out settings fall within their reported error bands.

## Consequences

The lab becomes usable for wargaming and for fast policy exploration without weakening
reproducibility.
