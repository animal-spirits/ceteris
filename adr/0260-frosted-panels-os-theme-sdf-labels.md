# 0260. Frosted panels, theme from the OS, SDF map labels

Status: accepted (owner, 2026-10-30); amended by 0261
Date: 2026-10-30
Design sections: 19.1 (Map UI), 20; amends ADR 0137; ADRs 0088, 0221, 0244, 0252, 0255
Phase: P3, with the hillshade (0255 stage 1) and the chart kit

## Context

The owner asked how Slint and other Rust crates can make the native app look polished. Research
(https://claude.ai/artifact/2nENnk6RjLKDC2hjmr9C39) checked Slint 1.18.1 in its source: it has
drop shadows, gradients, eased animations, `animation-tick()`, SVG `colorize` and
`Palette.color-scheme`, but no backdrop blur, blend modes or per-element shaders. Three choices
were left open: how panels sit over the globe (0137 says flat), which theme the app opens in
(0137 defines paper and slate but no default), and what draws the curved map names of 0252.
The owner chose to defer all polish code to P3.

## Options

1. Panels: flat opaque with a drop shadow (0137 as written); frosted blur of the map under every
   panel; blur only on floating overlays.
2. Theme: follow the OS; slate always; paper always.
3. Map labels: own signed-distance-field (SDF) glyph atlas with parley shaping; glyphon
   (cosmic-text), which lays out straight lines only.

## Decision

- **Frosted panels.** The map pass blurs the map at reduced resolution (dual Kawase) inside the
  panel rectangles Slint reports, and panels draw a translucent token fill over it. Text sits
  on that fill, so it meets the 0221 contrast floor over any map mode. A View setting and the
  reduced-motion/reduced-transparency setting turn the blur off, leaving the opaque fill.
  Panels keep hairline rules and no textures or bevels.
- **Theme from the OS.** The app opens in paper or slate per `Palette.color-scheme`, with a View
  override saved in settings. Both token sets live in `ui/tokens.slint` and `src/tokens.rs`,
  and the 0221 ramp check runs on both.
- **SDF label atlas.** Country, sea and physical-feature names (0252, 0244) are shaped with
  parley, the engine Slint already uses, and drawn from an SDF glyph atlas in wgpu, which gives
  curved baselines, halos and smooth zoom in one shader. glyphon is not used.

## Consequences

- 0137: "Panels: flat" now means flat over a frosted backdrop; status "amended by 0255, 0260".
- The map pass gains a blur pass and takes panel rectangles from Slint each frame; its cost is
  checked against the 0088 frame budget on the owner's Mac.
- Fonts and shaping stay one stack (parley); no second font database.
- The SDF atlas work sits inside the 0252 curved-names estimate (about 1.5 to 2 weeks).
- Presentation only; determinism and golden hashes are unaffected.
