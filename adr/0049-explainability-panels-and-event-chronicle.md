# 0049. Explainability panels and event chronicle

Status: proposed
Date: 2026-10-24
Design sections: 14.7, 20.2, 12.3, 11.2
Phase: P3 workbench onward; P6 and P7 for diplomacy and war panels

## Context

The commonest complaints about systemic strategy games are opaque markets and opaque AI
decisions. The engine already computes what is needed to explain outcomes: Shapley
decompositions over interventions and channel shutdowns (14.7), utility breakdowns of state
decisions (12.3) and bargaining ranges (11.2). The design shows explainers per number (20.2)
but not causal answers to "why did this change?", nor a narrative of what happened. Dwarf
Fortress's legends show that an event chronicle generated from simulation state gives narrative
without scripting.

## Options

1. Status quo: explainers per number.
2. Causal panels built on channel shutdowns and Shapley values, bargaining-range tooltips,
   uncertainty bands on all reported estimates, and a chronicle generated from the event log.
3. LLM-written narration in the loop.

## Decision

Option 2.

- "Why did X change?" panels: for a selected variable, province and window, run the
  channel-shutdown set (prices, migration, politics, finance, climate) in the background under
  common random numbers and show each channel's share with intervals.
- Diplomacy and war tooltips show the Fearon range decomposition (power estimate, costs,
  information, commitment).
- Every reported statistic in Steward mode carries the uncertainty of its measuring institution
  (statistical capacity), consistent with the dossier principle. The published series and their
  noise come from ADR 0065; this panel only displays them.
- Chronicle: specified in ADR 0078.
- Excluded: war score, mana and other abstract currencies.

## Migration

- P3: background channel-shutdown jobs and the panel; P6 and P7: tooltips.

## Tests

- Channel shares sum to the total change within the Shapley tolerance.

## Consequences

The model's main analytic tools become its main interface, which is also its strongest
differentiator as a game.
