//! Spherical helpers on WGS84 lon/lat degrees, with a spherical Earth of mean radius.

use geo::{
    Area, BoundingRect, Centroid, Contains, Coord, LineString, MultiPolygon, Point, Polygon,
    Winding,
};

use crate::numeric::{pairwise_sum, py_sum};

pub const R_KM: f64 = 6371.0088;

/// Great-circle distance in km.
pub fn haversine_km(lon1: f64, lat1: f64, lon2: f64, lat2: f64) -> f64 {
    let (lon1, lat1, lon2, lat2) = (
        lon1.to_radians(),
        lat1.to_radians(),
        lon2.to_radians(),
        lat2.to_radians(),
    );
    let s = ((lat2 - lat1) / 2.0).sin();
    let t = ((lon2 - lon1) / 2.0).sin();
    let a = s * s + lat1.cos() * lat2.cos() * (t * t);
    2.0 * R_KM * a.min(1.0).sqrt().asin()
}

/// Signed area of a closed ring: Chamberlain and Duquette (2007), the spherical-excess
/// approximation used by d3 and turf.
fn ring_area(ring: &LineString) -> f64 {
    let terms: Vec<f64> = ring
        .0
        .windows(2)
        .map(|w| {
            let (a, b) = (w[0], w[1]);
            (b.x.to_radians() - a.x.to_radians())
                * (2.0 + a.y.to_radians().sin() + b.y.to_radians().sin())
        })
        .collect();
    pairwise_sum(&terms) * (R_KM * R_KM) / 2.0
}

/// Area in km² of the polygon parts, holes subtracted.
pub fn area_km2(geom: &MultiPolygon) -> f64 {
    let mut total = 0.0;
    for poly in geom {
        total += ring_area(poly.exterior()).abs();
        total -= py_sum(poly.interiors().iter().map(|r| ring_area(r).abs()));
    }
    total
}

pub fn crosses_antimeridian(geom: &MultiPolygon) -> bool {
    geom.bounding_rect()
        .is_some_and(|r| r.min().x < -170.0 && r.max().x > 170.0)
}

/// Transport anchor: centroid of the largest part, or a point inside it if the centroid falls
/// outside. None for a geometry without parts.
pub fn anchor(geom: &MultiPolygon) -> Option<(f64, f64)> {
    let mut parts = geom.iter();
    let mut largest = parts.next()?;
    for p in parts {
        if p.unsigned_area() > largest.unsigned_area() {
            largest = p;
        }
    }
    let c = centroid(largest).or_else(|| largest.centroid().map(|c| c.0))?;
    let c = if largest.contains(&Point::from(c)) {
        c
    } else {
        interior_point(largest)
    };
    Some((c.x, c.y))
}

/// GEOS's `Centroid` of a polygon with area, sum for sum: triangles fanned from the first
/// shell vertex, signed by ring orientation. None for a polygon without area; the caller then
/// takes geo's centroid, which need not match GEOS's line or point centroid bit for bit.
fn centroid(poly: &Polygon) -> Option<Coord> {
    let base = *poly.exterior().0.first()?;
    let (mut cx, mut cy, mut area2) = (0.0, 0.0, 0.0);
    let mut add = |ring: &LineString, positive: bool| {
        let sign = if positive { 1.0 } else { -1.0 };
        for w in ring.0.windows(2) {
            let (p1, p2) = (w[0], w[1]);
            let (tx, ty) = (base.x + p1.x + p2.x, base.y + p1.y + p2.y);
            let a2 = (p1.x - base.x) * (p2.y - base.y) - (p2.x - base.x) * (p1.y - base.y);
            cx += sign * a2 * tx;
            cy += sign * a2 * ty;
            area2 += sign * a2;
        }
    };
    let ccw = |r: &LineString| r.is_ccw();
    add(poly.exterior(), !ccw(poly.exterior()));
    for hole in poly.interiors() {
        add(hole, ccw(hole));
    }
    (area2 != 0.0).then(|| Coord {
        x: cx / 3.0 / area2,
        y: cy / 3.0 / area2,
    })
}

