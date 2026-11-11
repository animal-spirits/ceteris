# 0159. Situations as tracked processes

Status: proposed
Date: 2026-10-28
Design sections: 9.2, 9.4, 9.13, 23.8; ADRs 0078, 0049
Phase: P5 (financial situations), P3 (trade situations such as a chokepoint closure)

## Context

Europa Universalis V's situations, Victoria 3's journal entries and Democracy 4's situations give
a long, multi-actor process its own panel with participants, progress and end conditions. In
Democracy 4 situations also carry authored effects, which Ceteris must not have. The event-log v2
plan (Dwarf Fortress path) adds detectors and named arcs, but nothing tracks a process while it
runs. Survey of other games' mechanics (2026-10-28)

## Options

1. Situations with effects, as in Democracy 4.
2. Situations as views only: detectors with hysteresis and a tracking panel.

## Decision

Option 2. A situation never changes state.

- A situation type declares entry and exit predicates on model variables with hysteresis (enter
  at one level, leave at another) and a minimum duration, so it does not flicker.
- First types: peg under pressure, sudden stop, sovereign restructuring, banking crisis,
  commodity boom, chokepoint closure.
- Each live situation has a panel: participants (states, markets, creditors), the variables that
  define it with their entry and exit levels, its duration and a name from the chronicle grammar
  (0078).
- Entry and exit are logged events with causes, so the chronicle and legends cite them.
- Types are data files, so mods add situation types without code.

## Tests

- No situation changes a state hash.
- A series oscillating between the entry and exit levels opens one situation, not several.

## Consequences

Slow crises read as stories with a beginning and an end. Cost about 4-6 part-time weeks after
event-log v2.
