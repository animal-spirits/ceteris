//! The pack build: its configuration, the country names it writes, and the invariants of the
//! pack's tables.

mod common;

use std::collections::BTreeSet;
use std::fs::File;
use std::path::Path;

use arrow_array::cast::AsArray;
use arrow_array::types::{Float64Type, UInt32Type};
use arrow_array::{ArrowPrimitiveType, RecordBatch};
use arrow_ipc::reader::FileReader;
use ceteris_pack::build::{ADMIN0, country_names, split_area_km2};
use ceteris_pack::geo::provinces::borders;
use ceteris_pack::sources::{CONFIG_DIR, workspace_root};
use common::{cached, pack_table, strings};

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

fn config() -> toml::Table {
    let path = workspace_root().join(CONFIG_DIR).join("pack.toml");
    std::fs::read_to_string(path).unwrap().parse().unwrap()
}

#[test]
fn the_2000_borders_remap_successor_states() {
    let b = borders().unwrap();
    assert_eq!(b.country["SDS"], "SDN");
    assert_eq!(b.country["MNE"], "YUG");
    assert_eq!(b.province["RUS-283"], "UKR");
    assert_eq!(b.names["YUG"], "Federal Republic of Yugoslavia");
}

#[test]
fn countries_are_named_from_admin_0_and_the_borders() {
    let Some(admin0) = cached(ADMIN0) else {
        return;
    };
    let names = country_names(&admin0, &borders().unwrap().names).unwrap();
    assert_eq!(names["FRA"].as_deref(), Some("France"));
    assert_eq!(
        names["YUG"].as_deref(),
        Some("Federal Republic of Yugoslavia")
    );
}

#[test]
fn the_pack_tables_hold_their_invariants() {
    let (Some(provinces), Some(counties)) =
        (pack_table("provinces.arrow"), pack_table("counties.arrow"))
    else {
        return;
    };
    let ids = strings(&provinces, "id");
    assert_eq!(ids.iter().collect::<BTreeSet<_>>().len(), ids.len());
    let area = column::<Float64Type>(&provinces, "area_km2");
    let total: f64 = area.iter().sum();
    assert!((total - 134.5e6).abs() <= 0.01 * 134.5e6, "{total}");
    let largest = area.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    assert!(largest < 2.0 * split_area_km2(&config()).unwrap());
    let home = column::<UInt32Type>(&counties, "province");
    assert!(home.iter().all(|&h| (h as usize) < ids.len()));
    assert!(home.windows(2).all(|w| w[0] <= w[1]));

    let Some(manifest) = pack_table("manifest.json") else {
        return;
    };
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(manifest).unwrap()).unwrap();
    let stats = &manifest["stats"];
    assert!(stats["county_area_outside_province"].as_f64().unwrap() < 0.05);
    assert!(
        stats["split_by_counties"].as_u64().unwrap() > stats["split_by_voronoi"].as_u64().unwrap()
    );
}
