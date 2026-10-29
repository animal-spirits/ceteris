# Geometry spike (Task 3a, ADR 0113 owner decision 4)

Question: which shapely/GEOS operations of the Python geo build can move to pure Rust with the
parity ADR 0113 asks for (same province and county for every H3 cell, area per province within
1e-9 relative, 1e-12 for other float columns, adjacency sets equal), and which need the owner-side
`geos` feature.

Answer in one line: every operation except `coverage_simplify` keeps H3 cell membership and
adjacency exactly in pure Rust, but polygon overlay (`intersection`, `difference`, multi-input
`union_all`) through `i_overlay` cannot meet the 1e-9 area clause. That needs an owner decision
(section 6). `coverage_simplify` goes to the `geos` feature, where it is byte-identical.

## 1. Method

- Data: the real cache (`data/cache`) and the 2000 pack. The Python build was run up to
  provinces, counties, county clipping, adjacency, the land union and the simplification, with
  every shapely call recorded (inputs, output, arguments, calling function) as WKB: 621,859 calls
  over 431,522 distinct geometries. The recorded run reproduces `provinces.arrow` exactly (ids,
  `area_km2`, `lon` bit-identical for all 4,796 rows).
- Replay: a scratch Rust crate (outside the repository) reads the recorded inputs, runs each
  operation, and writes WKB back. A Python script compares Rust and shapely outputs per call:
  spherical area (`geo.area_km2`), planar symmetric difference, bit identity after `normalize`,
  and the H3 centre fill `cells_of` at resolution 4 (all calls) and 6 (county calls), using the
  same h3 library for both sides so only the geometry operation is under test.
- End to end where the decision is discrete: county assignment, gap assignment, adjacency pairs
  and rail crossings, sea grid, point location, coastal flags, anchors, H3 fill of the final
  provinces and counties.
- Versions: Rust 1.93.0; geo 0.33.1, i_overlay 4.5.2, spade 2.15.1, rstar 0.12.2, h3o 0.9.4
  (geo feature, in its own crate, see 6.3), geos-sys 2.0.10 linked to GEOS 3.14.1; Python
  shapely 2.2.0 on GEOS 3.14.1, h3 4.5.0. All crates are MIT, Apache-2.0, BSD-3-Clause, ISC or
  Zlib (deny.toml allowlist); geos-sys is MIT and links the LGPL GEOS C library.
- Determinism: two runs of the Rust replay (all intersections, the land union) give identical
  bytes.
- Runtimes: Rust in release mode; Python times come from the recorded run and include the
  recording overhead, so they overstate Python somewhat.

## 2. Inventory: shapely/GEOS operations of the geo build

Counts are calls in the recorded build of the 2000 pack.

| Operation | Call site | Arguments | Calls |
|---|---|---|---|
| `is_valid`, `make_valid` | `provinces.load` | default method (linework), only when invalid | 1 (BRA-1294 Goiás, ring self-intersection) |
| `make_valid` | `counties.load` | every county, then `polygonal` | 53,441 (all valid) |
| `union_all` of parts | `geo.polygonal` | after `make_valid`, `intersection`, `difference` | 112,917 |
| `union_all` | `split_by_counties` | bisection groups, `covered`, final parts | 601 |
| `union_all`, `intersection` with boxes, `transform` | `geo.unwrap`, `geo.wrap` | `box(-180,-90,180,90)`, `box(180,-90,540,90)`, shift 360 | 3 unions, 4 intersections |
| `intersection` (vectorised) | `counties.assign` | STRtree `intersects` pairs, then planar `area` | 170,647 (two passes: base and final provinces) |
| `intersection` | `split_by_counties` | county ∩ province, then `polygonal` | 5,856 |
| `difference` | `split_by_counties` | province minus union of parts (gaps) | 83 |
| `intersection` line ∩ line, `length` | `split_by_counties` | `gap.boundary` ∩ `part.boundary` | 132,330 (29,851 gaps) |
| `distance` polygon to polygon | `split_by_counties` | gap with no shared border | 31 gaps |
| `voronoi_polygons` | `provinces.split` | `extend_to=plane.envelope.buffer(1.0)`, `ordered=True` | 67 |
| `buffer(1.0)` of an envelope | `provinces.split` | round joins, `quad_segs=8` | 67 |
| `intersection` | `provinces.split` | stretched plane ∩ Voronoi region | 192 |
| `intersection` | `build.build` | county ∩ its province (clipping), then `polygonal` | 53,084 |
| `centroid`, `contains`, `point_on_surface`, planar `area` | `geo.anchor` | largest part by planar area | every province and county |
| STRtree `dwithin(0.005)` | `transport.adjacency` | province pairs | 11,615 candidates |
| `boundary`, `buffer(0.005)`, `intersection`, `length`, `bounds` | `transport.adjacency` | `a.boundary ∩ b.buffer(0.005)` | 11,615 buffers, 11,615 clips |
| `buffer(0.01)`, STRtree `intersects` | `transport.adjacency` | border vs railways | 11,455 |
| `union_all`, `prepare`, `contains_xy` | `transport.sea_grid` | union of `ne_10m_land`, res-3 cell centres | 1 union, 41,162 points |
| STRtree `within`, `query_nearest(max_distance=0.25)` | `transport._locate`, `resources.counts` | ports, airports, MRDS sites | 173,927 points |
| `boundary`, STRtree `dwithin(0.01)` | `transport.build` (coastal) | land boundaries vs provinces | 4,796 |
| `ops.nearest_points` | `transport.build` (minor ports) | province vs sea cell centre | per coastal province without a port |
| `coverage_simplify(0.01)` | `build.simplify` | provinces, counties | 4,796 + 53,084 |
| `simplify(0.01, preserve_topology=True)` | `build.simplify` fallback | if the coverage is invalid | 0 (not reached on current data) |
| `union_all` of simplified provinces | `build.build` (countries tile layer) | per country | 242 |
| `h3.geo_to_cells` (h3, not GEOS) | `provinces.cells_of` | per part, centre containment | every province and county |

