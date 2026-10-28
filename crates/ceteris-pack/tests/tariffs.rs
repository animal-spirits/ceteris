//! Tariff tables on hand-made rates and, when the pack is present, the built 2000 table.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;

use arrow_array::cast::AsArray;
use arrow_array::types::Float64Type;
use ceteris_pack::econ::tariffs::{GROUPS, IndexPairs, columns, pack_pairs, table};
use ceteris_pack::read::people::Observed;
use ceteris_pack::read::tariffs::Pairs;
use ceteris_pack::read::wdi::Names;
use common::{pack_table, strings};

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| (*x).to_owned()).collect()
}

fn rates(v: &[&[(&str, f64)]]) -> Vec<BTreeMap<String, f64>> {
    v.iter()
        .map(|g| g.iter().map(|(k, x)| ((*k).to_owned(), *x)).collect())
        .collect()
}

/// Economies in one region, GDP per person rising in the order given.
fn observed(income: &[(&str, f64)]) -> Observed {
    Observed {
        region: income
            .iter()
            .map(|(k, _)| ((*k).to_owned(), "r".to_owned()))
            .collect(),
        gdp_per_capita: income.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect(),
        ..Observed::default()
    }
}

fn ranked(codes: &[&str]) -> Observed {
    let income: Vec<(&str, f64)> = codes
        .iter()
        .enumerate()
        .map(|(i, c)| (*c, 1000.0 * (i as f64 + 1.0)))
        .collect();
    observed(&income)
}

fn pairs(v: &[(usize, usize)]) -> IndexPairs {
    v.iter().copied().collect()
}

#[test]
fn rates_are_zero_inside_agreements_and_on_the_diagonal() {
    let codes = s(&["AAA", "BBB", "CCC"]);
    let r = rates(&[
        &[("AAA", 0.10), ("BBB", 0.05), ("CCC", 0.20)],
        &[("AAA", 0.08), ("BBB", 0.04), ("CCC", 0.12)],
    ]);
    let t = table(
        &r,
        &pairs(&[(0, 1), (1, 0)]),
        &codes,
        &Names::new(),
        &ranked(&["AAA", "BBB", "CCC"]),
        &BTreeMap::new(),
    )
    .unwrap();
    for i in 0..3 {
        assert_eq!(t.rate[i][i], [0.0; GROUPS]);
    }
    assert_eq!(t.rate[0][1], [0.0; GROUPS]);
    assert_eq!(t.rate[1][0], [0.0; GROUPS]);
    assert_eq!(t.rate[0][2], [0.10, 0.08]);
    assert_eq!(t.rate[2][0], [0.20, 0.12]);
    assert!(t.imputed.iter().all(|x| !x));
    assert!(t.agreement[0][1] && !t.agreement[0][2]);
}

#[test]
fn an_importer_without_a_rate_takes_the_nearest_donor_flagged_imputed() {
    let codes = s(&["AAA", "BBB", "CCC"]);
    let r = rates(&[
        &[("AAA", 0.10), ("BBB", 0.30)],
        &[("AAA", 0.08), ("BBB", 0.25)],
    ]);
    let obs = observed(&[("AAA", 1000.0), ("BBB", 9000.0), ("CCC", 8000.0)]);
    let t = table(
        &r,
        &IndexPairs::new(),
        &codes,
        &Names::new(),
        &obs,
        &BTreeMap::new(),
    )
    .unwrap();
    assert_eq!(t.imputed, [false, false, true]);
    assert_eq!(t.donor[2], "BBB");
    assert_eq!(t.rate[2][0], [0.30, 0.25]);
}

#[test]
fn a_dependency_trades_free_with_its_sovereign_and_takes_its_rate() {
    let codes = s(&["AAA", "BBB", "TER"]);
    let r = rates(&[
        &[("AAA", 0.10), ("BBB", 0.30)],
        &[("AAA", 0.08), ("BBB", 0.25)],
    ]);
    let sovereign: BTreeMap<String, String> = [("TER", "AAA"), ("AAA", "AAA")]
        .iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect();
    let t = table(
        &r,
        &IndexPairs::new(),
        &codes,
        &Names::new(),
        &ranked(&["AAA", "BBB", "TER"]),
        &sovereign,
    )
    .unwrap();
    assert_eq!(t.rate[2][0], [0.0; GROUPS]);
    assert_eq!(t.rate[0][2], [0.0; GROUPS]);
    assert_eq!(t.rate[2][1], [0.10, 0.08]);
    assert_eq!(t.imputed, [false, false, true]);
    assert_eq!(t.donor[2], "AAA");
}

