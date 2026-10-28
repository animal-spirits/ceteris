# 0153. Intelligence and fog of war as belief states

Status: proposed
Date: 2026-10-28
Design sections: 11.2, 23.4, 23.5, 23.6, A13; ADRs 0065, 0073, 0145, 0147
Phase: P7 (2.0-beta)

## Context

23.6 gives each state an intelligence service that reduces the noise of A13 on chosen targets, and
23.5 shows enemy formations through reconnaissance. Neither says what a side knows day to day, or
what plans are computed on.

## Options

1. Visibility masks over true state.
2. Per-side belief states updated by sensor search, with plans and forecasts computed on beliefs.

## Decision

Option 2.

- **Belief state** per side about each enemy: estimated positions, identities and strengths of
  formations, front positions, stocks and intent, each with an error that shrinks with contact.
- **Detection** per formation per day: 1 − exp(−Σ effort × sweep(signature, weather, EW) / area).
  Decoys and camouflage lower signature or add false tracks.
- **Sources.** Satellites (a 23.7 milestone), AEW, ISR drones, signals and human intelligence, and
  open-source leakage that grows through the 2010s.
- **Use.** Planners (0147), the battle planner and every forecast run on beliefs. Strategic
  intelligence feeds dossiers (23.6) and narrows the bargaining range of 11.2.
- **Deception** is an order (feints, decoys, radio silence, false concentrations); its success
  depends on the enemy's sensors.
- **Views.** The lab sees truth. In the game, fog strength is a difficulty setting that changes
  information only (23.4); Ironman locks it.

## Migration

- P7 stage W8, in `ceteris-war` (intel).

## Tests

- With fog off, a run reproduces the same run without the intel module exactly.
- With fog on, only decisions change; given the same orders, outcomes are identical.

## Consequences

Surprise and deception become mechanisms. Belief states add memory per side but no new state that
outcomes depend on directly.
