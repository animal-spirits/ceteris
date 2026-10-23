# 0008. Pack format and map delivery in P1

Status: accepted; amended by ADR 0009 (map geometry moved to PMTiles, pack format 2)
Date: 2026-10-23
Design sections: 16.1, 19.1 (Map UI, App shell, Bindings), 22.1 (App)

## Context

Section 19.1 names Parquet for the run store, PMTiles for province geometry, Arrow buffers
over Tauri IPC, and a Tauri app with the Rust core in-process. P1 has static tables, about
4,800 provinces, one query (least cost from a province) and no tick state.

## Options

1. Parquet tables, PMTiles built with tippecanoe, Arrow over IPC.
2. Arrow IPC files, simplified GeoJSON coloured through MapLibre feature state, raw f32
   buffers over IPC.

## Decision

Option 2 for P1.

- Pack tables are Arrow IPC files (memory-mappable, per 19.4), one record batch each, with
  `pack_format` schema metadata. Parquet stays the run-store format (P3).
- Province geometry is GeoJSON simplified with a coverage-preserving simplifier (0.01°, about
  1 km; 12.6 MB). Map modes set colours through feature state, so a mode change never
  re-uploads geometry. Measured: 60 fps mean, p99 frame 16.8 ms, while panning and zooming
  across the world at 1440x900, device scale 2, on Apple M4 Pro.
- `cost_from` returns costs then hours per province as little-endian f32 through
  `tauri::ipc::Response`. Arrow over IPC arrives with tabular tick state in P2.
- The Tauri crate (`app/src-tauri`) is its own Cargo workspace, so the core's CI and lockfile
  do not carry Tauri's system dependencies.
- For development in a plain browser, `cargo run -p ceteris-geo --example engine` answers the
  same query over HTTP on 127.0.0.1:7878, proxied by Vite. It is a development tool; the app
  has no server.

## Consequences

- PMTiles is revisited if geometry grows (resolution-5 cells near cities, finer coastlines)
  or load time matters; the map code depends only on a vector source with numeric feature
  IDs.
- The map's attribute table (`web/provinces.json`) duplicates columns of `provinces.arrow`;
  both come from the same build.
