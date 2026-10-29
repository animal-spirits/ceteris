//! Multimodal transport graph: nodes and undirected edges with physical attributes (Section 5.3,
//! ADR 0007).
//!
//! Costs are not computed here: the core prices edges from mode parameters (register P04), so
//! calibration never needs a pack rebuild.

use std::sync::Arc;

use arrow_array::{ArrayRef, BooleanArray, Float64Array, RecordBatch, StringArray};
use arrow_array::{UInt8Array, UInt32Array};
use h3o::{CellIndex, Resolution};
use toml::Value;

use super::sphere::haversine_km;
use crate::numeric::pairwise_sum;
use crate::sources::{CONFIG_DIR, read_table, workspace_root};
use crate::table;

/// Ports farther than this from the sea grid (inland lakes, rivers) are dropped until waterways
/// exist.
pub const MAX_PORT_SNAP_KM: f64 = 150.0;
/// Shared borders shorter than this are corner contacts, not adjacency.
pub const MIN_BORDER_KM: f64 = 1.0;
pub const AIR_NEAREST: usize = 12;
pub const AIR_PER_CONTINENT: usize = 3;
/// Sea components smaller than this without a port are fjord and lagoon fragments.
pub const MIN_SEA_COMPONENT: usize = 20;

/// Written for a node or province field that has none.
const NONE: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum NodeKind {
    Province = 0,
    Rail = 1,
    Port = 2,
    Airport = 3,
    Sea = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Mode {
    Road = 0,
    Rail = 1,
    Sea = 2,
    Air = 3,
    RailAccess = 4,
    PortAccess = 5,
    MinorPortAccess = 6,
    AirAccess = 7,
    Canal = 8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Edge {
    /// The lower node index.
    pub a: usize,
    pub b: usize,
    pub mode: Mode,
    pub length_km: f64,
    pub tri_m: f64,
    /// 1-based index into the chokepoints, or 0.
    pub chokepoint: u8,
}

#[derive(Debug, Default)]
pub struct Graph {
    pub kind: Vec<NodeKind>,
    pub province: Vec<Option<usize>>,
    pub lon: Vec<f64>,
    pub lat: Vec<f64>,
    pub label: Vec<String>,
    pub zone: Vec<Option<usize>>,
    pub edges: Vec<Edge>,
}

impl Graph {
    pub fn node(
        &mut self,
        kind: NodeKind,
        province: Option<usize>,
        lon: f64,
        lat: f64,
        label: String,
        zone: Option<usize>,
    ) -> usize {
        self.kind.push(kind);
        self.province.push(province);
        self.lon.push(lon);
        self.lat.push(lat);
        self.label.push(label);
        self.zone.push(zone);
        self.kind.len() - 1
    }

    pub fn edge(
        &mut self,
        a: usize,
        b: usize,
        mode: Mode,
        length_km: f64,
        tri_m: f64,
        chokepoint: u8,
    ) {
        self.edges.push(Edge {
            a: a.min(b),
            b: a.max(b),
            mode,
            length_km,
            tri_m,
            chokepoint,
        });
    }

    /// Edges in tuple order (a, b, mode, length, ruggedness, chokepoint); equal floats keep
    /// their order, as Python's stable sort of the tuples does.
    pub fn sort_edges(&mut self) {
        let float = |x: f64, y: f64| x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal);
        self.edges.sort_by(|x, y| {
            (x.a, x.b, x.mode)
                .cmp(&(y.a, y.b, y.mode))
                .then(float(x.length_km, y.length_km))
                .then(float(x.tri_m, y.tri_m))
                .then(x.chokepoint.cmp(&y.chokepoint))
        });
    }
}

/// A maritime chokepoint of `chokepoints.toml`: sea cells within `radius_km` of the centre carry
/// its tag; a `link` adds a sea edge along the polyline.
#[derive(Clone, Debug, PartialEq)]
pub struct Chokepoint {
    pub name: String,
    pub lon: f64,
    pub lat: f64,
    pub radius_km: f64,
    pub canal: bool,
    pub link: Option<Vec<[f64; 2]>>,
}

fn number(v: &Value) -> Option<f64> {
    v.as_float().or_else(|| v.as_integer().map(|i| i as f64))
}

pub fn chokepoints() -> Result<Vec<Chokepoint>, String> {
    let path = workspace_root().join(CONFIG_DIR).join("chokepoints.toml");
    parse_chokepoints(&read_table(&path)?).map_err(|e| format!("{}: {e}", path.display()))
}

/// The chokepoints of a `chokepoints.toml` table: a missing name, centre or radius, or a link
/// of fewer than two points, is an error, as in the Python build.
pub fn parse_chokepoints(t: &toml::Table) -> Result<Vec<Chokepoint>, String> {
    let list = t
        .get("chokepoint")
        .and_then(Value::as_array)
        .ok_or("no [[chokepoint]]")?;
    list.iter()
        .enumerate()
        .map(|(i, c)| {
            let name = c
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("chokepoint {i}: no name"))?;
            let field = |k: &str| {
                c.get(k)
                    .and_then(number)
                    .ok_or_else(|| format!("{name}: no {k}"))
            };
            let link = match c.get("link") {
                None => None,
                Some(v) => {
                    let path = v
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|p| match p.as_array().map(|xy| xy.as_slice()) {
                            Some([x, y]) => Some([number(x)?, number(y)?]),
                            _ => None,
                        })
                        .collect::<Option<Vec<_>>>()
                        .ok_or_else(|| format!("{name}: link is not a list of [lon, lat]"))?;
                    if path.len() < 2 {
                        return Err(format!("{name}: a link needs two points or more"));
                    }
                    Some(path)
                }
            };
            Ok(Chokepoint {
                name: name.to_owned(),
                lon: field("lon")?,
                lat: field("lat")?,
                radius_km: field("radius_km")?,
                canal: c.get("canal").and_then(Value::as_bool).unwrap_or(false),
                link,
            })
        })
        .collect()
}

