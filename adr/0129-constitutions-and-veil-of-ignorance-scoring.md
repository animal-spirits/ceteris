# 0129. Constitutional wind tunnel and veil-of-ignorance scoring

Status: proposed
Date: 2026-10-28
Design sections: 10.1, 10.3 to 10.6, 23.3, 23.4
Phase: P6 (constitution modules); 1.3 (veil scoring)

## Context

Institutions are a vector (10.1) and the legislative process is fixed per state. Constitutional
rules (electoral system, federalism, veto players, term limits, emergency powers) are where
institutional design actually happens, and they can be tested across many societies. Scoring
rewards the state's metrics; nothing asks how the player's choices land on a random citizen.
Brainstorm: https://claude.ai/artifact/N9zi5kznSaqvjg9ztw4WMT (ideas 33 and 35).

## Options

1. Status quo.
2. Constitutions as composable rule modules over the P6 process, testable across ensembles of
   societies.
3. Option 2 plus a veil-of-ignorance scoring mode.

## Decision

Option 3.

- Constitution modules: electoral formula, chamber structure, federal transfers and powers,
  veto players, term limits, central-bank independence, emergency powers. A constitution is a
  set of modules applied by edit. The wind tunnel runs one constitution across the real 2000
  states and synthetic worlds (23.3) and reports frequencies of gridlock, backsliding, coups,
  civil war and stable alternation.
- Veil scoring: at the end of a run the player is assigned a person drawn by population weight
  from their state, shown through the lineage view. The score is the expected welfare of a
  random person (Harsanyi) or of the worst-off decile (Rawls), in consumption-equivalent
  welfare (Jones and Klenow 2016).

## Migration

- 1.3: veil scoring with Steward mode. P6: constitution modules; wind tunnel after synthetic
  worlds.

## Tests

- The default modules reproduce each state's 2000 process exactly.
- Veil score with every person identical equals the state average.

## Consequences

Institutional design becomes testable at scale, and players gain an objective that is not GDP.
