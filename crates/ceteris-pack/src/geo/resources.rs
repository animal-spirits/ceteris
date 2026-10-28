//! Mineral deposits per province from USGS MRDS (Section 5.2). Coverage is US-heavy (about 89%
//! of sites).

use std::collections::BTreeSet;
use std::path::Path;

use csv::StringRecord;
use geo::{BoundingRect, Contains, MultiPolygon, Point};
use rstar::primitives::{GeomWithData, Rectangle};
use rstar::{AABB, RTree};

use crate::numeric::parse_f64;
use crate::read::{Csv, read_member};

pub const MRDS: &str = "mrds-csv.zip";

/// Primary commodity (MRDS commod1) to resource group; construction materials are left out.
pub const GROUPS: [(&str, &[&str]); 5] = [
    (
        "precious",
        &[
            "Gold",
            "Silver",
            "Platinum",
            "Palladium",
            "PGE",
            "Diamond",
            "Gemstone",
        ],
    ),
    (
        "base",
        &[
            "Copper",
            "Lead",
            "Zinc",
            "Nickel",
            "Tin",
            "Aluminum",
            "Antimony",
            "Mercury",
            "Cobalt",
            "Molybdenum",
            "Tungsten",
        ],
    ),
    (
        "ferrous",
        &["Iron", "Manganese", "Chromium", "Vanadium", "Titanium"],
    ),
    ("energy", &["Uranium", "Thorium", "Coal"]),
    (
        "industrial",
        &[
            "Phosphorus-Phosphates",
            "Potassium",
            "Sulfur",
            "Salt",
            "Boron-Borates",
            "Fluorine-Fluorite",
            "Barium-Barite",
            "Graphite",
            "Lithium",
        ],
    ),
];
const PRODUCING: [&str; 2] = ["Producer", "Past Producer"];

/// The polygons containing a point in their interior: points on a boundary lie in none.
pub struct Locator<'a> {
    geoms: &'a [MultiPolygon],
    tree: RTree<GeomWithData<Rectangle<[f64; 2]>, usize>>,
}

impl<'a> Locator<'a> {
    pub fn new(geoms: &'a [MultiPolygon]) -> Self {
        let boxes = geoms
            .iter()
            .enumerate()
            .filter_map(|(i, g)| {
                let r = g.bounding_rect()?;
                let corners = Rectangle::from_corners(r.min().into(), r.max().into());
                Some(GeomWithData::new(corners, i))
            })
            .collect();
        Locator {
            geoms,
            tree: RTree::bulk_load(boxes),
        }
    }

    /// The lowest index of a polygon containing `(x, y)`.
    pub fn within(&self, x: f64, y: f64) -> Option<usize> {
        let p = Point::new(x, y);
        self.tree
            .locate_in_envelope_intersecting(&AABB::from_point([x, y]))
            .map(|it| it.data)
            .filter(|&i| self.geoms[i].contains(&p))
            .min()
    }
}

/// Producing and past-producing sites per province, by group of their primary commodities, in
/// the order of `GROUPS`.
pub fn counts(mrds_zip: &Path, provinces: &[MultiPolygon]) -> Result<Vec<Vec<u32>>, String> {
    let bytes = read_member(mrds_zip, |n| n == "mrds.csv")?;
    // The file is Latin-1: each byte is the code point of its character.
    let text: String = bytes.iter().map(|&b| char::from(b)).collect();
    let mut csv = Csv::new(text.as_bytes(), mrds_zip)?;
    let [dep_id, dev_stat, lon, lat, commod1] =
        ["dep_id", "dev_stat", "longitude", "latitude", "commod1"].map(|c| csv.col(c));
    let (dep_id, dev_stat, lon, lat, commod1) = (dep_id?, dev_stat?, lon?, lat?, commod1?);
    let mut sites = Vec::new();
    let mut row = StringRecord::new();
    while csv.next(&mut row)? {
        let cell = |i: usize| row.get(i).unwrap_or("");
        if !PRODUCING.contains(&cell(dev_stat)) || cell(lon).is_empty() || cell(lat).is_empty() {
            continue;
        }
        let number = |i: usize| {
            parse_f64(cell(i))
                .ok_or_else(|| format!("{}: {:?} is not a number", mrds_zip.display(), cell(i)))
        };
        sites.push((
            cell(dep_id).to_owned(),
            number(lon)?,
            number(lat)?,
            cell(commod1).to_owned(),
        ));
    }
    sites.sort_by(|a, b| a.0.cmp(&b.0));
    let locator = Locator::new(provinces);
    let mut out = vec![vec![0u32; provinces.len()]; GROUPS.len()];
    for (_, x, y, commodities) in &sites {
        let Some(p) = locator.within(*x, *y) else {
            continue;
        };
        let commodities: BTreeSet<&str> = commodities.split(',').map(str::trim).collect();
        for (g, (_, members)) in GROUPS.iter().enumerate() {
            if members.iter().any(|m| commodities.contains(m)) {
                out[g][p] += 1;
            }
        }
    }
    Ok(out)
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

    #[test]
    fn points_on_a_boundary_lie_in_no_polygon() {
        let geoms = [square(0.0, 0.0, 1.0, 1.0), square(1.0, 0.0, 2.0, 1.0)];
        let l = Locator::new(&geoms);
        assert_eq!(l.within(0.5, 0.5), Some(0));
        assert_eq!(l.within(1.5, 0.5), Some(1));
        assert_eq!(l.within(1.0, 0.5), None);
        assert_eq!(l.within(0.5, 0.0), None);
        assert_eq!(l.within(3.0, 0.5), None);
    }

    #[test]
    fn overlapping_polygons_give_the_lower_index() {
        let geoms = [square(0.0, 0.0, 3.0, 3.0), square(1.0, 1.0, 2.0, 2.0)];
        let l = Locator::new(&geoms[..]);
        assert_eq!(l.within(1.5, 1.5), Some(0));
        let reversed = [geoms[1].clone(), geoms[0].clone()];
        assert_eq!(Locator::new(&reversed).within(1.5, 1.5), Some(0));
    }
}