/// Length of a polyline in km, its segments summed as numpy sums them.
pub fn path_km(path: &[[f64; 2]]) -> f64 {
    let legs: Vec<f64> = path
        .windows(2)
        .map(|w| haversine_km(w[0][0], w[0][1], w[1][0], w[1][1]))
        .collect();
    pairwise_sum(&legs)
}

/// Index of and distance to the nearest target of each point, by brute-force haversine; the
/// first target on a tie. None for every point when there are no targets.
pub fn nearest(
    lon: &[f64],
    lat: &[f64],
    to_lon: &[f64],
    to_lat: &[f64],
) -> Option<Vec<(usize, f64)>> {
    if to_lon.is_empty() {
        return None;
    }
    Some(
        lon.iter()
            .zip(lat)
            .map(|(&x, &y)| {
                let mut best = (0, haversine_km(x, y, to_lon[0], to_lat[0]));
                for (j, (&tx, &ty)) in to_lon.iter().zip(to_lat).enumerate().skip(1) {
                    let d = haversine_km(x, y, tx, ty);
                    if d < best.1 {
                        best = (j, d);
                    }
                }
                best
            })
            .collect(),
    )
}

/// Connected component of each of `n` items, named by its lowest member.
pub fn components(n: usize, pairs: &[(usize, usize)]) -> Vec<usize> {
    let mut parent: Vec<usize> = (0..n).collect();
    let find = |parent: &mut Vec<usize>, mut x: usize| {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    };
    for &(a, b) in pairs {
        let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
        if ra != rb {
            parent[ra.max(rb)] = ra.min(rb);
        }
    }
    (0..n).map(|i| find(&mut parent, i)).collect()
}

/// Every cell at `res`, in the order of their string ids: fixed-width lowercase hex, so the
/// order of the indices.
pub fn all_cells(res: Resolution) -> Vec<CellIndex> {
    let mut cells: Vec<CellIndex> = CellIndex::base_cells()
        .flat_map(|c| c.children(res))
        .collect();
    cells.sort_unstable();
    cells
}

/// What `build` counts, as the pack manifest's stats name them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub land_borders: usize,
    pub rail_borders: usize,
    pub ports_dropped: usize,
    pub ports: usize,
    pub minor_ports: usize,
    pub airports: usize,
    pub air_links: usize,
    pub coastal: usize,
}

