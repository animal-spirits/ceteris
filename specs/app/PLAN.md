# App shell plan

Goal: ADR 0088 stages 1 and 2 (ADR 0207's App shell track, 3 to 5 owner-weeks): a native app at
parity with the P1 Tauri map, plus the panel host, inspector and time controls over the P2
economy. Spec: `specs/app/SPEC.md`. Each step is one PR; the app runs after each.

| Step | PR | Content | Exit |
|---|---|---|---|
| 0 | docs | This plan and the SPEC | Owner OK |
| 1 | `feat(app)` skeleton | `ceteris-app` in its own workspace `native/`, winit + Slint (femtovg wgpu renderer) window, a wgpu pass drawn into a Slint `Image`, About with `AboutSlint`, token set as Slint globals (0137), CI build | Window opens on macOS; offscreen test renders a cleared texture |
| 2 | `feat(app)` fills and picking (native spike) | `provinces.fill` and the feature colour texture, flat projection, pan and zoom, ID-buffer picking, political mode, frame benchmark | 0088 exit criteria measured on the owner's Mac (120 fps, recolour within a frame plus 2 ms) |
| 3 | `feat(app)` globe, counties, borders, labels | Globe projection, county layer and toggle, borders from the id target and labels as Slint text (ADR 0244) | Visual check against the Tauri map; county recolour benchmark on the owner's Mac |
| 4 | `feat(app)` P1 panels and modes | Mode picker, legend, terrain, resources by group, freight cost from an origin (`ceteris-geo` in-process), inspector, error banner, UI scale and the ramp check (0221) | Parity list of ADR 0011 stage 2 ticked |
| 5 | `feat(app)` time controls | Sim thread on the econ world, published views, run-ahead ring (0208), play, pause, step, speeds, daily date (0232), command registry and the Time and Map keymap groups (0188) | Same hash headless and in the app; ring on and off |
| 6 | `feat(app)` P2 modes and province panel | Price, output, net exports, wage, unemployment; province panel of markets, enterprises and pops; tick interpolation in the shader | econ SPEC App section met |
| 7 | `chore(app)` stage 3 | Remove the Tauri app (React, MapLibre, ECharts, `pack://`), move `native/` to `app/`; DESIGN 19.1, 19.3, 22.1 and the living document updated | Owner sign-off on parity |

Not in the shell: hillshade, difference maps and fan charts (P3), outliner, edit log and
branch manager (P3), multiple windows (0196), music and sound, anything of ADRs 0239 and 0240.

Verification: CI has no display and no Metal, so pipelines are tested offscreen (wgpu on
Vulkan or the GL fallback where the runner has one, skipped otherwise) and the frame-rate exit
criteria are measured on the owner's Mac.
