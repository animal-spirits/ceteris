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

/// The SHA-256 of every file under `dir` but `web/map.pmtiles`, by path relative to `dir`.
#[cfg(feature = "geos")]
fn digests(dir: &Path) -> std::collections::BTreeMap<String, String> {
    use sha2::{Digest, Sha256};
    let mut out = std::collections::BTreeMap::new();
    let mut todo = vec![dir.to_owned()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let path = e.unwrap().path();
            if path.is_dir() {
                todo.push(path);
                continue;
            }
            let name = path
                .strip_prefix(dir)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            if name != "web/map.pmtiles" {
                let digest = Sha256::digest(std::fs::read(&path).unwrap());
                out.insert(name, format!("{digest:x}"));
            }
        }
    }
    out
}

/// Two builds of the whole pack write the same bytes: every file, the manifest with its
/// digests among them, but the tiles archive, whose tiles tippecanoe writes in a varying order
/// (the manifest pins each layer's GeoJSON instead).
#[cfg(feature = "geos")]
#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "builds the whole pack twice; run with --release"
)]
fn two_builds_write_the_same_bytes() {
    use ceteris_pack::build::build;
    use ceteris_pack::fetch;
    use ceteris_pack::geo::provinces::Centres;

    let missing: Vec<String> = fetch::entries(&workspace_root())
        .unwrap()
        .into_iter()
        .filter(|e| !e.optional && !fetch::path_of(&e.name).exists())
        .map(|e| e.name)
        .collect();
    if !missing.is_empty() {
        eprintln!("skipping: {} files not in the fetch cache", missing.len());
        return;
    }
    if std::process::Command::new("tippecanoe")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("skipping: no tippecanoe on PATH");
        return;
    }
    let dirs = [tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap()];
    let built: Vec<_> = dirs
        .iter()
        .map(|d| build(d.path(), &Centres::H3o, None).unwrap())
        .collect();
    assert_eq!(built[0].tables, built[1].tables);
    let (a, b) = (digests(dirs[0].path()), digests(dirs[1].path()));
    assert!(a.contains_key("manifest.json") && a.contains_key("web/counties.fill"));
    assert_eq!(a, b);
    // Each file the manifest lists carries that file's digest; a tile layer's is its GeoJSON's.
    let text = std::fs::read_to_string(dirs[0].path().join("manifest.json")).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&text).unwrap();
    let tables = manifest["tables"].as_object().unwrap();
    assert_eq!(tables.len(), built[0].tables.len());
    for (name, digest) in tables {
        if !name.starts_with("web/map.pmtiles:") {
            assert_eq!(digest.as_str(), Some(a[name].as_str()), "{name}");
        }
    }
}
