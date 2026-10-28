//! Raster counts summed per province through the pack's H3 cells: each pixel with a positive
//! value goes to the province owning the H3 cell of its centre.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{BufReader, Read, Seek};
use std::path::Path;

use arrow_array::{Array, UInt32Array, UInt64Array};
use arrow_ipc::reader::FileReader;
use h3o::{CellIndex, LatLng, Resolution};
use zip::ZipArchive;

use super::geotiff::{self, Raster};

/// Rings of base cells searched for the owner of a pixel whose own cell belongs to no province.
pub const MAX_RING: u32 = 4;
/// Rows read and summed at a time.
pub const BLOCK_ROWS: usize = 512;

/// A single band read by row windows, as float64.
pub trait Grid {
    fn width(&self) -> usize;
    fn height(&self) -> usize;
    /// `(a, b, c, d, e, f)` in rasterio's order.
    fn transform(&self) -> [f64; 6];
    /// The nodata value as the band's float32 compares it.
    fn nodata(&self) -> Option<f32>;
    fn rows(&mut self, row0: usize, rows: usize) -> Result<Vec<f64>, String>;
}

impl<R: Read + Seek> Grid for Raster<R> {
    fn width(&self) -> usize {
        self.width
    }
    fn height(&self) -> usize {
        self.height
    }
    fn transform(&self) -> [f64; 6] {
        self.transform
    }
    fn nodata(&self) -> Option<f32> {
        self.nodata.map(|v| v as f32)
    }
    fn rows(&mut self, row0: usize, rows: usize) -> Result<Vec<f64>, String> {
        Ok(self
            .window(row0, rows)?
            .into_iter()
            .map(f64::from)
            .collect())
    }
}

/// A float64 band held in memory, without nodata.
pub struct Band {
    pub width: usize,
    pub height: usize,
    pub transform: [f64; 6],
    pub values: Vec<f64>,
}

impl Grid for Band {
    fn width(&self) -> usize {
        self.width
    }
    fn height(&self) -> usize {
        self.height
    }
    fn transform(&self) -> [f64; 6] {
        self.transform
    }
    fn nodata(&self) -> Option<f32> {
        None
    }
    fn rows(&mut self, row0: usize, rows: usize) -> Result<Vec<f64>, String> {
        Ok(self.values[row0 * self.width..(row0 + rows) * self.width].to_vec())
    }
}

/// The `*_A.tif` members of a MapSPAM zip (physical area of each crop, all technologies) summed
/// in name order, negative values (nodata) read as zero; georeferenced as the last member.
pub fn spam_physical_area(spam: &Path) -> Result<Band, String> {
    let fail = |e: zip::result::ZipError| format!("{}: {e}", spam.display());
    let file = File::open(spam).map_err(|e| format!("{}: {e}", spam.display()))?;
    let mut zip = ZipArchive::new(BufReader::new(file)).map_err(fail)?;
    let mut members = Vec::new();
    for i in 0..zip.len() {
        let name = zip
            .by_index(i)
            .map_err(fail)?
            .name()
            .map_err(fail)?
            .into_owned();
        if name.ends_with("_A.tif") {
            members.push((name, i));
        }
    }
    members.sort();
    let mut total: Option<Band> = None;
    for (name, i) in members {
        let mut member = zip.by_index(i).map_err(fail)?;
        // Sized up front: a buffer grown by doubling leaves freed blocks the allocator keeps.
        let mut bytes = Vec::with_capacity(usize::try_from(member.size()).unwrap_or(0));
        member
            .read_to_end(&mut bytes)
            .map_err(|e| format!("{}!{name}: {e}", spam.display()))?;
        let mut raster = geotiff::from_bytes(bytes, &name)?;
        let values = raster.window(0, raster.height)?.into_iter().map(|v| {
            let v = f64::from(v);
            if v < 0.0 { 0.0 } else { v }
        });
        match &mut total {
            None => {
                total = Some(Band {
                    width: raster.width,
                    height: raster.height,
                    transform: raster.transform,
                    values: values.collect(),
                });
            }
            Some(t) => {
                if (t.width, t.height) != (raster.width, raster.height) {
                    return Err(format!("{name}: grid differs from the other members"));
                }
                for (s, v) in t.values.iter_mut().zip(values) {
                    *s += v;
                }
                t.transform = raster.transform;
            }
        }
    }
    total.ok_or_else(|| format!("{}: no _A.tif member", spam.display()))
}

