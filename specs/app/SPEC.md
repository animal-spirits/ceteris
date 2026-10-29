# App SPEC

Status: draft
Phase: App shell (ADR 0207), then P3 onward
Design sections: 19.1 (App shell, Map UI), 19.2, 19.4, 20, 20.3, 22.1; ADRs 0008, 0010, 0011, 0088,
0137, 0188, 0207, 0208, 0221, 0232, 0240

## Purpose

The native desktop app: one Rust process with a winit window, Slint panels and a wgpu map drawn
into a texture that Slint shows (ADR 0088), and the core in-process on its own thread pool
(ADR 0208). It reads the pack from disk and core columns directly. It owns presentation only:
it never writes state except by appending edits to the core's edit log, so it enters no run
hash. It replaces `app/` (Tauri, React, MapLibre, ECharts) when it reaches parity (ADR 0011
stage 3).

1.0 is a pure simulator (ADRs 0239, 0240). The app has no seats, scores, objectives,
narration, era themes or local-flavour chrome; it is a lab tool for reading, editing and
comparing runs.

## Scope of the shell (ADR 0207: 0088 stages 1 and 2, time-boxed)

| Area | In the shell | Later |
|---|---|---|
| Window | winit window, Slint 1.18 on the femtovg wgpu renderer (`unstable-wgpu-30`), one wgpu device shared with the map | Multiple windows (0196, 1.x) |
| Map | Province and county fills from `provinces.fill` and `counties.fill`, feature colour texture (0010), flat and globe projections, pan and zoom, ID-buffer picking, borders and labels from `web/map.pmtiles` | Hillshade (0137), difference maps (P3), units and arcs |
| Map modes | P1: political, terrain, resources (by group), freight cost from an origin. P2: price by good, output by sector, net exports by good, wage, unemployment (econ SPEC, App) | Every later phase adds its own |
| Panels | Mode picker, legend, inspector (province, county), province panel (markets, enterprises, pops), error banner, About with `AboutSlint` | Outliner, edit log, branch manager (P3) |
| Time | Play, pause, step, speeds 1-5, date counting days between monthly ticks, sim-bound chip; two loops and the run-ahead ring of 0208 | Step back, fork, run to event (P3) |
| Input | Commands through a registry; the standard keymap's Time and Map groups (0188) | Palette, vim mode, edits from the keyboard |
| Look | 0137 token set, light and dark, shared by Slint globals and the map shader; bundled OFL fonts; UI scale 85-200% and the ramp check (0221) | Music and sound (0137) |

## Architecture

- **Crate layout.** `ceteris-app` binary: `map` (wgpu pipelines, camera, picking, PMTiles
  reader), `ui` (`.slint` files compiled in by `slint-build`), `sim` (sim thread, view
  publishing, ring), `commands` (registry and keymap). Pack reading goes through `ceteris-geo`
  and `ceteris-econ`; no pack format is parsed twice.
- **Frame.** Slint's rendering notifier (`BeforeRendering`) runs the map pass into a texture
  the size of the map viewport and hands it to an `Image`. Picking renders feature ids into an
  R32Uint target and reads back one pixel on pointer move.
- **Colour.** Each mode maps its column to classes on the CPU once per presented tick and
  uploads 4 bytes per feature (0010). Interpolation between two ticks mixes two textures in the
  shader (0208). Panels show tick values only.
- **Sim thread.** Owns the `EconWorld`. Each finished tick publishes an immutable view (mode
  columns per province, panel rows for the selection, the date) into the ring; the frame loop
  takes the latest by atomic swap. No lock is held while drawing.
- **Determinism.** The app reads; edits go through the core. A headless run and an app run
  of the same ticks give the same state hash (test below).

## Invariants

| Invariant | Statement | Checked where |
|---|---|---|
| Read-only view | Running N ticks in the app and headless gives the same state hash | `tests/sim_hash.rs` |
| Run-ahead neutrality | Ring on and off give the same hash at every tick (0208) | `tests/sim_hash.rs` |
| Picking | The id under a pixel is the feature whose triangle covers it | `tests/picking.rs`, offscreen |
| Ramps | Adjacent classes stay apart under simulated CVD; sequential lightness monotonic (0221) | `tests/ramps.rs` |

## Parameters

None in the model. Presentation settings (speed, theme, scale, keymap) live in the user
config and enter no hash.

## Equilibrium nesting

n/a: the app computes nothing the core does not.

## Test plan

- Offscreen wgpu tests (no window): fill pipeline renders the fixture's provinces; picking;
  projection round trips (screen to lon/lat to screen) on flat and globe.
- Sim tests as in Invariants, on the econ fixture.
- Ramp check in CI.
- Slint panel tests with Slint's testing backend: the inspector shows the fields of a picked
  province; every panel renders at 200% without clipping.
- Benchmark (owner's Mac, `CETERIS_BENCH=1` as in the Tauri app): frame time over a scripted
  camera path, and county recolour to screen. Exit criteria (0088, 0011 stage 1): 120 fps on
  ProMotion; a county recolour on screen within one frame plus 2 ms; panels working over the
  map.

## Open questions

- Workspace placement (main workspace or its own, as `app/src-tauri` is): owner pick pending.
- Slint licence for runtime-loaded mod markup (0088): moot in 1.0 if mods change themes and
  data only; to confirm with Slint before mods reach panel markup.
- PMTiles over the local file needs no HTTP Range server; confirm the `pmtiles` crate's sync
  file reader at step 3.