impl Stats {
    /// Name and value of each, in name order.
    pub fn named(&self) -> [(&'static str, usize); 8] {
        [
            ("air_links", self.air_links),
            ("airports", self.airports),
            ("coastal", self.coastal),
            ("land_borders", self.land_borders),
            ("minor_ports", self.minor_ports),
            ("ports", self.ports),
            ("ports_dropped", self.ports_dropped),
            ("rail_borders", self.rail_borders),
        ]
    }
}

/// The graph, the sea zones (H3 ids) its sea nodes index, the stats, and which provinces touch
/// the coast.
#[derive(Debug)]
pub struct Transport {
    pub graph: Graph,
    pub zones: Vec<String>,
    pub stats: Stats,
    pub coastal: Vec<bool>,
}

fn index(i: Option<usize>) -> Result<u32, String> {
    match i {
        None => Ok(NONE),
        Some(i) => u32::try_from(i)
            .ok()
            .filter(|&i| i != NONE)
            .ok_or_else(|| format!("index {i} does not fit the table")),
    }
}

/// The pack tables of the graph: `nodes.arrow`, `edges.arrow`, `sea_zones.arrow` and
/// `chokepoints.arrow`, by file name.
pub fn tables(
    graph: &Graph,
    zones: &[String],
    chokepoints: &[Chokepoint],
) -> Result<Vec<(&'static str, RecordBatch)>, String> {
    let named = |columns: Vec<(&str, ArrayRef)>| {
        table::batch(
            columns
                .into_iter()
                .map(|(n, a)| (n.to_owned(), a))
                .collect(),
        )
    };
    let u32s = |v: Vec<u32>| -> ArrayRef { Arc::new(UInt32Array::from(v)) };
    let f64s = |v: Vec<f64>| -> ArrayRef { Arc::new(Float64Array::from(v)) };
    let strings = |v: Vec<&str>| -> ArrayRef { Arc::new(StringArray::from(v)) };
    let indices = |v: &[Option<usize>]| v.iter().map(|&i| index(i)).collect::<Result<Vec<_>, _>>();
    let e = &graph.edges;
    Ok(vec![
        (
            "nodes.arrow",
            named(vec![
                (
                    "kind",
                    Arc::new(UInt8Array::from_iter_values(
                        graph.kind.iter().map(|&k| k as u8),
                    )),
                ),
                ("province", u32s(indices(&graph.province)?)),
                ("lon", f64s(graph.lon.clone())),
                ("lat", f64s(graph.lat.clone())),
                (
                    "label",
                    strings(graph.label.iter().map(String::as_str).collect()),
                ),
                ("sea_zone", u32s(indices(&graph.zone)?)),
            ])?,
        ),
        (
            "edges.arrow",
            named(vec![
                (
                    "a",
                    u32s(
                        e.iter()
                            .map(|x| index(Some(x.a)))
                            .collect::<Result<_, _>>()?,
                    ),
                ),
                (
                    "b",
                    u32s(
                        e.iter()
                            .map(|x| index(Some(x.b)))
                            .collect::<Result<_, _>>()?,
                    ),
                ),
                (
                    "mode",
                    Arc::new(UInt8Array::from_iter_values(e.iter().map(|x| x.mode as u8))),
                ),
                ("length_km", f64s(e.iter().map(|x| x.length_km).collect())),
                ("tri_m", f64s(e.iter().map(|x| x.tri_m).collect())),
                (
                    "chokepoint",
                    Arc::new(UInt8Array::from_iter_values(e.iter().map(|x| x.chokepoint))),
                ),
            ])?,
        ),
        (
            "sea_zones.arrow",
            named(vec![(
                "h3",
                strings(zones.iter().map(String::as_str).collect()),
            )])?,
        ),
        (
            "chokepoints.arrow",
            named(vec![
                (
                    "name",
                    strings(chokepoints.iter().map(|c| c.name.as_str()).collect()),
                ),
                (
                    "canal",
                    Arc::new(BooleanArray::from(
                        chokepoints.iter().map(|c| c.canal).collect::<Vec<_>>(),
                    )),
                ),
            ])?,
        ),
    ])
}

#[cfg(feature = "geos")]
pub use graph::build;

#[cfg(feature = "geos")]
mod graph {
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::Path;

    use geo::{Coord, Geometry as Shape};
    use geos::{CoordSeq, CoordType, Geom, Geometry};
    use h3o::{CellIndex, Resolution};
    use rstar::{AABB, RTree, RTreeObject};

    use super::*;
    use crate::geo::overlay::{self, from_geo, is_empty, union_all};
    use crate::geo::provinces::{Centres, Province};
    use crate::geo::sphere::cos_lat;
    use crate::read::natural_earth::{Dbf, shapes_from_zip};
    use crate::read::{cell, float, open_csv};

    fn fail(e: geos::Error) -> String {
        e.to_string()
    }

    struct Boxed(usize, AABB<[f64; 2]>);

    impl RTreeObject for Boxed {
        type Envelope = AABB<[f64; 2]>;
        fn envelope(&self) -> Self::Envelope {
            self.1
        }
    }

    /// The envelope of a geometry, grown by `pad` on every side.
    fn envelope(g: &impl Geom, pad: f64) -> Result<AABB<[f64; 2]>, String> {
        Ok(AABB::from_corners(
            [
                g.get_x_min().map_err(fail)? - pad,
                g.get_y_min().map_err(fail)? - pad,
            ],
            [
                g.get_x_max().map_err(fail)? + pad,
                g.get_y_max().map_err(fail)? + pad,
            ],
        ))
    }

    /// The envelopes of the geometries, as an STRtree holds them: empty ones left out.
    struct Index(RTree<Boxed>);

    impl Index {
        fn new(geoms: &[Geometry]) -> Result<Index, String> {
            let mut boxes = Vec::with_capacity(geoms.len());
            for (i, g) in geoms.iter().enumerate() {
                if !is_empty(g)? {
                    boxes.push(Boxed(i, envelope(g, 0.0)?));
                }
            }
            Ok(Index(RTree::bulk_load(boxes)))
        }

        /// Indices whose envelope meets `env`, ascending.
        fn meeting(&self, env: &AABB<[f64; 2]>) -> Vec<usize> {
            let mut out: Vec<usize> = self
                .0
                .locate_in_envelope_intersecting(env)
                .map(|b| b.0)
                .collect();
            out.sort_unstable();
            out
        }
    }

    /// Segments per quarter circle of `BaseGeometry.buffer`, which transport.py calls (the
    /// function `shapely.buffer` takes 8).
    const QUAD_SEGS: i32 = 16;

    fn buffer(g: &impl Geom, distance: f64) -> geos::GResult<Geometry> {
        g.buffer(distance, QUAD_SEGS)
    }

    fn point(x: f64, y: f64) -> Result<Geometry, String> {
        CoordSeq::new_from_buffer(&[x, y], 1, CoordType::XY)
            .and_then(Geometry::create_point)
            .map_err(fail)
    }

