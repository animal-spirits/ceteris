//! GPU-ready fill buffers for the map (ADR 0010): triangulated polygons in Web Mercator.
//!
//! The map uploads a file once and recolours features through a texture indexed by feature, so
//! an update never re-tessellates or rewrites vertex data. Layout, all little-endian:
//!
//! ```text
//! magic   b"CFIL"
//! u32     format version (1)
//! u32     vertices V
//! u32     indices I (a multiple of 3)
//! u32     features F
//! f32     x, y for each vertex (Web Mercator, 0 to 1, y down), 2V values
//! u32     feature index of each vertex, V values
//! u32     triangle indices, I values
//! ```

use std::collections::BTreeMap;
use std::f64::consts::PI;
use std::path::Path;

use earcut::Earcut;
use geo::{MultiPolygon, Polygon};

pub const MAGIC: &[u8; 4] = b"CFIL";
pub const VERSION: u32 = 1;
pub const MAX_LAT: f64 = 85.0511287798066;
/// On the globe a triangle is drawn as a flat chord; edges longer than this (in Mercator units,
/// about 1.4 degrees at the equator) are split so fills stay on the surface.
pub const MAX_EDGE: f64 = 0.004;

pub fn mercator(lon: f64, lat: f64) -> [f64; 2] {
    let lat = lat.clamp(-MAX_LAT, MAX_LAT);
    let x = (lon + 180.0) / 360.0;
    let y = 0.5 - (PI / 4.0 + lat.to_radians() / 2.0).tan().ln() / (2.0 * PI);
    [x, y]
}

/// Vertices (projected) and triangle indices of one polygon with holes; rings of fewer than
/// three distinct vertices are left out.
pub fn triangulate(poly: &Polygon) -> (Vec<[f64; 2]>, Vec<u32>) {
    let rings = std::iter::once(poly.exterior()).chain(poly.interiors());
    let mut xy = Vec::new();
    let mut starts = Vec::new();
    for ring in rings {
        let open = &ring.0[..ring.0.len().saturating_sub(1)];
        if open.len() < 3 {
            continue;
        }
        starts.push(xy.len() as u32);
        xy.extend(open.iter().map(|c| mercator(c.x, c.y)));
    }
    let mut tri = Vec::new();
    if !starts.is_empty() {
        Earcut::new().earcut(xy.iter().copied(), &starts[1..], &mut tri);
    }
    (xy, tri)
}

fn squared(a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (a[0] - b[0], a[1] - b[1]);
    dx * dx + dy * dy
}