Not GEOS, out of scope here: `mapbox_earcut` in `fills.py` (the plan already compares fills by
decoded triangles and area) and the spherical area formula in `geo.area_km2` (plain arithmetic).

## 3. Results

Area relative differences are spherical (`geo.area_km2`). "Cells equal" means the H3 centre fill
of the Rust output equals that of the shapely output.

### 3.1 Validity and repair

- geo `Validation::is_valid` agrees with GEOS on all 53,442 `make_valid` inputs (one invalid).
- geo 0.33 `MakeValid` (constrained Delaunay, odd-even) repairs Goiás identically to GEOS (equal
  after `normalize`, area difference 2.2e-15).
- Ungated, geo `MakeValid` rewrites 1,395 of the valid county inputs (area off by up to 3.7e-2),
  so it must run only when `is_valid` is false, which is what the Python does for provinces and
  what GEOS does in effect for counties.
- `polygonal` (`union_all` of parts): on all 112,917 calls the GEOS output equals the input parts
  after `normalize` (the inputs are always valid). In Rust it is the identity on valid input.
  Running it through `i_overlay` instead moves every vertex (0/112,917 identical; area p50 2.1e-10,
  p99 1.6e-9, max 1.5e-4), so the identity is required, not optional.

### 3.2 Polygon overlay (`i_overlay` through geo `BooleanOps`)

| Call site | Calls | Bit-identical | Area rel. p50 / p99 / max | Cells equal |
|---|---|---|---|---|
| `counties.assign` | 169,105 polygonal (1,542 empty or lower-dimensional in GEOS, empty in Rust) | 0 | 3.1e-9 / 1.7e-6 / sliver outliers | 169,105 (res 4 and 6) |
| county clipping (`build`) | 53,084 | 0 | 1.3e-9 / 4.2e-8 / 7.7e-5 (0.89 km²) | 53,084 (res 4 and 6) |
| county ∩ province (split) | 5,856 | 0 | 1.1e-9 / 9.4e-8 / 1.8e-5 | 5,856 |
| plane ∩ Voronoi region | 192 | 0 | 1.6e-9 / 2.1e-6 / 1.2e-5 | 192 |
| `union_all` in `split_by_counties` | 601 | 0 | 2.2e-10 / 1.2e-6 / 5.8e-6 | 601 |
| `wrap`/`unwrap` | 7 | 0 | max 5.3e-7 | 7 |
| `difference` (gaps) | 83 | 0 | 9.3e-6 / 3.2e-3 / 3.8e-3 of the gap area (21 km² max) | 83 |

Two causes, both structural rather than rounding: `i_overlay` snaps every coordinate to an i32
grid over the operands' bounding box (about 1e-9 of the extent), and it removes vertices that are
collinear in the plane. The second dominates the outliers: the 0.89 km² county differs from GEOS
by one dropped vertex, 7e-11 in planar area, but `area_km2` integrates in (λ, sin φ), where a
vertex collinear in (λ, φ) is not. GEOS also keeps slivers narrower than the grid that
`i_overlay` merges or drops (the 21 km² of gaps).

