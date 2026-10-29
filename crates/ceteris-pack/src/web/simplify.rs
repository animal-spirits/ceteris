//! Map geometry simplified through GEOS, as `build.simplify` and the countries layer of
//! `build.build` do it with shapely.

use std::collections::BTreeMap;

use geos::{Geom, Geometry, GeometryTypes, WKBWriter};

use super::SIMPLIFY_DEG;
use crate::geo::overlay;

fn fail(e: geos::Error) -> String {
    e.to_string()
}

/// `shapely.coverage_simplify(geoms, SIMPLIFY_DEG)`: GEOS's coverage simplification of the
/// collection of `geoms`, boundary included, its members in order. The inner error is GEOS
/// raising in the simplification (shapely's GEOSException, with GEOS's message); the outer one
/// any other failure.
pub fn coverage(geoms: &[Geometry]) -> Result<Result<Vec<Geometry>, String>, String> {
    use ceteris_geos_coverage::{Error, coverage_simplify_vw};

    let members = geoms
        .iter()
        .map(|g| Geom::clone(g).map_err(fail))
        .collect::<Result<_, _>>()?;
    let collection = Geometry::create_geometry_collection(members).map_err(fail)?;
    let wkb = WKBWriter::new()
        .and_then(|mut w| w.write_wkb(&collection))
        .map_err(fail)?;
    match coverage_simplify_vw(&wkb, SIMPLIFY_DEG, false) {
        Ok(out) => Ok(Ok(overlay::parts(
            &Geometry::new_from_wkb(&out).map_err(fail)?,
        )?)),
        Err(e @ Error::Simplify(_)) => Ok(Err(e.to_string())),
        Err(e) => Err(e.to_string()),
    }
}

/// `build.simplify`: the coverage simplification, or where GEOS raises in it (clipped counties
/// are not always a valid coverage), each geometry simplified on its own with its topology
/// kept. Anything but polygons fails, as shapely refuses it before GEOS sees it.
pub fn simplify(geoms: &[Geometry]) -> Result<Vec<Geometry>, String> {
    for g in geoms {
        let kind = g.geometry_type().map_err(fail)?;
        if !matches!(kind, GeometryTypes::Polygon | GeometryTypes::MultiPolygon) {
            return Err(format!("simplify: a {kind:?} among the polygons"));
        }
    }
    match coverage(geoms)? {
        Ok(simple) => Ok(simple),
        Err(_) => geoms
            .iter()
            .map(|g| g.topology_preserve_simplify(SIMPLIFY_DEG).map_err(fail))
            .collect(),
    }
}

/// The countries layer: `shapely.union_all` of the simplified provinces of each country, the
/// countries in code order.
pub fn countries(simple: &[Geometry], country: &[String]) -> Result<Vec<Geometry>, String> {
    let mut groups: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, c) in country.iter().enumerate() {
        groups.entry(c).or_default().push(i);
    }
    groups
        .values()
        .map(|members| {
            let geoms = members
                .iter()
                .map(|&i| Geom::clone(&simple[i]).map_err(fail))
                .collect::<Result<_, _>>()?;
            overlay::union_all(geoms)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::{LineString, Polygon};

    fn polygon(xy: &[(f64, f64)]) -> Geometry {
        overlay::from_polygon(&Polygon::new(LineString::from(xy.to_vec()), vec![])).unwrap()
    }

    /// Two squares sharing a zigzag edge whose wiggles are well under the tolerance.
    fn coverage_pair() -> [Geometry; 2] {
        let zig = [
            (1.0, 0.0),
            (1.0001, 0.25),
            (1.0, 0.5),
            (1.0001, 0.75),
            (1.0, 1.0),
        ];
        let mut left = vec![(0.0, 0.0)];
        left.extend(zig);
        left.extend([(0.0, 1.0), (0.0, 0.0)]);
        let mut right: Vec<(f64, f64)> = zig.iter().rev().copied().collect();
        right.extend([(2.0, 0.0), (2.0, 1.0), (1.0, 1.0)]);
        [polygon(&left), polygon(&right)]
    }

    fn wkt(g: &Geometry) -> String {
        let mut w = geos::WKTWriter::new().unwrap();
        w.set_trim(true);
        w.write(g).unwrap()
    }

    #[test]
    fn coverage_simplification_is_shapely_s() {
        // shapely 2.2.0 on GEOS 3.15.0: shapely.coverage_simplify([a, b], 0.01)
        let out = coverage(&coverage_pair()).unwrap().unwrap();
        let out: Vec<String> = out.iter().map(wkt).collect();
        assert_eq!(
            out,
            [
                "POLYGON ((1 0, 1 1, 0 1, 0 0, 1 0))",
                "POLYGON ((1 0, 1 1, 2 1, 2 0, 1 0))"
            ]
        );
    }

    #[test]
    fn simplify_falls_back_to_each_geometry_where_geos_fails() {
        // A spike: shapely.coverage_simplify raises GEOSException on it.
        let spike = polygon(&[
            (0.0, 0.0),
            (1.0, 0.0),
            (1.0, 1.0),
            (1.0, 2.0),
            (1.0, 1.0),
            (0.0, 1.0),
            (0.0, 0.0),
        ]);
        let square = polygon(&[(5.0, 0.0), (6.0, 0.0), (6.0, 1.0), (5.0, 1.0), (5.0, 0.0)]);
        let set = [spike, square];
        let raised = coverage(&set).unwrap().err().unwrap();
        assert!(raised.contains("point array must contain"), "{raised}");
        let out = simplify(&set).unwrap();
        assert_eq!(out.len(), 2);
        for (a, b) in set.iter().zip(&out) {
            let want = a.topology_preserve_simplify(SIMPLIFY_DEG).unwrap();
            assert!(want.equals_exact(b, 0.0).unwrap());
        }
    }

    #[test]
    fn simplify_refuses_anything_but_polygons() {
        let line = Geometry::new_from_wkt("LINESTRING (0 0, 1 1)").unwrap();
        assert!(simplify(&[line]).is_err());
    }

    #[test]
    fn countries_union_their_provinces_in_code_order() {
        let [a, b] = coverage_pair();
        let c = polygon(&[(5.0, 0.0), (6.0, 0.0), (6.0, 1.0), (5.0, 0.0)]);
        let country = ["ZZZ", "AAA", "ZZZ"].map(String::from);
        let out = countries(&[a, c, b], &country).unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].area().unwrap(), 0.5);
        assert!((out[1].area().unwrap() - 2.0).abs() < 1e-9);
    }
}
