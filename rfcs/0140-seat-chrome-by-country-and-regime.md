# 0140. Seat chrome by country and regime

Status: accepted (owner, 2026-10-28); amended by ADR 0240 (game and flavour items to 2.0+)
Date: 2026-10-28
Design sections: 20 (Interface and Visualization); ADRs 0065, 0078, 0088, 0124, 0137
Phase: P3 (tokens and accent), P5-P6 (regime wording once regimes are endogenous)

## Context

Paradox games reskin their interface by ideology or culture. ADR 0137 fixes one token set (Modern
Atlas) so the app reads as one product. The owner asked for the header, background, accent and
UI elements to change with the seat, as Paradox does, and approved the mock in the proposal (section "Seat chrome").

## Options

1. Fixed chrome for every seat.
2. Full per-culture skins (frames, textures, ornaments). Needs artists and drifts into caricature.
3. Atlas tokens stay fixed; the seat country sets the accent and the regime class sets shape and
   wording.

## Decision

Option 3. Chrome is presentation and never writes state.

- Accent: taken from the seat's current flag colours (ADR 0142), toned in OKLab until it passes
  WCAG AA against paper and slate; text on the accent picks ink or paper by contrast. The accent
  fills the header rule, focus rings and the seat's own map border. Edits stay proof magenta and
  forks violet (ADR 0137), whatever the seat.
- Header: flag, English name with the local official name beneath it (ADR 0143), the date with a
  second line in the seat's calendar (ADR 0141), and four headline figures.
- Shape by regime class. Six classes from V-Dem's Regimes of the World, extended where it has no
  class: liberal democracy, electoral democracy, electoral autocracy, one-party state, military
  junta, absolute monarchy. Each sets border weight, label type, header weight and the register
  of every card: press items and an "Approval" figure in democracies; state television,
  bulletins, communiqués or a court circular elsewhere, with "Support (est.)" and its interval.
- Wording: templates keyed by regime class in the localisation files (the Fluent grammar of ADR
  0078), so a translator sees each register as its own string. Panel titles change ("Cabinet",
  "Central Committee", "Ruling council", "Royal court"); actions and buttons do not, because the
  action space is the same.
- Fog: where ADR 0065 is on and the regime manipulates statistics, estimated figures carry a halo
  and the "official" stamp.
- Class changes with the simulated regime (P5-P6); until then it is read from the V-Dem 2000
  value for the seat.
- Era themes of ADR 0124 stay a separate, off-by-default option layered on top.
- Excluded: ethnicity or religion as a skin, ornament sets per culture, faith symbols in chrome.

## Tests

- Changing seat or regime class changes no state hash.
- For every flag in the pack, the toned accent passes AA against both themes.
- Every card template exists in every regime class for every shipped language.

## Consequences

Each seat feels different with no art assets: one token set, a colour rule and six wording
registers. The cost is about 1-2 part-time weeks in P3 for accent and header, and the register
templates grow with every new card type.
