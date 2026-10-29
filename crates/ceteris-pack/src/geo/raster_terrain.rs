//! H3 base raster: cell membership and terrain statistics (Section 5.1).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use geo::MultiPolygon;
use h3o::geom::{ContainmentMode, TilerBuilder};
use h3o::{CellIndex, LatLng, Resolution};

use crate::raster::geotiff;

/// Samples per cell are its children this many resolutions finer (7^2 = 49).
pub const ETOPO: &str = "ETOPO_2022_v1_60s_N90W180_surface.tif";
pub const SAMPLE_DEPTH: u8 = 2;
pub const FINEST: Resolution = Resolution::Nine;

/// H3 cells whose centres fall inside the geometry, one polygon part at a time (parts never
/// cross 180).
pub fn cells_of(geom: &MultiPolygon, res: Resolution) -> Result<BTreeSet<CellIndex>, String> {
    let mut cells = BTreeSet::new();
    for part in geom {
        let mut tiler = TilerBuilder::new(res)
            .containment_mode(ContainmentMode::ContainsCentroid)
            .build();
        tiler.add(part.clone()).map_err(|e| e.to_string())?;
        cells.extend(tiler.into_coverage());
    }
    Ok(cells)
}

/// Cells and their province index, ordered by province then cell. A cell whose centre lies in
/// two provinces goes to the lower index.
///
/// Provinces too small to own a cell centre at `res` get cells at the first finer resolution that
/// has one, or else the `FINEST` cell of their anchor `(lon, lat)`.
pub fn assign_cells(
    geoms: &[MultiPolygon],
    anchors: &[(f64, f64)],
    res: Resolution,
) -> Result<(Vec<CellIndex>, Vec<u32>), String> {
    let mut owner: BTreeMap<CellIndex, u32> = BTreeMap::new();
    for (i, (geom, &(lon, lat))) in geoms.iter().zip(anchors).enumerate() {
        let mut r = res;
        let mut cells = cells_of(geom, r)?;
        while cells.is_empty() && r < FINEST {
            r = r.succ().unwrap_or(FINEST);
            cells = cells_of(geom, r)?;
        }
        if cells.is_empty() {
            let at = LatLng::new(lat, lon).map_err(|e| format!("anchor of {i}: {e}"))?;
            cells.insert(at.to_cell(FINEST));
        }
        let i = u32::try_from(i).map_err(|e| e.to_string())?;
        for c in cells {
            owner.entry(c).or_insert(i);
        }
    }
    let mut ids: Vec<(u32, CellIndex)> = owner.into_iter().map(|(c, i)| (i, c)).collect();
    ids.sort_unstable();
    Ok(ids.into_iter().map(|(i, c)| (c, i)).unzip())
}

/// A single-band elevation raster held in memory.
pub struct Elevation {
    width: usize,
    height: usize,
    transform: [f64; 6],
    values: Vec<f32>,
}

impl Elevation {
    pub fn open(path: &Path) -> Result<Self, String> {
        let mut raster = geotiff::open(path)?;
        let values = raster.window(0, raster.height)?;
        Ok(Elevation {
            width: raster.width,
            height: raster.height,
            transform: raster.transform,
            values,
        })
    }

    /// Row and column of the pixel holding `(lon, lat)`, as rasterio's `rowcol` floors them;
    /// rows outside the raster are clamped to its edge and columns wrap around it.
    fn pixel(&self, lon: f64, lat: f64) -> (usize, usize) {
        let [a, _, c, _, e, f] = self.transform;
        // numpy solves the affine system by LU, whose triangular solve multiplies by the
        // reciprocal of the diagonal.
        let col = ((lon - c) * (1.0 / a)).floor() as i64;
        let row = ((lat - f) * (1.0 / e)).floor() as i64;
        let row = row.clamp(0, self.height as i64 - 1) as usize;
        (row, col.rem_euclid(self.width as i64) as usize)
    }

    /// The value at `row + dr` (clamped) and `col + dc` (wrapped).
    fn at(&self, row: usize, col: usize, dr: i64, dc: i64) -> f64 {
        let r = (row as i64 + dr).clamp(0, self.height as i64 - 1) as usize;
        let c = (col as i64 + dc).rem_euclid(self.width as i64) as usize;
        f64::from(self.values[r * self.width + c])
    }
}

/// Per cell: mean and standard deviation of elevation, and mean terrain ruggedness index
/// (Riley et al. 1999).
#[derive(Debug, PartialEq)]
pub struct Terrain {
    pub elev_mean_m: Vec<f64>,
    pub elev_std_m: Vec<f64>,
    pub tri_m: Vec<f64>,
}

