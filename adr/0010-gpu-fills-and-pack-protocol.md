# 0010. GPU fill layer, pack protocol and globe

Status: accepted (owner, 2026-10-23); amends ADR 0008 and ADR 0009
Date: 2026-10-23
Design sections: 19.1 (Map UI, App shell), 19.4, 20.1

## Context

The in-app benchmark (`CETERIS_BENCH=1`, WKWebView, M4 Pro) showed that recolouring all
53,084 counties took 81 to 107 ms to reach the screen, with a 58 to 70 ms frame: MapLibre
evaluates feature state on the CPU and rewrites per-vertex colour data for every loaded tile.
IPC (4 ms for 212 KB) and the JavaScript loop (8 ms) were not the bottleneck. At 6 to 12
ticks per second (19.2), per-tick choropleths would stutter.

Separately, the app read the pack's tiles from Vite's public directory: a release build
would embed 62 MB of tiles in the binary, and Tauri's asset serving may not answer the HTTP
Range requests PMTiles needs.

## Options

1. Keep MapLibre feature state; recolour less often.
2. A custom WebGL layer: triangles uploaded once with a feature index per vertex, colours
   read from a texture indexed by feature, so a recolour uploads 4 bytes per feature.
3. A native renderer (ADR 0011).

For the pack: (a) bundle it with the app; (b) a `pack://` URI scheme serving files from the
pack directory.

## Decision

Option 2 and (b).

- The pack build writes `web/provinces.fill` and `web/counties.fill`: Web Mercator vertices,
  feature indices and earcut triangles from the same simplified geometry as the tiles, so
  fills meet the drawn borders. Edges longer than 0.004 Mercator units (about 1.4 degrees)
  are split at midpoints, decided per edge so neighbouring triangles never crack; on the
  globe a long triangle would otherwise cut through the sphere. Format in
  `python/ceteris/packs/fills.py`.
- `FillLayer` (`app/src/fills.ts`) is a MapLibre custom layer. Vertices go through MapLibre's
  projection prelude (`projectTile`), so the same buffers render on the flat map (repeated
  across world copies) and on the globe. Hover is a shader uniform. MapLibre keeps tiles,
  borders, labels, hit-testing (an invisible fill layer) and selection outlines.
- The map gains a globe toggle (MapLibre's globe projection).
- `pack://` (`app/src-tauri/src/pack_protocol.rs`) serves pack files from disk with Range
  support and CORS, refusing paths outside the pack. The frontend builds pack URLs with
  `convertFileSrc`; in a plain browser it falls back to Vite. Release builds no longer copy
  the pack into the bundle.

## Consequences

Measured in the desktop app (same benchmark, debug build):

| Recolour of 53,084 counties | Feature state (before) | Fill layer (after) |
|---|---|---|
| Query and IPC | 38 ms (includes path search) | 4 ms |
| JavaScript | 8 ms | 1 ms |
| To screen, p50 / p95 | 99 / 120 ms | 19 / 35 ms |
| Worst frame, p50 / p95 | 52 / 72 ms | 14 / 29 ms |

The camera flight, now drawing 1.0M province vertices per frame through the fill layer,
held 100 fps mean (p95 11 ms) on a display running above 60 Hz, with no frame over 33 ms.

- Fill buffers add 76 MB to the pack (17.5 MB provinces, 58.8 MB counties); counties load
  only when a county colouring is shown.
- Every feature is drawn at full detail at every zoom (no tile level of detail); fine at
  2.6M vertices on Apple Silicon, revisit if geometry grows.
- Anything that colours provinces or counties per tick uses `FillLayer.setColours`, never
  feature state.
