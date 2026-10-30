# 0261. Glass loupe and glass edges on floating overlays

Status: accepted (owner, 2026-10-30); amended by 0262
Date: 2026-10-30
Design sections: 19.1 (Map UI), 20; amends ADR 0260; ADRs 0137, 0221
Phase: P3, after the 0260 frosted backdrop

## Context

ADR 0260 put panels over a frosted blur of the map. The owner asked whether "liquid glass"
effects (refraction, specular rims, edge dispersion, shapes that merge) are possible and
desirable. They are cheap once the blur exists, because the map pass already owns the pixels
behind each panel. Over a data map, though, refraction bends the map where borders and colours
are read, and strains the 0221 contrast floor.

## Options

1. Frosted only (0260 as written).
2. A magnifier loupe and thin glass edges on floating overlays; docked panels stay frosted.
3. Full liquid glass on every panel, including merging shapes.

## Decision

Option 2 (owner, 2026-10-30).

- **Loupe.** A movable magnifier over the map: the map pass samples a zoomed view inside a
  circle, with refraction at its rim from the circle's distance field. It is a tool, opened from
  View or a key, and closes with the selection.
- **Glass edges.** Floating overlays (time-control pill, notices, tooltips over the map) get a
  4 to 8 px rim that refracts the map along the shape's distance-field normal, with a specular
  highlight from a fixed light and a slight chromatic split. Interiors stay frosted per 0260.
- **Docked panels** (inspector, legend, toolbars) stay frosted with no refraction.
- Glass only bends the map. An overlay above another panel falls back to frosted.
- The reduced-transparency setting removes refraction and blur; reduced motion removes glint
  animation.

## Consequences

- The map pass takes a shape list (rounded rect or circle, radius, rim width) from Slint instead
  of rectangles only. About 3 to 5 days in P3 on top of 0260, checked against the 0088 frame
  budget on the owner's Mac.
- 0260: status "amended by 0261".
- Presentation only; determinism and golden hashes are unaffected.
