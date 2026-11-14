//! `pack subset` on a synthetic pack: countries A, B, C and D; A and B are kept.

use std::path::Path;
use std::sync::Arc;

use arrow_array::{ArrayRef, BooleanArray, Float64Array, StringArray, UInt8Array, UInt32Array};
use ceteris_pack::subset::{Config, Table, read, schema, subset};
use ceteris_pack::table;

const NONE: u32 = u32::MAX;

fn f(v: &[f64]) -> ArrayRef {
    Arc::new(Float64Array::from(v.to_vec()))
}
fn u(v: &[u32]) -> ArrayRef {
    Arc::new(UInt32Array::from(v.to_vec()))
}
fn b(v: &[u8]) -> ArrayRef {
    Arc::new(UInt8Array::from(v.to_vec()))
}
fn s(v: &[&str]) -> ArrayRef {
    Arc::new(StringArray::from(v.to_vec()))
}
fn flags(v: &[bool]) -> ArrayRef {
    Arc::new(BooleanArray::from(v.to_vec()))
}

fn put(dir: &Path, name: &str, cols: Vec<(&str, ArrayRef)>) {
    let batch = table::batch(cols.into_iter().map(|(n, a)| (n.to_owned(), a)).collect()).unwrap();
    table::write(&dir.join(name), &batch, &Default::default()).unwrap();
}

/// Countries A B C D (indices 0..4); provinces A0 A1 B0 C0 D0; nodes: one per province, then a
/// sea node; edges chain them.
fn pack(dir: &Path) {
    put(
        dir,
        "provinces.arrow",
        vec![
            ("id", s(&["a0", "a1", "b0", "c0", "d0"])),
            ("country", s(&["A", "A", "B", "C", "D"])),
        ],
    );
    put(
        dir,
        "countries.arrow",
        vec![("code", s(&["A", "B", "C", "D"]))],
    );
    put(
        dir,
        "cells.arrow",
        vec![("cell", u(&[1, 2, 3, 4])), ("province", u(&[0, 2, 3, 4]))],
    );
    put(dir, "counties.arrow", vec![("province", u(&[1, 4, NONE]))]);
    put(
        dir,
        "nodes.arrow",
        vec![
            ("kind", b(&[0, 0, 0, 0, 0, 1])),
            ("province", u(&[0, 1, 2, 3, 4, NONE])),
        ],
    );
    put(
        dir,
        "edges.arrow",
        vec![("a", u(&[0, 1, 2, 3, 5])), ("b", u(&[1, 2, 3, 4, 2]))],
    );
    put(dir, "sea_zones.arrow", vec![("h3", s(&["z"]))]);
    put(dir, "chokepoints.arrow", vec![("name", s(&["c"]))]);
    put(dir, "econ_goods.arrow", vec![("name", s(&["g"]))]);
    put(
        dir,
        "econ_countries.arrow",
        vec![
            ("code", s(&["A", "B", "C", "D"])),
            ("gdp", f(&[10.0, 20.0, 30.0, 10.0])),
            ("fd_0", f(&[0.5, 0.5, 0.2, 0.6])),
            ("owner_0", f(&[0.0, 0.1, 0.2, 0.3])),
            ("owner_1", f(&[0.1, 0.0, 0.2, 0.3])),
            ("owner_2", f(&[0.1, 0.1, 0.0, 0.1])),
            ("owner_3", f(&[0.2, 0.2, 0.2, 0.0])),
            ("x_imputed", flags(&[false, false, true, false])),
            ("x_source", s(&["a", "b", "c", "d"])),
        ],
    );
    put(
        dir,
        "econ_io.arrow",
        vec![
            ("country", u(&[0, 1, 2, 3])),
            ("input", b(&[0; 4])),
            ("good", b(&[0; 4])),
            ("a", f(&[0.1, 0.2, 0.3, 0.5])),
        ],
    );
    put(
        dir,
        "econ_provinces.arrow",
        vec![("population", f(&[1.0, 2.0, 3.0, 4.0, 5.0]))],
    );
    put(
        dir,
        "econ_pops.arrow",
        vec![("province", u(&[0, 2, 3, 4])), ("edu", b(&[0; 4]))],
    );
    let (mut imp, mut exp, mut grp, mut rate, mut agr, mut imputed) =
        (vec![], vec![], vec![], vec![], vec![], vec![]);
    for i in 0..4u32 {
        for j in 0..4u32 {
            imp.push(i);
            exp.push(j);
            grp.push(0u8);
            rate.push(f64::from(i * 4 + j));
            agr.push(i == 3);
            imputed.push(false);
        }
    }
    put(
        dir,
        "econ_tariffs.arrow",
        vec![
            ("importer", u(&imp)),
            ("exporter", u(&exp)),
            ("group", b(&grp)),
            ("rate", f(&rate)),
            ("agreement", flags(&agr)),
            ("imputed", flags(&imputed)),
        ],
    );
    put(
        dir,
        "gateways.arrow",
        vec![
            ("node", u(&[1, 3, 4])),
            ("country", u(&[0, 2, 3])),
            ("mass", f(&[1.0, 2.0, 3.0])),
        ],
    );
    put(
        dir,
        "econ_home_shares.arrow",
        vec![
            ("country", u(&[0, 1, 2, 3])),
            ("good", b(&[0; 4])),
            ("home_share", f(&[0.9, 0.8, 0.5, f64::NAN])),
            ("absorption", f(&[1.0, 2.0, 3.0, f64::NAN])),
            ("icio", flags(&[true, true, true, false])),
        ],
    );
    std::fs::write(
        dir.join("manifest.json"),
        r#"{"pack_format":3,"year":2000,"sources":[
            {"source":"baci","name":"b.zip","sha256":"1"},
            {"source":"epr-v1","name":"e.zip","sha256":"2"},
            {"source":"exiobase-3","name":"x.zip","sha256":"3"}]}"#,
    )
    .unwrap();
}

