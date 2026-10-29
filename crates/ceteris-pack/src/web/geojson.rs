//! The GeoJSON that tippecanoe reads: `json.dumps` of the features as `build.write_geojson`
//! writes them, byte for byte.

use std::fmt::Write;
use std::path::Path;

use geo::{Geometry, LineString, Polygon};

use crate::numeric::{repr_f64, round};

/// Coordinates are rounded to this many decimals (about 11 m).
pub const DIGITS: usize = 4;

/// A float as `json.dumps` writes it.
fn number(x: f64, out: &mut String) {
    match x {
        x if x.is_nan() => out.push_str("NaN"),
        f64::INFINITY => out.push_str("Infinity"),
        f64::NEG_INFINITY => out.push_str("-Infinity"),
        x => out.push_str(&repr_f64(x)),
    }
}

fn ring(r: &LineString, out: &mut String) -> Result<(), String> {
    if r.0.is_empty() {
        return Err("an empty ring".into());
    }
    out.push('[');
    for (i, c) in r.0.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('[');
        number(round(c.x, DIGITS), out);
        out.push(',');
        number(round(c.y, DIGITS), out);
        out.push(']');
    }
    out.push(']');
    Ok(())
}

fn polygon(p: &Polygon, out: &mut String) -> Result<(), String> {
    out.push('[');
    for (i, r) in std::iter::once(p.exterior())
        .chain(p.interiors())
        .enumerate()
    {
        if i > 0 {
            out.push(',');
        }
        ring(r, out)?;
    }
    out.push(']');
    Ok(())
}

/// The geometry member of a feature; Python fails on empty geometries and on anything but
/// polygons, and so does this.
fn geometry(g: &Geometry, out: &mut String) -> Result<(), String> {
    match g {
        Geometry::Polygon(p) => {
            out.push_str(r#"{"type":"Polygon","coordinates":"#);
            polygon(p, out)?;
        }
        Geometry::MultiPolygon(m) if !m.0.is_empty() => {
            out.push_str(r#"{"type":"MultiPolygon","coordinates":["#);
            for (i, p) in m.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                polygon(p, out)?;
            }
            out.push(']');
        }
        g => return Err(format!("not a non-empty polygon: {g:?}")),
    }
    out.push('}');
    Ok(())
}

/// A FeatureCollection of `geoms`, features numbered from 1 (tippecanoe drops ID 0), with a
/// tippecanoe minzoom on each feature when `minzoom` is not zero.
pub fn text(geoms: &[Geometry], minzoom: u32) -> Result<String, String> {
    let mut out = String::from(r#"{"type":"FeatureCollection","features":["#);
    for (i, g) in geoms.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write!(
            out,
            r#"{{"type":"Feature","id":{},"properties":{{}},"#,
            i + 1
        )
        .expect("write to String");
        if minzoom != 0 {
            write!(out, r#""tippecanoe":{{"minzoom":{minzoom}}},"#).expect("write to String");
        }
        out.push_str(r#""geometry":"#);
        geometry(g, &mut out).map_err(|e| format!("feature {}: {e}", i + 1))?;
        out.push('}');
    }
    out.push_str("]}");
    Ok(out)
}

pub fn write(path: &Path, geoms: &[Geometry], minzoom: u32) -> Result<(), String> {
    std::fs::write(path, text(geoms, minzoom)?).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::MultiPolygon;

    fn square(x: f64, y: f64) -> Polygon {
        Polygon::new(
            LineString::from(vec![(x, y), (x + 1.0, y), (x + 1.0, y + 1.0), (x, y)]),
            vec![],
        )
    }

    #[test]
    fn polygons_are_written_as_python_dumps_them() {
        // The text of `build.write_geojson(path, [p, m], 4)` on the same polygons.
        let p = Polygon::new(
            LineString::from(vec![
                (-0.00004, 1.03125),
                (179.99996, -1e-5),
                (12.5, 0.1),
                (-0.00004, 1.03125),
            ]),
            vec![LineString::from(vec![
                (1.0, 1.0),
                (2.0, 1.0),
                (1.0, 2.0),
                (1.0, 1.0),
            ])],
        );
        let m = MultiPolygon(vec![square(0.0, 0.0), square(2.0, 0.0)]);
        let text = text(&[Geometry::Polygon(p), Geometry::MultiPolygon(m)], 4).unwrap();
        assert_eq!(
            text,
            concat!(
                r#"{"type":"FeatureCollection","features":["#,
                r#"{"type":"Feature","id":1,"properties":{},"tippecanoe":{"minzoom":4},"#,
                r#""geometry":{"type":"Polygon","coordinates":[[[-0.0,1.0312],[180.0,-0.0],"#,
                r#"[12.5,0.1],[-0.0,1.0312]],[[1.0,1.0],[2.0,1.0],[1.0,2.0],[1.0,1.0]]]}},"#,
                r#"{"type":"Feature","id":2,"properties":{},"tippecanoe":{"minzoom":4},"#,
                r#""geometry":{"type":"MultiPolygon","coordinates":[[[[0.0,0.0],[1.0,0.0],"#,
                r#"[1.0,1.0],[0.0,0.0]]],[[[2.0,0.0],[3.0,0.0],[3.0,1.0],[2.0,0.0]]]]}}]}"#,
            )
        );
    }

    #[test]
    fn minzoom_zero_leaves_tippecanoe_out() {
        let text = text(&[Geometry::Polygon(square(0.0, 0.0))], 0).unwrap();
        assert!(text.starts_with(r#"{"type":"FeatureCollection","features":[{"type":"Feature","id":1,"properties":{},"geometry":"#));
        assert_eq!(
            super::text(&[], 0).unwrap(),
            r#"{"type":"FeatureCollection","features":[]}"#
        );
    }

    #[test]
    fn empty_and_non_polygonal_geometries_fail() {
        let empty = Geometry::Polygon(Polygon::new(LineString(vec![]), vec![]));
        assert!(text(&[empty], 0).is_err());
        assert!(text(&[Geometry::MultiPolygon(MultiPolygon(vec![]))], 0).is_err());
        assert!(text(&[Geometry::Point(geo::Point::new(0.0, 0.0))], 0).is_err());
    }
}
