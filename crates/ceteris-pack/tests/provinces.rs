//! Provinces and counties on the fetch cache, against the pack and the county lock.

mod common;

use std::collections::BTreeMap;

use ceteris_pack::geo::{counties, provinces};
use ceteris_pack::sources::{CONFIG_DIR, workspace_root};
use common::{cached, pack_table, strings};
use serde_json::Value;

const ADMIN1: &str = "ne_10m_admin_1_states_provinces.zip";
const ADMIN0: &str = "ne_10m_admin_0_countries.zip";

#[test]
fn loading_gives_the_regions_of_the_pack() {
    let (Some(admin1), Some(admin0), Some(pack)) = (
        cached(ADMIN1),
        cached(ADMIN0),
        pack_table("provinces.arrow"),
    ) else {
        return;
    };
    let base = provinces::regions(&admin1, &admin0).unwrap();
    let ids = strings(&pack, "id");
    assert_eq!(base.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ids);
    let by_id: BTreeMap<&str, &provinces::Province> =
        base.iter().map(|p| (p.id.as_str(), p)).collect();
    for (column, get) in [
        (
            "country",
            (|p: &provinces::Province| p.country.clone()) as fn(&_) -> String,
        ),
        ("sovereign", |p| p.sovereign.clone()),
        ("country_now", |p| p.country_now.clone()),
    ] {
        for (id, value) in ids.iter().zip(strings(&pack, column)) {
            assert_eq!(get(by_id[id.as_str()]), value, "{column} of {id}");
        }
    }
    for (id, name) in ids.iter().zip(strings(&pack, "name")) {
        assert_eq!(by_id[id.as_str()].name, name);
    }
}

