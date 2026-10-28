//! Polygon overlay through GEOS, called as shapely calls it: `union_all` is a unary union of a
//! geometry collection, and the antimeridian helpers of `geo.py` (`polygonal`, `wrap`, `unwrap`).

use geo::{Coord, LineString, MultiPolygon, Polygon};
use geos::{CoordSeq, CoordType, Geom, Geometry, GeometryTypes};

fn fail(e: geos::Error) -> String {
    e.to_string()
}

fn ring(r: &LineString) -> Result<Geometry, String> {
    let flat: Vec<f64> = r.0.iter().flat_map(|c| [c.x, c.y]).collect();
    CoordSeq::new_from_buffer(&flat, r.0.len(), CoordType::XY)
        .and_then(Geometry::create_linear_ring)
        .map_err(fail)
}

pub fn from_polygon(p: &Polygon) -> Result<Geometry, String> {
    if p.exterior().0.is_empty() {
        return Geometry::create_empty_polygon().map_err(fail);
    }
    let holes = p.interiors().iter().map(ring).collect::<Result<_, _>>()?;
    Geometry::create_polygon(ring(p.exterior())?, holes).map_err(fail)
}

pub fn from_multipolygon(m: &MultiPolygon) -> Result<Geometry, String> {
    let parts = m.iter().map(from_polygon).collect::<Result<_, _>>()?;
    Geometry::create_multipolygon(parts).map_err(fail)
}

/// A Polygon or MultiPolygon, keeping its type.
pub fn from_geo(g: &geo::Geometry) -> Result<Geometry, String> {
    match g {
        geo::Geometry::Polygon(p) => from_polygon(p),
        geo::Geometry::MultiPolygon(m) => from_multipolygon(m),
        g => Err(format!("not polygonal: {g:?}")),
    }
}

fn line(g: &impl Geom) -> Result<LineString, String> {
    let xy = g.get_coord_seq().map_err(fail)?;
    let flat = xy.as_buffer(Some(CoordType::XY)).map_err(fail)?;
    Ok(LineString(
        flat.as_chunks::<2>()
            .0
            .iter()
            .map(|&[x, y]| Coord { x, y })
            .collect(),
    ))
}

fn to_polygon(p: &impl Geom) -> Result<Polygon, String> {
    if p.is_empty().map_err(fail)? {
        return Ok(Polygon::new(LineString(vec![]), vec![]));
    }
    let exterior = line(&p.get_exterior_ring().map_err(fail)?)?;
    let holes = (0..p.get_num_interior_rings().map_err(fail)?)
        .map(|i| line(&p.get_interior_ring_n(i).map_err(fail)?))
        .collect::<Result<_, _>>()?;
    Ok(Polygon::new(exterior, holes))
}

fn kind(g: &impl Geom) -> Result<GeometryTypes, String> {
    g.geometry_type().map_err(fail)
}

/// `shapely.get_parts`: the members of a multi-part geometry or collection, or the geometry itself.
pub fn parts(g: &impl Geom) -> Result<Vec<Geometry>, String> {
    match kind(g)? {
        GeometryTypes::MultiPoint
        | GeometryTypes::MultiLineString
        | GeometryTypes::MultiPolygon
        | GeometryTypes::GeometryCollection => (0..g.get_num_geometries().map_err(fail)?)
            .map(|i| Geom::clone(&g.get_geometry_n(i).map_err(fail)?).map_err(fail))
            .collect(),
        _ => Ok(vec![Geom::clone(g).map_err(fail)?]),
    }
}

/// The polygons of a Polygon or MultiPolygon, coordinates as GEOS holds them. An empty
/// geometry has none.
pub fn to_multipolygon(g: &impl Geom) -> Result<MultiPolygon, String> {
    if g.is_empty().map_err(fail)? {
        return Ok(MultiPolygon(vec![]));
    }
    match kind(g)? {
        GeometryTypes::Polygon => Ok(MultiPolygon(vec![to_polygon(g)?])),
        GeometryTypes::MultiPolygon => Ok(MultiPolygon(
            (0..g.get_num_geometries().map_err(fail)?)
                .map(|i| to_polygon(&g.get_geometry_n(i).map_err(fail)?))
                .collect::<Result<_, _>>()?,
        )),
        t => Err(format!("not polygonal: {t:?}")),
    }
}

/// A Polygon or MultiPolygon as geo holds it, keeping its type; anything empty is the empty
/// polygon.
pub fn to_geo(g: &impl Geom) -> Result<geo::Geometry, String> {
    if g.is_empty().map_err(fail)? {
        return Ok(geo::Geometry::Polygon(Polygon::new(
            LineString(vec![]),
            vec![],
        )));
    }
    match kind(g)? {
        GeometryTypes::Polygon => Ok(geo::Geometry::Polygon(to_polygon(g)?)),
        _ => Ok(geo::Geometry::MultiPolygon(to_multipolygon(g)?)),
    }
}

pub fn is_empty(g: &impl Geom) -> Result<bool, String> {
    g.is_empty().map_err(fail)
}

pub fn area(g: &impl Geom) -> Result<f64, String> {
    g.area().map_err(fail)
}