/// The centre `(lat, lng)` of pixel `(row, col)`.
pub fn pixel_centre(transform: &[f64; 6], row: usize, col: usize) -> (f64, f64) {
    let [a, _, x0, _, e, y0] = *transform;
    (y0 + (row as f64 + 0.5) * e, x0 + (col as f64 + 0.5) * a)
}

/// H3 cell of a point at `res`.
pub fn cell_of(lat: f64, lng: f64, res: Resolution) -> Result<CellIndex, String> {
    LatLng::new(lat, lng)
        .map(|p| p.to_cell(res))
        .map_err(|e| format!("({lat}, {lng}): {e}"))
}

/// Province owning a point through the pack's cells table.
///
/// Cells of several resolutions may coexist, and the finest owned cell containing the point
/// decides. A point in a cell no province owns (a coastal cell whose centre is at sea) goes to the
/// nearest owned cell within `MAX_RING` rings; further out it has no owner (-1).
pub struct CellOwner {
    owner_of: BTreeMap<u64, u32>,
    pub n: usize,
    pub base: Resolution,
    finer: Vec<Resolution>,
    split: BTreeSet<u64>,
    nearest: BTreeMap<u64, i64>,
}

impl CellOwner {
    /// From `(cell, province)` pairs, the later of a repeated cell winning.
    pub fn new(cells: impl IntoIterator<Item = (u64, u32)>) -> Result<Self, String> {
        let owner_of: BTreeMap<u64, u32> = cells.into_iter().collect();
        let n = owner_of
            .values()
            .max()
            .map(|m| *m as usize + 1)
            .ok_or("no cells")?;
        let mut index = BTreeMap::new();
        for &c in owner_of.keys() {
            let cell = CellIndex::try_from(c).map_err(|e| format!("cell {c}: {e}"))?;
            index.insert(c, cell);
        }
        let resolutions: BTreeSet<Resolution> = index.values().map(|c| c.resolution()).collect();
        let mut resolutions = resolutions.into_iter();
        let base = resolutions.next().ok_or("no cells")?;
        let finer = resolutions.collect();
        let split = index
            .values()
            .filter(|c| c.resolution() > base)
            .filter_map(|c| c.parent(base))
            .map(u64::from)
            .collect();
        Ok(CellOwner {
            owner_of,
            n,
            base,
            finer,
            split,
            nearest: BTreeMap::new(),
        })
    }

    /// From the pack's `cells.arrow` (`cell` uint64, `province` uint32).
    pub fn read(path: &Path) -> Result<Self, String> {
        let fail = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
        let reader = FileReader::try_new(File::open(path).map_err(|e| fail(&e))?, None)
            .map_err(|e| fail(&e))?;
        let mut pairs = Vec::new();
        for batch in reader {
            let batch = batch.map_err(|e| fail(&e))?;
            let column = |name: &str| {
                batch
                    .column_by_name(name)
                    .ok_or(fail(&format!("no {name}")))
            };
            let cell = column("cell")?;
            let cell = cell
                .as_any()
                .downcast_ref::<UInt64Array>()
                .ok_or(fail(&"cell is not uint64"))?;
            let province = column("province")?;
            let province = province
                .as_any()
                .downcast_ref::<UInt32Array>()
                .ok_or(fail(&"province is not uint32"))?;
            if cell.null_count() + province.null_count() > 0 {
                return Err(fail(&"null cell or province"));
            }
            pairs.extend(
                cell.values()
                    .iter()
                    .copied()
                    .zip(province.values().iter().copied()),
            );
        }
        CellOwner::new(pairs)
    }

    /// Owner of a base cell, or of the nearest owned base cell within `MAX_RING` rings.
    pub fn of_base(&mut self, cell: CellIndex) -> i64 {
        let key = u64::from(cell);
        if let Some(&p) = self.owner_of.get(&key) {
            return i64::from(p);
        }
        if let Some(&p) = self.nearest.get(&key) {
            return p;
        }
        let centre = LatLng::from(cell);
        let (lat, lng) = (centre.lat(), centre.lng());
        let mut found = -1;
        for k in 1..=MAX_RING {
            let mut best: Option<(f64, u64)> = None;
            for near in cell.grid_disk::<Vec<_>>(k) {
                let c = u64::from(near);
                if !self.owner_of.contains_key(&c) {
                    continue;
                }
                let p = LatLng::from(near);
                // `powf`, not `powi`: the libm `pow` Python's `** 2` calls.
                let d = (p.lat() - lat).powf(2.0) + (p.lng() - lng).powf(2.0);
                if best.is_none_or(|(bd, bc)| d < bd || (d == bd && c < bc)) {
                    best = Some((d, c));
                }
            }
            if let Some((_, c)) = best {
                found = i64::from(self.owner_of[&c]);
                break;
            }
        }
        self.nearest.insert(key, found);
        found
    }

