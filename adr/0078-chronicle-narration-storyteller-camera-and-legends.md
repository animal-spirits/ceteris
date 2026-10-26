# 0078. Chronicle narration, storyteller camera and legends

Status: proposed
Date: 2026-10-24
Design sections: 20, ADR 0049, ADR 0066
Phase: P3 onward

## Context

ADR 0049 adds a template-based event chronicle. Three ideas from games make it richer without
touching causal state. Caves of Qud generates history events first and rationalizes them
afterwards with a grammar (Grinblat and Bucklew 2017); here causes are real (Shapley
attributions), so the same trick can write narration from them, including contested in-world
accounts. RimWorld's storyteller paces events like Left 4 Dead's AI director; injecting events
would break validity, but choosing what to surface and when does not. Dwarf Fortress's legends
mode is a browsable history of every generated person and event. Players read meaning into
sparse cues, so terse chronicles work better than cinematic ones.

## Options

1. Template chronicle only (0049).
2. Narration generated from attributed causes with in-world perspectives; a storyteller that
   selects and paces what the UI surfaces; a legends browser for generated leaders, officers
   and dissidents.

## Decision

Option 2, all in the presentation layer:

- Chronicle entries (moved here from ADR 0049): discrete events (laws, crises, wars, records)
  become dated entries with their causes linked. LLM-written flavour text may be added later
  only as cached presentation, never as input to the simulation, so determinism holds.
- Narration: each chronicle entry is written by a grammar from its event, its top attributed
  causes and the narrator's perspective (state media, opposition, foreign press), so the same
  event can carry competing accounts. Deterministic from the event log and seed.
- Storyteller camera: a selector ranks events by salience, surprise relative to the ensemble,
  and the player's focus, and decides what to surface, follow and zoom to; personas differ in
  pacing. It never changes simulation state.
- Legends: a queryable history of generated leaders (0066), officers and dissidents, their
  careers and the events they touched.

## Migration

- P3: narration grammar and storyteller selector on the event log; P6 and P7: legends as
  leaders and officers arrive.

## Tests

- Narration and storyteller output are deterministic from the event log and seed; switching
  personas changes no state hash.

## Consequences

The game gets narrative depth with zero risk to the research model.
