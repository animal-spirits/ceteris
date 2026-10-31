//! Province set: Natural Earth admin-1 at 2000 borders, whole (Section 5.2, ADRs 0006, 0251).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use geo::{Geometry, MultiPolygon};
use toml::Value;

use crate::read::natural_earth::{Dbf, shapes_from_zip};
use crate::sources::{CONFIG_DIR, read_table, workspace_root};

#[derive(Clone, Debug)]
pub struct Province {
    /// Natural Earth adm1_code.
    pub id: String,
    pub name: String,
    pub country: String,
    pub sovereign: String,
    pub country_now: String,
    /// A Polygon or MultiPolygon.
    pub geom: Geometry,
    pub area_km2: f64,
    pub lon: f64,
    pub lat: f64,
}

/// The polygons of a Polygon or MultiPolygon, as `shapely.get_parts` lists them; an empty
/// polygon has none.
pub fn parts(g: &Geometry) -> MultiPolygon {
    match g {
        Geometry::Polygon(p) if p.exterior().0.is_empty() => MultiPolygon(vec![]),
        Geometry::Polygon(p) => MultiPolygon(vec![p.clone()]),
        Geometry::MultiPolygon(m) => m.clone(),
        _ => MultiPolygon(vec![]),
    }
}

/// `borders_2000.toml`: successor states and transferred regions mapped to their 2000 country.
#[derive(Debug, Default)]
pub struct Borders {
    pub exclude: BTreeSet<String>,
    pub country: BTreeMap<String, String>,
    pub province: BTreeMap<String, String>,
    pub sovereign: BTreeMap<String, String>,
    /// Names for codes Natural Earth does not carry.
    pub names: BTreeMap<String, String>,
}

pub fn borders() -> Result<Borders, String> {
    let path = workspace_root().join(CONFIG_DIR).join("borders_2000.toml");
    let t = read_table(&path)?;
    let map = |key: &str| -> BTreeMap<String, String> {
        t.get(key)
            .and_then(Value::as_table)
            .into_iter()
            .flatten()
            .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned())))
            .collect()
    };
    let exclude = t
        .get("exclude")
        .and_then(|e| e.get("countries"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|c| c.as_str().map(str::to_owned))
        .collect();
    Ok(Borders {
        exclude,
        country: map("country"),
        province: map("province"),
        sovereign: map("sovereign"),
        names: map("names"),
    })
}

/// Where the sea grid takes the centres of H3 cells from.
#[derive(Debug, Default)]
pub enum Centres {
    #[default]
    H3o,
}

impl Centres {
    /// The (lng, lat) centre of a cell, in degrees.
    pub fn lng_lat(&self, cell: h3o::CellIndex) -> Result<[f64; 2], String> {
        match self {
            Centres::H3o => {
                let c = h3o::LatLng::from(cell);
                Ok([c.lng(), c.lat()])
            }
        }
    }
}

/// Natural Earth sov_a3 (which uses codes like GB1, NL1) to the sovereign's adm0_a3.
fn sovereign_codes(admin0: &Path) -> Result<BTreeMap<Option<String>, Option<String>>, String> {
    let dbf = Dbf::from_zip(admin0)?;
    let rows = dbf.select(["SOV_A3", "SOVEREIGNT", "ADMIN", "ADM0_A3"])?;
    let own = |s: Option<&str>| s.map(str::to_owned);
    let by_name: BTreeMap<_, _> = rows.iter().map(|r| (own(r[2]), own(r[3]))).collect();
    Ok(rows
        .iter()
        .map(|r| {
            let code = by_name.get(&own(r[1])).cloned().unwrap_or(own(r[0]));
            (own(r[0]), code)
        })
        .collect())
}

/// The admin-1 regions at 2000 borders, ordered by (country, id), their geometry as read (the
/// geo build repairs it).
pub fn regions(admin1: &Path, admin0: &Path) -> Result<Vec<Province>, String> {
    let b = borders()?;
    let sov = sovereign_codes(admin0)?;
    let dbf = Dbf::from_zip(admin1)?;
    let rows = dbf.select(["adm1_code", "name", "adm0_a3", "sov_a3"])?;
    let shapes = shapes_from_zip(admin1)?;
    if rows.len() != shapes.len() {
        return Err(format!(
            "{}: {} records but {} shapes",
            admin1.display(),
            rows.len(),
            shapes.len()
        ));
    }
    let mut out = Vec::new();
    for ([code, name, now, sov_a3], shape) in rows.into_iter().zip(shapes) {
        let (code, now) = (code.unwrap_or_default(), now.unwrap_or_default());
        if b.exclude.contains(now) {
            continue;
        }
        let country = b
            .province
            .get(code)
            .or_else(|| b.country.get(now))
            .map_or(now, String::as_str)
            .to_owned();
        // A remapped province is under its 2000 country's sovereignty.
        let default = if country != now {
            country.clone()
        } else {
            let key = sov_a3.map(str::to_owned);
            sov.get(&key).cloned().unwrap_or(key).unwrap_or_default()
        };
        let sovereign = b.sovereign.get(&country).cloned().unwrap_or(default);
        let geom = shape.ok_or_else(|| format!("{code}: no geometry"))?;
        out.push(Province {
            id: code.to_owned(),
            name: name.unwrap_or(code).to_owned(),
            country,
            sovereign,
            country_now: now.to_owned(),
            geom,
            area_km2: 0.0,
            lon: 0.0,
            lat: 0.0,
        });
    }
    out.sort_by(|a, b| (&a.country, &a.id).cmp(&(&b.country, &b.id)));
    Ok(out)
}

#[cfg(feature = "geos")]
pub use repaired::{build, load};

#[cfg(feature = "geos")]
mod repaired {
    use super::{Province, parts};
    use crate::geo::overlay::{self, from_geo, to_geo};
    use crate::geo::sphere::{anchor, area_km2};

    /// The admin-1 regions at 2000 borders, an invalid geometry repaired as shapely's
    /// `make_valid` repairs it.
    pub fn load(
        admin1: &std::path::Path,
        admin0: &std::path::Path,
    ) -> Result<Vec<Province>, String> {
        let mut out = super::regions(admin1, admin0)?;
        for p in &mut out {
            let g = from_geo(&p.geom)?;
            if !overlay::is_valid(&g)? {
                p.geom = to_geo(&overlay::make_valid(&g)?)?;
            }
        }
        Ok(out)
    }

    /// Provinces at 2000 borders, whole (ADR 0251).
    pub fn build(
        admin1: &std::path::Path,
        admin0: &std::path::Path,
    ) -> Result<Vec<Province>, String> {
        let mut out = load(admin1, admin0)?;
        for p in &mut out {
            let shape = parts(&p.geom);
            p.area_km2 = area_km2(&shape);
            (p.lon, p.lat) = anchor(&shape).ok_or_else(|| format!("{}: no parts", p.id))?;
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_2000_borders_remap_successor_states() {
        let b = borders().unwrap();
        assert_eq!(b.country["SDS"], "SDN");
        assert_eq!(b.country["MNE"], "YUG");
        assert_eq!(b.province["RUS-283"], "UKR");
    }

    #[test]
    fn centres_come_from_h3o() {
        let cell = h3o::LatLng::new(45.0, 7.0)
            .unwrap()
            .to_cell(h3o::Resolution::Four);
        let h = h3o::LatLng::from(cell);
        assert_eq!(Centres::H3o.lng_lat(cell).unwrap(), [h.lng(), h.lat()]);
    }
}