    /// Whether a base cell is split into finer owned cells.
    pub fn is_split(&self, cell: CellIndex) -> bool {
        self.split.contains(&u64::from(cell))
    }

    /// Owner of the finest owned cell containing a point inside a split base cell, if any.
    pub fn finest(&self, lat: f64, lng: f64) -> Result<Option<u32>, String> {
        for &r in self.finer.iter().rev() {
            if let Some(&p) = self.owner_of.get(&u64::from(cell_of(lat, lng, r)?)) {
                return Ok(Some(p));
            }
        }
        Ok(None)
    }

    /// Province owning a point, -1 for none.
    pub fn of_point(&mut self, lat: f64, lng: f64) -> Result<i64, String> {
        let cell = cell_of(lat, lng, self.base)?;
        let found = if self.is_split(cell) {
            self.finest(lat, lng)?
        } else {
            None
        };
        Ok(match found {
            Some(p) => i64::from(p),
            None => self.of_base(cell),
        })
    }
}

/// Sum per province of the positive, non-nodata pixels of `grid`, read `block_rows` rows at a
/// time: within a block each province sums its pixels in row-major order from zero, and the block
/// sums are added to the totals block by block.
pub fn province_population(
    grid: &mut impl Grid,
    owner: &mut CellOwner,
    block_rows: usize,
) -> Result<Vec<f64>, String> {
    let (width, height, transform, nodata) =
        (grid.width(), grid.height(), grid.transform(), grid.nodata());
    let mut total = vec![0.0; owner.n];
    let mut block_sum = vec![0.0; owner.n];
    for row0 in (0..height).step_by(block_rows) {
        let rows = block_rows.min(height - row0);
        let block = grid.rows(row0, rows)?;
        block_sum.fill(0.0);
        for (i, &v) in block.iter().enumerate() {
            let valid = v > 0.0 && nodata.is_none_or(|n| v != f64::from(n));
            if !valid {
                continue;
            }
            let (lat, lng) = pixel_centre(&transform, row0 + i / width, i % width);
            let p = owner.of_point(lat, lng)?;
            if p >= 0 {
                block_sum[p as usize] += v;
            }
        }
        for (t, s) in total.iter_mut().zip(&block_sum) {
            *t += s;
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(lat: f64, lng: f64, res: u8) -> u64 {
        u64::from(cell_of(lat, lng, Resolution::try_from(res).unwrap()).unwrap())
    }

    #[test]
    fn split_cells_defer_to_their_finest_owned_child() {
        let base = cell(10.0, 10.0, 4);
        let child = cell(10.0, 10.0, 6);
        let mut owner = CellOwner::new([(base, 0), (child, 3)]).unwrap();
        assert_eq!(owner.n, 4);
        assert_eq!(owner.of_point(10.0, 10.0).unwrap(), 3);
        let outside_child = CellIndex::try_from(base)
            .unwrap()
            .children(Resolution::try_from(6).unwrap())
            .map(u64::from)
            .find(|&c| c != child)
            .unwrap();
        let p = LatLng::from(CellIndex::try_from(outside_child).unwrap());
        assert_eq!(owner.of_point(p.lat(), p.lng()).unwrap(), 0);
    }

    #[test]
    fn unowned_cells_take_the_nearest_owned_cell_within_the_rings() {
        let base = CellIndex::try_from(cell(10.0, 10.0, 4)).unwrap();
        let ring2 = base.grid_ring_fast(2).flatten().next().unwrap();
        let mut owner = CellOwner::new([(u64::from(ring2), 1)]).unwrap();
        assert_eq!(owner.of_base(base), 1);
        let far = base.grid_ring_fast(MAX_RING + 3).flatten().next().unwrap();
        assert_eq!(owner.of_base(far), -1);
    }

    #[test]
    fn blocks_skip_nodata_and_non_positive_pixels() {
        let base = cell(0.5, 0.5, 4);
        let mut owner = CellOwner::new([(base, 0)]).unwrap();
        let mut band = Band {
            width: 2,
            height: 3,
            transform: [0.01, 0.0, 0.49, 0.0, -0.01, 0.51],
            values: vec![1.0, -2.0, 0.0, f64::NAN, 0.25, 4.0],
        };
        let total = province_population(&mut band, &mut owner, 2).unwrap();
        assert_eq!(total, vec![5.25]);
    }
}
