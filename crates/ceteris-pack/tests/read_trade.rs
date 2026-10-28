//! Readers of BACI, the OECD HS to CPA key, WDI tariff rates and the trade agreement lists.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ceteris_pack::read::baci::{
    KEY_SHEET, baci_by_product, baci_values, read_hs_cpa, trade_by_country,
};
use ceteris_pack::read::tariffs::{read_cepii, read_dta};
use ceteris_pack::read::wdi::plausible_rates;
use common::{V, approx, cached, write_rows, write_zip};

/// The OECD key with one HS 1988/92 line per entry of `lines` and one line of a later HS version.
fn key(path: &Path, lines: &[(&str, &str)]) -> PathBuf {
    let mut rows: Vec<Vec<(&str, V)>> = lines
        .iter()
        .map(|(hs, cpa)| {
            vec![
                ("HS-version", V::N(0.0)),
                ("HS-code", V::T(hs)),
                ("CPA", V::T(cpa)),
                ("ENDUSE", V::T("INT")),
            ]
        })
        .collect();
    rows.push(vec![
        ("HS-version", V::N(1.0)),
        ("HS-code", V::T(lines[0].0)),
        ("CPA", V::T("_X")),
        ("ENDUSE", V::T("INT")),
    ]);
    write_rows(path, &[(KEY_SHEET, rows)])
}

fn names(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect()
}

#[test]
fn read_hs_cpa_keeps_hs_1988_92_lines() {
    let dir = tempfile::tempdir().unwrap();
    let lines = [("010111", "A01"), ("270900", "B06")];
    assert_eq!(
        read_hs_cpa(&key(&dir.path().join("key.xlsx"), &lines)).unwrap(),
        names(&lines)
    );
}

#[test]
fn read_hs_cpa_moves_gold_to_basic_metals_and_books_to_printing() {
    let dir = tempfile::tempdir().unwrap();
    let lines = [
        ("710812", "B072"),
        ("710813", "C24B"),
        ("490199", "J58"),
        ("852410", "J59_60"),
        ("271600", "D35"),
        ("970110", "R90"),
    ];
    let got = read_hs_cpa(&key(&dir.path().join("key.xlsx"), &lines)).unwrap();
    let want = [
        ("710812", "C24B"),
        ("710813", "C24B"),
        ("490199", "C18"),
        ("852410", "C18"),
        ("271600", "D35"),
        ("970110", "R90"),
    ];
    assert_eq!(got, names(&want));
}

/// BACI 2000 rows (t,i,j,k,v,q) and a 2001 member the readers must ignore.
fn baci(dir: &Path, rows: &[&str], codes: &str) -> PathBuf {
    let year = format!("t,i,j,k,v,q\n{}\n", rows.join("\n"));
    write_zip(
        &dir.join("baci.zip"),
        &[
            ("BACI_HS92_Y2000_V202601.csv", &year),
            (
                "BACI_HS92_Y2001_V202601.csv",
                "t,i,j,k,v,q\n2001,1,2,10121,1,999\n",
            ),
            ("country_codes_V202601.csv", codes),
        ],
    )
}

#[test]
fn baci_by_product_sums_flows_with_a_quantity_by_padded_line() {
    let dir = tempfile::tempdir().unwrap();
    // BACI v is thousands of USD, q is tonnes; k carries no leading zero.
    let rows = [
        "2000,1,2,10121,2,3",
        "2000,1,3,10121,3,1",
        "2000,2,3,10121,100,", // no q: out of both sums
        "2000,1,2,10121,0.5,NA",
        "2000,1,2,270900,8,2",
    ];
    let (tonnes, value) = baci_by_product(&baci(dir.path(), &rows, "")).unwrap();
    assert_eq!(
        tonnes,
        [("010121".to_owned(), 4.0), ("270900".to_owned(), 2.0)].into()
    );
    assert_eq!(
        value,
        [("010121".to_owned(), 5000.0), ("270900".to_owned(), 8000.0)].into()
    );
}