/// JTS `InteriorPointArea` of one polygon, as GEOS's `point_on_surface`: the midpoint of the
/// widest interval where a horizontal scan line crosses the polygon. The scan line lies halfway
/// between the two vertex ordinates closest to the envelope's centre from below and above, so it
/// meets no vertex.
pub fn interior_point(poly: &Polygon) -> Coord {
    let avg = |a: f64, b: f64| (a + b) / 2.0;
    let rings = || std::iter::once(poly.exterior()).chain(poly.interiors());
    let first = poly.exterior().0.first().copied().unwrap_or_default();
    let Some(env) = poly.bounding_rect() else {
        return first;
    };
    let (mut lo, mut hi) = (env.min().y, env.max().y);
    let centre = avg(lo, hi);
    for c in rings().flat_map(|r| &r.0) {
        if c.y <= centre {
            if c.y > lo {
                lo = c.y;
            }
        } else if c.y < hi {
            hi = c.y;
        }
    }
    let scan = avg(hi, lo);
    let mut crossings = Vec::new();
    for ring in rings() {
        let Some(r) = ring.bounding_rect() else {
            continue;
        };
        if scan < r.min().y || scan > r.max().y {
            continue;
        }
        for w in ring.0.windows(2) {
            let (p0, p1) = (w[0], w[1]);
            if (p0.y > scan && p1.y > scan) || (p0.y < scan && p1.y < scan) {
                continue;
            }
            // Horizontal edges do not cross; a vertex on the line counts for one edge only.
            if p0.y == p1.y || (p0.y == scan && p1.y < scan) || (p1.y == scan && p0.y < scan) {
                continue;
            }
            crossings.push(if p0.x == p1.x {
                p0.x
            } else {
                let m = (p1.y - p0.y) / (p1.x - p0.x);
                p0.x + (scan - p0.y) / m
            });
        }
    }
    crossings.sort_by(f64::total_cmp);
    let (mut best, mut width) = (first, 0.0);
    for pair in crossings.as_chunks::<2>().0 {
        let w = pair[1] - pair[0];
        if w > width {
            width = w;
            best = Coord {
                x: avg(pair[0], pair[1]),
                y: scan,
            };
        }
    }
    best
}

