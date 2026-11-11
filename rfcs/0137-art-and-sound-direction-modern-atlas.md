# 0137. Art and sound direction: Modern Atlas

Status: accepted (owner, 2026-10-28); amended by 0255, 0260
Date: 2026-10-28
Design sections: 20 (Interface and Visualization), 19.1; ADRs 0011, 0088, 0124
Phase: P3 (tokens, map compositor, panels, charts), after the event log has causes (audio)

## Context

Nothing fixes how the app looks or sounds. The Iberia mock assumes dark panels with a brass
accent and Cormorant Garamond, which no one decided. Constraints: one part-time developer with
AI help and no artists; native Rust UI (winit, Slint panels, wgpu map; ADR 0088); a real world
from 2000 onwards. Sonification (data turned into sound) was rejected in the bold-ideas
brainstorm. Proposal with rendered options

## Options

1. Modern Atlas: relief shaded from elevation under muted political tints, cartographic
   lettering, flat statistical-yearbook panels, editorial charts.
2. Paradox Cabinet: saturated nation colours, paper grain, brass-trimmed dark panels, Garamond.
   Needs drawn frames and textures.
3. Situation Room: dark vector map, glowing outlines, terminal panels. Hides terrain.
4. Diorama Earth: tilted 3D relief as the main view. Largest renderer job; hurts data reading.

## Decision

Option 1. Option 3's restraint informs the dark theme; option 4 is reserved for the deepest
zoom, where the map hands off to the city companion.

- Map: multidirectional hillshade baked offline from Copernicus GLO-30 (bathymetry from GEBCO)
  into tiles, multiplied under every map mode at reduced strength. Political tints are pale;
  neighbours outside the seat fade toward paper. Stepped bathymetry bands.
- Colour: one token set shared by Slint globals and the map shader, light (paper) and dark
  (slate) themes. Paper `#EDEFEA`, ink `#1C292F`, sea `#BCD1DB`. Edits are proof magenta
  (`#AE2459`), forks violet (`#5E45A3`); semantic ok / warn / crisis colours mark state only.
  Levels use one sequential ramp, changes one diverging ramp centred on zero; no rainbow, no
  red-green pair.
- Type, all OFL and bundled: Source Serif 4 for map labels and headings, IBM Plex Sans
  Condensed for panels, IBM Plex Mono for numbers, hashes and the console; Noto for scripts
  they lack. Map lettering follows atlas convention (tracked capitals for countries, italic
  for water, small capitals for provinces). Cormorant Garamond is dropped.
- Panels: flat, hairline rules, no textures or bevels, built only from tokens. Units on every
  number, sparkline columns, nested tooltips with cause breakdowns (20.2 explainer).
- Charts: title states the finding, subtitle gives units, source line names run, branch and
  lane count; direct labels instead of legends; fan charts for ensembles.
- Motion: only simulated time moves on its own. Tick recolour eases 180 ms at speeds 1-2 and
  snaps above; panels 120 ms; no bounce or camera shake; trade particles at fixed speed with
  density for volume. Reduced-motion setting removes eases and particles.
- Music: post-minimal chamber music, adaptive by mood state (calm, strain, crisis, war from
  P7) chosen by event detectors, with stems faded at bar lines and decade suites. Never driven
  by a continuous data series. v1 uses CC0 recordings (licence checked per track) and a
  player music folder tagged in a text file; an original score may be commissioned before
  release. Mixer: `kira`.
- Generative synth (owner request, 2026-10-28): a procedural modular synth, a patch graph on
  `fundsp` on the audio thread, as an alternative to the recorded score (player setting:
  Score, Synth or Mixed). One energy axis runs from ambient minimal through minimal techno and
  progressive to trance (118-138 bpm); mood state, a tension level raised by named events and
  decaying over months, the decade and a regional flavour are its only inputs, updated
  monthly. Named events shape form (breakdown on a crisis, build and drop on recovery) at 16-
  or 32-bar boundaries. Patterns are seeded from the run hash and date on a stream separate
  from the sim's.
- Regional music (owner request, 2026-10-28): open-licence recordings chosen by the seat's
  country, then culture region, then a world pool. Tracks are listed in a moddable manifest
  (URL, checksum, licence, attribution, country, culture region, mood, decade), fetched on
  the player's machine like the data packs, with credits generated from it. Sources:
  Wikimedia Commons, Musopen, Free Music Archive, Jamendo, ccMixter, Freesound, public-domain
  national archives; not the Internet Archive 78 rpm collections. Religion is not a selector,
  and liturgical recitation is excluded from background play by default.
- UI sounds: short, dry paper-and-mechanism sounds (click, month tick off by default, year
  bell, edit stamp, fork, crisis call) from home foley and CC0 packs.
- Icons: Lucide (ISC) plus custom glyphs; flags as valid in 2000 from public-domain SVGs.

## Consequences

- The Iberia mock is re-skinned to these tokens.
- Tokens and the map compositor's layer order (tint or ramp, times hillshade, then borders
  and labels) are fixed with the ADR 0011 native spike; the rest follows in P3.
- The pack pipeline gains a hillshade and bathymetry step; licence entries for GLO-30 and
  GEBCO go in the pack manifest.
- The printed atlas and era chrome of ADR 0124 use the same tokens.
- Label placement (collision, zoom tiers, later curved water labels) is the largest new piece.
