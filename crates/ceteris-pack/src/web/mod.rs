//! The map's web outputs (Section 16.1): the province, country and county layers of the vector
//! tiles (ADR 0009) and the fill buffers (ADR 0010), all from geometry simplified for the map,
//! so fills meet the drawn borders.

#[cfg(feature = "geos")]
pub mod dump;
pub mod geojson;
#[cfg(feature = "geos")]
pub mod simplify;
pub mod tiles;

use std::collections::BTreeMap;
use std::path::Path;

use geo::Geometry;
use sha2::{Digest, Sha256};

use crate::geo::fills;
use crate::geo::provinces::parts;

/// Map geometry simplification tolerance in degrees (about 1 km).
pub const SIMPLIFY_DEG: f64 = 0.01;
/// Map tiles: the deepest zoom, and the zoom from which counties are drawn.
pub const MAX_ZOOM: u32 = 9;
pub const COUNTY_MINZOOM: u32 = 4;

/// The simplified geometry the map draws: provinces, countries in code order, counties.
pub struct Layers {
    pub provinces: Vec<Geometry>,
    pub countries: Vec<Geometry>,
    pub counties: Vec<Geometry>,
}

/// What the web outputs add to the manifest.
pub struct Written {
    /// `web/map.pmtiles:<layer>` (the SHA-256 of the layer's GeoJSON) and `web/<name>.fill`.
    pub tables: BTreeMap<String, String>,
    /// Vertex and triangle counts by fill file, `provinces` and `counties`.
    pub fills: BTreeMap<String, fills::Stats>,
}

/// `map.pmtiles`, `provinces.fill` and `counties.fill` of `layers` in `web`.
pub fn write_layers(web: &Path, layers: &Layers) -> Result<Written, String> {
    let mut tables = tiles::write(
        &web.join("map.pmtiles"),
        &[
            tiles::Layer {
                name: "provinces",
                geoms: &layers.provinces,
                minzoom: 0,
            },
            tiles::Layer {
                name: "countries",
                geoms: &layers.countries,
                minzoom: 0,
            },
            tiles::Layer {
                name: "counties",
                geoms: &layers.counties,
                minzoom: COUNTY_MINZOOM,
            },
        ],
    )?;
    let mut stats = BTreeMap::new();
    for (name, geoms) in [
        ("provinces", &layers.provinces),
        ("counties", &layers.counties),
    ] {
        let path = web.join(format!("{name}.fill"));
        let polygons: Vec<_> = geoms.iter().map(parts).collect();
        stats.insert(name.to_owned(), fills::write(&path, &polygons)?);
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        tables.insert(
            format!("web/{name}.fill"),
            tiles::hex(Sha256::digest(bytes)),
        );
    }
    Ok(Written {
        tables,
        fills: stats,
    })
}

/// The layers of the provinces (with their country codes) and the clipped counties:
/// each set simplified, the countries the unions of their simplified provinces.
#[cfg(feature = "geos")]
pub fn layers(
    provinces: &[Geometry],
    country: &[String],
    counties: &[Geometry],
) -> Result<Layers, String> {
    use crate::geo::overlay;

    if country.len() != provinces.len() {
        return Err(format!(
            "{} provinces, {} country codes",
            provinces.len(),
            country.len()
        ));
    }

    let to_geos = |geoms: &[Geometry]| -> Result<Vec<geos::Geometry>, String> {
        geoms.iter().map(overlay::from_geo).collect()
    };
    let to_geo = |geoms: &[geos::Geometry]| -> Result<Vec<Geometry>, String> {
        geoms.iter().map(overlay::to_geo).collect()
    };
    let simple = simplify::simplify(&to_geos(provinces)?)?;
    let countries = simplify::countries(&simple, country)?;
    let counties = simplify::simplify(&to_geos(counties)?)?;
    Ok(Layers {
        provinces: to_geo(&simple)?,
        countries: to_geo(&countries)?,
        counties: to_geo(&counties)?,
    })
}

/// The web outputs of the provinces, their countries and the clipped counties, written to `web`.
#[cfg(feature = "geos")]
pub fn write(
    web: &Path,
    provinces: &[Geometry],
    country: &[String],
    counties: &[Geometry],
) -> Result<Written, String> {
    write_layers(web, &layers(provinces, country, counties)?)
}

#[cfg(all(test, feature = "geos"))]
mod tests {
    use super::*;
    use geo::{LineString, Polygon};

    #[test]
    fn layers_refuse_a_country_list_of_another_length() {
        let square = Geometry::Polygon(Polygon::new(
            LineString::from(vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 0.0)]),
            vec![],
        ));
        let provinces = [square.clone(), square];
        let err = layers(&provinces, &["AAA".to_owned()], &[]).err().unwrap();
        assert_eq!(err, "2 provinces, 1 country codes");
    }
}
