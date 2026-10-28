//! Spherical helpers on WGS84 lon/lat degrees, with a spherical Earth of mean radius.

use geo::{Area, BoundingRect, Centroid, Contains, Coord, LineString, MultiPolygon, Polygon};

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
    let c = largest.centroid()?;
    let c = if largest.contains(&c) {
        c.0
    } else {
        interior_point(largest)
    };
    Some((c.x, c.y))
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