Tried and rejected:
- `i_overlay` directly with `preserve_input_collinear`/`preserve_output_collinear` and
  `multithreading: None`: no material change (union p99 1.5e-9, max 1.2e-4).
- Restoring every output vertex that falls in an input vertex's grid cell to the exact input
  coordinate: outputs become invalid (83/83 differences).
- geo 0.28, the last float-coordinate boolean implementation: panics on 79 of the 83 differences.

What the overlay differences do not change, measured end to end:
- County assignment (max-overlap) with Rust areas: 53,441/53,441 identical. Smallest relative
  margin between the best and second overlap: 5.5e-4.
- H3 cell membership of every recorded overlay output, at res 4 and res 6.

What they do change: `area_km2` of the 295 split provinces and of all 53,084 counties, and the
anchors (`lon`, `lat`) computed from those geometries. Anchor differences: p50 2e-10°, p99 3e-9°
(above 1e-12 relative for 50,034 of 53,084 clipped counties), with three jumps where the
centroid-in-polygon test or the interior-point scan line flips (0.0041° on one county, up to 0.77°
on one union inside `split_by_counties`). Edge `length_km` of edges incident to split provinces
follows the anchors.

### 3.3 Shared border length (`gap.boundary ∩ part.boundary`)

Rust: exact collinear overlap of the two linework sets (robust `orient2d`, overlap of collinear
segments, rstar index). Lengths match GEOS within 1e-9 on 132,293 of 132,330 calls; the gap
decision (`argmax` of shared length) differs on 6 of 29,851 gaps, where GEOS reports an overlap of
0.003 to 0.13° along nearly collinear segments that the exact test does not see. One gap has an
exact tie in GEOS (two parts at 1.0373°), area 7e-12 km², no cells. In a pure-Rust pipeline the
gaps themselves come from `i_overlay` and differ anyway (3.2), so this operation follows decision
6.1.

### 3.4 Buffer and adjacency

- geo `Buffer` polygons are not GEOS buffers (area differs by about 1e-4 relative; the GEOS
  `quad_segs=8` arc cannot be reproduced with `LineJoin::Round(angle)`).
- `envelope.buffer(1.0)` in `split`: the GEOS result's bounds equal the envelope ± 1.0 exactly
  (67/67), and only those bounds reach the Voronoi clip, so no buffer is needed.
- `transport.adjacency` end to end on the final provinces (rstar candidates, geo Euclidean
  distance for `dwithin`, geo `Buffer(0.005)` of `b`, `i_overlay` clip of `a.boundary`, threshold
  as in Python): 11,615 candidates, 11,455 pairs and 4,050 rail crossings, all identical to Python.
  Rail crossing tested both as `intersects(border.buffer(0.01))` and as exact
  `distance(rail, border) <= 0.01`: identical either way.
- Fragility: the smallest relative margin of a border length to its threshold is 0.21% (10 pairs
  within 1%), while the replayed clip of `a.boundary` by the GEOS buffer differs from GEOS by up to
  6.1% in length on one call. Parity holds on this data, not by construction.
- Runtime: 16 to 18 s in Rust (8.2 s in the brute-force polygon distance) against 9.5 s in Python.

### 3.5 Voronoi split

- geo 0.33 `Voronoi` (spade): `CollinearInput` error on two-seed inputs and only 20 of 67 calls
  matched. Not usable.
- Own half-plane construction: each seed's cell is the GEOS diagram envelope (seed envelope
  expanded by max(width, height), then by `extend_to`'s envelope) clipped by every bisector half
  plane, in seed order (`ordered=True`). 67/67 matched, symmetric difference at most 7.8e-15
  relative, 19/67 bit-identical. About 40 lines.

### 3.6 Coverage simplification

- geo `SimplifyVwPreserve` per polygon (area tolerance 1e-4): 111/4,796 provinces identical, area
  off by up to 96%, coverage overlap 0.150 deg² against GEOS's 0.032, 14 invalid outputs.
  Structurally different; no coverage simplifier exists in the Rust ecosystem.
- `GEOSCoverageSimplifyVW_r` through geos-sys 2.0.10 on GEOS 3.14.1: WKB byte-identical to
  shapely for 4,796/4,796 provinces and 53,084/53,084 counties, 0.86 s and 7.0 s.
- It depends on input vertex order: given the same geometries normalised (other ring start and
  part order), 13 provinces and 508 counties simplify differently.
