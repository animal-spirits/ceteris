# 0250. Globe-only map, Equal Earth for print

Status: accepted (owner, 2026-10-30)
Date: 2026-10-30
Design sections: 19.1 (Map UI, App shell), 20.1; amends ADRs 0010, 0011 and 0124
Phase: App shell (after step 6 of `specs/app/PLAN.md`); print projection with the P3 atlas

## Context

The native map has two projections: a flat Web Mercator map that wraps in x, and a globe
toggled from it (ADR 0010 added the toggle, and ADR 0011 makes the globe the primary view
with "flat projections as alternatives"). Both draw the same pack buffers, which store Web
Mercator x, y clamped at 85.05 degrees. The globe un-projects them in the vertex shader.
Picking and borders come from the id target (ADR 0244), so neither depends on the projection.

Mercator inflates area by sec² of latitude. That is 4x at 60 degrees, 10x at 72 degrees
(Greenland) and 33x at 80 degrees. On a world-scale choropleth Russia, Canada and Greenland
dominate, and the dense low-latitude provinces shrink. Labels are ranked by Mercator area, so
the bias reaches them too.

## Options

1. Keep both as now, and only rank labels by true area.
2. Replace the flat map with Miller. Wrap is unchanged, inflation drops to 3x at 60 degrees
   and 6x at 72, but northern shapes are squashed by 25 to 40%.
3. Replace the flat map with Equal Earth. Areas are true, but there is no endless wrap, and
   high-latitude shapes are flattened when zoomed in (about half height at 60 degrees).
4. Keep flat Mercator for panning, and add a fixed Equal Earth whole-world view.
5. Globe only in the app, with Equal Earth for flat print and export.

## Decision

Option 5.

- The app draws only the globe. The flat path is removed: the `globe` flag and
  `set_globe`, the instanced world copies and x-wrap in `map.wgsl`, and the flat branch of
  the camera. Once zoomed into a region, the globe is locally as true as Mercator, so
  regional play loses nothing.
- Pan rotates the sphere with north kept up. The centre latitude is clamped to the pack's
  85.05 degrees.
- Labels are ranked and fitted by area on the sphere, not by Mercator area.
- Pack fill buffers stay Web Mercator as an encoding of lon/lat. The pack format and its
  hashes do not change.
- Flat images (the ADR 0124 atlas, screenshot and map exports) render Equal Earth
  offscreen from the same buffers. This is built with the atlas in P3. The interactive app
  has no flat view.

## Consequences

- Only half the world is visible at a time. A world-scale comparison (the Americas against
  Asia) needs a spin or an export.
- The pack clamp at 85.05 degrees leaves small holes at the poles on the globe, mostly over
  inland Antarctica. They are painted as ice or paper, not fixed in the pack.
- Deep-zoom precision is of the same order as the flat path (f32 vertices, about 1 px at
  the deepest zoom). This is checked on the owner's Mac with the change.
- Cost: about 1 to 2 days, mostly removal. It lands on main after app shell step 6 (#92),
  and is sequenced with step 7 (moving `native/` to `app/`). `specs/app/SPEC.md` and DESIGN
  19.1 lose "flat and globe projections" in the same PR. The Tauri map keeps its toggle until
  step 7 deletes it.