    fn line(l: &geo::LineString) -> Result<Geometry, String> {
        let flat: Vec<f64> = l.0.iter().flat_map(|c| [c.x, c.y]).collect();
        CoordSeq::new_from_buffer(&flat, l.0.len(), CoordType::XY)
            .and_then(Geometry::create_line_string)
            .map_err(fail)
    }

    /// A shapefile's lines, polygons or points as GEOS geometries; null shapes left out.
    fn geos_shapes(path: &Path) -> Result<Vec<Geometry>, String> {
        let mut out = Vec::new();
        for s in shapes_from_zip(path)?.into_iter().flatten() {
            out.push(match &s {
                Shape::LineString(l) => line(l)?,
                Shape::MultiLineString(m) => Geometry::create_multiline_string(
                    m.0.iter().map(line).collect::<Result<_, _>>()?,
                )
                .map_err(fail)?,
                Shape::Point(p) => point(p.x(), p.y())?,
                s => from_geo(s)?,
            });
        }
        Ok(out)
    }

    /// Province containing each point (the lowest index if several), else the nearest within
    /// `max_deg` (the lowest index on a tie), else None.
    fn locate(
        points: &[Coord],
        geoms: &[Geometry],
        index: &Index,
        max_deg: f64,
    ) -> Result<Vec<Option<usize>>, String> {
        let mut out = Vec::with_capacity(points.len());
        for c in points {
            let p = point(c.x, c.y)?;
            let prepared = p.to_prepared_geom().map_err(fail)?;
            let mut found = None;
            for g in index.meeting(&envelope(&p, 0.0)?) {
                if prepared.within(&geoms[g]).map_err(fail)? {
                    found = Some(g);
                    break;
                }
            }
            if found.is_none() {
                // The lowest index on a tie; shapely takes GEOS's STRtree traversal order.
                let mut best: Option<(usize, f64)> = None;
                for g in index.meeting(&envelope(&p, max_deg)?) {
                    let d = geoms[g].distance(&p).map_err(fail)?;
                    if d <= max_deg && best.is_none_or(|(_, b)| d < b) {
                        best = Some((g, d));
                    }
                }
                found = best.map(|(g, _)| g);
            }
            out.push(found);
        }
        Ok(out)
    }

    /// Pairs of provinces sharing a land border, and whether a railway crosses it.
    fn adjacency(
        geoms: &[Geometry],
        index: &Index,
        rail: &[Geometry],
        rail_index: &Index,
    ) -> Result<Vec<(usize, usize, bool)>, String> {
        let mut pairs = BTreeSet::new();
        for (a, g) in geoms.iter().enumerate() {
            let prepared = g.to_prepared_geom().map_err(fail)?;
            for b in index.meeting(&envelope(g, 0.005)?) {
                if a < b && prepared.dwithin(&geoms[b], 0.005).map_err(fail)? {
                    pairs.insert((a, b));
                }
            }
        }
        let mut out = Vec::new();
        for (a, b) in pairs {
            let border = geoms[a]
                .boundary()
                .and_then(|edge| edge.intersection(&buffer(&geoms[b], 0.005)?))
                .map_err(fail)?;
            if is_empty(&border)? {
                continue;
            }
            let lat = (border.get_y_min().map_err(fail)? + border.get_y_max().map_err(fail)?) / 2.0;
            let km_per_deg = 111.2 * cos_lat(lat).max(0.5);
            // Enclaves smaller than the threshold (Vatican) still border their host.
            let perimeter = |g: &Geometry| g.length().map_err(fail);
            let smaller = perimeter(&geoms[a])?.min(perimeter(&geoms[b])?) * km_per_deg;
            if border.length().map_err(fail)? * km_per_deg < MIN_BORDER_KM.min(smaller / 4.0) {
                continue;
            }
            let zone = buffer(&border, 0.01).map_err(fail)?;
            let prepared = zone.to_prepared_geom().map_err(fail)?;
            let mut crossed = false;
            for r in rail_index.meeting(&envelope(&zone, 0.0)?) {
                if prepared.intersects(&rail[r]).map_err(fail)? {
                    crossed = true;
                    break;
                }
            }
            out.push((a, b, crossed));
        }
        Ok(out)
    }

    /// Cells whose centre is not on land, and their centres.
    #[derive(Default)]
    struct SeaGrid {
        cells: Vec<CellIndex>,
        lon: Vec<f64>,
        lat: Vec<f64>,
    }

    fn sea_grid(land: &Geometry, res: Resolution, centres: &Centres) -> Result<SeaGrid, String> {
        let prepared = land.to_prepared_geom().map_err(fail)?;
        let mut sea = SeaGrid::default();
        for c in all_cells(res) {
            let [x, y] = centres.lng_lat(c)?;
            if !prepared.contains_xy(x, y).map_err(fail)? {
                sea.cells.push(c);
                sea.lon.push(x);
                sea.lat.push(y);
            }
        }
        Ok(sea)
    }

    struct Port {
        province: usize,
        x: f64,
        y: f64,
        name: String,
        cell: usize,
        listed: bool,
    }

    fn nearest_one(x: f64, y: f64, to_lon: &[f64], to_lat: &[f64]) -> Result<(usize, f64), String> {
        nearest(&[x], &[y], to_lon, to_lat)
            .map(|v| v[0])
            .ok_or_else(|| "no sea cell to snap to".to_owned())
    }