#[test]
fn baci_values_weigh_every_flow() {
    let dir = tempfile::tempdir().unwrap();
    let rows = [
        "2000,1,2,520100,1,",
        "2000,1,3,520100,2,5",
        "2000,1,2,20110,4,1",
    ];
    let got = baci_values(&baci(dir.path(), &rows, "")).unwrap();
    assert_eq!(
        got,
        [("020110".to_owned(), 4000.0), ("520100".to_owned(), 3000.0)].into()
    );
}

#[test]
fn trade_by_country_sums_exports_and_imports_of_the_economies_a_country_combines() {
    let dir = tempfile::tempdir().unwrap();
    let codes = "country_code,country_name,country_iso2,country_iso3\n\
                 4,Afghanistan,AF,AFG\n490,\"Other Asia, nes\",,S19\n688,Serbia,RS,SRB\n891,Serbia and Montenegro,CS,SCG\n499,Montenegro,ME,MNE\n";
    let rows = [
        "2000,4,490,10121,2,",
        "2000,891,4,10121,3,",
        "2000,499,688,10121,1,",
    ];
    let iso = names(&[("TWN", "TWN"), ("AFG", "AFG")]);
    let pack: Vec<String> = ["AFG", "TWN", "YUG", "FRA"].map(String::from).into();
    let got = trade_by_country(&baci(dir.path(), &rows, codes), &iso, &pack).unwrap();
    // Taiwan is BACI's "Other Asia, nes"; YUG combines Serbia (with Serbia and Montenegro) and Montenegro.
    assert_eq!(
        got,
        [
            ("AFG".to_owned(), 5000.0),
            ("TWN".to_owned(), 2000.0),
            ("YUG".to_owned(), 5000.0)
        ]
        .into()
    );
}

#[test]
fn tariff_rates_skip_implausible_years() {
    let series: BTreeMap<String, BTreeMap<i64, f64>> = [
        (
            "DJI".to_owned(),
            [(1998, 100.13), (1999, 534.76), (2002, 20.97)].into(),
        ),
        ("EGY".to_owned(), [(1999, 12.53), (2000, 12.88)].into()),
        ("XXX".to_owned(), [(1990, 5.0)].into()),
    ]
    .into();
    assert_eq!(
        plausible_rates(&series),
        [("DJI".to_owned(), 0.2097), ("EGY".to_owned(), 0.1288)].into()
    );
}

#[test]
fn read_dta_keeps_goods_fta_and_customs_unions_in_force_on_new_year() {
    let agreements = vec![
        vec![
            ("WB ID", V::N(1.0)),
            ("Date of Entry into Force (G)", V::N(30000.0)),
        ], // 1982
        vec![
            ("WB ID", V::N(2.0)),
            ("Date of Entry into Force (G)", V::N(36526.0)),
        ], // 1 Jan 2000
        vec![
            ("WB ID", V::N(3.0)),
            ("Date of Entry into Force (G)", V::N(36708.0)),
        ], // 1 Jul 2000
    ];
    let row = |other: &'static str, wb: f64, year: f64, kind: &'static str| {
        vec![
            ("iso1", V::T("AAA")),
            ("iso2", V::T(other)),
            ("WBID", V::N(wb)),
            ("year", V::N(year)),
            ("type", V::T(kind)),
            ("entry", V::N(year)),
        ]
    };
    let pairs = vec![
        row("BBB", 1.0, 1999.0, "CU"),
        row("BBB", 1.0, 2000.0, "CU"),
        row("CCC", 1.0, 2000.0, "CU"), // joined the 1982 agreement during 2000
        row("DDD", 2.0, 2000.0, "FTA & EIA"),
        row("EEE", 3.0, 2000.0, "FTA"),
        row("FFF", 1.0, 1999.0, "PSA"),
        row("FFF", 1.0, 2000.0, "PSA"),
        row("GGG", 1.0, 1999.0, "FTA"), // left before 2000; a 2000 row without a year is an agreement not in force
        vec![
            ("iso1", V::T("AAA")),
            ("iso2", V::T("HHH")),
            ("WBID", V::N(1.0)),
            ("type", V::T("CU")),
        ],
    ];
    let dir = tempfile::tempdir().unwrap();
    let path = write_rows(
        &dir.path().join("dta.xlsx"),
        &[("Agreements", agreements), ("Bilateral Information", pairs)],
    );
    let want = [
        ("AAA".to_owned(), "BBB".to_owned()),
        ("AAA".to_owned(), "DDD".to_owned()),
    ]
    .into();
    assert_eq!(read_dta(&path).unwrap(), want);
}

