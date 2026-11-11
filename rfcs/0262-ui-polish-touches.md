# 0262. UI and UX polish touches

Status: accepted (owner, 2026-10-30)
Date: 2026-10-30
Design sections: 19.1 (Map UI), 20; amends ADR 0261; ADRs 0118, 0137, 0188, 0221, 0252, 0255, 0258, 0260
Phase: P3, in the polish pass with the 0260 frosted backdrop and the chart kit

## Context

The owner asked for mocks of small polish for the native app's UI and UX. The Polish Bench mocks 34 touches as playable demos on the
decided look (0137 tokens, 0260 frost, 0261 glass, 0252 cues, 0258 unit chips, 0188 keys) and
marks each for Slint 1.18 feasibility and cost. It avoids the charm set the owner rejected on
the art page and anything springy, since 0137 rules out bounce and shake.

## Options

1. Which set: a recommended cut of 27 (about 19 focused days); all 34 (about 26 days); map
   stage and numbers only (about 7 days).
2. Numbers on a tick: roll changed digits at speeds 1 and 2 and snap above; cross-fade at every
   speed; always snap.
3. Destructive actions: an undo toast, with a 700 ms hold only where there is no undo; undo
   toast only; a hold on every destructive action.

## Decision

All 34 touches (owner, 2026-10-30). Numbers roll at speeds 1 and 2 and snap above (owner). The
0261 loupe is dropped (owner). Destructive actions take the undo toast, with a 700 ms hold only
on actions that cannot be undone, such as discarding a branch (owner).

| Id | Touch | Behaviour | Build |
|---|---|---|---|
| M1 | Hover brushing | Hovering an outliner row outlines its province on the globe; hovering the globe lights the row. One hover id shared by Slint and the id-buffer pass. | map pass, 1 d |
| M2 | Mode cross-fade | Map mode changes fade the tint layer over 180 ms and swap the legend with it; relief flattens in data modes (0255). | map pass, 0.5 d |
| M3 | Cursor readout | A status chip reads the place and value under the pointer with its unit. | map pass, 0.5 d |
| P1 | Control states | Rest, hover, press, keyboard focus, disabled on every control; 120 ms tints; press darkens and drops 1 px; focus ring only for keyboard focus. In `widgets.slint`. | Slint, 1 d |
| P2 | Hold to confirm | A ring fills over 700 ms while held; early release cancels. Only on actions with no undo. | Slint, 0.5 d |
| P3 | Slider detents | Snap near round values (0.25 points on a policy rate), value bubble, quiet detent cue; Shift drags without snapping. | Slint, 1 d |
| P4 | Rail labels | Icon rail; labels slide out after 500 ms hover; a setting keeps them on. | Slint, 0.5 d |
| P5 | Context menu with keys | Right-click or long-press on a value lists the palette commands for it, each with its key. `ContextMenuArea`. | Slint, 0.5 d |
| N1 | Number ticker | Changed digits roll 140 ms at speeds 1 and 2 and snap above, as the 0137 tick recolour does. Numbers in Plex Mono. | Slint, 1 d |
| N2 | Delta mark | After a tick, changed values show the change in direction colour (0252 #17) and fade over 1.2 s. | Slint, 0.5 d |
| N3 | Unit chip | Clicking the unit chip opens the 0258 transform stages and writes the grammar line. | Slint, 1.5 d |
| N4 | Compact numbers | Three significant figures and a scale word; hover, focus or tap shows the exact value. | Slint, 0.5 d |
| N5 | Copy as citation | Ctrl+C on a hovered value copies value, unit, place, date, run and branch. | `arboard`, 0.5 d |
| N6 | Edited mark | Values set by an edit are underlined in proof magenta; hover names the edit and its timing. | Slint, 0.5 d |
| T1 | Warm tooltips | First tooltip after 450 ms; neighbours within 400 ms show at once. | Slint, 1 d |
| T2 | Key in tooltip | Tooltips end with the key, or the ex command in vim mode (0188). | Slint, 0.25 d |
| T3 | Edge flip | Tooltips flip and clamp inside the window, arrow kept on the anchor. | Slint, 0.5 d |
| K1 | Which-key | After the leader Ctrl+Space, the next keys appear if nothing is typed within 300 ms. | Slint, 1.5 d |
| K2 | Mode line | NORMAL, INSERT or VISUAL chip, pending keys and counts, ex line. | Slint, 1 d |
| K3 | Palette polish | Fuzzy-match highlight, recent first, key and ex verb per row. | `nucleo`, 1 d |
| K4 | Hint overlay | `f` in a panel labels every control with a letter; typing it presses the control. | workaround, 2 d |
| K5 | Gliding focus ring | One ring glides 120 ms between controls on Tab and arrows. | workaround, 1 d |
| F1 | Toast stack | At most three; older shrink behind; 4 s, held while hovered; edit toasts carry Undo. | Slint, 1.5 d |
| F2 | Notice count bump | A repeat of a coalesced notice (0252 #11) raises its count with a brief tint. | Slint, 0.25 d |
| F3 | Save stamp | Time and run hash in the status bar, fading over 3 s; a dot marks unsaved changes. | Slint, 0.25 d |
| F4 | Speed honesty | Requested and achieved speed with a lag bar when the machine falls behind. | Slint, 0.5 d |
| F5 | Inline errors | Field rule turns crisis colour and the message states the accepted range; no shake. | Slint, 0.25 d |
| L1 | Load stages | Real load stages with counts and a bar each; 0252 #13 facts below. | Slint, 0.5 d |
| L2 | Skeleton rows | Tables slower than 150 ms show grey rows with a slow sheen, static under reduced motion. | Slint, 0.5 d |
| L3 | Teaching empty states | Each empty panel says what will appear and the one key that fills it. | Slint, 0.75 d |
| L4 | Replay progress | Fork replay shows months done of total; Esc cancels and writes nothing. | Slint, 0.5 d |
| S1 | State cues | Quiet cues for pause, resume, copied, error, toast. | `kira`, 1.5 d |
| S2 | Cue limits | No cue repeats within 150 ms; tick cues stop above speed 3; notices duck music 3 dB. | `kira`, 0.5 d |
| A1 | OS accessibility flags | Reduced motion and reduced transparency read from the OS at start, overridable in View. | workaround, 0.5 d |

Known Slint 1.18 limits, with fallbacks: no hover on links in running text (links always
underlined); panel text not selectable (N5 instead); no tabular-figure switch (Plex Mono for
numbers, 0137); no backdrop blur in Slint (the map pass blurs the map only, 0260).

The loupe is removed from 0261. Glass edges on floating overlays stay.

## Consequences

- 0261: status "amended by 0262"; the loupe and its key are gone. The map pass still takes the
  shape list for glass edges, so the 0261 estimate shrinks by about a day.
- About 26 focused days in P3, on top of the ADRs that own each surface (0188 keymap, 0252 cues,
  0258 registry). Three new crates: `arboard`, `nucleo`, `kira` (already planned by 0137).
- All motion obeys 0137 and the reduced-motion setting; all glass and frost obey reduced
  transparency (0260, 0261).
- Presentation only; determinism and golden hashes are unaffected.
