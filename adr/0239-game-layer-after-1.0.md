# 0239. Game layer after 1.0

Status: proposed (owner set the rule, 2026-10-29)
Date: 2026-10-29
Design sections: 2.1, 20, 23.4, 23.7, 23.8; ADRs 0078, 0124, 0129, 0132, 0136, 0140, 0141, 0144, 0161, 0162, 0207, 0238
Phase: none on the 1.0 path; the moved items become 2.0+ work

## Context

The owner set the scope rule on 2026-10-29: "1.0 is pure simulator. Fog of data is a game
mechanic. Game is 2.0+." ADR 0238 applied it to data revisions and the published layer of 0065.
Several other proposed ADRs place game mechanics, meaning objectives, scoring of a player,
challenges, narration and role-play, in P3 or in 1.x minors. Seats and the Steward loop
(0130, 0160, 0203, 0206, 0237) are re-scoped separately, and the wording of the major-version
rule (0136) belongs to that ADR's own thread.

## Options

1. Leave each ADR's phase as written and apply the rule case by case later.
2. Re-scope every game-layer item now, keeping lab tools and data presentation in 1.0.

## Decision

Option 2.

**Moves to 2.0+.**

| ADR | Item |
| --- | --- |
| 0078 | Narration by narrator perspective, the storyteller camera, legends. Chronicle entries with linked causes stay in 1.0 as explainability (0049). |
| 0124 | Diegetic era themes. Cartogram, receipt and atlas stay. |
| 0129 | Veil-of-ignorance scoring (was 1.3). |
| 0132 | All of it: cost screen, detective mode, century relay, timeline library (were 1.3), and the 2.0 items already there. |
| 0140 | Regime-class shape and wording, and the fog halo. Accent and header stay (see 0141 below). |
| 0141 | Letterhead and emblems, the news register by regime and narrator, statistical fog. |
| 0144 | Rare-event codex, New Year front page, seed of the week. Ghost of real history, run names and covers, and postcards stay. |
| 0161 | Ambitions, Pareto histograms against a player, ascension rungs. Policy paths stay as a lab tool once the 0079 surrogate exists (P9 or a 1.x minor). |
| 0162 | Commitments as a Steward verb. Commitments as edits and their AI use stay in P5. |

**Stays in 1.0.** Data presentation of the focused country: the accent and header of 0140;
names in their own script, calendars, CLDR formats, local standards, seasons and night lights of
0141. These read data and never voice a role.

**Rule for later ADRs.** An item whose purpose is play (an objective, a score of a player, a
challenge, a role-voiced text, a collection) is phased 2.0 or later. An item that helps a user
read, edit or compare runs is a lab tool and may sit on the 1.0 path.

## Migration

- Each listed ADR gains an "amended by 0239" note on its status line.
- DESIGN 20 and 23 point here when accepted.

## Tests

None; no state or hash is affected.

## Consequences

The 1.0 path loses about 14-20 part-time weeks of game work (0144 about 6-8, 0161 about 8-10,
parts of 0078, 0132 and 0140), all of it presentation over existing state. 2.0 carries a game
layer built on P6 politics, where objectives and voiced registers have the most to work with.