    /// Builds the graph over `provinces` (their anchors and geometries, `tri_m` their mean
    /// ruggedness) from the Natural Earth railways, land and ports and the airports of `cache`,
    /// on a sea grid of H3 cells at `sea_res` whose centres `centres` gives, zoned by their
    /// parents at `zone_res`.
    pub fn build(
        provinces: &[Province],
        tri_m: &[f64],
        cache: &Path,
        sea_res: Resolution,
        zone_res: Resolution,
        centres: &Centres,
    ) -> Result<Transport, String> {
        let chokepoints = chokepoints()?;
        let mut g = Graph::default();
        let mut stats = Stats::default();
        let geoms = provinces
            .iter()
            .map(|p| from_geo(&p.geom))
            .collect::<Result<Vec<_>, _>>()?;
        let index = Index::new(&geoms)?;
        for (i, p) in provinces.iter().enumerate() {
            g.node(
                NodeKind::Province,
                Some(i),
                p.lon,
                p.lat,
                p.id.clone(),
                None,
            );
        }
        let plon: Vec<f64> = provinces.iter().map(|p| p.lon).collect();
        let plat: Vec<f64> = provinces.iter().map(|p| p.lat).collect();
        let span = |a: usize, b: usize| haversine_km(plon[a], plat[a], plon[b], plat[b]);

        // Land borders: road everywhere, rail where a line crosses the border.
        let rail = geos_shapes(&cache.join("ne_10m_railroads.zip"))?;
        let pairs = adjacency(&geoms, &index, &rail, &Index::new(&rail)?)?;
        let mut rail_node: BTreeMap<usize, usize> = BTreeMap::new();
        for &(a, b, crossed) in &pairs {
            g.edge(a, b, Mode::Road, span(a, b), (tri_m[a] + tri_m[b]) / 2.0, 0);
            if crossed {
                rail_node.insert(a, 0);
                rail_node.insert(b, 0);
            }
        }
        for (&p, node) in &mut rail_node {
            let label = format!("rail:{}", provinces[p].id);
            *node = g.node(NodeKind::Rail, Some(p), plon[p], plat[p], label, None);
            g.edge(p, *node, Mode::RailAccess, 0.0, 0.0, 0);
        }
        for &(a, b, crossed) in &pairs {
            if crossed {
                let tri = (tri_m[a] + tri_m[b]) / 2.0;
                g.edge(rail_node[&a], rail_node[&b], Mode::Rail, span(a, b), tri, 0);
            }
        }
        stats.land_borders = pairs.len();
        stats.rail_borders = pairs.iter().filter(|p| p.2).count();

        // Sea grid, chokepoint links, and components.
        let land_geoms = geos_shapes(&cache.join("ne_10m_land.zip"))?;
        let land = union_all(
            land_geoms
                .iter()
                .map(|l| Geom::clone(l).map_err(fail))
                .collect::<Result<_, _>>()?,
        )?;
        let SeaGrid {
            cells: sea_cells,
            lon: slon,
            lat: slat,
        } = sea_grid(&land, sea_res, centres)?;
        let n = sea_cells.len();
        let at = |c: CellIndex| sea_cells.binary_search(&c).ok();
        let mut sea_pairs = BTreeSet::new();
        for (i, &c) in sea_cells.iter().enumerate() {
            for nb in c.grid_disk::<Vec<_>>(1) {
                if let Some(j) = at(nb).filter(|&j| j > i) {
                    sea_pairs.insert((i, j));
                }
            }
        }
        let sea_pairs: Vec<(usize, usize)> = sea_pairs.into_iter().collect();
        let mut tag = vec![0u8; n];
        let mut links = Vec::new();
        for (k, cp) in chokepoints.iter().enumerate() {
            let k = u8::try_from(k + 1).map_err(|_| "more than 255 chokepoints")?;
            for i in 0..n {
                if tag[i] == 0 && haversine_km(slon[i], slat[i], cp.lon, cp.lat) <= cp.radius_km {
                    tag[i] = k;
                }
            }
            if let Some(path) = &cp.link {
                let (first, last) = (path[0], path[path.len() - 1]);
                let ends = nearest(&[first[0], last[0]], &[first[1], last[1]], &slon, &slat)
                    .ok_or("no sea cells")?;
                links.push((ends[0].0, ends[1].0, k, path_km(path), cp.canal));
            }
        }
        let mut joined = sea_pairs.clone();
        joined.extend(links.iter().map(|&(a, b, ..)| (a, b)));
        let comp = components(n, &joined);
        let mut sizes = vec![0usize; n];
        for &c in &comp {
            sizes[c] += 1;
        }

        // Ports: listed (Natural Earth) and minor ones for coastal provinces without a listed
        // port.
        let ports_zip = cache.join("ne_10m_ports.zip");
        let names: Vec<Option<String>> = Dbf::from_zip(&ports_zip)?
            .select(["name"])?
            .into_iter()
            .map(|[n]| n.map(str::to_owned))
            .collect();
        let shapes = shapes_from_zip(&ports_zip)?;
        if shapes.len() != names.len() {
            return Err(format!(
                "{}: {} shapes but {} records",
                ports_zip.display(),
                shapes.len(),
                names.len()
            ));
        }
        let mut found: Vec<(Coord, Option<String>)> = shapes
            .into_iter()
            .zip(names)
            .map(|(s, n)| match s {
                Some(Shape::Point(p)) => Ok((p.0, n)),
                s => Err(format!("{}: not a point: {s:?}", ports_zip.display())),
            })
            .collect::<Result<_, _>>()?;
        // Equal floats keep their order, as numpy's stable sorts and Python's sort leave them.
        let by = |x: f64, y: f64| x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal);
        found.sort_by(|(p, _), (q, _)| by(p.x, q.x).then(by(p.y, q.y)));
        let port_pts: Vec<Coord> = found.iter().map(|(c, _)| *c).collect();
        let owner = locate(&port_pts, &geoms, &index, 0.25)?;
        let big: Vec<usize> = (0..n)
            .filter(|&i| sizes[comp[i]] >= MIN_SEA_COMPONENT)
            .collect();
        let (blon, blat): (Vec<f64>, Vec<f64>) = big.iter().map(|&i| (slon[i], slat[i])).unzip();
        let xs: Vec<f64> = port_pts.iter().map(|c| c.x).collect();
        let ys: Vec<f64> = port_pts.iter().map(|c| c.y).collect();
        let snap = nearest(&xs, &ys, &blon, &blat).ok_or("no sea cell to snap to")?;
        let mut ports = Vec::new();
        for (i, ((c, name), &(j, d))) in found.into_iter().zip(&snap).enumerate() {
            if let Some(p) = owner[i]
                && d <= MAX_PORT_SNAP_KM
            {
                ports.push(Port {
                    province: p,
                    x: c.x,
                    y: c.y,
                    name: name.unwrap_or_else(|| format!("port {i}")),
                    cell: big[j],
                    listed: true,
                });
            }
        }
        stats.ports_dropped = port_pts.len() - ports.len();
        let mut coast = Vec::new();
        for l in &land_geoms {
            coast.extend(overlay::parts(&l.boundary().map_err(fail)?)?);
        }
        let coast_index = Index::new(&coast)?;
        let has_port: BTreeSet<usize> = ports.iter().map(|p| p.province).collect();
        let mut coastal = vec![false; provinces.len()];
        for (i, g) in geoms.iter().enumerate() {
            let prepared = g.to_prepared_geom().map_err(fail)?;
            for c in coast_index.meeting(&envelope(g, 0.01)?) {
                if prepared.dwithin(&coast[c], 0.01).map_err(fail)? {
                    coastal[i] = true;
                    break;
                }
            }
        }
        for i in (0..provinces.len()).filter(|&i| coastal[i] && !has_port.contains(&i)) {
            let (j, _) = nearest_one(plon[i], plat[i], &blon, &blat)?;
            let target = point(blon[j], blat[j])?;
            let near = geoms[i].nearest_points(&target).map_err(fail)?;
            let (x, y) = (near.get_x(0).map_err(fail)?, near.get_y(0).map_err(fail)?);
            let (j, d) = nearest_one(x, y, &blon, &blat)?;
            if d <= MAX_PORT_SNAP_KM {
                ports.push(Port {
                    province: i,
                    x,
                    y,
                    name: format!("minor port of {}", provinces[i].name),
                    cell: big[j],
                    listed: false,
                });
            }
        }
        let largest = (0..n).fold(0, |m, i| if sizes[i] > sizes[m] { i } else { m });
        let used: BTreeSet<usize> = ports
            .iter()
            .map(|p| comp[p.cell])
            .chain(std::iter::once(comp[largest]))
            .collect();
        let keep: Vec<bool> = (0..n).map(|i| used.contains(&comp[i])).collect();

