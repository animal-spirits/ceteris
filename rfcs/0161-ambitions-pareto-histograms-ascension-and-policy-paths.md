# 0161. Ambitions, Pareto histograms, ascension and policy paths

Status: proposed; amended by ADR 0239 (seats and scoring to 2.0-alpha)
Date: 2026-10-28
Design sections: 23.4, 14.2, 14.4; ADRs 0065, 0079, 0129, 0132
Phase: 2.0-alpha (Steward mode, ADR 0239)

## Context

Section 23.4 defines objectives, the score against the AI baseline and difficulty by information
and frictions. Four mechanisms from other games improve that loop without bending the model: Old
World's ambitions (a few generated goals offered over time), Zachtronics' solution histograms (no
single score, every axis shown against all players), Slay the Spire's Ascension (numbered
difficulty, one modifier per step) and Kerbal Space Program's maneuver nodes (drag a plan, see
the predicted path, then commit). Survey of other games' mechanics (2026-10-28): https://claude.ai/artifact/LcDMgZuczpSefSVeadiZWT

## Options

1. Keep 23.4 as written.
2. Add the four, all as game-layer features over existing machinery.

## Decision

Option 2.

- Generated ambitions: every few game years, offer three objectives built from the seat's data
  (gaps against peers, live situations of 0159), for example "inflation under 10% by 2004". Each
  shows its difficulty as the share of AI-baseline lanes that reach it. A chosen ambition joins the
  objective vector with a weight fixed at choice. No ambition is scripted.
- Pareto histograms: at the end of a scenario, the result is placed on per-axis histograms
  (welfare, debt, emissions, bottom-40 income, veil score of 0129) against the AI-baseline
  ensemble, the player's earlier runs, and runs shared by hash through the timeline library
  (0132). No server.
- Ascension: 23.4 frictions ordered into numbered rungs (reporting lag, statistical fog of 0065,
  revisions, markets that learn the player's rule faster, longer AI planning horizon). A rung
  unlocks when the one below is cleared in Ironman. No rung changes model equations.
- Policy paths: the player drags nodes onto the timeline to draft a lever schedule (a rate path, a
  tariff phase-in); the surrogate of 0079 redraws its fan as nodes move, labelled with its error;
  "commit" turns nodes into timed edits and launches the confirming ensemble.

## Tests

- An ambition's displayed difficulty equals the AI-baseline lane share recomputed from the
  stored lanes.
- Ascension rungs change only observation and frictions listed in 23.4; the lab model hash is the
  same at every rung.

## Consequences

The Steward loop gains direction, replay value and a planning tool, all on existing ensembles.
Cost about 8-10 part-time weeks in total.
