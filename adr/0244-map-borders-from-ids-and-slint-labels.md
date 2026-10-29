# 0244. Map borders from the id buffer, labels in Slint

Status: accepted (owner, 2026-10-29)
Date: 2026-10-29
Design sections: 19.1 (Map UI), 20; amends ADRs 0011 and 0088
Phase: App shell (step 3 of `specs/app/PLAN.md`)

## Context

ADR 0011 has the native map take borders and labels from the pack's PMTiles archive: decode
vector tiles per zoom, tessellate lines with `lyon`, and draw text from a `cosmic-text` glyph
atlas. ADR 0088 keeps that. The native map as built (app shell steps 2 and 3) already renders
province and county ids into a second target for picking, on the flat map and on the globe.

## Options

1. As 0011: tiles, tile level of detail and caching, line tessellation in both projections, a
   glyph atlas and label placement in our own renderer.
2. Borders from the id target: a screen pass marks pixels whose neighbours differ in country,
   province or county id. Labels are Slint `Text` items placed over the map image from
   projected anchor points.

## Decision

Option 2 (owner, 2026-10-29).

- **Borders.** One full-screen pass after the fills reads the id target. A pixel is a country
  border when a neighbour's country differs (sea is country 0, so coasts come with it), drawn
  on both sides, two pixels wide. Province and county borders are one pixel. Countries come from
  a province-to-country texture. Borders are exact at every zoom, in both projections, and cost
  one pass whatever the number of features.
- **Labels.** Anchors (countries, then provinces past a zoom) are projected on the CPU each
  frame the camera moves and handed to a Slint model, which shapes and draws the text with the
  0137 type. Overlaps are dropped by priority.
- **Tiles.** `web/map.pmtiles` is no longer read by the native app. It stays in the pack while
  the Tauri app needs it and can return for line features with no fill (rivers, roads,
  railways).

## Consequences

- Border width is in pixels, not map units, and lines are not antialiased beyond one pixel.
  Width and alpha by kind are tokens.
- Borders of features smaller than a pixel vanish at small scales, which tiles would have
  generalized instead; at world view small countries show as specks without outlines.
- No tile decoder, `lyon` or glyph atlas in the shell, so 0011's stage 2 list loses its most
  expensive items.
- Text shaping and fallback fonts are Slint's.

## Tests

Offscreen: a country edge is drawn on both sides at the country strength, a province edge on
one side, a county edge only with county borders on, and interior pixels keep their fill
(`native/tests/offscreen.rs`).
