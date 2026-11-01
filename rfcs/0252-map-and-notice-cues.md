# 0252. Map and notice cues borrowed from grand strategy games

Status: accepted (owner, 2026-10-30)
Date: 2026-10-30
Design sections: 19.1 (Map UI), 20, 23.8; amends ADRs 0137 and 0244; ADRs 0049, 0118, 0188, 0209, 0232, 0240, 0241, 0242, 0245, 0250
Phase: P3 (app shell and inspector); the 2.0+ backlog lands with war and seats

## Context

The owner asked which small visual cues of the Paradox games Ceteris can take, as patterns and
not assets. A survey of 28 cues (map, notices, tooltips, sound, war, game layer), deduped
against the art page, the instrument-charm register and the borrowed-mechanics note, is at
`research/paradox-cues-2026-10-30.html` in the project files. Each cue was tagged by the rule of
ADR 0240: a cue that helps a user read, edit or compare runs may sit on the 1.0 path; flavour,
voice and judgement wait for 2.0+.

## Options

1. A cut of seven cheap cues (selection answers, breathing edge, event pins, visible pause,
   coalesced notices, direction colour) plus semantic zoom.
2. Every 1.0 cue, about 8-12 part-time weeks.
3. Semantic zoom alone.

## Decision

Option 2 (owner, 2026-10-30), less the two tooltip cues, which wait for the explainer.

**On the 1.0 path, built in P3.**

| # | Cue | Rule |
| --- | --- | --- |
| 1 | Semantic zoom | One crossfade by globe radius: far is flat pale tints and country names; middle adds hillshade and province borders; near adds strong relief, county borders and sector glyphs. A tint-strength slider in View. |
| 2 | Selection answers | Selecting a country tints every other country by its import or export share with it; Esc returns to the active mode. |
| 3 | Breathing edge | The selected feature's border glows in proof magenta on a two-second cycle; steady under reduced motion. Drawn in the id-buffer border pass. |
| 4 | Curved names | Country names follow the territory's long axis on a gentle curve, sized by extent; seas, ranges and deserts in italic atlas lettering. |
| 5 | Sector glyphs | Flat pictograms for each province's top sectors by value added, sized by output decile, near zoom only. |
| 6 | Sun terminator | Day and night for the presented date (0232). Off by default, a View setting; no city lights (0240). Hourly war keeps 0209. |
| 7 | Event pins | Detector events (0049) raise a pin at their province, height by size, fading over twelve months. Every notice has a go-to that flies the globe there. |
| 8 | Visible pause | Paused: the map desaturates, a thin frame appears, the date chip reads Paused. No flashing. |
| 10 | Notice settings | One table of detector event types with pause, pop up, log and ignore columns and a near-focus filter, stored as 0188 pause-on rules. |
| 11 | Coalesced notices | The same event type in the same month is one line with a count, expanding to the list, sorted by size. |
| 12 | Notice card | Headed by evidence: a locator crop of the globe and the series that crossed its threshold. No art, no options. |
| 13 | Loading facts | While the pack loads, one sentence computed from the 1 January 2000 data, with its source. Never about later dates (0241). |
| 14 | Start-screen globe | The last saved run's globe at its last date, turning slowly. |
| 17 | Direction colour | Changes are coloured by direction in a blue and umber pair, never green for good and red for bad: 1.0 has no seat. |
| 26 | Scars that fade | A worn texture where capital stock was destroyed, fading as it is rebuilt; shown only once a hazard of 0242 destroys capital. |

**Deferred until the explainer (0118) is built.** 15, a lock ring that turns a tooltip into a
window with nested tooltips; 16, breakdown tooltips that show each number's accounting.
Tooltips stay plain until then.

**2.0+ backlog.** With war: 22 counters with strength and readiness bars, 23 front lines and
plan arrows, 24 battle marks with a two-sided bar, 25 occupation hatch by control share (0245),
27 movement path ending in the arrival day, 28 controller tint washing in from the border. With
the game layer: 9 headline moments (with the 0144 front page), 18 seat-coloured numbers, 19 zoom
ambience by terrain, 20 a quiet sting for world-scale events, 21 an advisor voice (with 0078).

**Amendments.**
- 0244: country, sea and physical-feature names leave Slint `Text` for a glyph atlas drawn in
  wgpu, so they can follow a path. Province and county labels may stay in Slint. Borders are
  unchanged.
- 0137: the direction pair (up, down) joins the tokens, with values for both themes and a check
  under the 0221 colour-vision rule.

## Consequences

About 7-10 part-time weeks of P3 presentation work, mostly shader uniforms, sprites and Slint
state over data the engine already produces. Curved names are the largest item (about two
weeks) and bring `cosmic-text` back into the app. Event pins and notices depend on detector
events reaching the view (0049, 0208). No state or hash is affected.
