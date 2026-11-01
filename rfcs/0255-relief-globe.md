# 0255. Relief globe

Status: accepted (owner, 2026-10-30)
Date: 2026-10-30
Design sections: 19.1 (Map UI), 20.1; amends ADRs 0137, 0248 and 0250
Phase: stage 1 with the P3 hillshade; stage 2 before 1.0

## Context

ADR 0137 picked Modern Atlas and rejected Diorama Earth (tilted 3D relief) as the main view,
keeping 3D only for a hand-off at the deepest zoom. ADR 0250 made the app globe only. The owner
saw a working mock of the globe with exaggerated relief and tilt
(https://claude.ai/artifact/Anr2PRTTGdCjAJRbEuMFHw) and wants it.

The 0137 objections still hold for data reading: tilt hides valleys behind ridges and shrinks the
far side, so areas stop comparing fairly. The mock answers this with relief that eases flat in
data modes.

The current fills cannot be displaced: province triangles from polygon triangulation are large
and thin, so raised vertices give spikes. Relief needs its own terrain mesh with the fills draped
over it. Picking and borders (0244) read the id target, so they survive draping unchanged.

## Options

1. Keep 0137: flat-shaded globe with hillshade.
2. Relief globe, full build before 1.0 (tilt and tiled terrain included).
3. Relief globe, staged: raised relief without tilt first, tilt and tiled terrain in 1.x.

## Decision

Option 2 (owner, 2026-10-30). Built in two stages, both before 1.0.

- **Relief.** A terrain sphere displaced by elevation times a relief factor, 0 to 100x,
  default 40x, set in View settings. The 0137 hillshade shades it in every mode.
- **Data modes.** Relief eases to flat (0137 motion timing) in every mode that encodes a value
  by colour, and returns in terrain and political modes. A setting keeps it raised.
- **Draping.** Province and county ids are rendered into lon/lat id tiles from the pack fills
  and sampled by the terrain; the feature colour texture (0010) and the id target are unchanged.
- **Elevation.** The 0137 hillshade source, resampled to a coarse global grid of about 1 to
  2 km (ETOPO or GEBCO class, under 1 GB) for displacement. Pack format version and hashes
  change with the step that adds it; the licence entry goes in `sources.toml`.
- **Stage 1 (with the P3 hillshade, about 1 to 2 weeks).** Raised relief, depth buffer, labels
  lifted to the surface. North-up camera without tilt, so relief shows mostly toward the rim.
- **Stage 2 (before 1.0, about 3 to 6 weeks).** Tilt up to about 70 degrees, tiled terrain with
  level of detail, labels hidden behind ridges, camera-relative precision for deep zoom.
- Flat exports (0250, Equal Earth) stay flat.

## Consequences

- 0248: the 1.0 estimate grows by about 4 to 8 owner-weeks (both stages), to be folded in at
  the pre-P3 re-rate.
- 0137: the Diorama Earth rejection is narrowed to the main view of data modes; status
  "amended by 0255". 0250: the globe gains relief; nothing else changes.
- The 0137 deepest-zoom hand-off to the city companion is unaffected.
- Determinism is unaffected: presentation only.
- Frame budget (0088 exit criteria, 120 fps on the owner's Mac) is rechecked at each stage.
- `specs/app/SPEC.md` and DESIGN 19.1 gain the relief globe when stage 1 starts.