pub fn intersection(a: &impl Geom, b: &impl Geom) -> Result<Geometry, String> {
    a.intersection(b).map_err(fail)
}

pub fn difference(a: &impl Geom, b: &impl Geom) -> Result<Geometry, String> {
    a.difference(b).map_err(fail)
}

/// `shapely.union_all`: the unary union of a collection of the geometries.
pub fn union_all(geoms: Vec<Geometry>) -> Result<Geometry, String> {
    Geometry::create_geometry_collection(geoms)
        .and_then(|c| c.unary_union())
        .map_err(fail)
}

/// Applies `f` to every coordinate, as `shapely.transform` does.
pub fn transform(g: &impl Geom, f: impl Fn(f64, f64) -> (f64, f64)) -> Result<Geometry, String> {
    g.transform_xy(|x, y| Ok::<_, geos::Error>(f(x, y)))
        .map_err(fail)
}

/// The points and lines that intersections leave behind dropped; the polygons unioned.
pub fn polygonal(g: &impl Geom) -> Result<Geometry, String> {
    let mut keep = Vec::new();
    for p in parts(g)? {
        if matches!(
            kind(&p)?,
            GeometryTypes::Polygon | GeometryTypes::MultiPolygon
        ) {
            keep.push(p);
        }
    }
    if keep.is_empty() {
        return Geometry::create_empty_polygon().map_err(fail);
    }
    union_all(keep)
}

/// `shapely.box(xmin, ymin, xmax, ymax)`, its ring in shapely's order.
pub fn rectangle(xmin: f64, ymin: f64, xmax: f64, ymax: f64) -> Result<Geometry, String> {
    from_polygon(&Polygon::new(
        LineString::from(vec![
            (xmax, ymin),
            (xmax, ymax),
            (xmin, ymax),
            (xmin, ymin),
            (xmax, ymin),
        ]),
        vec![],
    ))
}

/// Western-hemisphere parts shifted east by 360 degrees so the geometry is contiguous.
pub fn unwrap(g: &impl Geom) -> Result<Geometry, String> {
    let mut out = Vec::new();
    for p in parts(g)? {
        let x = p.get_centroid().and_then(|c| c.get_x()).map_err(fail)?;
        out.push(if x < 0.0 {
            transform(&p, |x, y| (x + 360.0, y + 0.0))?
        } else {
            p
        });
    }
    union_all(out)
}

pub fn wrap(g: &impl Geom) -> Result<Geometry, String> {
    let west = intersection(g, &rectangle(-180.0, -90.0, 180.0, 90.0)?)?;
    let east = intersection(g, &rectangle(180.0, -90.0, 540.0, 90.0)?)?;
    let east = transform(&east, |x, y| (x - 360.0, y - 0.0))?;
    polygonal(&union_all(vec![west, east])?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geo::sphere::{area_km2, crosses_antimeridian};

    fn area(g: &Geometry) -> f64 {
        area_km2(&to_multipolygon(g).unwrap())
    }

    #[test]
    fn unwrap_then_wrap_keeps_area_across_the_antimeridian() {
        let g = Geometry::create_multipolygon(vec![
            rectangle(178.0, 60.0, 180.0, 62.0).unwrap(),
            rectangle(-180.0, 60.0, -177.0, 62.0).unwrap(),
        ])
        .unwrap();
        assert!(crosses_antimeridian(&to_multipolygon(&g).unwrap()));
        let u = unwrap(&g).unwrap();
        let (x0, x1) = (u.get_x_min().unwrap(), u.get_x_max().unwrap());
        assert!(x0 >= 178.0 && x1 <= 183.0, "{x0} {x1}");
        let (a, b) = (area(&wrap(&u).unwrap()), area(&g));
        assert!((a / b - 1.0).abs() < 1e-9, "{a} {b}");
    }

    #[test]
    fn polygonal_drops_lines_and_points_and_unions_the_polygons() {
        let a = rectangle(0.0, 0.0, 1.0, 1.0).unwrap();
        let b = rectangle(1.0, 0.0, 2.0, 1.0).unwrap();
        let touch = intersection(&a, &b).unwrap();
        assert!(is_empty(&polygonal(&touch).unwrap()).unwrap());
        let both = Geometry::create_geometry_collection(vec![
            Geom::clone(&a).unwrap(),
            touch,
            Geom::clone(&b).unwrap(),
        ])
        .unwrap();
        let joined = polygonal(&both).unwrap();
        assert_eq!(kind(&joined).unwrap(), GeometryTypes::Polygon);
        assert_eq!(joined.area().unwrap(), 2.0);
    }

    #[test]
    fn coordinates_round_trip_through_geos_bit_for_bit() {
        let m = MultiPolygon(vec![Polygon::new(
            LineString::from(vec![(0.1, 0.2), (3.3, 0.7), (1.0 / 3.0, 2.9), (0.1, 0.2)]),
            vec![LineString::from(vec![
                (0.5, 0.5),
                (0.6, 0.9),
                (1.1, 0.6),
                (0.5, 0.5),
            ])],
        )]);
        assert_eq!(to_multipolygon(&from_multipolygon(&m).unwrap()).unwrap(), m);
    }
}