const CONFIG: &str = r#"
countries = ["A", "B"]
row = "ROW"
cap_mb = 25
exclude_sources = ["epr*", "exiobase*"]
[rules.econ_countries]
code = "const:ROW"
gdp = "sum"
fd_0 = "mean"
"owner_*" = "mean"
"*_imputed" = "any"
"*_source" = "first"
__country = "key"
[rules.econ_io]
country = "key"
input = "key"
good = "key"
a = "mean"
[rules.econ_tariffs]
importer = "key"
exporter = "key"
group = "key"
rate = "mean"
agreement = "any"
imputed = "any"
[rules.econ_home_shares]
country = "key"
good = "key"
home_share = "mean"
absorption = "sum"
icio = "any"
"#;

fn run(dir: &Path, config: &str) -> (Config, std::path::PathBuf, ceteris_pack::subset::Report) {
    let input = dir.join("in");
    std::fs::create_dir_all(&input).unwrap();
    pack(&input);
    let out = dir.join("out");
    let cfg = Config::parse(config).unwrap();
    let report = subset(&input, &cfg, &out).unwrap();
    (cfg, out, report)
}

fn col_f(t: &Table, name: &str) -> Vec<f64> {
    let i = t.names.iter().position(|n| n == name).unwrap();
    match &t.cols[i] {
        ceteris_pack::subset::Col::F(v) => v.clone(),
        _ => panic!("{name}"),
    }
}

#[test]
fn geography_is_filtered_and_renumbered() {
    let dir = tempfile::tempdir().unwrap();
    let (_, out, report) = run(dir.path(), CONFIG);
    assert_eq!((report.provinces, report.countries), (3, 3));
    let t = |n: &str| read(&out.join(n)).unwrap();
    assert_eq!(t("provinces.arrow").rows(), 3);
    assert_eq!(t("countries.arrow").rows(), 2);
    // cells of province 0, 2 survive as 0, 2 -> new 0, 2; counties: only province 1 -> 1
    assert_eq!(t("cells.arrow").rows(), 2);
    assert_eq!(t("counties.arrow").rows(), 1);
    // nodes 0, 1, 2 and the sea node survive; edges 0-1, 1-2 and sea-2 survive
    assert_eq!(t("nodes.arrow").rows(), 4);
    assert_eq!(t("edges.arrow").rows(), 3);
    assert_eq!(t("econ_provinces.arrow").rows(), 3);
    assert_eq!(t("econ_pops.arrow").rows(), 2);
    // gateway of country A at node 1 only; C and D have none
    assert_eq!(t("gateways.arrow").rows(), 1);
}

