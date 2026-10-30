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
hash. It replaced the Tauri app (React, MapLibre, ECharts) at ADR 0011 stage 3 (owner sign-off
2026-10-30).

1.0 is a pure simulator (ADRs 0239, 0240). The app has no seats, scores, objectives,
narration, era themes or local-flavour chrome; it is a lab tool for reading, editing and
comparing runs.

## Scope of the shell (ADR 0207: 0088 stages 1 and 2, time-boxed)

| Area | In the shell | Later |
|---|---|---|
| Window | winit window, Slint 1.18 on the femtovg wgpu renderer (`unstable-wgpu-30`), one wgpu device shared with the map | Multiple windows (0196, 1.x) |
| Map | Province and county fills from `provinces.fill` and `counties.fill`, feature colour texture (0010), globe only (0250: pan rotates the sphere, north up, centre latitude clamped to 85.05 degrees; labels ranked by area on the sphere), pan and zoom, ID-buffer picking; borders from the id target and labels as Slint text (ADR 0244) | Hillshade (0137), difference maps (P3), units and arcs |
| Map modes | P1: political, terrain, resources (by group), freight cost from an origin. P2: price by good, output by sector, net exports by good, wage, unemployment (econ SPEC, App) | Every later phase adds its own |
| Panels | Mode picker, legend, inspector (province, county), province panel (markets, enterprises, pops), error banner, About with `AboutSlint` | Outliner, edit log, branch manager (P3) |
| Time | Play, pause, step, speeds 1-5, date counting days between monthly ticks, sim-bound chip; two loops and the run-ahead ring of 0208 | Step back, fork, run to event (P3) |
| Input | Commands through a registry; the standard keymap's Time and Map groups (0188) | Palette, vim mode, edits from the keyboard |
| Look | 0137 token set, light and dark, shared by Slint globals and the map shader; bundled OFL fonts; UI scale 85-200% and the ramp check (0221) | Music and sound (0137) |

## Architecture

- **Workspace.** Its own cargo workspace in `app/` (owner, 2026-10-29), so Slint and wgpu stay
  out of the core's lockfile, CI and `cargo deny`. It reaches the core by path dependencies.
- **Crate layout.** `ceteris-app` binary: `map` (wgpu pipelines, camera, picking, borders,
  selection marks), `labels` (anchors and placement), `modes` (colour per feature and legend),
  `ramps` (the 0221 check), `inspect` (inspector rows), `freight` (least-cost trees on a worker
  thread), `ui` (`.slint` files compiled in by `slint-build`, flat controls in `widgets.slint`),
  `sim` (sim thread, view publishing, ring), `commands` (registry and keymap). Arrow tables are
  read through `ceteris-geo` and `ceteris-econ`. The `.fill` reader is the app's own (`fill.rs`,
  a few lines against the format of `ceteris_pack::geo::fills`), so the app does not link the
  pack builder.
- **Freight cost.** A worker thread loads `ceteris_geo::pack::Pack` at start (about 10 ms on
  the 2000 pack), checks its provinces match `web/provinces.json`, and answers origins with
  `PathCache` trees (about 7 ms each); only the latest origin is computed. A failure shows in
  the error banner and leaves the other modes working.
- **Look.** Tokens are Slint globals and `tokens.rs` for the shader. Fonts are bundled from
  `ui/fonts/` (OFL): Source Serif 4 for map labels and headings, IBM Plex Sans Condensed for
  panels, IBM Plex Mono for numbers. Controls are flat token-built toggles, not the std
  widgets' accent style. The dark (slate) theme waits for its token values, which 0137 names
  but does not give.
- **UI scale.** `Tokens.scale` (85 to 200%, View menu) multiplies type, spacing and map
  labels; panels cap at half the window so the legend and inspector never overlap. It is not
  yet saved or read from the macOS text size; both arrive with the user config (step 5).
- **Frame.** Slint's rendering notifier (`BeforeRendering`) runs the map pass into a texture
  the size of the map viewport and hands it to an `Image`. Picking renders feature ids into an
  R32Uint target and reads back one pixel on pointer move.
- **Culling.** At load, each fill's triangles are regrouped by a 2-degree Mercator grid cell
  (`chunks`), each chunk bounded by a cap on the unit sphere; the pack's `.fill` format is
  unchanged. A frame draws only chunks that face the viewer and overlap the viewport, adjacent
  ones merged into one draw. Counties are drawn only when their fill or borders are on, or the
  sphere radius is at least 1,600 px (about 4 km per pixel); below that the hover line names no
  county.
- **Start-up.** The fills' chunks and the label anchors are built on worker threads while the
  window and GPU device start; labels appear when the anchors arrive.
- **Labels.** Placed where the camera changes (pan, zoom, keys), before the frame renders, so
  labels and map show the same frame; the label model is updated row by row, so unchanged
  labels keep their Slint items.
- **Profiling.** The `profile` cargo feature adds Tracy zones (UI frame, present, paint, labels,
  map render; sim step and view on the sim thread), frame marks and plots of the fill and border
  passes' GPU time from timestamp queries. Off by default; without it the zones compile to
  nothing.