/// Splits long edges at their midpoints until none exceeds `max_edge`.
///
/// Whether an edge splits depends only on the edge, so the two triangles sharing it split it the
/// same way and no cracks open between them.
pub fn subdivide(mut xy: Vec<[f64; 2]>, tri: Vec<u32>, max_edge: f64) -> (Vec<[f64; 2]>, Vec<u32>) {
    let short = |t: &[u32; 3]| {
        (0..3).all(|k| squared(xy[t[k] as usize], xy[t[(k + 2) % 3] as usize]).sqrt() <= max_edge)
    };
    if tri.as_chunks::<3>().0.iter().all(short) {
        return (xy, tri);
    }
    let limit = max_edge * max_edge;
    let mut mid: BTreeMap<(u32, u32), u32> = BTreeMap::new();
    let mut midpoint = |xy: &mut Vec<[f64; 2]>, a: u32, b: u32| -> u32 {
        *mid.entry((a.min(b), a.max(b))).or_insert_with(|| {
            let (p, q) = (xy[a as usize], xy[b as usize]);
            xy.push([(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0]);
            (xy.len() - 1) as u32
        })
    };
    let long = |xy: &[[f64; 2]], a: u32, b: u32| squared(xy[a as usize], xy[b as usize]) > limit;
    let mut out = Vec::with_capacity(tri.len());
    let mut stack: Vec<[u32; 3]> = tri.as_chunks::<3>().0.to_vec();
    while let Some([mut a, mut b, mut c]) = stack.pop() {
        let (la, lb, lc) = (long(&xy, a, b), long(&xy, b, c), long(&xy, c, a));
        if !(la || lb || lc) {
            out.extend([a, b, c]);
        } else if la && lb && lc {
            let ab = midpoint(&mut xy, a, b);
            let bc = midpoint(&mut xy, b, c);
            let ca = midpoint(&mut xy, c, a);
            stack.extend([[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]]);
        } else {
            // Rotate so (a, b) is long, then split it; the other edges are handled on recursion.
            while !long(&xy, a, b) {
                (a, b, c) = (b, c, a);
            }
            let m = midpoint(&mut xy, a, b);
            stack.extend([[a, m, c], [m, b, c]]);
        }
    }
    (xy, out)
}

/// Vertex and triangle counts of a written fill file.
#[derive(Debug, PartialEq)]
pub struct Stats {
    pub vertices: usize,
    pub triangles: usize,
}

/// Writes the fill buffers of `geoms`, one feature each, to `path`.
pub fn write(path: &Path, geoms: &[MultiPolygon]) -> Result<Stats, String> {
    let (mut xy, mut feature, mut index) = (Vec::new(), Vec::new(), Vec::new());
    for (f, g) in geoms.iter().enumerate() {
        for poly in g {
            let (v, t) = triangulate(poly);
            if t.is_empty() {
                continue;
            }
            let (v, t) = subdivide(v, t, MAX_EDGE);
            let base = xy.len() as u32;
            feature.extend(std::iter::repeat_n(f as u32, v.len()));
            index.extend(t.iter().map(|i| i + base));
            xy.extend(v);
        }
    }
    let count = |n: usize| u32::try_from(n).map_err(|e| format!("{}: {e}", path.display()));
    let mut bytes = MAGIC.to_vec();
    for v in [
        VERSION,
        count(xy.len())?,
        count(index.len())?,
        count(geoms.len())?,
    ] {
        bytes.extend(v.to_le_bytes());
    }
    bytes.extend(xy.iter().flatten().flat_map(|&v| (v as f32).to_le_bytes()));
    bytes.extend(feature.iter().flat_map(|v| v.to_le_bytes()));
    bytes.extend(index.iter().flat_map(|v| v.to_le_bytes()));
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(Stats {
        vertices: xy.len(),
        triangles: index.len() / 3,
    })
}

/// A decoded fill file.
#[derive(Debug, PartialEq)]
pub struct Fill {
    pub xy: Vec<[f32; 2]>,
    pub feature: Vec<u32>,
    pub index: Vec<u32>,
    pub features: u32,
}

pub fn read(path: &Path) -> Result<Fill, String> {
    let fail = |e: &str| format!("{}: {e}", path.display());
    let data = std::fs::read(path).map_err(|e| fail(&e.to_string()))?;
    if data.get(..4) != Some(MAGIC.as_slice()) || data.len() < 20 {
        return Err(fail("not a fill file"));
    }
    let words: Vec<u32> = data[4..]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| u32::from_le_bytes(*b))
        .collect();
    let [version, v, i, f] = [words[0], words[1], words[2], words[3]];
    if version != VERSION {
        return Err(fail(&format!("format version {version}")));
    }
    let (v, i) = (v as usize, i as usize);
    if data.len() != 20 + 12 * v + 4 * i {
        return Err(fail("length does not match the header"));
    }
    let body = &words[4..];
    Ok(Fill {
        xy: body[..2 * v]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| p.map(f32::from_bits))
            .collect(),
        feature: body[2 * v..3 * v].to_vec(),
        index: body[3 * v..].to_vec(),
        features: f,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::{Area, BooleanOps, LineString, Rect, coord};

    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Polygon {
        Rect::new(coord! { x: x0, y: y0 }, coord! { x: x1, y: y1 }).to_polygon()
    }

    fn tri_area(xy: &[[f64; 2]], t: &[u32]) -> f64 {
        t.as_chunks::<3>()
            .0
            .iter()
            .map(|&[a, b, c]| {
                let (a, b, c) = (xy[a as usize], xy[b as usize], xy[c as usize]);
                ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])).abs() / 2.0
            })
            .sum()
    }

    fn projected(ring: &LineString) -> Polygon {
        Polygon::new(
            ring.0
                .iter()
                .map(|c| {
                    let [x, y] = mercator(c.x, c.y);
                    coord! { x: x, y: y }
                })
                .collect(),
            vec![],
        )
    }

    #[test]
    fn fill_buffers_round_trip_and_cover_their_polygons() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.fill");
        let donut = square(0.0, 0.0, 20.0, 10.0).difference(&square(5.0, 2.0, 8.0, 6.0));
        let geoms = [
            donut,
            MultiPolygon(vec![
                square(30.0, 30.0, 31.0, 31.0),
                square(40.0, 30.0, 42.0, 31.0),
            ]),
        ];
        let stats = write(&path, &geoms).unwrap();
        let fill = read(&path).unwrap();
        assert_eq!(fill.features, 2);
        assert_eq!(fill.feature.len(), fill.xy.len());
        assert_eq!(stats.vertices, fill.xy.len());
        assert_eq!(stats.triangles * 3, fill.index.len());
        assert_eq!(
            fill.feature
                .iter()
                .collect::<std::collections::BTreeSet<_>>(),
            [0, 1].iter().collect()
        );
        let xy: Vec<[f64; 2]> = fill.xy.iter().map(|p| p.map(f64::from)).collect();
        for (f, g) in geoms.iter().enumerate() {
            let mine: Vec<u32> = fill
                .index
                .as_chunks::<3>()
                .0
                .iter()
                .filter(|t| t.iter().all(|&i| fill.feature[i as usize] == f as u32))
                .flatten()
                .copied()
                .collect();
            let expected: f64 = g
                .iter()
                .map(|p| {
                    projected(p.exterior()).unsigned_area()
                        - p.interiors()
                            .iter()
                            .map(|r| projected(r).unsigned_area())
                            .sum::<f64>()
                })
                .sum();
            let got = tri_area(&xy, &mine);
            assert!((got / expected - 1.0).abs() < 1e-4, "{got} vs {expected}");
        }
    }

    #[test]
    fn read_rejects_a_file_longer_or_shorter_than_its_header_says() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.fill");
        let square = MultiPolygon(vec![
            Rect::new(coord! { x: 0.0, y: 0.0 }, coord! { x: 1.0, y: 1.0 }).to_polygon(),
        ]);
        write(&path, &[square]).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(read(&path).is_ok());
        for bad in [
            &bytes[..bytes.len() - 1],
            &[&bytes[..], &[0][..]].concat(),
            &[&bytes[..], &[0; 4][..]].concat(),
        ] {
            std::fs::write(&path, bad).unwrap();
            assert!(
                read(&path).unwrap_err().contains("length does not match"),
                "{}",
                bad.len()
            );
        }
    }

    #[test]
    fn subdivision_bounds_edges_and_leaves_no_cracks() {
        // Two triangles sharing a long diagonal must split it at the same points.
        let xy = vec![[0.0, 0.0], [0.03, 0.0], [0.03, 0.02], [0.0, 0.02]];
        let (v, t) = subdivide(xy, vec![0, 1, 2, 0, 2, 3], MAX_EDGE);
        let longest = t
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|&[a, b, c]| [(a, b), (b, c), (c, a)])
            .map(|(a, b)| squared(v[a as usize], v[b as usize]).sqrt())
            .fold(0.0, f64::max);
        assert!(longest <= MAX_EDGE);
        assert!((tri_area(&v, &t) - 0.03 * 0.02).abs() < 1e-15);
        let mut edges: BTreeMap<(u32, u32), u32> = BTreeMap::new();
        for &[a, b, c] in t.as_chunks::<3>().0 {
            for (p, q) in [(a, b), (b, c), (c, a)] {
                *edges.entry((p.min(q), p.max(q))).or_default() += 1;
            }
        }
        // Each boundary edge lies on the square's outline: no T-junction gaps inside.
        let on_outline = |p: [f64; 2]| {
            [p[0], p[0] - 0.03, p[1], p[1] - 0.02]
                .iter()
                .any(|d| d.abs() < 1e-12)
        };
        for ((a, b), k) in edges {
            if k == 1 {
                assert!(on_outline(v[a as usize]) && on_outline(v[b as usize]));
            }
        }
    }

    #[test]
    fn short_edges_leave_the_triangulation_as_it_is() {
        let xy = vec![[0.0, 0.0], [0.001, 0.0], [0.0, 0.001]];
        assert_eq!(
            subdivide(xy.clone(), vec![0, 1, 2], MAX_EDGE),
            (xy, vec![0, 1, 2])
        );
    }

    #[test]
    fn mercator_clamps_the_poles_and_maps_the_equator_to_the_middle() {
        assert_eq!(mercator(0.0, 0.0), [0.5, 0.5]);
        assert_eq!(mercator(-180.0, 90.0), mercator(-180.0, MAX_LAT));
        let [_, top] = mercator(0.0, 90.0);
        assert!(top.abs() < 1e-12);
    }
}