#[test]
fn dropped_countries_aggregate_by_rule() {
    let dir = tempfile::tempdir().unwrap();
    let (_, out, _) = run(dir.path(), CONFIG);
    let c = read(&out.join("econ_countries.arrow")).unwrap();
    assert_eq!(c.rows(), 3);
    assert_eq!(col_f(&c, "gdp"), vec![10.0, 20.0, 40.0]); // sum
    let fd = col_f(&c, "fd_0");
    assert!((fd[2] - (0.2 * 30.0 + 0.6 * 10.0) / 40.0).abs() < 1e-12); // gdp-weighted
    // owners: kept owners keep their column, dropped owners (C, D) sum into ROW's column
    let owner_row = col_f(&c, "owner_2");
    assert!((owner_row[0] - 0.3).abs() < 1e-12); // host A: 0.1 + 0.2
    let io = read(&out.join("econ_io.arrow")).unwrap();
    assert!((col_f(&io, "a")[2] - (0.3 * 30.0 + 0.5 * 10.0) / 40.0).abs() < 1e-12);
    let tar = read(&out.join("econ_tariffs.arrow")).unwrap();
    assert_eq!(tar.rows(), 9);
    let hs = read(&out.join("econ_home_shares.arrow")).unwrap();
    assert_eq!(col_f(&hs, "absorption")[2], 3.0); // NaN skipped
    assert_eq!(col_f(&hs, "home_share")[2], 0.5);
}

#[test]
fn subset_is_deterministic() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (_, out_a, ra) = run(a.path(), CONFIG);
    let (_, out_b, rb) = run(b.path(), CONFIG);
    assert_eq!(ra.tables, rb.tables);
    assert_eq!(
        std::fs::read(out_a.join("manifest.json")).unwrap(),
        std::fs::read(out_b.join("manifest.json")).unwrap()
    );
}

#[test]
fn manifest_excludes_share_alike_and_validation_sources() {
    let dir = tempfile::tempdir().unwrap();
    let (_, out, _) = run(dir.path(), CONFIG);
    let m: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("manifest.json")).unwrap()).unwrap();
    let kept: Vec<&str> = m["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["source"].as_str().unwrap())
        .collect();
    assert_eq!(kept, ["baci"]);
    assert_eq!(
        m["excluded_sources"],
        serde_json::json!(["epr-v1", "exiobase-3"])
    );
}

#[test]
fn missing_rule_and_cap_are_errors() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in");
    std::fs::create_dir_all(&input).unwrap();
    pack(&input);
    let no_rule = CONFIG.replace("fd_0 = \"mean\"\n", "");
    let err = subset(
        &input,
        &Config::parse(&no_rule).unwrap(),
        &dir.path().join("o1"),
    )
    .unwrap_err();
    assert!(err.contains("fd_0"), "{err}");
    let (_, missing) = schema(&input, &Config::parse(&no_rule).unwrap()).unwrap();
    assert_eq!(missing, ["econ_countries.arrow.fd_0"]);
    let tiny = CONFIG.replace("cap_mb = 25", "cap_mb = 0");
    let err = subset(
        &input,
        &Config::parse(&tiny).unwrap(),
        &dir.path().join("o2"),
    )
    .unwrap_err();
    assert!(err.contains("cap"), "{err}");
}

#[test]
fn calibrated_pack_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in");
    std::fs::create_dir_all(&input).unwrap();
    pack(&input);
    put(&input, "epsilon.arrow", vec![("x", f(&[1.0]))]);
    let err = subset(
        &input,
        &Config::parse(CONFIG).unwrap(),
        &dir.path().join("o"),
    )
    .unwrap_err();
    assert!(err.contains("epsilon.arrow"), "{err}");
}