        // Sea nodes, zones and edges.
        let parent = |c: CellIndex| {
            c.parent(zone_res)
                .ok_or_else(|| format!("{c} has no parent at {zone_res}"))
        };
        let mut zone_cells = BTreeSet::new();
        for i in (0..n).filter(|&i| keep[i]) {
            zone_cells.insert(parent(sea_cells[i])?);
        }
        let zone_cells: Vec<CellIndex> = zone_cells.into_iter().collect();
        let mut sea_node = vec![usize::MAX; n];
        for i in (0..n).filter(|&i| keep[i]) {
            let zone = zone_cells.binary_search(&parent(sea_cells[i])?).ok();
            let label = sea_cells[i].to_string();
            sea_node[i] = g.node(NodeKind::Sea, None, slon[i], slat[i], label, zone);
        }
        for &(a, b) in &sea_pairs {
            if keep[a] && keep[b] {
                let length = haversine_km(slon[a], slat[a], slon[b], slat[b]);
                let cp = if tag[a] == tag[b] { tag[a] } else { 0 };
                g.edge(sea_node[a], sea_node[b], Mode::Sea, length, 0.0, cp);
            }
        }
        for &(a, b, k, length, canal) in &links {
            if keep[a] && keep[b] {
                let mode = if canal { Mode::Canal } else { Mode::Sea };
                g.edge(sea_node[a], sea_node[b], mode, length, 0.0, k);
            }
        }

