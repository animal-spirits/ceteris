# 0088. Fully native app with Slint panels

Status: accepted (owner, 2026-10-24); amended by 0244 (borders and labels)
Date: 2026-10-24
Design sections: 19.1 (App shell, Map UI), 19.3, 22.1 (App); amends ADR 0011

Numbering: 0023 to 0087 are taken by ADRs on open design branches, so this is 0088.

## Context

ADR 0011 moves the map to Rust on wgpu by way of a hybrid (a wgpu surface under a transparent
Tauri webview that keeps the React panels) and goes fully native only if the hybrid proves
awkward. The P2 spec (`specs/econ/SPEC.md`, App) has since recorded the owner's choice of a
fully native app with no React, webview or Tauri, panels in iced or Slint picked by a spike.
The owner has now picked Slint, and no ADR recorded either step.

What the panels must do: dense, skinned Paradox-style panels (province, market, enterprise and
pop tables, the edit and fork panels, the outliner and edit log), nested tooltips, charts, and
a layout that mods can restyle.

## Options

1. Hybrid, as ADR 0011 stage 1. Transparent-webview stacking over a native surface on macOS
   is the spike's main risk, and the React panels it keeps are deleted later anyway.
2. Native with egui. Fastest to build and the easiest wgpu integration, but immediate-mode
   styling makes a heavily skinned UI expensive, and layout lives in Rust code.
3. Native with iced (MIT). Retained, Elm-style, wgpu-based, with a shader widget for the map.
   Layout lives in Rust code, so mods cannot change it without a layout layer of our own.
4. Native with Slint. Declarative `.slint` markup with styling and a live-preview tool, wgpu
   interop, and `slint-interpreter` to load markup at run time.

## Decision

Option 4. The app is one native Rust process: winit window, Slint for panels, the map in wgpu
on the device Slint uses, the core in-process. Stage 1 of ADR 0011 changes from the hybrid
spike to a native spike; its exit criteria stay (120 fps on ProMotion, a county recolour on
screen within one frame plus 2 ms, panels working over the map).

- Map: rendered by our wgpu pipeline into a texture each frame from Slint's rendering notifier
  (`set_rendering_notifier`, `BeforeRendering`) and shown as a Slint `Image`, through the
  `unstable-wgpu-NN` feature. Picking stays on our ID buffer; Slint forwards pointer events.
  Everything else in ADR 0011 (pack `.fill` buffers, PMTiles, the feature colour texture,
  label and text crates) carries over.
- Panels: `.slint` files compiled into the binary for the shipped UI. Panel data comes from
  core columns through Slint models, with no copy of the tick state beyond the visible rows.
- Charts: drawn by us (Slint `Path`, or into a wgpu or software image); Slint has no chart
  widget. ECharts goes away with the webview.
- Developer inspector: egui is allowed behind a debug feature, on the same wgpu device.

## Licence

Slint is GPL-3.0, or the Slint Royalty-free Desktop, Mobile and Web Applications Licence 2.0,
or commercial. The project uses the royalty-free licence. It permits closed-source and paid
desktop builds (the Steam plan) and requires either the `AboutSlint` widget in an About screen
reachable from the top-level menu or the Slint badge on the download page. It does not cover
embedded systems or redistributing Slint on its own.

Open points, checked with Slint before P2's UI work starts:

- Runtime-loaded mod markup through `slint-interpreter`: the licence forbids distributing an
  application that exposes Slint's APIs. Whether loading mod `.slint` files counts is unclear.
  Until it is confirmed, mods change themes and data, not panel markup.
- Forks under Apache-2.0 / MIT on opening inherit the royalty-free terms (or GPL-3.0) for the
  Slint dependency, not the project's licence. State this in the README when the code opens.

## Consequences

- `app/` (Tauri, React, MapLibre, ECharts) stays the shipped UI until the native app reaches
  ADR 0011 stage 2 parity, then is removed with stage 3. New web-map features keep the porting
  rules of ADR 0011.
- The `pack://` protocol and Tauri IPC disappear; the app reads pack files from disk directly.
- `unstable-wgpu-NN` can change in Slint minor releases, so Slint and wgpu are pinned together
  and upgraded together.
- Determinism is unaffected: the UI only reads state and submits edits through the core's
  edit log.
- Update 19.1, 19.3 and 22.1 of the living design document; the snapshot in `specs/DESIGN.md`
  is updated in the same change so the two agree until the next refresh.
