//! GeoJSON feature collections of polygons, as GDAL's GeoJSON driver reads them.

use std::path::Path;

use geo::{Coord, Geometry, LineString, MultiPolygon, Polygon};
use serde_json::Value;

use super::err;

/// A feature's text property and geometry.
pub type Feature = (Option<String>, Option<Geometry>);

/// One text property and the polygonal geometry of each feature, in file order. A null property
/// or geometry reads as None.
pub fn features(path: &Path, property: &str) -> Result<Vec<Feature>, String> {
    let text = std::fs::read(path).map_err(|e| err(path, e))?;
    let doc: Value = serde_json::from_slice(&text).map_err(|e| err(path, e))?;
    let features = doc
        .get("features")
        .and_then(Value::as_array)
        .ok_or_else(|| err(path, "no features"))?;
    features
        .iter()
        .map(|f| {
            let name = f
                .get("properties")
                .and_then(|p| p.get(property))
                .and_then(Value::as_str)
                .map(str::to_owned);
            let geom = match f.get("geometry") {
                None | Some(Value::Null) => None,
                Some(g) => Some(geometry(g).map_err(|e| err(path, e))?),
            };
            Ok((name, geom))
        })
        .collect()
}

fn geometry(g: &Value) -> Result<Geometry, String> {
    let coords = g.get("coordinates").ok_or("geometry without coordinates")?;
    match g.get("type").and_then(Value::as_str) {
        Some("Polygon") => Ok(Geometry::Polygon(polygon(coords)?)),
        Some("MultiPolygon") => Ok(Geometry::MultiPolygon(MultiPolygon(
            array(coords)?
                .iter()
                .map(polygon)
                .collect::<Result<_, _>>()?,
        ))),
        t => Err(format!("geometry type {t:?} is not polygonal")),
    }
}

fn array(v: &Value) -> Result<&Vec<Value>, String> {
    v.as_array()
        .ok_or_else(|| format!("expected an array, got {v}"))
}

fn polygon(v: &Value) -> Result<Polygon, String> {
    let mut rings = array(v)?.iter().map(ring).collect::<Result<Vec<_>, _>>()?;
    if rings.is_empty() {
        return Ok(Polygon::new(LineString(vec![]), vec![]));
    }
    let exterior = rings.remove(0);
    Ok(Polygon::new(exterior, rings))
}

fn ring(v: &Value) -> Result<LineString, String> {
    array(v)?
        .iter()
        .map(|p| {
            let p = array(p)?;
            match (
                p.first().and_then(Value::as_f64),
                p.get(1).and_then(Value::as_f64),
            ) {
                (Some(x), Some(y)) => Ok(Coord { x, y }),
                _ => Err(format!("bad position {p:?}")),
            }
        })
        .collect::<Result<Vec<_>, _>>()
        .map(LineString)
}