        ports.sort_by(|p, q| {
            (p.province, !p.listed)
                .cmp(&(q.province, !q.listed))
                .then(by(p.x, q.x))
                .then(by(p.y, q.y))
        });
        for p in &ports {
            let node = g.node(
                NodeKind::Port,
                Some(p.province),
                p.x,
                p.y,
                p.name.clone(),
                None,
            );
            let access = if p.listed {
                Mode::PortAccess
            } else {
                Mode::MinorPortAccess
            };
            let length = haversine_km(plon[p.province], plat[p.province], p.x, p.y);
            g.edge(p.province, node, access, length, 0.0, 0);
            let length = haversine_km(p.x, p.y, slon[p.cell], slat[p.cell]);
            g.edge(node, sea_node[p.cell], Mode::Sea, length, 0.0, 0);
        }
        stats.ports = ports.iter().filter(|p| p.listed).count();
        stats.minor_ports = ports.len() - stats.ports;

        // Airports: large airports with scheduled service, linked to near neighbours and
        // across continents.
        let path = cache.join("airports.csv");
        let origin = path.display().to_string();
        let mut csv = open_csv(&path)?;
        let [kind, service, ident, lon, lat, continent] = [
            "type",
            "scheduled_service",
            "ident",
            "longitude_deg",
            "latitude_deg",
            "continent",
        ]
        .map(|c| csv.col(c));
        let (kind, service, ident) = (kind?, service?, ident?);
        let (lon, lat, continent) = (lon?, lat?, continent?);
        let mut row = csv::StringRecord::new();
        let mut rows = Vec::new();
        while csv.next(&mut row)? {
            if cell(&row, kind) == "large_airport" && cell(&row, service) == "yes" {
                rows.push((
                    cell(&row, ident).to_owned(),
                    cell(&row, lon).to_owned(),
                    cell(&row, lat).to_owned(),
                    cell(&row, continent).to_owned(),
                ));
            }
        }
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        let mut sites = Vec::with_capacity(rows.len());
        for (_, x, y, _) in &rows {
            sites.push(Coord {
                x: float(x, &origin)?,
                y: float(y, &origin)?,
            });
        }
        let aowner = locate(&sites, &geoms, &index, 0.25)?;
        let air: Vec<(usize, usize)> = aowner
            .iter()
            .enumerate()
            .filter_map(|(i, o)| Some((i, (*o)?)))
            .collect();
        let mut air_node = Vec::with_capacity(air.len());
        for &(i, p) in &air {
            let (x, y) = (sites[i].x, sites[i].y);
            air_node.push(g.node(NodeKind::Airport, Some(p), x, y, rows[i].0.clone(), None));
            let length = haversine_km(plon[p], plat[p], x, y);
            g.edge(
                p,
                air_node[air_node.len() - 1],
                Mode::AirAccess,
                length,
                0.0,
                0,
            );
        }
        let m = air.len();
        let d: Vec<Vec<f64>> = (0..m)
            .map(|a| {
                (0..m)
                    .map(|b| {
                        let (p, q) = (sites[air[a].0], sites[air[b].0]);
                        if a == b {
                            f64::INFINITY
                        } else {
                            haversine_km(p.x, p.y, q.x, q.y)
                        }
                    })
                    .collect()
            })
            .collect();
        let mut by_continent: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
        for (a, &(i, _)) in air.iter().enumerate() {
            by_continent.entry(rows[i].3.as_str()).or_default().push(a);
        }
        // The first `k` of `among` by distance from `a`, a stable argsort.
        let closest = |a: usize, among: &[usize], k: usize| {
            let mut order = among.to_vec();
            order.sort_by(|&x, &y| by(d[a][x], d[a][y]));
            order.truncate(k);
            order
        };
        let everyone: Vec<usize> = (0..m).collect();
        let mut links_air = BTreeSet::new();
        for a in 0..m {
            for b in closest(a, &everyone, AIR_NEAREST) {
                links_air.insert((a.min(b), a.max(b)));
            }
            let own = rows[air[a].0].3.as_str();
            for (_, other) in by_continent.iter().filter(|(c, _)| **c != own) {
                for b in closest(a, other, AIR_PER_CONTINENT) {
                    links_air.insert((a.min(b), a.max(b)));
                }
            }
        }
        for &(a, b) in &links_air {
            g.edge(air_node[a], air_node[b], Mode::Air, d[a][b], 0.0, 0);
        }
        stats.airports = m;
        stats.air_links = links_air.len();