/// The lock's formatting and digests: the API answers here carry only the level each country
/// was recorded at, so the level choice is the unit tests' (`counties::tests`).
#[test]
fn recording_the_cached_files_reproduces_the_lock() {
    let Some(admin1) = cached(ADMIN1) else {
        return;
    };
    let root = workspace_root();
    let lock = std::fs::read_to_string(root.join(CONFIG_DIR).join(counties::LOCK)).unwrap();
    let table: toml::Table = lock.parse().unwrap();
    let mut meta: BTreeMap<&str, BTreeMap<String, counties::Meta>> = BTreeMap::new();
    for c in table["country"].as_array().unwrap() {
        let text = |k: &str| c[k].as_str().unwrap().to_owned();
        let record: counties::Meta = [
            (
                "admUnitCount",
                Value::from(c["units"].as_integer().unwrap().to_string()),
            ),
            ("simplifiedGeometryGeoJSON", Value::from(text("url"))),
            ("boundaryLicense", Value::from(text("licence"))),
            ("boundaryYearRepresented", Value::from(text("year"))),
            ("boundaryID", Value::from(text("boundary_id"))),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
        let level = counties::LEVELS
            .into_iter()
            .find(|l| *l == c["level"].as_str().unwrap())
            .unwrap();
        meta.entry(level).or_default().insert(text("iso"), record);
    }
    let per_country = counties::regions_per_country(&admin1).unwrap();
    let mut missing = false;
    let text = counties::lock_text(&meta, &per_country, |e| {
        let path = ceteris_pack::fetch::path_of(&e.name);
        missing |= !path.exists();
        Ok(path)
    });
    if missing {
        eprintln!("skipping: geoBoundaries files missing from the cache");
        return;
    }
    // The first line names the command that wrote the lock.
    let body = |t: &str| t.split_once('\n').unwrap().1.to_owned();
    assert_eq!(body(&text.unwrap()), body(&lock));
}

/// The owner-side build: the provinces and clipped counties of the cache, as the pack holds
/// them.
#[cfg(feature = "geos")]
#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "builds every province and county; run with --release"
)]
fn the_geo_build_gives_the_provinces_counties_and_cells_of_the_pack() {
    use arrow_array::types::{Float64Type, UInt32Type, UInt64Type};
    use ceteris_pack::geo::raster_terrain::assign_cells;
    use ceteris_pack::geo::{overlay, sphere};
    use h3o::Resolution;
    use std::fs::File;
    use std::path::Path;

    use arrow_array::cast::AsArray;
    use arrow_array::{ArrowPrimitiveType, RecordBatch};
    use arrow_ipc::reader::FileReader;

    fn column<T: ArrowPrimitiveType>(path: &Path, name: &str) -> Vec<T::Native> {
        let reader = FileReader::try_new(File::open(path).unwrap(), None).unwrap();
        let batches: Vec<RecordBatch> = reader.map(Result::unwrap).collect();
        batches
            .iter()
            .flat_map(|b| {
                b.column_by_name(name)
                    .unwrap()
                    .as_primitive::<T>()
                    .values()
                    .to_vec()
            })
            .collect()
    }

    let (Some(admin1), Some(admin0), Some(pack), Some(cells), Some(pack_counties)) = (
        cached(ADMIN1),
        cached(ADMIN0),
        pack_table("provinces.arrow"),
        pack_table("cells.arrow"),
        pack_table("counties.arrow"),
    ) else {
        return;
    };
    let root = workspace_root();
    let config: toml::Table = std::fs::read_to_string(root.join(CONFIG_DIR).join("pack.toml"))
        .unwrap()
        .parse()
        .unwrap();
    let res = Resolution::try_from(u8::try_from(config["land_res"].as_integer().unwrap()).unwrap())
        .unwrap();
    let units = counties::load(&root).unwrap();
    let provs = provinces::build(&admin1, &admin0).unwrap();
    let ids: Vec<&str> = provs.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, strings(&pack, "id"));
    let names: Vec<&str> = provs.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, strings(&pack, "name"));
    let near = |a: &[f64], b: &[f64], tol: f64, what: &str| {
        let worst = a
            .iter()
            .zip(b)
            .map(|(x, y)| (x / y - 1.0).abs())
            .fold(0.0, f64::max);
        assert!(worst <= tol, "{what}: {worst}");
    };
    let area: Vec<f64> = provs.iter().map(|p| p.area_km2).collect();
    near(
        &area,
        &column::<Float64Type>(&pack, "area_km2"),
        1e-7,
        "province areas",
    );

    let geoms: Vec<_> = provs.iter().map(|p| provinces::parts(&p.geom)).collect();
    let anchors: Vec<(f64, f64)> = provs.iter().map(|p| (p.lon, p.lat)).collect();
    let (ids, owner) = assign_cells(&geoms, &anchors, res).unwrap();
    let ids: Vec<u64> = ids.into_iter().map(u64::from).collect();
    assert_eq!(ids, column::<UInt64Type>(&cells, "cell"));
    assert_eq!(owner, column::<UInt32Type>(&cells, "province"));

    let clipped = counties::clip(&units, &provs).unwrap();
    assert_eq!(
        clipped.province,
        column::<UInt32Type>(&pack_counties, "province")
    );
    let names: Vec<&str> = clipped.counties.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, strings(&pack_counties, "name"));
    let area: Vec<f64> = clipped
        .counties
        .iter()
        .map(|c| sphere::area_km2(&overlay::to_multipolygon(&c.geom).unwrap()))
        .collect();
    near(
        &area,
        &column::<Float64Type>(&pack_counties, "area_km2"),
        1e-7,
        "county areas",
    );
}

/// Python keeps an admin-1 repair as `make_valid` returns it, Rust its polygonal part: the same
/// geometry while no repair is a collection.
#[cfg(feature = "geos")]
#[test]
fn admin_1_repairs_leave_no_collection() {
    use ceteris_pack::geo::overlay;
    use geos::{Geom, GeometryTypes};

    let (Some(admin1), Some(admin0)) = (cached(ADMIN1), cached(ADMIN0)) else {
        return;
    };
    let mut repaired = Vec::new();
    for p in provinces::regions(&admin1, &admin0).unwrap() {
        let g = overlay::from_geo(&p.geom).unwrap();
        if !overlay::is_valid(&g).unwrap() {
            let kind = overlay::make_valid(&g).unwrap().geometry_type().unwrap();
            repaired.push((p.id, kind));
        }
    }
    assert_eq!(repaired.len(), 1, "{repaired:?}");
    assert!(
        repaired
            .iter()
            .all(|(_, k)| *k != GeometryTypes::GeometryCollection),
        "{repaired:?}"
    );
}
