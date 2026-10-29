//! Vertex tables of polygon geometries: one row per vertex (geom, part, ring, x, y), rings
//! closed, in their stored order.

use std::path::Path;
use std::sync::Arc;

use arrow_array::types::{Float64Type, UInt32Type};
use arrow_array::{ArrayRef, Float64Array, UInt32Array};
use geo::{Coord, LineString, MultiPolygon, Polygon};

use crate::table::{self, values};

/// The geometries of a vertex table, in `geom` order.
pub fn read_geoms(path: &Path) -> Result<Vec<MultiPolygon>, String> {
    let geom = values::<UInt32Type>(path, "geom")?;
    let part = values::<UInt32Type>(path, "part")?;
    let ring = values::<UInt32Type>(path, "ring")?;
    let x = values::<Float64Type>(path, "x")?;
    let y = values::<Float64Type>(path, "y")?;
    let mut rows: Vec<(u32, u32, u32, Coord)> = Vec::with_capacity(geom.len());
    for i in 0..geom.len() {
        rows.push((geom[i], part[i], ring[i], Coord { x: x[i], y: y[i] }));
    }
    let mut out: Vec<Vec<Vec<LineString>>> = Vec::new();
    for (g, p, r, c) in rows {
        let (g, p, r) = (g as usize, p as usize, r as usize);
        if out.len() <= g {
            out.resize(g + 1, Vec::new());
        }
        if out[g].len() <= p {
            out[g].resize(p + 1, Vec::new());
        }
        if out[g][p].len() <= r {
            out[g][p].resize(r + 1, LineString(Vec::new()));
        }
        out[g][p][r].0.push(c);
    }
    out.into_iter()
        .enumerate()
        .map(|(g, parts)| {
            let polygons = parts
                .into_iter()
                .enumerate()
                .map(|(p, mut rings)| {
                    if rings.is_empty() {
                        return Err(format!(
                            "{}: geom {g} part {p} has no rings",
                            path.display()
                        ));
                    }
                    let exterior = rings.remove(0);
                    Ok(Polygon::new(exterior, rings))
                })
                .collect::<Result<_, _>>()?;
            Ok(MultiPolygon(polygons))
        })
        .collect()
}

/// A vertex table of the geometries: one row per vertex, rings closed, in their stored order.
pub fn write_geoms(path: &Path, geoms: &[MultiPolygon]) -> Result<(), String> {
    let (mut g, mut p, mut r, mut x, mut y) = (vec![], vec![], vec![], vec![], vec![]);
    let index = |i: usize| u32::try_from(i).map_err(|e| e.to_string());
    for (i, geom) in geoms.iter().enumerate() {
        for (j, part) in geom.iter().enumerate() {
            for (k, ring) in std::iter::once(part.exterior())
                .chain(part.interiors())
                .enumerate()
            {
                for c in &ring.0 {
                    g.push(index(i)?);
                    p.push(index(j)?);
                    r.push(index(k)?);
                    x.push(c.x);
                    y.push(c.y);
                }
            }
        }
    }
    let columns: Vec<(String, ArrayRef)> = vec![
        ("geom".to_owned(), Arc::new(UInt32Array::from(g))),
        ("part".to_owned(), Arc::new(UInt32Array::from(p))),
        ("ring".to_owned(), Arc::new(UInt32Array::from(r))),
        ("x".to_owned(), Arc::new(Float64Array::from(x))),
        ("y".to_owned(), Arc::new(Float64Array::from(y))),
    ];
    table::write(path, &table::batch(columns)?, &Default::default())
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_part_without_rings_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v.arrow");
        let square = |x: f64| {
            Polygon::new(
                LineString::from(vec![(x, 0.0), (x + 1.0, 0.0), (x + 1.0, 1.0), (x, 0.0)]),
                vec![],
            )
        };
        let geoms = [MultiPolygon(vec![square(0.0), square(2.0)])];
        write_geoms(&path, &geoms).unwrap();
        assert_eq!(read_geoms(&path).unwrap(), geoms);

        let columns: Vec<(String, ArrayRef)> = vec![
            ("geom".to_owned(), Arc::new(UInt32Array::from(vec![0, 0]))),
            ("part".to_owned(), Arc::new(UInt32Array::from(vec![0, 2]))),
            ("ring".to_owned(), Arc::new(UInt32Array::from(vec![0, 0]))),
            ("x".to_owned(), Arc::new(Float64Array::from(vec![0.0, 1.0]))),
            ("y".to_owned(), Arc::new(Float64Array::from(vec![0.0, 1.0]))),
        ];
        table::write(&path, &table::batch(columns).unwrap(), &Default::default()).unwrap();
        let err = read_geoms(&path).unwrap_err();
        assert!(err.ends_with("geom 0 part 1 has no rings"), "{err}");
    }
}