pub fn cos_lat(lat: f64) -> f64 {
    lat.to_radians().cos().max(0.05)
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::{Rect, coord, polygon};

    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Polygon {
        Rect::new(coord! { x: x0, y: y0 }, coord! { x: x1, y: y1 }).to_polygon()
    }

    #[test]
    fn area_of_one_degree_square_at_equator() {
        // 111.19 km x 110.57 km on the sphere of mean radius.
        let a = area_km2(&MultiPolygon(vec![square(0.0, 0.0, 1.0, 1.0)]));
        assert!((a / 12_363.7 - 1.0).abs() < 1e-3, "{a}");
    }

    #[test]
    fn holes_are_subtracted_whatever_their_orientation() {
        let outer = square(0.0, 0.0, 4.0, 4.0);
        let hole = square(1.0, 1.0, 2.0, 2.0);
        let whole = area_km2(&MultiPolygon(vec![outer.clone()]));
        let small = area_km2(&MultiPolygon(vec![hole.clone()]));
        for ring in [hole.exterior().clone(), {
            let mut r = hole.exterior().clone();
            r.0.reverse();
            r
        }] {
            let donut = Polygon::new(outer.exterior().clone(), vec![ring]);
            assert_eq!(area_km2(&MultiPolygon(vec![donut])), whole - small);
        }
    }

    #[test]
    fn antimeridian_crossing_needs_both_far_sides() {
        let both = MultiPolygon(vec![
            square(178.0, 60.0, 180.0, 62.0),
            square(-180.0, 60.0, -177.0, 62.0),
        ]);
        assert!(crosses_antimeridian(&both));
        assert!(!crosses_antimeridian(&MultiPolygon(vec![square(
            160.0, 0.0, 179.0, 1.0
        )])));
    }

    #[test]
    fn anchor_is_the_centroid_of_the_largest_part() {
        let g = MultiPolygon(vec![
            square(0.0, 0.0, 1.0, 1.0),
            square(10.0, 0.0, 14.0, 2.0),
        ]);
        assert_eq!(anchor(&g), Some((12.0, 1.0)));
        assert_eq!(anchor(&MultiPolygon(vec![])), None);
    }

    #[test]
    fn anchor_falls_back_to_a_point_inside_a_concave_part() {
        // A U shape: the centroid lies in the notch.
        let u = polygon![
            (x: 0.0, y: 0.0), (x: 3.0, y: 0.0), (x: 3.0, y: 3.0), (x: 2.0, y: 3.0),
            (x: 2.0, y: 1.0), (x: 1.0, y: 1.0), (x: 1.0, y: 3.0), (x: 0.0, y: 3.0),
        ];
        let c = u.centroid().unwrap();
        assert!(!u.contains(&c));
        let (x, y) = anchor(&MultiPolygon(vec![u.clone()])).unwrap();
        // Scan line halfway between y = 1 (below the centre 1.5) and y = 3 (above it): the
        // widest crossing is the left arm, first in x order of the equal widths.
        assert_eq!((x, y), (0.5, 2.0));
        assert!(u.contains(&geo::Point::new(x, y)));
    }

    #[test]
    fn interior_point_takes_the_widest_interval_across_holes() {
        let outer = square(0.0, 0.0, 10.0, 4.0);
        let hole = square(1.0, 1.0, 3.0, 3.0);
        let p = Polygon::new(outer.exterior().clone(), vec![hole.exterior().clone()]);
        // Ordinates 0, 1, 3, 4 around the centre 2: scan line at y = 2, intervals [0, 1] and
        // [3, 10].
        assert_eq!(interior_point(&p), coord! { x: 6.5, y: 2.0 });
    }

    /// Centroids of shapely 2.2 on GEOS 3.15, bit for bit: a counter-clockwise and a clockwise
    /// shell and a shell with a clockwise hole.
    #[test]
    fn anchor_is_the_centroid_geos_computes() {
        let ring = |pts: &[(f64, f64)]| {
            let mut r = LineString::from(pts.to_vec());
            r.close();
            r
        };
        let cases: [(Polygon, (f64, f64)); 3] = [
            (
                Polygon::new(
                    ring(&[
                        (13.48665787080801, -41.662216879506936),
                        (12.476171637574677, -40.864779158556374),
                        (12.096630105541683, -40.93830483907907),
                        (11.808793865368385, -42.23684979053127),
                        (12.52193986290147, -42.78682829820448),
                        (12.69713027770754, -42.85573343422993),
                        (12.90597220191377, -42.86862805104179),
                        (13.810691745639664, -43.192580132275346),
                        (13.724021985216133, -42.739282007886935),
                    ]),
                    vec![],
                ),
                (12.780159981338539, -42.03304805050577),
            ),
            (
                Polygon::new(
                    ring(&[
                        (-170.7233601106244, 63.16993684547748),
                        (-169.7434593479054, 62.31785345155091),
                        (-171.5110499267871, 62.98743295345942),
                        (-171.99446294385606, 62.53972737637101),
                        (-171.80259152487628, 62.78052994515916),
                        (-172.83889937044054, 63.06561573241347),
                        (-171.93180855435025, 63.34620749460608),
                        (-171.23053419609198, 63.523628331880346),
                        (-170.90136569550947, 63.84000078550881),
                        (-170.15086219696036, 63.53558750412352),
                        (-170.10525363074356, 63.48242449266146),
                    ]),
                    vec![],
                ),
                (-171.17562155745122, 63.14737200834875),
            ),
            (
                Polygon::new(
                    ring(&[
                        (103.49752374670774, 3.8167001365000637),
                        (102.54807154390072, 4.668322649953331),
                        (102.29383184014495, 3.2801492387054294),
                        (101.70415008082372, 3.238660432435655),
                        (101.84782490782953, 3.074658249106841),
                        (102.33565876048621, 2.8611772174428136),
                        (102.60855408350378, 1.5329560300714642),
                        (103.26346320754995, 2.4945160922591674),
                        (104.12396040188075, 1.8089821819121266),
                        (103.6947879927093, 2.369731157699759),
                    ]),
                    vec![ring(&[
                        (102.6, 3.2),
                        (102.6, 3.45),
                        (102.9, 3.4),
                        (102.85, 3.15),
                    ])],
                ),
                (102.89691176232327, 3.1179827008077554),
            ),
        ];
        for (poly, want) in cases {
            let got = anchor(&MultiPolygon(vec![poly])).unwrap();
            assert_eq!(
                (got.0.to_bits(), got.1.to_bits()),
                (want.0.to_bits(), want.1.to_bits()),
                "{got:?} vs {want:?}"
            );
        }
    }

    #[test]
    fn haversine_of_a_quarter_meridian() {
        let d = haversine_km(0.0, 0.0, 0.0, 90.0);
        assert!((d - R_KM * std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        assert_eq!(haversine_km(10.0, 20.0, 10.0, 20.0), 0.0);
        let antipodal = haversine_km(0.0, 0.0, 180.0, 0.0);
        assert!((antipodal - R_KM * std::f64::consts::PI).abs() < 1e-9);
    }

    #[test]
    fn cos_lat_is_floored_near_the_poles() {
        assert_eq!(cos_lat(0.0), 1.0);
        assert_eq!(cos_lat(89.9), 0.05);
    }
}
