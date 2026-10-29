# 0240. Narration, chrome, flavour and replay hooks after 1.0

Status: proposed (owner chose the scope, 2026-10-29)
Date: 2026-10-29
Design sections: 20, 23.7, 23.8; ADRs 0049, 0078, 0124, 0137, 0140, 0141, 0144, 0207, 0238, 0239
Phase: none on the 1.0 path; the moved items become 2.0+ work

## Context

The owner set the scope rule on 2026-10-29: "1.0 is pure simulator. Fog of data is a game
mechanic. Game is 2.0+." ADR 0238 applied it to data revisions and the published layer of 0065,
and ADR 0239 to seats, scoring, ambitions, commitments as a verb, veil scoring, player stakes and
the agent gym. Presentation ADRs that exist for play or role-play still sit in P3 or 1.x: chronicle
narration, era themes, seat chrome, country flavour and replay hooks.

## Options

1. Keep data presentation of the focused country (accent, local scripts, calendars, CLDR formats)
   in 1.0 and move only role-voiced items.
2. Move all chrome and flavour with the other game items; 1.0 has one neutral chrome.
3. Keep 0140 and 0141 in P3 as planned.

## Decision

Option 2 (owner, 2026-10-29).

**Moves to 2.0+.**

| ADR | Item |
| --- | --- |
| 0078 | Narration by narrator perspective, the storyteller camera, legends. Chronicle entries with linked causes stay in 1.0 as explainability (0049). |
| 0124 | Diegetic era themes. Cartogram, receipt and atlas stay. |
| 0140 | All of it: accent, header, regime-class shape and wording, fog halo. 1.0 uses the neutral chrome of 0137. |
| 0141 | All of it: names in local script, second calendars, CLDR local formats and local standards, seasons and night-lights glow, letterhead, news register, party colours, regional music. 1.0 uses English names and one number and date format per UI language (0220). |
| 0144 | Rare-event codex, New Year front page, seed of the week. Ghost of real history, run names and covers, and postcards stay as lab tools. |

**Rule for later ADRs.** An item whose purpose is play or flavour (an objective, a score of a
player, a challenge, a role-voiced text, a country-specific skin, a collection) is phased 2.0 or
later. An item that helps a user read, edit or compare runs is a lab tool and may sit on the 1.0
path.

## Migration

- Each listed ADR gains an "amended by 0240" note on its status line.
- DESIGN 20 and 23 point here when accepted, alongside the 0239 edits.

## Tests

None; no state or hash is affected.

## Consequences

The 1.0 path loses the P3 chrome work of 0140 (about 1-2 part-time weeks), the 0141 flavour data
and views, the 1.x hooks of 0144, and the narration of 0078. 1.0 looks the same for every country.
2.0 gets these on top of P6 politics, where regime registers and voiced text have state to read.