#[test]
fn a_split_economy_takes_the_mean_of_its_parts() {
    let codes = s(&["YUG", "BBB"]);
    let r = rates(&[
        &[("SRB", 0.10), ("MNE", 0.20), ("BBB", 0.5)],
        &[("SRB", 0.30), ("BBB", 0.5)],
    ]);
    let t = table(
        &r,
        &IndexPairs::new(),
        &codes,
        &Names::new(),
        &ranked(&["YUG", "BBB"]),
        &BTreeMap::new(),
    )
    .unwrap();
    assert!(!t.imputed[0]);
    assert_eq!(t.own[0], [(0.10 + 0.20) / 2.0, 0.30]);
}

#[test]
fn source_codes_map_to_pack_countries_both_ways() {
    let codes = s(&["DEU", "FRA", "YUG"]);
    let source: Pairs = [
        ("DEU", "FRA"),
        ("SCG", "FRA"),
        ("SRB", "MNE"),
        ("XXX", "DEU"),
    ]
    .iter()
    .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
    .collect();
    assert_eq!(
        pack_pairs(&source, &codes, &Names::new()),
        pairs(&[(0, 1), (1, 0), (2, 1), (1, 2)])
    );
}

#[test]
fn columns_are_importer_major_with_groups_innermost() {
    let r = rates(&[&[("A", 0.1), ("B", 0.2)], &[("A", 0.3), ("B", 0.4)]]);
    let t = table(
        &r,
        &IndexPairs::new(),
        &s(&["A", "B"]),
        &Names::new(),
        &ranked(&["A", "B"]),
        &BTreeMap::new(),
    )
    .unwrap();
    let cols: BTreeMap<String, _> = columns(&t).unwrap().into_iter().collect();
    let rate = cols["rate"].as_primitive::<Float64Type>();
    assert_eq!(
        rate.values().to_vec(),
        [0.0, 0.0, 0.1, 0.3, 0.2, 0.4, 0.0, 0.0]
    );
    let u32s = |name: &str| {
        cols[name]
            .as_primitive::<arrow_array::types::UInt32Type>()
            .values()
            .to_vec()
    };
    assert_eq!(u32s("importer"), [0, 0, 0, 0, 1, 1, 1, 1]);
    assert_eq!(u32s("exporter"), [0, 0, 1, 1, 0, 0, 1, 1]);
    assert_eq!(
        cols["group"]
            .as_primitive::<arrow_array::types::UInt8Type>()
            .values()
            .to_vec(),
        [0, 1, 0, 1, 0, 1, 0, 1]
    );
    assert_eq!(cols["agreement"].as_boolean().true_count(), 0);
}

#[test]
fn the_built_table_covers_every_pair_with_fractional_rates() {
    let (Some(table_path), Some(countries)) = (
        pack_table("econ_tariffs.arrow"),
        pack_table("countries.arrow"),
    ) else {
        return;
    };
    let n = strings(&countries, "code").len();
    let reader =
        arrow_ipc::reader::FileReader::try_new(File::open(table_path).unwrap(), None).unwrap();
    let mut rows = 0;
    let mut importers = BTreeSet::new();
    for batch in reader {
        let batch = batch.unwrap();
        rows += batch.num_rows();
        let rate = batch.column_by_name("rate").unwrap();
        assert!(
            rate.as_primitive::<Float64Type>()
                .values()
                .iter()
                .all(|r| (0.0..1.0).contains(r))
        );
        importers.extend(
            batch
                .column_by_name("importer")
                .unwrap()
                .as_primitive::<arrow_array::types::UInt32Type>()
                .values()
                .iter()
                .copied(),
        );
    }
    assert_eq!(rows, GROUPS * n * n);
    assert_eq!(importers.len(), n);
}
