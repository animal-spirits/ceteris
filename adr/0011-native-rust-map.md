# 0011. Rewrite the map in Rust

Status: accepted (owner, 2026-10-23); path amended by 0088 (fully native, Slint panels); scheduling open
Date: 2026-10-23
Design sections: 19.1 (Map UI, App shell), 19.4, 20, 22.1 (App)

## Context

Section 19.1 put the map in TypeScript (MapLibre GL JS, deck.gl) inside a Tauri webview, with
the Rust core in-process behind IPC. The owner decided to move the map to Rust.

Measurements that bear on it (ADRs 0009 and 0010, M4 Pro, WKWebView):

- Drawing is not the limit: the map holds 60 to 100 fps with 4,796 provinces and 53,084
  counties.
- Per-tick recolouring was the limit with MapLibre feature state (99 ms to screen for all
  counties). The GPU fill layer of ADR 0010 brought it to 19 ms (p50). Of that, IPC is 4 ms
  and JavaScript 1 ms; the rest is waiting for the next frames.
- So a Rust renderer would remove about 5 ms per update and the IPC boundary, not a
  bottleneck. Its case rests on other grounds, below.

Reasons for Rust:

1. One language and one process for state and view: the renderer reads the core's
   structure-of-arrays columns directly (Section 19.4), with no serialization, and the
   simulation's time controls, branches and ensembles (Sections 14, 20) drive the view
   without a message protocol.
2. A globe as the primary view. The world is spherical end to end (H3 raster, great-circle
   distances, sea grid); a native renderer draws on the sphere directly instead of
   projecting Mercator buffers, and keeps flat projections as alternatives.
3. Full control of the GPU path (wgpu on Metal): instancing for armies and trade arcs,
   compute shaders for per-tick colouring, picking through an ID buffer.
4. No webview: predictable memory and frame pacing, no WebKit differences from Chrome.

## Options

1. Stay with the web map (ADR 0010).
2. Hybrid: a wgpu map on a native surface in the Tauri window, under a transparent webview
   that keeps the React panels.
3. Fully native: wgpu map and a Rust UI toolkit (egui, iced or Slint) for panels and charts.
4. MapLibre Native (C++, Metal) through FFI, with custom layers in C++. Same style spec and
   tile handling as today. Costs: a large C++ build, young Rust bindings, custom layers in
   C++, and no supported way to embed it under Tauri's webview. Globe support reached GL JS
   before Native; its status must be checked before relying on it.
5. Bevy as the application framework: an engine's scheduler and renderer, at the price of
   its ECS owning the frame loop the simulation already schedules (Section 4).

## Decision

Rewrite the map in Rust on wgpu, by way of option 2 and then, if the hybrid proves awkward,
option 3. MapLibre Native (4) and Bevy (5) are not taken: both bring a second runtime the
core does not need.

What the rewrite inherits rather than reinvents:

- Geometry: the pack's `.fill` buffers (ADR 0010) and the PMTiles archive (borders, labels)
  stay the interchange format; the Rust map reads the same files.
- Colouring: the feature-indexed colour texture of ADR 0010 carries over unchanged, and can
  move to a compute shader fed from core columns.
- Crates to evaluate in the spike: `wgpu`; `pmtiles` and an MVT decoder for tiles; `lyon` for
  line and label tessellation; `glyphon` or `cosmic-text` for text.

Staged, each stage gated by the in-app benchmark:

1. Spike: a wgpu surface in the Tauri window under a transparent webview, drawing provinces
   from `provinces.fill` on a globe and on a flat map, with ID-buffer picking. Exit: 120 fps
   on a ProMotion display, a county recolour on screen within one frame plus 2 ms, and the
   panels still working over it. If the transparent-webview stacking fails on macOS, go to
   option 3.
2. Parity: borders and country outlines from tiles, labels, all map modes, camera controls,
   hover and selection, freight-cost mode.
3. Remove MapLibre and the GPU fill layer's TypeScript code.

Scheduling: the stages need the core's tick stream (P2) to be measured against real
per-tick data, so the spike starts with or after P2 unless the owner schedules it earlier.

## Consequences

- Months of work that P1's web map already covers; labels, tile level of detail and text
  shaping are the expensive parts.
- Section 19.1's map stack and 22.1's app description change when stage 3 lands; update the
  design document then.
- Until then, new map features go into the web map in a form that ports: data in pack
  buffers or core columns, colouring through the feature texture, no logic in style
  expressions.
- Determinism is unaffected: the renderer only reads state.