- The safe `geos` crate (11.3.1) has no coverage simplification wrapper; only the raw binding
  exists, which needs `unsafe`.

### 3.7 Everything else

| Operation | Rust | Result |
|---|---|---|
| H3 centre fill (`h3.geo_to_cells` per part, with `assign_cells`' finer-resolution fallback) | h3o 0.9.4 `TilerBuilder`, `ContainsCentroid`, transmeridian heuristic on | identical cell sets: 4,796 provinces (78,483 cells, res 4), 53,084 counties (3,167,918 cells, res 6); 0.43 s and 3.8 s against 2.4 s and 19.6 s |
| Union of `ne_10m_land` and the sea grid | geo `unary_union`, then rstar + geo `Contains` on h3o res-3 centres | union area differs by 3.4e-9, but the 29,557 sea cells are identical (also with shapely's `contains_xy` on the Rust union); 0.08 s against 1.7 s. The union is not needed for the grid: "inside any land polygon" gives the same set |
| `centroid` | geo `Centroid` | max 7.3e-15 relative on the 4,700 provinces that use it |
| `point_on_surface` | geo `InteriorPoint` | 96/96 different (up to 0.040°): different algorithm |
| `point_on_surface` | port of JTS `InteriorPointArea` (scan line at the midpoint of the vertex ordinates nearest the envelope centre, widest crossing interval) | 96/96 bit-identical |
| STRtree `within` for points | rstar + geo `Contains` (boundary excluded) | 172,681 point-province pairs identical, 0.16 s |
| `dwithin(0.01)` coastal | geo Euclidean distance | 1,934 coastal provinces identical; 378 s, because geo's polygon-to-line distance is brute force: needs a segment index |
| Planar `area` for the overlap and largest-part choices | geo `Area` | decisions identical (3.2) |
| `query_nearest(max_distance)`, `nearest_points`, polygon `distance` (31 gaps) | | untested |
| `simplify(preserve_topology=True)` fallback, countries `union_all` of simplified provinces | | untested |

## 4. Verdicts

| # | Operation | Verdict | Basis |
|---|---|---|---|
| 1 | `make_valid` | pure Rust: geo `is_valid` gate, geo 0.33 `MakeValid` when invalid | 3.1 |
| 2 | `polygonal` / `union_all` of one valid geometry's parts | pure Rust: identity, no overlay | 3.1 |
| 3 | `intersection`, `difference`, multi-input `union_all`, `wrap`/`unwrap` | pure Rust (`i_overlay`) if decision 6.1 amends the area clause; otherwise `geos` feature | 3.2 |
| 4 | Shared border length | pure Rust (exact collinear overlap), under decision 6.1 | 3.3 |
| 5 | Land `union_all` and sea grid | pure Rust (point in any land polygon) | 3.7 |
| 6 | `buffer(0.005)`, `buffer(0.01)` in adjacency | pure Rust (geo `Buffer` + clip; exact distance for rail) | 3.4 |
| 7 | `envelope.buffer(1.0)` | pure Rust: envelope ± 1.0, no buffer | 3.4 |
| 8 | `voronoi_polygons` | pure Rust: own half-plane construction (not geo/spade) | 3.5 |
| 9 | `coverage_simplify(0.01)` | `geos` feature (owner-side, off by default) | 3.6 |
| 10 | `simplify(preserve_topology)` fallback, countries union of simplified provinces | `geos` feature, with 9 (untested) | 3.7 |
| 11 | H3 centre fill | pure Rust (h3o tiler) | 3.7 |
| 12 | `centroid` | pure Rust (geo) | 3.7 |
| 13 | `point_on_surface` | pure Rust: port of JTS `InteriorPointArea` | 3.7 |
| 14 | `within`, `contains_xy`, `dwithin` | pure Rust (rstar + geo; segment index for `dwithin`) | 3.7 |
| 15 | `query_nearest`, `nearest_points`, polygon `distance` | untested (3d's node and edge parity covers them) | |

## 5. What 3b to 3e adopt

No spike code is committed; these are the rules the ports follow.

- `polygonal(g)`: keep polygonal parts; overlay only when they are not a valid MultiPolygon.
- `make_valid` only when geo `is_valid` is false.
- Voronoi: half-plane cells clipped to the GEOS diagram envelope, envelope of `extend_to` taken as
  the plane envelope ± 1.0.
- Anchor: geo `Centroid` and `Contains`, JTS `InteriorPointArea` for the fallback.
- Sea grid: point in any `ne_10m_land` polygon, no union.
- Adjacency and coastal `dwithin`: an rstar index over segments, not geo's whole-geometry distance.
- geo with `default-features = false` (it defaults to `multithreading`, which turns on rayon in
  `i_overlay`); runs were deterministic either way.
- Ports of GEOS algorithms are written from JTS (EPL-2.0 or EDL-1.0, the latter BSD-3-Clause),
  never from GEOS sources (LGPL-2.1).

## 6. Owner decisions needed

### 6.1 Parity of overlay-derived geometry

Overlay output from `i_overlay` keeps every H3 cell and every discrete decision measured here, but
misses "area per province within 1e-9" for the 295 split provinces and the 1e-12 float clause for
`counties.area_km2`, the anchors of split provinces and counties, and edge lengths that use them.
The 4,501 provinces that are not split keep exact parity.

- A. Amend the ADR 0113 geometry clause for geometry produced by overlay (split provinces,
  clipped counties) and the floats computed from it: same province and county for every H3 cell
  (unchanged), area within 1e-4 relative (measured max 7.7e-5), anchors and their edge lengths
  compared within 1e-6° with listed exceptions for anchor jumps. Pure Rust.
- B. Overlay through the `geos` feature: exact, but province splitting and county clipping become
  owner-side, so the user-side `pack build` cannot produce the P1 geometry tables unless they ship
  prebuilt. That contradicts the pure-Rust user build of owner decision 4.
- C. Port JTS OverlayNG (floating precision, EDL) to Rust: exact or near-exact in pure Rust, but
  several thousand lines with noding and snapping, not in the plan.

Recommendation: A. The step-4f regeneration re-pins the pack from Rust anyway, and cell identity is
the parity that matters to the simulation. The tolerances should be confirmed end to end in 3c.

### 6.2 Coverage simplification and the web outputs

- Confirm that the web outputs (`web/map.pmtiles`, `web/*.fill`, the countries outline layer) are
  owner-side and ship prebuilt, as the tiles already are (ADR 0113). Then the user-side build never
  needs `coverage_simplify`.
- The safe `geos` crate lacks `GEOSCoverageSimplifyVW`, and `unsafe_code = "forbid"` is
  workspace-wide. Options: (i) upstream a safe wrapper to georust/geos and pin that release;
  (ii) one owner-side crate exempt from the `unsafe_code` lint, holding only that call; (iii) port
  JTS `CoverageSimplifier` and `TPVWSimplifier` at the revision GEOS 3.14.1 follows (about 800
  lines) and drop `geos`. Recommendation: (ii) now, (i) as its replacement.
- Pin GEOS 3.14.1 (geos-sys `static` bundles exactly 3.14.1; it needs cmake and a C++ compiler on
  the owner machine).
- Because the simplifier depends on vertex order, the GeoJSON layer hashes in the manifest and the
  fills cannot match Python for split provinces and counties under 6.1 A. Recommendation: compare
  the GeoJSON layers like the fills, by decoded geometry and per-feature area, in 3e.

### 6.3 Toolchain and crate versions

h3o 0.9.4 (pinned; 0.9.5 does not compile on Rust 1.93 because it calls `f64::mul_add` in a
`const fn`) depends on geo 0.32 and i_overlay 4.0 through its `geo` feature, which the H3 fill
needs. geo 0.32 has no `MakeValid` and no `Voronoi`; geo 0.33 is the version that repairs Goiás
exactly.

- A. Raise `rust-version` past 1.93 and move to h3o 0.9.5 and geo 0.33 (the h3o fill parity would
  be re-run on 0.9.5).
- B. Stay on 1.93 with geo 0.32: repair Goiás by an `i_overlay` union (area off by 3.6e-11, which
  breaks 1e-12 for that unsplit province unless 6.1 A covers it).
- C. geo 0.33 with h3o without its `geo` feature and an own polygon fill (to be re-verified).

Recommendation: A.

## 7. Reproduction

The spike lived outside the repository. To repeat it: record with a wrapper around
`shapely.union_all`, `intersection`, `make_valid`, `voronoi_polygons`, `coverage_simplify`,
`simplify` and `BaseGeometry.intersection`, `difference`, `buffer` while running
`counties.load`, `provinces.build`, `counties.assign`, the county clipping loop of `build.build`,
`transport.adjacency`, the land union and `build.simplify`; replay in Rust; compare with
`geo.area_km2`, `normalize`/`equals_exact` and `provinces.cells_of`. The GEOS check links geos-sys
to the `libgeos_c` shipped in the shapely wheel (`GEOS_LIB_DIR`, `GEOS_VERSION=3.14.1`).