- **Build.** `app/` is its own workspace, so its `[profile.release]` repeats the root's fat LTO
  and single codegen unit; the sim it links is built with them.
- **Colour.** Each mode maps its column to classes on the CPU once per presented tick and
  uploads 4 bytes per feature (0010). Interpolation between two ticks mixes two textures in the
  shader (0208). Panels show tick values only.
- **Sim thread.** Owns the `EconWorld`. Each finished tick publishes an immutable view (mode
  columns per province, panel rows for the selection, the date) into the ring; the frame loop
  takes the next one without blocking (step 5). A channel, not a latest-value swap, because
  the clock interpolates between consecutive ticks and must see every one. No lock is held
  while drawing.
- **Time (step 5).** `sim::Runner` steps the `EconWorld` (calibrated steady state when the
  pack has its calibration tables) on its own thread and publishes a view per tick into a
  bounded channel of 4, the run-ahead ring; it blocks when the ring is full. The frame loop
  takes at most the next view per frame without blocking. `clock::Clock` holds the presented
  tick and fraction of its month: speeds 1 to 4 are 1, 7, 30.4 and 91.3 days per second, speed
  5 presents each view as it arrives. Without the next view the date holds at the month's last
  day and the sim-bound chip shows. Tick 0 is 1 January 2000 and dates count real month lengths.
  Thinking further ahead while paused, eased slow-down and edits that drop the ring are P3
  (0208's edit rule needs the edit log).
- **Commands (step 5).** `commands` names every action and binds the standard keymap's Time
  group (Space, 1-5, +/-, F5, Shift+F5, F10) and Map group (WASD pan, Ctrl+=/-/0 zoom);
  buttons call commands by name. Keys arrive through a Slint `FocusScope`, not yet the winit
  router of 0188. Step back, run to cursor, go to date, events and alerts need P3 state; the
  registry moves to the crate P3 shares with the CLI (P3 plan, C).
- **Economy (step 6).** `econ::View` copies one tick's columns on the sim thread (prices,
  stocks, supply, orders, sales, wages, employment, output, profit, pop employment and income),
  so the frame loop never reads the world. Modes per pack province, NaN where the economy has
  no province: price of a good (buyer price over the world price index), output of a sector
  (at its market price), net exports of a traded good (market sales less the province's
  buyers' spending at delivered prices, so it counts shipments to other provinces of the same
  country too; local goods show no data), wage, unemployment (share of the labour force not
  employed). Net exports use a six-class diverging ramp (0137) split at the terciles of |x|; it
  passes the 0221 check. Moving modes upload the next view's colours to a second texture and
  the shader mixes by the clock's fraction (0208). The province panel adds the presented tick's
  markets, enterprises and pops, named by tick and date.
- **Determinism.** The app reads; edits go through the core. A headless run and an app run
  of the same ticks give the same state hash (test below).

## Invariants

| Invariant | Statement | Checked where |
|---|---|---|
| Read-only view | Views through the ring carry the same state hash as a headless run at every tick | `tests/sim_hash.rs` (fixture world, 24 ticks) |
| Run-ahead neutrality | Ring sizes 1 and 4 give the same hash at every tick (0208) | `tests/sim_hash.rs` |
| Picking | The id under a pixel is the feature whose triangle covers it | `tests/picking.rs`, offscreen |
| Culling | Every triangle with a vertex on screen is in a drawn chunk; chunks keep every triangle | `chunks.rs` unit tests |
| County level of detail | Counties pick only past the threshold, or with county fill or borders on | `tests/offscreen.rs` |
| Ramps | Adjacent classes at least 5 CIEDE2000 apart under normal vision and simulated protanopia, deuteranopia and tritanopia (Machado et al. 2009, severity 1); sequential lightness monotonic (0221) | `ramps.rs`, `modes.rs` unit tests |

## Parameters

None in the model. Presentation settings (speed, theme, scale, keymap) live in the user
config and enter no hash.

## Equilibrium nesting

n/a: the app computes nothing the core does not.

## Test plan

- Offscreen wgpu tests (no window): fill pipeline renders the fixture's provinces; picking;
  projection round trips (screen to lon/lat to screen) on the globe; zoom keeps the point under
  the cursor; pan follows the pointer.
- Sim tests as in Invariants, on the econ fixture.
- Ramp check in CI (unit tests in the `app` job). The political palette is categorical and
  not yet gated: two of its eight slots come within 4.7 under protanopia. The flag-hue country
  colours (art direction, 0137) replace it.
- Slint panel tests with Slint's testing backend: the inspector shows the fields of a picked
  province; every panel renders at 200% without clipping.
- Benchmark (owner's Mac, `CETERIS_BENCH=1`): frame time over a scripted
  camera path, and county recolour to screen. Exit criteria (0088, 0011 stage 1): 120 fps on
  ProMotion; a county recolour on screen within one frame plus 2 ms; panels working over the
  map.

## Open questions

- Slint licence for runtime-loaded mod markup (0088): moot in 1.0 if mods change themes and
  data only; to confirm with Slint before mods reach panel markup.
