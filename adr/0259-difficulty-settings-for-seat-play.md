# 0259. Difficulty settings for seat play

Status: proposed; owner rulings 2026-10-30 noted inline
Date: 2026-10-30
Design sections: 23.4, 23.8; ADRs 0065, 0129, 0153, 0161, 0198, 0199, 0237, 0238, 0239, 0241, 0242, 0258
Phase: 2.0-alpha (with Steward and office seats, ADR 0239)

## Context

Section 23.4 says difficulty "changes information and frictions, never cheats" and names three
levers: observation lag and noise, the AI's planner horizon, and how much of the score breakdown
the player sees. Since then: seats and scoring moved to 2.0+ (0239); runs are calendar-blind and
history is an optional replay set (0241, 0242); the published layer and vintages exist for play
(0065, 0238); 0258 made access to true values a per-run difficulty setting; 0161 proposed
numbered ascension rungs; 0198 kept the score per seat. Nothing yet says what counts as a
cheat, which settings exist, what the AI baseline sees, or how scores compare across settings.

## Options

1. Keep 23.4 and 0161 as written; define settings case by case.
2. Paradox model: AI and player economic modifiers per level.
3. One rule that bounds every setting, a fixed list of settings in five families, presets plus a
   custom vector, and a score that carries the vector.

## Decision

Option 3. Option 2 is rejected: a modifier is a parameter change outside the register (DESIGN
17) and breaks the lab model hash test of 0161.

### The line

A setting may change only:

- **what the seat observes** (source, lag, noise, coverage, what the score panel shows);
- **what the seat may do and when**, as a subset of its legal action space and calendar (0237);
- **which seat, start and objective** the run uses, chosen among legal initial conditions and
  presets;
- **what the interface offers** (forecasts, advice, undo).

It may never change equations, parameters, state, random draws, hazard rates, or the decision
rules or observations of any actor other than the seat. Test of the line: for a fixed log of
every actor's orders, the state hash is the same under every setting. A setting that cannot
pass that test is a model change and needs its own ADR and the register.

Consequences of the line:

- **No economic bonuses** to the player or the AI, no rubber-banding, no luck dial, no hazard
  tilt, no AI that reads true values while the world is fogged.
- **Rival AI strength is not a difficulty setting** (owner, 2026-10-30). Strategic AI horizon and rollouts (23.8)
  stay at the profile default for all actors in every run. A weaker or stronger rival changes
  the world being simulated, not the player's handicap. 23.4 and 23.8 lose "planner horizon"
  as a difficulty lever.
- **Institutional friction is chosen, not dialled.** Veto players, legislatures and lags keep
  their modelled values. The player gets more friction by taking a weaker seat (finance
  minister rather than head of government, a minority government, a low-capacity state) or by
  a player-side order rule from the list below, never by editing the bodies of 0195.
- **The 0161 rung "markets learn the player's rule faster" is dropped.** Market learning is a
  calibrated behaviour; varying it per run changes the model. The other 0161 rungs are in the
  families below.

### The five families

| Family | Setting | Values (default first) |
| --- | --- | --- |
| Information | Data source (0258) | published; published plus true; true only |
| | Revisions (0238) | on; off (first release is final) |
| | Lag | statistical office's own; one release faster; one slower |
| | Coverage | office's own (0258 "not published"); full series list |
| | Intelligence (0153, 0199) | belief states as bought; foreign and loyalty truth shown |
| | Score panel during play (23.4) | provisional score and breakdown; score only; hidden until end |
| Friction (player-side) | Order calendar | any tick; seat's decision points only (0237) |
| | Order budget | none; N discretionary orders per game year |
| | Plan lock | none; lever schedules commit for 12 months |
| Seat and start | Seat (0237) | any registered seat |
| | Start origin | 2000; 2005; 2010 packs (0241) |
| | Replay (0241) | off; shocks family |
| | Objective (23.4, 0161, 0198) | presets; custom vector |
| Time | Pause | free; at decision points and event cards only; none |
| | Speed floor | none; minimum game speed while unpaused |
| Assists | Forecast fans (0079, 0161 policy paths) | on; off |
| | Seat AI advice ("the baseline would do X") | off; on |
| | Explain panels (2.5) | on; off |
| | Branch, undo and god-mode (14.2) | off under Ironman; on in sandbox |
| | Mandate loss (0198) | continue as successor or opposition; run ends |

Replay in seat play is the shocks family only, plus the policy family for other states. The
seat's own state never runs its historical policy tape: the player holds those levers. A
replay run is a foresight setting: the player may know 2008 is coming and the baseline does not.

### The baseline mirrors the seat (owner, 2026-10-30)

The AI baseline of 23.4 is the profile-default AI holding the seat under the player's own
information and friction settings: it reads the same sources, lags, coverage and intelligence,
and obeys the same order calendar, budget and plan lock. Seat, origin, objective and seeds
(common random numbers) are the player's. Time and assist settings do not apply to an AI. The
score therefore measures decision skill on equal terms, and one baseline ensemble serves every
run with the same (pack, seat, origin, replay, information, friction) vector. True values reach
the baseline only when the player has them on; no other actor reads them. Replay stays the one
asymmetry: the baseline does not know history.

### Presets, custom, ascension

- **Presets.** Four named settings vectors. Guided: published plus true (0258), intelligence
  truth, advice on, free pause, continue after mandate loss. Standard (default): every default
  in the table. Hard: Standard plus forecast fans off, score hidden until the end, pause at
  decision points, Ironman. Real time: Hard plus order calendar, plan lock, no pause, speed
  floor.
- **Custom.** Any vector from the table. The run record and every score carry the full vector,
  the seed list and the pack hash (0258).
- **Ascension (0161).** Rungs are an ordered path through the same table, one setting per rung,
  unlocked by clearing the rung below under Ironman. Rungs add no settings of their own.

### Scores across settings

Scores compare only within an equal settings vector, with replay and origin part of the vector.
Each vector gets a measured difficulty: the mean score, against the baseline, of the scripted
players of the 0198 audit under that vector, computed in the same audit. The number is shown
beside the preset name; it is measured, never asserted, and no setting is tuned to hit it.
Histograms of 0161 filter by vector by default.

## Tests

- For a fixed order log, the state hash is identical under every preset and under a random
  custom vector.
- The baseline ensemble's hash depends only on the information and friction entries of the
  vector: changing a time or assist setting leaves it unchanged.
- With true values on, no actor other than the seat reads a true-source observation (an
  observation log check over one decade).
- Every score record deserializes with its settings vector; scores with unequal vectors are not
  ranked together.

## Consequences

- 23.4's difficulty paragraph is replaced by a pointer here; 23.8 drops "AI difficulty
  settings". 0161's ascension text points here and loses the market-learning rung. 0153's
  "fog strength is a difficulty setting" reads "the intelligence setting of 0259".
- Easy play costs nothing in fidelity: the world is the same; only the player's view, hands and
  help change.
- Players who want harder rivals get them only if the strategic AI is improved for everyone.
- A baseline ensemble is computed per distinct information and friction vector, so presets
  share cached baselines and a custom vector may need a fresh one before its score is final.
- Since the baseline faces the same handicaps, a hard setting lowers the expected score only
  where the player copes with it worse than the AI; the measured difficulty reports that.
- The order budget and plan lock are game rules with no real-world analogue; they are labelled
  as such in the settings screen.