/// Terrain of each cell, sampled at the centres of its children `depth` resolutions finer.
pub fn terrain(cells: &[CellIndex], z: &Elevation, depth: u8) -> Result<Terrain, String> {
    let mut out = Terrain {
        elev_mean_m: Vec::with_capacity(cells.len()),
        elev_std_m: Vec::with_capacity(cells.len()),
        tri_m: Vec::with_capacity(cells.len()),
    };
    for &cell in cells {
        let res = Resolution::try_from(u8::from(cell.resolution()) + depth)
            .map_err(|e| format!("{cell}: {e}"))?;
        let mut kids: Vec<CellIndex> = cell.children(res).collect();
        kids.sort_unstable();
        let (mut sum, mut squares, mut tri_sum) = (0.0, 0.0, 0.0);
        for k in &kids {
            let ll = LatLng::from(*k);
            let (row, col) = z.pixel(ll.lng(), ll.lat());
            let centre = z.at(row, col, 0, 0);
            let mut tri = 0.0;
            for dr in -1..=1 {
                for dc in -1..=1 {
                    if dr != 0 || dc != 0 {
                        tri += (z.at(row, col, dr, dc) - centre).abs();
                    }
                }
            }
            sum += centre;
            squares += centre * centre;
            tri_sum += tri / 8.0;
        }
        let n = kids.len() as f64;
        let mean = sum / n;
        let var = squares / n - mean * mean;
        out.elev_mean_m.push(mean);
        out.elev_std_m.push(var.max(0.0).sqrt());
        out.tri_m.push(tri_sum / n);
    }
    Ok(out)
}

/// Cell areas in km². H3's C library sums the same edge terms over a boundary whose vertices
/// can differ from h3o's in the last bits, which the sum magnifies to about 1e-11 relative at
/// resolution 4 and 1e-9 at resolution 9.
pub fn cell_area_km2(cells: &[CellIndex]) -> Vec<f64> {
    cells.iter().map(|c| c.area_km2()).collect()
}

/// Weighted mean of `values` per owner; NaN for an owner without cells.
pub fn province_means(owner: &[u32], weight: &[f64], values: &[f64], n: usize) -> Vec<f64> {
    let (mut num, mut den) = (vec![0.0; n], vec![0.0; n]);
    for ((&o, &w), &v) in owner.iter().zip(weight).zip(values) {
        num[o as usize] += w * v;
        den[o as usize] += w;
    }
    num.iter().zip(&den).map(|(a, b)| a / b).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::{Rect, coord};

    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> MultiPolygon {
        MultiPolygon(vec![
            Rect::new(coord! { x: x0, y: y0 }, coord! { x: x1, y: y1 }).to_polygon(),
        ])
    }

    /// A 4 x 3 raster of 1-degree pixels from (0, 3) whose value is 10 * row + col.
    fn grid() -> Elevation {
        Elevation {
            width: 4,
            height: 3,
            transform: [1.0, 0.0, 0.0, 0.0, -1.0, 3.0],
            values: (0..3)
                .flat_map(|r| (0..4).map(move |c| (10 * r + c) as f32))
                .collect(),
        }
    }

    #[test]
    fn pixels_clamp_rows_and_wrap_columns() {
        let z = grid();
        assert_eq!(z.pixel(0.5, 2.5), (0, 0));
        assert_eq!(z.pixel(3.0, 1.0), (2, 3));
        assert_eq!(z.pixel(-0.5, 9.0), (0, 3));
        assert_eq!(z.pixel(4.5, -7.0), (2, 0));
        assert_eq!(z.at(0, 0, -1, -1), 3.0);
        assert_eq!(z.at(2, 3, 1, 1), 20.0);
    }

    #[test]
    fn province_means_weight_cells_and_leave_empty_owners_nan() {
        let m = province_means(&[0, 0, 2], &[1.0, 3.0, 2.0], &[4.0, 8.0, 5.0], 3);
        assert_eq!(m[0], 7.0);
        assert!(m[1].is_nan());
        assert_eq!(m[2], 5.0);
    }

    #[test]
    fn small_provinces_take_finer_cells_and_overlaps_go_to_the_lower_index() {
        let big = square(10.0, 10.0, 12.0, 12.0);
        let tiny = square(11.0, 11.0, 11.01, 11.01);
        let speck = square(30.0, 30.0, 30.000001, 30.000001);
        let geoms = [big.clone(), tiny, big, speck];
        let anchors = [(11.0, 11.0), (11.005, 11.005), (11.0, 11.0), (30.0, 30.0)];
        let (cells, owner) = assign_cells(&geoms, &anchors, Resolution::Four).unwrap();
        let of = |i: u32| -> Vec<CellIndex> {
            cells
                .iter()
                .zip(&owner)
                .filter(|(_, o)| **o == i)
                .map(|(c, _)| *c)
                .collect()
        };
        let base = of(0);
        assert!(!base.is_empty());
        assert!(base.iter().all(|c| c.resolution() == Resolution::Four));
        let tiny = of(1);
        assert!(!tiny.is_empty() && tiny.iter().all(|c| c.resolution() > Resolution::Four));
        assert!(of(2).is_empty(), "the duplicate keeps no cell");
        let anchor = LatLng::new(30.0, 30.0).unwrap().to_cell(FINEST);
        assert_eq!(of(3), vec![anchor]);
        assert!(owner.is_sorted());
        assert!(cells[..base.len()].is_sorted());
    }

    #[test]
    fn flat_terrain_has_no_spread_or_ruggedness() {
        let z = Elevation {
            width: 360,
            height: 180,
            transform: [1.0, 0.0, -180.0, 0.0, -1.0, 90.0],
            values: vec![250.0; 360 * 180],
        };
        let cell = LatLng::new(45.0, 7.0).unwrap().to_cell(Resolution::Two);
        let t = terrain(&[cell], &z, SAMPLE_DEPTH).unwrap();
        assert_eq!(
            t,
            Terrain {
                elev_mean_m: vec![250.0],
                elev_std_m: vec![0.0],
                tri_m: vec![0.0]
            }
        );
    }
}