        g.sort_edges();
        stats.coastal = coastal.iter().filter(|&&c| c).count();
        Ok(Transport {
            graph: g,
            zones: zone_cells.iter().map(ToString::to_string).collect(),
            stats,
            coastal,
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Shapely's `box(0, 0, 1, 1).buffer(0.5)` on GEOS 3.15: 69 ring points.
        #[test]
        fn buffers_take_the_segments_of_the_buffer_method() {
            let square = overlay::rectangle(0.0, 0.0, 1.0, 1.0).unwrap();
            let b = buffer(&square, 0.5).unwrap();
            let ring = b.get_exterior_ring().unwrap();
            assert_eq!(ring.get_num_points().unwrap(), 69);
            assert_eq!(b.area().unwrap(), 3.7841371226364857);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_link_endpoint_is_named_and_ordered() {
        for cp in chokepoints().unwrap() {
            assert!(cp.radius_km > 0.0 && !cp.name.is_empty(), "{cp:?}");
            assert!(cp.link.as_ref().is_none_or(|l| l.len() >= 2), "{cp:?}");
        }
    }

    #[test]
    fn chokepoints_need_a_name_and_links_two_points() {
        let parse = |text: &str| parse_chokepoints(&text.parse().unwrap());
        let ok = "[[chokepoint]]\nname = \"A\"\nlon = 1\nlat = 2.5\nradius_km = 3\n";
        let cp = parse(ok).unwrap();
        assert_eq!((cp[0].lon, cp[0].radius_km, cp[0].canal), (1.0, 3.0, false));
        let unnamed = "[[chokepoint]]\nlon = 1\nlat = 2\nradius_km = 3\n";
        assert_eq!(parse(unnamed).unwrap_err(), "chokepoint 0: no name");
        let short = format!("{ok}link = [[1.0, 2.0]]\n");
        assert_eq!(
            parse(&short).unwrap_err(),
            "A: a link needs two points or more"
        );
        let long = format!("{ok}link = [[1.0, 2.0], [3, 4]]\n");
        assert_eq!(
            parse(&long).unwrap()[0].link,
            Some(vec![[1.0, 2.0], [3.0, 4.0]])
        );
    }

    #[test]
    fn nearest_takes_the_first_target_on_a_tie() {
        let got = nearest(
            &[0.0, 5.0],
            &[0.0, 0.0],
            &[1.0, -1.0, 4.0],
            &[0.0, 0.0, 0.0],
        )
        .unwrap();
        assert_eq!(got[0].0, 0);
        assert_eq!(got[0].1, haversine_km(0.0, 0.0, 1.0, 0.0));
        assert_eq!(got[1].0, 2);
        assert!(nearest(&[0.0], &[0.0], &[], &[]).is_none());
    }

    #[test]
    fn components_are_named_by_their_lowest_member() {
        assert_eq!(
            components(7, &[(5, 6), (1, 3), (3, 6), (2, 4)]),
            [0, 1, 2, 1, 2, 1, 1]
        );
    }

    #[test]
    fn edges_are_undirected_and_sorted_as_tuples() {
        let mut g = Graph::default();
        g.edge(3, 1, Mode::Sea, 2.0, 0.0, 0);
        g.edge(1, 3, Mode::Road, 5.0, 1.0, 0);
        g.edge(0, 2, Mode::Sea, 1.0, 0.0, 2);
        g.edge(1, 3, Mode::Road, 5.0, 0.5, 0);
        g.sort_edges();
        let key: Vec<_> = g
            .edges
            .iter()
            .map(|e| (e.a, e.b, e.mode, e.tri_m))
            .collect();
        assert_eq!(
            key,
            [
                (0, 2, Mode::Sea, 0.0),
                (1, 3, Mode::Road, 0.5),
                (1, 3, Mode::Road, 1.0),
                (1, 3, Mode::Sea, 0.0)
            ]
        );
    }

    #[test]
    fn link_length_sums_its_legs() {
        let path = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]];
        let legs = haversine_km(0.0, 0.0, 1.0, 0.0) + haversine_km(1.0, 0.0, 1.0, 1.0);
        assert_eq!(path_km(&path), legs);
    }

    #[test]
    fn all_cells_are_sorted_as_their_ids() {
        let cells = all_cells(Resolution::One);
        assert_eq!(cells.len(), 842);
        let ids: Vec<String> = cells.iter().map(ToString::to_string).collect();
        assert!(
            ids.windows(2)
                .all(|w| w[0] < w[1] && w[0].len() == w[1].len())
        );
    }

    #[test]
    fn tables_mark_missing_provinces_and_zones() {
        let mut g = Graph::default();
        let p = g.node(NodeKind::Province, Some(0), 1.0, 2.0, "P".into(), None);
        let s = g.node(
            NodeKind::Sea,
            None,
            3.0,
            4.0,
            "81003ffffffffff".into(),
            Some(0),
        );
        g.edge(s, p, Mode::Sea, 7.5, 0.0, 1);
        let t = tables(&g, &["81003ffffffffff".into()], &[]).unwrap();
        let names: Vec<_> = t.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            names,
            [
                "nodes.arrow",
                "edges.arrow",
                "sea_zones.arrow",
                "chokepoints.arrow"
            ]
        );
        let nodes = &t[0].1;
        let col = |b: &RecordBatch, c: &str| {
            b.column_by_name(c)
                .unwrap()
                .as_any()
                .downcast_ref::<UInt32Array>()
                .unwrap()
                .values()
                .to_vec()
        };
        assert_eq!(col(nodes, "province"), [0, u32::MAX]);
        assert_eq!(col(nodes, "sea_zone"), [u32::MAX, 0]);
        assert_eq!((col(&t[1].1, "a"), col(&t[1].1, "b")), (vec![0], vec![1]));
    }
}