#[test]
fn read_cepii_needs_a_goods_agreement_in_1999_and_2000() {
    let mut rows = vec!["year,iso3_o,iso3_d,fta_wto,rta_type,dist".to_owned()];
    for y in [1999, 2000] {
        rows.extend([
            format!("{y},AAA,BBB,1,1,5"),
            format!("{y},AAA,CCC,1,7,5"),
            format!("{y},AAA,DDD,0,,5"),
        ]);
    }
    rows.extend([
        "2000,AAA,EEE,1,4,5".to_owned(),
        "2001,AAA,FFF,1,4,5".to_owned(),
    ]);
    let dir = tempfile::tempdir().unwrap();
    let path = write_zip(
        &dir.path().join("g.zip"),
        &[("Gravity_V202211.csv", &(rows.join("\n") + "\n"))],
    );
    assert_eq!(
        read_cepii(&path).unwrap(),
        [("AAA".to_owned(), "BBB".to_owned())].into()
    );
}

#[test]
fn real_key_and_baci_cover_the_traded_lines() {
    let (Some(hs_key), Some(baci)) = (
        cached("OECD-Bilateral-Trade-in-Goods-End-use-Conversion-Key.xlsx"),
        cached("BACI_HS92_V202601.zip"),
    ) else {
        return;
    };
    let key = read_hs_cpa(&hs_key).unwrap();
    assert_eq!(
        (key["270112"].as_str(), key["270900"].as_str()),
        ("B05", "B06")
    );
    assert!(key.len() > 5000);
    let (tonnes, value) = baci_by_product(&baci).unwrap();
    let all = baci_values(&baci).unwrap();
    assert!(value.keys().all(|k| k.len() == 6 && all.contains_key(k)));
    // Flows without a quantity are only in the full values.
    let (with_q, every): (f64, f64) = (value.values().sum(), all.values().sum());
    assert!(
        with_q < every && approx(with_q, every, 0.2),
        "{with_q} {every}"
    );
    assert!(tonnes.values().all(|t| *t >= 0.0));
}

#[test]
fn baci_sums_each_1_mib_block_then_across_blocks() {
    // Line 010121 has 1 kUSD in the first block and two flows of 1e-16 kUSD past the first MiB:
    // summed per block they survive, summed over the whole file they round away.
    let filler = "2000,1,2,999999,1,1";
    let mut rows = vec!["2000,1,2,10121,1,1"];
    rows.extend(std::iter::repeat_n(filler, (1 << 20) / filler.len() + 100));
    rows.extend(["2000,1,2,10121,1e-16,1", "2000,1,2,10121,1e-16,1"]);
    let dir = tempfile::tempdir().unwrap();
    let path = baci(dir.path(), &rows, "");
    let per_block = 1.0 * 1000.0 + (0.0 + 1e-16 + 1e-16) * 1000.0;
    let whole_file = (1.0 + 1e-16 + 1e-16) * 1000.0;
    assert_ne!(per_block, whole_file);
    assert_eq!(baci_values(&path).unwrap()["010121"], per_block);
    assert_eq!(baci_by_product(&path).unwrap().1["010121"], per_block);
}
