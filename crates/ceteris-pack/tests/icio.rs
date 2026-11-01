//! Imputation of the countries outside ICIO, on hand-made donors and, when the cache and the pack
//! are present, on the 2000 sources.

mod common;

use std::collections::BTreeMap;
use std::sync::OnceLock;

use ceteris_pack::econ::icio::{
    Basis, Economy, ICIO, INSIDE, IncomeSource, IoBasis, REGIONS, Rules, home_share_columns,
    impute, impute_pack, median_va_over_gdp, oil_gas_most_of_industry, oil_gas_va_over_rents,
    sector_shares, world_oil_gas_column,
};
use ceteris_pack::raster::geotiff;
use ceteris_pack::raster::population::{BLOCK_ROWS, CellOwner, province_population};
use ceteris_pack::read::baci::Trade;
use ceteris_pack::read::icio::{CountryIo, HomeUse, N_GOODS, Wdi, aggregate, load_wdi};
use ceteris_pack::read::natural_earth::read_codes;
use ceteris_pack::read::people::{Observed, load_observed};
use ceteris_pack::read::wdi::Names;
use common::{approx, cached, pack_table, strings};

const DONOR_VA: [f64; N_GOODS] = [10.0, 4.0, 1.0, 6.0, 2.0, 1.0, 1.0, 3.0, 2.0, 70.0];

fn map<T: Clone>(v: &[(&str, T)]) -> BTreeMap<String, T> {
    v.iter()
        .map(|(k, x)| ((*k).to_owned(), x.clone()))
        .collect()
}

fn with<T: Clone>(base: &[(&str, T)], more: &[(&str, T)]) -> BTreeMap<String, T> {
    let mut out = map(base);
    out.extend(map(more));
    out
}

/// A donor whose VA is `DONOR_VA` and whose inputs take `colsum` of every column's output.
fn country(colsum: f64, fd: [f64; N_GOODS]) -> CountryIo {
    let mut c = CountryIo {
        output: DONOR_VA.map(|v| v / (1.0 - colsum)),
        va: DONOR_VA,
        io: [[colsum / 10.0; N_GOODS]; N_GOODS],
        final_demand_shares: fd,
        coal_share: 0.0,
        extraction: [0.0; N_GOODS],
        extraction_output: 0.0,
    };
    extraction_is_good_one(&mut c);
    c
}

/// All of good 1 is extraction (no refining in it).
fn extraction_is_good_one(c: &mut CountryIo) {
    c.extraction_output = c.output[1];
    c.extraction = std::array::from_fn(|k| c.io[k][1] * c.output[1]);
}

fn donors() -> BTreeMap<String, CountryIo> {
    let even = [0.1; N_GOODS];
    let mut half = [0.0; N_GOODS];
    (half[0], half[3]) = (0.5, 0.5);
    map(&[
        ("AAA", country(0.1, even)),
        ("BBB", country(0.2, half)),
        ("CCC", country(0.3, even)),
    ])
}

// USD; donor VA is 100 million USD, so VA over GDP is 1, 0.5, 0.8.
const DONOR_GDP: [(&str, f64); 3] = [("AAA", 100e6), ("BBB", 200e6), ("CCC", 125e6)];
const DONOR_POP: [(&str, f64); 3] = [("AAA", 1e6), ("BBB", 5e6), ("CCC", 1e6)];
const PWT: [(&str, f64); 3] = [("AAA", 1000.0), ("BBB", 20000.0), ("CCC", 10000.0)];
const WDI_INCOME: [(&str, f64); 3] = [("AAA", 10000.0), ("BBB", 50000.0), ("CCC", 3000.0)];

fn obs_with(
    pwt: Option<BTreeMap<String, f64>>,
    region: &[(&str, &str)],
    population: &[(&str, f64)],
) -> Observed {
    let regions = [
        ("AAA", "R1"),
        ("BBB", "R1"),
        ("CCC", "R2"),
        ("XXX", "R1"),
        ("YYY", "R2"),
    ];
    let pop = [
        DONOR_POP.as_slice(),
        &[("XXX", 1e6), ("YYY", 2e6), ("ZZZ", 3e6)],
    ]
    .concat();
    Observed {
        region: with(&regions, region)
            .into_iter()
            .map(|(k, v)| (k, v.to_owned()))
            .collect(),
        gdp_per_capita: pwt.unwrap_or_else(|| with(&PWT, &[("XXX", 11000.0), ("YYY", 11000.0)])),
        population: with(&pop, population),
        ..Observed::default()
    }
}

fn obs() -> Observed {
    obs_with(None, &[], &[])
}

fn no_pwt_for_xxx() -> Observed {
    obs_with(Some(map(&PWT)), &[], &[])
}

fn wdi() -> Wdi {
    Wdi {
        gdp: with(&DONOR_GDP, &[("XXX", 1e9)]),
        income: map(&WDI_INCOME),
        income_source: WDI_INCOME
            .iter()
            .map(|(k, _)| ((*k).to_owned(), "wdi".to_owned()))
            .collect(),
        agriculture: map(&[("XXX", 0.1)]),
        industry: map(&[("XXX", 0.35)]),
        manufacturing: map(&[("XXX", 0.2)]),
        services: map(&[("XXX", 0.45)]),
        ..Wdi::default()
    }
}

/// `impute` with the given rule tables, no ISO renames and parts from `split` (else the code).
struct Case<'a> {
    worldpop: &'a [(&'a str, f64)],
    inside: &'a [(&'a str, &'a str)],
    regions: &'a [(&'a str, &'a str)],
    split: &'a [(&'a str, &'a [&'a str])],
    /// BACI trade by good, USD.
    trade: &'a [(&'a str, Trade)],
}

const PLAIN: Case = Case {
    worldpop: &[],
    inside: &[],
    regions: &[],
    split: &[],
    trade: &[],
};

fn run(
    countries: &[(&str, &str)],
    icio: &BTreeMap<String, CountryIo>,
    wdi: &Wdi,
    obs: &Observed,
    case: &Case,
) -> BTreeMap<String, Economy> {
    let parts_of = |c: &str| -> Vec<String> {
        match case.split.iter().find(|(k, _)| *k == c) {
            Some((_, ps)) => ps.iter().map(|p| (*p).to_owned()).collect(),
            None => vec![c.to_owned()],
        }
    };
    let rules = Rules {
        parts: &parts_of,
        inside: case.inside,
        regions: case.regions,
    };
    let countries: Vec<(String, String)> = countries
        .iter()
        .map(|(c, s)| ((*c).to_owned(), (*s).to_owned()))
        .collect();
    impute(
        &countries,
        icio,
        wdi,
        obs,
        &Names::new(),
        &map(case.worldpop),
        &map(case.trade),
        &rules,
    )
    .unwrap()
}

fn one(code: &str, wdi: &Wdi, obs: &Observed) -> Economy {
    run(&[(code, code)], &donors(), wdi, obs, &PLAIN)[code].clone()
}

fn col_sum(io: &[[f64; N_GOODS]; N_GOODS], g: usize) -> f64 {
    io.iter().map(|row| row[g]).sum()
}

fn total(x: &[f64; N_GOODS]) -> f64 {
    x.iter().sum()
}

/// VA + inputs = output, good by good.
fn closes(c: &CountryIo, rtol: f64, atol: f64) -> bool {
    (0..N_GOODS).all(|g| {
        let lhs = c.va[g] + col_sum(&c.io, g) * c.output[g];
        (lhs - c.output[g]).abs() <= atol + rtol * c.output[g].abs()
    })
}

fn all_close(a: &[f64], b: &[f64], rel: f64) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| approx(*x, *y, rel))
}

fn proportions(va: &[f64; N_GOODS]) -> Vec<f64> {
    let t = total(va);
    va.iter().map(|v| v / t).collect()
}

fn donor_proportions() -> Vec<f64> {
    DONOR_VA.iter().map(|v| v / 100.0).collect()
}

#[test]
fn imputed_country_uses_nearest_donor() {
    let donors = donors();
    let wdi = Wdi {
        gdp: with(&DONOR_GDP, &[("XXX", 1e9), ("YYY", 1e9)]),
        ..wdi()
    };
    let got = run(
        &[("XXX", "XXX"), ("YYY", "YYY")],
        &donors,
        &wdi,
        &obs(),
        &PLAIN,
    );
    // XXX: same region as AAA and BBB, closest log income among them is BBB, although CCC (other
    // region) is closer overall.
    let (x, y) = (&got["XXX"], &got["YYY"]);
    assert_eq!(x.donor.as_deref(), Some("BBB"));
    assert_eq!(y.donor.as_deref(), Some("CCC"));
    assert_eq!(x.region.as_deref(), Some("R1"));
    assert_eq!(x.income_source, IncomeSource::Pwt);
    assert_eq!(x.table.io, donors["BBB"].io);
    assert_eq!(
        x.table.final_demand_shares,
        donors["BBB"].final_demand_shares
    );
    assert_eq!(x.io_basis, IoBasis::Donor);
}

#[test]
fn icio_economies_keep_their_own_tables() {
    let donors = donors();
    let got = run(
        &[("AAA", "AAA"), ("XXX", "XXX")],
        &donors,
        &wdi(),
        &obs(),
        &PLAIN,
    );
    let a = &got["AAA"];
    assert_eq!(a.donor, None);
    assert_eq!(a.va_basis, Basis::Icio);
    assert_eq!(a.table, donors["AAA"]);
    assert_eq!(a.io_basis, IoBasis::Icio);
    assert_eq!(got["XXX"].donor.as_deref(), Some("BBB"));
}

#[test]
fn imputed_va_follows_wdi_shares_split_by_donor_proportions() {
    let got = one("XXX", &wdi(), &obs());
    // GDP 1000 million USD; agriculture 100, services 450, manufacturing 200 over goods 3 to 6 as
    // the donor (6, 2, 1, 1), industry other than manufacturing 150 over goods 1, 2, 7, 8 as the
    // donor (4, 1, 3, 2).
    let expected = [
        100.0, 60.0, 15.0, 120.0, 40.0, 20.0, 20.0, 45.0, 30.0, 450.0,
    ];
    assert!(all_close(&got.table.va, &expected, 1e-12));
    // Output keeps the donor's output per unit of VA of each good (BBB's inputs take 0.2 of output).
    let want: Vec<f64> = got.table.va.iter().map(|v| v / 0.8).collect();
    assert!(all_close(&got.table.output, &want, 1e-12));
    assert_eq!(got.va_basis, Basis::Wdi);
    assert!(closes(&got.table, 1e-9, 1e-12));
}

#[test]
fn missing_sector_share_leaves_the_rest_of_the_icio_median_va_over_gdp_to_its_goods() {
    let w = Wdi {
        services: BTreeMap::new(),
        ..wdi()
    };
    let got = one("XXX", &w, &obs());
    // Agriculture 100, industry other than manufacturing 150, manufacturing 200 as with all shares;
    // services take the ICIO median VA over GDP (0.8) less the 0.45 there are: 350 million USD.
    let expected = [
        100.0, 60.0, 15.0, 120.0, 40.0, 20.0, 20.0, 45.0, 30.0, 350.0,
    ];
    assert_eq!(got.va_basis, Basis::WdiGdp);
    assert_eq!(got.donor.as_deref(), Some("BBB"));
    assert!(all_close(&got.table.va, &expected, 1e-12));
    assert!(closes(&got.table, 1e-9, 1e-12));
}

#[test]
fn agriculture_and_manufacturing_alone_leave_the_rest_to_every_other_good() {
    // Kuwait in 2000: no industry or services share.
    let w = Wdi {
        industry: BTreeMap::new(),
        services: BTreeMap::new(),
        ..wdi()
    };
    let got = one("XXX", &w, &obs());
    // 800 - 100 - 200 = 500 over goods 1, 2, 7, 8, 9 as the donor (4, 1, 3, 2, 70) / 80.
    let rest = [1, 2, 7, 8, 9].map(|g| 500.0 * DONOR_VA[g] / 80.0);
    assert!(approx(total(&got.table.va), 800.0, 1e-12));
    assert!(approx(got.table.va[0], 100.0, 1e-12));
    assert!(approx(got.table.va[3..7].iter().sum(), 200.0, 1e-12));
    for (g, want) in [1, 2, 7, 8, 9].into_iter().zip(rest) {
        assert!(approx(got.table.va[g], want, 1e-12), "{g}");
    }
}

#[test]
fn known_shares_above_the_median_leave_nothing_to_the_rest() {
    let w = Wdi {
        agriculture: map(&[("XXX", 0.5)]),
        industry: map(&[("XXX", 0.4)]),
        services: BTreeMap::new(),
        ..wdi()
    };
    let got = one("XXX", &w, &obs());
    assert!(approx(total(&got.table.va), 900.0, 1e-12));
    assert_eq!(got.table.va[9], 0.0);
}

/// `wdi()` with oil rents for donor AAA (2 percent of GDP against VA 4 of 100: VA over rents 2)
/// and the given oil and gas rents for XXX.
fn with_rents(oil: f64, gas: f64) -> Wdi {
    Wdi {
        oil_rents: map(&[("AAA", 0.02), ("XXX", oil)]),
        gas_rents: map(&[("XXX", gas)]),
        ..wdi()
    }
}

/// `donors()` with donor CCC (not XXX's) at crude oil and gas 40 of its industry VA of 49, total
/// VA still 100: the largest share of oil and gas in industry is 40 / 49.
fn oil_donors() -> BTreeMap<String, CountryIo> {
    let mut icio = donors();
    let c = icio.get_mut("CCC").unwrap();
    c.va = [10.0, 40.0, 1.0, 2.0, 1.0, 1.0, 1.0, 2.0, 1.0, 41.0];
    c.output = c.va.map(|v| v / 0.7);
    extraction_is_good_one(c);
    icio
}

fn oil(wdi: &Wdi) -> Economy {
    run(&[("XXX", "XXX")], &oil_donors(), wdi, &obs(), &PLAIN)["XXX"].clone()
}

#[test]
fn oil_gas_most_of_industry_is_the_largest_icio_share() {
    assert!(approx(
        oil_gas_most_of_industry(&oil_donors()),
        40.0 / 49.0,
        1e-12
    ));
}

#[test]
fn oil_gas_va_over_rents_sums_the_icio_economies_with_a_rent() {
    assert!(approx(
        oil_gas_va_over_rents(&donors(), &with_rents(0.0, 0.0)),
        2.0,
        1e-12
    ));
}

/// Gives donor `code` refining in good 1: its crude oil and gas extraction (B06, B09) has output
/// `output` and inputs summing to `inputs`, the rest of good 1 being refining (C19).
fn refines(icio: &mut BTreeMap<String, CountryIo>, code: &str, output: f64, inputs: f64) {
    let c = icio.get_mut(code).unwrap();
    c.extraction_output = output;
    c.extraction = [0.0; N_GOODS];
    (c.extraction[1], c.extraction[3]) = (0.5 * inputs, 0.5 * inputs);
}

/// The rents measure extraction, so the factor is extraction VA (B06, B09 output less their
/// inputs) over rents, not good 1's VA with refining in it (ADR 0281): AAA's good-1 VA is 4, of
/// which extraction 3 - 0.5 = 2.5, against rents of 2.
#[test]
fn oil_gas_va_over_rents_takes_extraction_value_added() {
    let mut icio = donors();
    refines(&mut icio, "AAA", 3.0, 0.5);
    assert!(approx(
        oil_gas_va_over_rents(&icio, &with_rents(0.0, 0.0)),
        1.25,
        1e-12
    ));
}

/// The cap is the largest extraction VA over the VA of goods 1 to 8 (refining stays in
/// industry): CCC's good 1 holds 40 of its industry's 49, extraction 30 of it.
#[test]
fn oil_gas_most_of_industry_takes_extraction_value_added() {
    let mut icio = oil_donors();
    refines(&mut icio, "CCC", 36.0, 6.0);
    assert!(approx(oil_gas_most_of_industry(&icio), 30.0 / 49.0, 1e-12));
}

#[test]
fn oil_and_gas_rents_set_crude_oil_and_gas_out_of_its_group() {
    let got = oil(&with_rents(0.05, 0.01));
    // 0.06 of GDP times 2: 120 of the 150 of industry other than manufacturing; the other 30 over
    // goods 2, 7, 8 as the donor (1, 3, 2).
    assert!(approx(got.table.va[1], 120.0, 1e-12));
    for (g, want) in [(2, 5.0), (7, 15.0), (8, 10.0)] {
        assert!(approx(got.table.va[g], want, 1e-12), "{g}");
    }
    assert!(approx(total(&got.table.va), 1000.0 * 0.9, 1e-12));
    assert_eq!(got.va_basis, Basis::Wdi);
    assert!(closes(&got.table, 1e-9, 1e-12));
}

#[test]
fn oil_and_gas_above_their_group_take_from_manufacturing() {
    // 0.2 of GDP exceeds the 0.15 of industry other than manufacturing, so all industry (0.35)
    // is one group: oil and gas 200, the other 150 over goods 2 to 8 as the donor (1, 6, 2, 1, 1,
    // 3, 2) / 16.
    let got = oil(&with_rents(0.1, 0.0));
    assert!(approx(got.table.va[1], 200.0, 1e-12));
    let want: Vec<f64> = DONOR_VA[2..9].iter().map(|v| 150.0 * v / 16.0).collect();
    assert!(all_close(&got.table.va[2..9], &want, 1e-12));
    assert!(approx(total(&got.table.va), 900.0, 1e-12));
}

#[test]
fn oil_and_gas_take_at_most_the_largest_icio_share_of_industry() {
    // Turkmenistan in 2000: rents above all of industry. Oil and gas 350 * 40 / 49; the rest
    // over goods 2 to 8 as the donor, so electricity and construction keep a producer.
    let got = oil(&with_rents(0.2, 0.0));
    let rest = 350.0 * 9.0 / 49.0;
    assert!(approx(got.table.va[1], 350.0 * 40.0 / 49.0, 1e-12));
    let want: Vec<f64> = DONOR_VA[2..9].iter().map(|v| rest * v / 16.0).collect();
    assert!(all_close(&got.table.va[2..9], &want, 1e-12));
}

#[test]
fn oil_and_gas_with_no_sector_share_come_out_of_the_median_level() {
    let zero = map(&[("XXX", 0.0)]);
    let w = Wdi {
        agriculture: zero.clone(),
        industry: zero.clone(),
        manufacturing: zero.clone(),
        services: zero,
        ..with_rents(0.1, 0.0)
    };
    let got = oil(&w);
    // Venezuela in 2000: 200 of the 800, the other 600 over the other goods as the donor.
    assert!(approx(total(&got.table.va), 800.0, 1e-12));
    assert!(approx(got.table.va[1], 200.0, 1e-12));
    assert!(approx(got.table.va[9], 600.0 * 70.0 / 96.0, 1e-12));
}

#[test]
fn a_donor_without_oil_and_gas_still_gives_oil_and_gas_output() {
    let mut icio = oil_donors();
    let b = icio.get_mut("BBB").unwrap();
    (b.va[1], b.output[1]) = (0.0, 0.0);
    for row in &mut b.io {
        row[1] = 0.0;
    }
    extraction_is_good_one(b);
    let got = run(
        &[("XXX", "XXX")],
        &icio,
        &with_rents(0.05, 0.01),
        &obs(),
        &PLAIN,
    )["XXX"]
        .clone();
    // The donor has no oil and gas column, so the world's (AAA, CCC) gives the inputs.
    let world = world_oil_gas_column(&icio);
    assert!(approx(got.table.va[1], 120.0, 1e-12));
    assert!(approx(
        world[0],
        (0.01 * 4.0 / 0.9 + 0.03 * 40.0 / 0.7) / (4.0 / 0.9 + 40.0 / 0.7),
        1e-12
    ));
    assert!(approx(
        got.table.output[1],
        120.0 / (1.0 - 10.0 * world[0]),
        1e-12
    ));
    assert!(closes(&got.table, 1e-9, 1e-12));
}

#[test]
fn world_oil_gas_column_weights_inputs_by_oil_and_gas_output() {
    // Every input takes 0.01, 0.02, 0.03 of output in AAA, BBB, CCC, whose oil and gas outputs
    // are 4 / 0.9, 4 / 0.8, 40 / 0.7.
    let out = [4.0 / 0.9, 4.0 / 0.8, 40.0 / 0.7];
    let want = (0.01 * out[0] + 0.02 * out[1] + 0.03 * out[2]) / (out[0] + out[1] + out[2]);
    assert!(
        world_oil_gas_column(&oil_donors())
            .iter()
            .all(|&x| approx(x, want, 1e-12))
    );
}

#[test]
fn world_oil_gas_column_leaves_out_refining() {
    // Refining in good 1 (ADR 0092) changes its column but not the extraction one (ADR 0270).
    let mut icio = oil_donors();
    let want = world_oil_gas_column(&icio);
    for c in icio.values_mut() {
        c.output[1] *= 3.0;
        for row in &mut c.io {
            row[1] = 0.06;
        }
    }
    assert_eq!(world_oil_gas_column(&icio), want);
}

#[test]
fn oil_and_gas_from_rents_produce_with_the_world_column() {
    let got = oil(&with_rents(0.05, 0.01));
    let world = world_oil_gas_column(&oil_donors());
    let donor = &oil_donors()["BBB"];
    for ((row, w), d) in got.table.io.iter().zip(world).zip(&donor.io) {
        assert_eq!((row[1], row[2]), (w, d[2]));
    }
    assert!(approx(
        got.table.output[1],
        120.0 / (1.0 - world.iter().sum::<f64>()),
        1e-12
    ));
    assert!(closes(&got.table, 1e-9, 1e-12));
}

#[test]
fn without_rents_oil_and_gas_keep_the_donor_column() {
    let got = one("XXX", &wdi(), &obs());
    assert_eq!(got.table.io, donors()["BBB"].io);
}

#[test]
fn missing_manufacturing_splits_industry_over_goods_one_to_eight() {
    let w = Wdi {
        manufacturing: BTreeMap::new(),
        ..wdi()
    };
    let got = one("XXX", &w, &obs());
    assert_eq!(got.va_basis, Basis::WdiGdp);
    assert!(approx(got.table.va[0], 100.0, 1e-6) && approx(got.table.va[9], 450.0, 1e-6));
    // Industry 350 over goods 1 to 8 as the donor (4, 1, 6, 2, 1, 1, 3, 2) / 20.
    let want: Vec<f64> = [4.0, 1.0, 6.0, 2.0, 1.0, 1.0, 3.0, 2.0]
        .iter()
        .map(|x| 350.0 * x / 20.0)
        .collect();
    assert!(all_close(&got.table.va[1..9], &want, 1e-12));
    assert!(closes(&got.table, 1e-9, 1e-12));
}

#[test]
fn industry_below_manufacturing_counts_as_missing_manufacturing() {
    let w = Wdi {
        industry: map(&[("XXX", 0.1)]),
        ..wdi()
    };
    let got = one("XXX", &w, &obs());
    assert_eq!(got.va_basis, Basis::WdiGdp);
    assert!(approx(total(&got.table.va), 650.0, 1e-6));
    assert!(approx(got.table.va[1..9].iter().sum(), 100.0, 1e-6));
}

#[test]
fn all_zero_sector_shares_count_as_missing_and_use_the_icio_median_va_over_gdp() {
    let zero = map(&[("XXX", 0.0)]);
    let w = Wdi {
        agriculture: zero.clone(),
        industry: zero.clone(),
        manufacturing: zero.clone(),
        services: zero,
        ..wdi()
    };
    let got = one("XXX", &w, &obs());
    // No usable share: GDP 1000 million USD times the median of the donors' VA over GDP (1, 0.5,
    // 0.8), not donor BBB's 0.5.
    assert_eq!(got.donor.as_deref(), Some("BBB"));
    assert_eq!(got.va_basis, Basis::WdiGdp);
    assert!(approx(total(&got.table.va), 800.0, 1e-6));
    assert!(all_close(
        &proportions(&got.table.va),
        &donor_proportions(),
        1e-12
    ));
    assert!(approx(median_va_over_gdp(&donors(), &wdi()), 0.8, 1e-6));
}

#[test]
fn donor_basis_scales_the_donors_level_by_income() {
    let w = Wdi {
        gdp: map(&DONOR_GDP),
        ..wdi()
    };
    let got = one("XXX", &w, &obs());
    // Donor VA 100 million USD for 5 million people, so 20 USD per person; XXX has 1 million
    // people at 11000/20000 of its income.
    assert_eq!(got.va_basis, Basis::Donor);
    assert_eq!(got.donor.as_deref(), Some("BBB"));
    assert_eq!(got.income_source, IncomeSource::Pwt);
    assert!(approx(total(&got.table.va), 20.0 * 11000.0 / 20000.0, 1e-6));
    assert!(all_close(
        &proportions(&got.table.va),
        &donor_proportions(),
        1e-12
    ));
    assert!(closes(&got.table, 1e-9, 1e-12));
}

fn wdi_income(source: &str) -> Wdi {
    let mut income_source: BTreeMap<String, String> = WDI_INCOME
        .iter()
        .map(|(k, _)| ((*k).to_owned(), "wdi".to_owned()))
        .collect();
    income_source.insert("XXX".into(), source.into());
    Wdi {
        gdp: map(&DONOR_GDP),
        income: with(&WDI_INCOME, &[("XXX", 12000.0)]),
        income_source,
        ..wdi()
    }
}

#[test]
fn without_pwt_income_donors_are_ranked_and_scaled_by_wdi_gdp_per_person() {
    let got = one("XXX", &wdi_income("wdi"), &no_pwt_for_xxx());
    // PWT would pick BBB (20000 is the closest in its region to 12000); WDI incomes pick AAA
    // (10000, BBB is 50000).
    assert_eq!(got.donor.as_deref(), Some("AAA"));
    assert_eq!(got.income_source, IncomeSource::Wdi);
    // AAA: 100 million USD for 1 million people; XXX has 1 million people at 12000/10000 of its income.
    assert!(approx(
        total(&got.table.va),
        100.0 * 12000.0 / 10000.0,
        1e-6
    ));
}

/// XXX with PWT and WDI incomes; the US at a PWT over WDI ratio of 1.
fn pwt_against_wdi(pwt: f64, income: f64) -> (Observed, Wdi) {
    let o = obs_with(
        Some(with(&PWT, &[("XXX", pwt), ("USA", 30000.0)])),
        &[],
        &[],
    );
    let mut w = wdi_income("wdi");
    w.income.extend(map(&[("XXX", income), ("USA", 30000.0)]));
    (o, w)
}

#[test]
fn pwt_far_below_wdi_ranks_by_wdi_income() {
    // PWT 3000 against WDI 40000 is 0.075 of the US ratio, below a third: WDI ranks, and picks
    // BBB (50000) where PWT would pick AAA (1000).
    let (o, w) = pwt_against_wdi(3000.0, 40000.0);
    let got = one("XXX", &w, &o);
    assert_eq!(got.income_source, IncomeSource::WdiNotPwt);
    assert_eq!(got.donor.as_deref(), Some("BBB"));
    assert_eq!(IncomeSource::WdiNotPwt.as_str(), "wdi_not_pwt");
    // 3000 against 8000 is 0.375 of it: PWT stays.
    let (o, w) = pwt_against_wdi(3000.0, 8000.0);
    let got = one("XXX", &w, &o);
    assert_eq!(got.income_source, IncomeSource::Pwt);
    assert_eq!(got.donor.as_deref(), Some("AAA"));
}

#[test]
fn adb_income_is_flagged() {
    let got = one("XXX", &wdi_income("adb"), &no_pwt_for_xxx());
    assert_eq!(got.income_source, IncomeSource::Adb);
    assert_eq!(got.donor.as_deref(), Some("AAA"));
}

#[test]
fn without_pwt_or_wdi_income_donors_are_ranked_and_scaled_by_maddison_gdp_per_person() {
    let w = Wdi {
        gdp: map(&DONOR_GDP),
        maddison: map(&[
            ("AAA", 1500.0),
            ("BBB", 4000.0),
            ("CCC", 2000.0),
            ("XXX", 3000.0),
        ]),
        ..wdi()
    };
    let got = one("XXX", &w, &no_pwt_for_xxx());
    // In region R1, Maddison values pick BBB (4000 is closer in logs to 3000 than 1500), where WDI
    // incomes would pick AAA.
    assert_eq!(got.donor.as_deref(), Some("BBB"));
    assert_eq!(got.income_source, IncomeSource::Maddison);
    assert_eq!(got.va_basis, Basis::Donor);
    // BBB: 100 million USD for 5 million people; XXX has 1 million people at 3000/4000 of BBB's
    // Maddison income.
    assert!(approx(total(&got.table.va), 20.0 * 3000.0 / 4000.0, 1e-6));
}

#[test]
fn wdi_income_takes_precedence_over_maddison() {
    let w = Wdi {
        maddison: map(&[("AAA", 1500.0), ("BBB", 4000.0), ("XXX", 3000.0)]),
        ..wdi_income("wdi")
    };
    let got = one("XXX", &w, &no_pwt_for_xxx());
    assert_eq!(got.donor.as_deref(), Some("AAA"));
    assert_eq!(got.income_source, IncomeSource::Wdi);
}

#[test]
fn without_any_income_the_donor_is_the_region_median_and_the_level_is_unscaled() {
    let w = Wdi {
        gdp: map(&DONOR_GDP),
        ..wdi()
    };
    let got = one("XXX", &w, &no_pwt_for_xxx());
    // Region R1 holds AAA (1000) and BBB (20000): the median of their logs is equidistant, so the
    // name decides.
    assert_eq!(got.donor.as_deref(), Some("AAA"));
    assert_eq!(got.income_source, IncomeSource::None);
    assert_eq!(got.va_basis, Basis::Donor);
    assert!(approx(total(&got.table.va), 100.0, 1e-6));
}

#[test]
fn region_override_places_an_unclassified_country() {
    let mut o = obs();
    o.region.remove("XXX");
    let case = Case {
        regions: &[("XXX", "R2")],
        ..PLAIN
    };
    let got = &run(&[("XXX", "XXX")], &donors(), &wdi(), &o, &case)["XXX"];
    assert_eq!(got.region.as_deref(), Some("R2"));
    assert_eq!(got.donor.as_deref(), Some("CCC"));
    // Anguilla and Montserrat, which the World Bank does not classify, take Latin America and
    // Caribbean donors through the pack table, not their sovereign's European region.
    let lac = "Latin America & Caribbean";
    let o = obs_with(None, &[("AAA", "Europe & Central Asia"), ("CCC", lac)], &[]);
    let case = Case {
        regions: &REGIONS,
        ..PLAIN
    };
    let got = run(
        &[("AIA", "AAA"), ("MSR", "AAA")],
        &donors(),
        &wdi(),
        &o,
        &case,
    );
    for code in ["AIA", "MSR"] {
        assert_eq!(got[code].region.as_deref(), Some(lac), "{code}");
        assert_eq!(got[code].donor.as_deref(), Some("CCC"), "{code}");
    }
}

#[test]
fn people_come_from_worldpop_when_wdi_has_none() {
    let o = obs_with(None, &[("WWW", "R1")], &[]);
    let w = Wdi {
        gdp: map(&DONOR_GDP),
        ..wdi()
    };
    let case = Case {
        worldpop: &[("WWW", 2e6)],
        ..PLAIN
    };
    let got = &run(&[("WWW", "WWW")], &donors(), &w, &o, &case)["WWW"];
    // No income of its own, so donor AAA's level: 2 million people at 100 USD per person.
    assert_eq!(got.donor.as_deref(), Some("AAA"));
    assert_eq!(got.va_basis, Basis::Donor);
    assert!(approx(total(&got.table.va), 200.0, 1e-6));
}

#[test]
fn people_come_from_wpp_before_worldpop() {
    let mut o = obs_with(None, &[("WWW", "R1")], &[]);
    o.population_wpp = map(&[("WWW", 3e6)]);
    let w = Wdi {
        gdp: map(&DONOR_GDP),
        ..wdi()
    };
    let case = Case {
        worldpop: &[("WWW", 2e6)],
        ..PLAIN
    };
    let got = &run(&[("WWW", "WWW")], &donors(), &w, &o, &case)["WWW"];
    // WPP's 3 million people at donor AAA's 100 USD per person, not WorldPop's 2 million.
    assert_eq!(got.va_basis, Basis::Donor);
    assert!(approx(total(&got.table.va), 300.0, 1e-6));
}

#[test]
fn uninhabited_territory_keeps_zero_value_added() {
    let o = obs_with(None, &[("ZZZ", "R1")], &[("ZZZ", 0.0)]);
    let w = Wdi {
        gdp: map(&DONOR_GDP),
        ..wdi()
    };
    let got = one("ZZZ", &w, &o);
    assert_eq!(got.va_basis, Basis::None);
    assert_eq!(total(&got.table.va), 0.0);
    assert_eq!(total(&got.table.output), 0.0);
    assert!(got.donor.is_some());
    assert!(closes(&got.table, 1e-9, 1e-12));
}

#[test]
fn territory_inside_a_parent_takes_its_population_share_from_the_parent() {
    let o = obs_with(None, &[("TER", "R1")], &[]);
    let plain = &run(&[("XXX", "XXX")], &donors(), &wdi(), &o, &PLAIN)["XXX"];
    let case = Case {
        worldpop: &[("TER", 1e5)],
        inside: &[("TER", "XXX")],
        ..PLAIN
    };
    let got = run(
        &[("XXX", "XXX"), ("TER", "XXX")],
        &donors(),
        &wdi(),
        &o,
        &case,
    );
    let (ter, par) = (&got["TER"], &got["XXX"]);
    // 100,000 of the parent's 1,000,000 people: 10 percent of its VA moves out, nothing is added.
    assert_eq!(ter.va_basis, Basis::Parent);
    assert_eq!(ter.donor.as_deref(), Some("BBB"));
    assert!(approx(par.shed, 0.1, 1e-6));
    let scaled = |f: f64| plain.table.va.map(|v| v * f);
    assert!(all_close(&ter.table.va, &scaled(0.1), 1e-12));
    assert!(all_close(&par.table.va, &scaled(0.9), 1e-12));
    let sum: Vec<f64> = (0..N_GOODS)
        .map(|g| par.table.output[g] + ter.table.output[g])
        .collect();
    assert!(all_close(&sum, &plain.table.output, 1e-12));
    assert_eq!(ter.table.io, plain.table.io);
    assert!(closes(&ter.table, 1e-9, 1e-12));
    assert!(closes(&par.table, 1e-9, 1e-12));
}

#[test]
fn territory_inside_an_icio_economy_shrinks_that_economy() {
    let donors = donors();
    let case = Case {
        worldpop: &[("TER", 1e5)],
        inside: &[("TER", "AAA")],
        ..PLAIN
    };
    let got = run(
        &[("AAA", "AAA"), ("TER", "AAA")],
        &donors,
        &wdi(),
        &obs(),
        &case,
    );
    // AAA has 1 million people.
    let scaled = |f: f64| donors["AAA"].va.map(|v| v * f);
    assert!(all_close(&got["AAA"].table.va, &scaled(0.9), 1e-12));
    assert!(all_close(&got["TER"].table.va, &scaled(0.1), 1e-12));
    assert_eq!(got["TER"].donor.as_deref(), Some("AAA"));
    assert_eq!(got["AAA"].va_basis, Basis::Icio);
    assert!(approx(got["AAA"].shed, 0.1, 1e-6));
    assert!(closes(&got["AAA"].table, 1e-9, 1e-12));
}

#[test]
fn split_economies_sum_gdp_and_weight_shares_by_gdp() {
    let w = Wdi {
        gdp: with(&DONOR_GDP, &[("PA", 1e9), ("PB", 3e9)]),
        income: map(&WDI_INCOME),
        agriculture: map(&[("PA", 0.2), ("PB", 0.0)]),
        industry: map(&[("PA", 0.3), ("PB", 0.3)]),
        manufacturing: map(&[("PA", 0.1), ("PB", 0.1)]),
        services: map(&[("PA", 0.4), ("PB", 0.6)]),
        ..Wdi::default()
    };
    let o = obs_with(None, &[("PA", "R1")], &[("PA", 1e6), ("PB", 1e6)]);
    let case = Case {
        split: &[("XXX", &["PA", "PB"])],
        ..PLAIN
    };
    let got = &run(&[("XXX", "XXX")], &donors(), &w, &o, &case)["XXX"];
    let want = 4000.0 * (0.2 / 4.0 + 0.3 + 0.4 / 4.0 + 0.6 * 3.0 / 4.0);
    assert!(approx(total(&got.table.va), want, 1e-6));
    assert!(approx(got.table.va[0], 4000.0 * 0.05, 1e-6));
}

/// XXX's BACI trade in USD: imports and exports of traded goods that leave every domestic use
/// positive (XXX makes 125 of good 0 and 75 of good 1, million USD, at donor BBB's output per VA).
fn xxx_trade() -> Trade {
    let mut t = Trade::default();
    t.imports[..7].copy_from_slice(&[30e6, 50e6, 5e6, 40e6, 25e6, 10e6, 60e6]);
    t.exports[..7].copy_from_slice(&[80e6, 10e6, 2e6, 20e6, 40e6, 5e6, 15e6]);
    t
}

/// `donors()` with BBB's final demand on every good but good 2, so the balance is feasible.
fn spread_donors() -> BTreeMap<String, CountryIo> {
    let mut d = donors();
    d.get_mut("BBB").unwrap().final_demand_shares =
        [0.1, 0.1, 0.0, 0.15, 0.1, 0.1, 0.1, 0.1, 0.1, 0.15];
    d
}

/// [Z | F] of an economy: intermediate use Z_kg = a_kg X_g and final demand F_k = f_k F.
fn use_table(t: &ceteris_pack::read::icio::CountryIo, final_demand: f64) -> [[f64; 11]; N_GOODS] {
    std::array::from_fn(|k| {
        std::array::from_fn(|g| {
            if g < N_GOODS {
                t.io[k][g] * t.output[g]
            } else {
                t.final_demand_shares[k] * final_demand
            }
        })
    })
}

#[test]
fn ras_balances_the_donor_use_table_to_domestic_use() {
    let trade = [("XXX", xxx_trade())];
    let case = Case {
        trade: &trade,
        ..PLAIN
    };
    let plain = &run(&[("XXX", "XXX")], &spread_donors(), &wdi(), &obs(), &PLAIN)["XXX"];
    let got = &run(&[("XXX", "XXX")], &spread_donors(), &wdi(), &obs(), &case)["XXX"];
    assert_eq!(got.io_basis, IoBasis::Ras);
    let t = &got.table;
    // Output and VA stay; only the uses move.
    assert_eq!(t.output, plain.table.output);
    assert_eq!(t.va, plain.table.va);
    assert!(closes(t, 1e-9, 1e-12));
    let (m, e) = (
        xxx_trade().imports.map(|v| v / 1e6),
        xxx_trade().exports.map(|v| v / 1e6),
    );
    let final_demand = total(&t.va) + total(&m) - total(&e);
    let z = use_table(t, final_demand);
    for k in 0..N_GOODS {
        let row: f64 = z[k].iter().sum();
        let want = t.output[k] + m[k] - e[k];
        assert!(approx(row, want, 1e-10), "row {k}: {row} vs {want}");
    }
    for g in 0..N_GOODS {
        let inputs: f64 = z.iter().map(|row| row[g]).sum();
        assert!(approx(inputs, t.output[g] - t.va[g], 1e-10), "column {g}");
    }
    assert!(approx(total(&t.final_demand_shares), 1.0, 1e-12));
    assert!(t.io.iter().flatten().all(|a| *a >= 0.0));
    assert_ne!(t.io, plain.table.io);
    assert_ne!(t.final_demand_shares, plain.table.final_demand_shares);
    // The donor's zeros stay zero.
    assert_eq!(t.final_demand_shares[2], 0.0);
}

#[test]
fn negative_domestic_use_keeps_the_donor_table() {
    let mut t = xxx_trade();
    // Exports of good 0 above its output plus imports: 125 + 30 - 200 < 0.
    t.exports[0] = 200e6;
    let trade = [("XXX", t)];
    let case = Case {
        trade: &trade,
        ..PLAIN
    };
    let got = &run(&[("XXX", "XXX")], &donors(), &wdi(), &obs(), &case)["XXX"];
    assert_eq!(got.io_basis, IoBasis::Donor);
    assert_eq!(got.table.io, donors()["BBB"].io);
    assert_eq!(
        got.table.final_demand_shares,
        donors()["BBB"].final_demand_shares
    );
}

#[test]
fn no_trade_keeps_the_donor_table() {
    let trade = [("YYY", xxx_trade())];
    let case = Case {
        trade: &trade,
        ..PLAIN
    };
    let got = &run(&[("XXX", "XXX")], &donors(), &wdi(), &obs(), &case)["XXX"];
    assert_eq!(got.io_basis, IoBasis::Donor);
    assert_eq!(got.table.io, donors()["BBB"].io);
}

#[test]
fn territory_inside_takes_its_parents_balanced_table() {
    let o = obs_with(None, &[("TER", "R1")], &[]);
    let trade = [("XXX", xxx_trade())];
    let balanced = Case {
        trade: &trade,
        ..PLAIN
    };
    let alone = &run(&[("XXX", "XXX")], &spread_donors(), &wdi(), &o, &balanced)["XXX"];
    let case = Case {
        worldpop: &[("TER", 1e5)],
        inside: &[("TER", "XXX")],
        trade: &trade,
        ..PLAIN
    };
    let got = run(
        &[("XXX", "XXX"), ("TER", "XXX")],
        &spread_donors(),
        &wdi(),
        &o,
        &case,
    );
    let (ter, par) = (&got["TER"], &got["XXX"]);
    assert_eq!(alone.io_basis, IoBasis::Ras);
    for e in [ter, par] {
        assert_eq!(e.io_basis, IoBasis::Ras);
        assert_eq!(e.table.io, alone.table.io);
        assert_eq!(e.table.final_demand_shares, alone.table.final_demand_shares);
    }
}

// Tests below need the downloaded sources and the 2000 pack; CI is data-free.

const NAMES: [&str; 20] = [
    "BL_v3_MF1564.csv",
    "BPJresult_AGE_V01.csv",
    "ilostat_EAP_2WAP_SEX_AGE_RT_A_2000.csv",
    "ilostat_UNE_2EAP_SEX_AGE_RT_A_2000.csv",
    "pwt110.xlsx",
    "wdi_SI.POV.GINI.zip",
    "wdi_SP.POP.TOTL.zip",
    "wdi_SP.POP.1564.TO.ZS.zip",
    "swiid9_92.zip",
    "ne_10m_admin_0_countries.zip",
    "wdi_NV.AGR.TOTL.ZS.zip",
    "wdi_NV.IND.TOTL.ZS.zip",
    "wdi_NV.IND.MANF.ZS.zip",
    "wdi_NV.SRV.TOTL.ZS.zip",
    "wdi_NY.GDP.MKTP.CD.zip",
    "wdi_NY.GDP.PETR.RT.ZS.zip",
    "wdi_NY.GDP.NGAS.RT.ZS.zip",
    "adb_kidb_NY_GDP_PCAP_CD_COO.csv",
    "mpd2023_web.xlsx",
    "ppp_2000_1km_Aggregated.tif",
];

struct Data {
    real: BTreeMap<String, CountryIo>,
    countries: Vec<(String, String)>,
    iso: Names,
    obs: Observed,
    wdi: Wdi,
    imputed: BTreeMap<String, Economy>,
}

/// The 2000 ICIO economies, the pack countries imputed from them, and their sources, in
/// optimised builds only (the WorldPop national totals take minutes unoptimised).
fn data() -> Option<&'static Data> {
    static DATA: OnceLock<Option<Data>> = OnceLock::new();
    DATA.get_or_init(|| {
        if cfg!(debug_assertions) {
            eprintln!(
                "skipping: summing WorldPop takes over 10 minutes unoptimised; run with --release"
            );
            return None;
        }
        if NAMES.iter().any(|n| cached(n).is_none()) {
            return None;
        }
        let real = aggregate(&cached(ICIO)?, 2000).unwrap();
        let (countries, provinces, cells) = (
            pack_table("countries.arrow")?,
            pack_table("provinces.arrow")?,
            pack_table("cells.arrow")?,
        );
        let admin0 = cached("ne_10m_admin_0_countries.zip").unwrap();
        let cache = admin0.parent().unwrap();
        let obs = load_observed(cache, &admin0).unwrap();
        let (iso, _) = read_codes(&admin0).unwrap();
        let wdi = load_wdi(cache).unwrap();
        let countries: Vec<(String, String)> = strings(&countries, "code")
            .into_iter()
            .zip(strings(&countries, "sovereign"))
            .collect();
        let mut owner = CellOwner::read(&cells).unwrap();
        let mut raster = geotiff::open(&cached("ppp_2000_1km_Aggregated.tif")?).unwrap();
        let raw = province_population(&mut raster, &mut owner, BLOCK_ROWS).unwrap();
        let mut worldpop: BTreeMap<String, f64> = BTreeMap::new();
        for (c, p) in strings(&provinces, "country").into_iter().zip(raw) {
            *worldpop.entry(c).or_insert(0.0) += p;
        }
        let imputed = impute_pack(
            &countries,
            &real,
            &wdi,
            &obs,
            &iso,
            &worldpop,
            &BTreeMap::new(),
        )
        .unwrap();
        Some(Data {
            real,
            countries,
            iso,
            obs,
            wdi,
            imputed,
        })
    })
    .as_ref()
}

const UNINHABITED: [&str; 14] = [
    "ATC", "ATF", "CLP", "CSI", "HMD", "PGA", "SGS", "UMI", "PCN", "VAT", "IOT", "IOA", "NFK",
    "NIU",
];

#[test]
fn real_imputation_covers_every_pack_country() {
    let Some(d) = data() else { return };
    assert_eq!(d.imputed.len(), d.countries.len());
    assert_eq!(d.imputed.len(), 242);
    assert!(d.countries.iter().all(|(c, _)| d.imputed.contains_key(c)));
    let own: Vec<&String> = d
        .imputed
        .iter()
        .filter(|(_, v)| v.donor.is_none())
        .map(|(c, _)| c)
        .collect();
    let expected: Vec<&String> = d
        .countries
        .iter()
        .map(|(c, _)| c)
        .filter(|c| {
            let i = d.iso.get(*c).unwrap_or(c);
            i != "ROW" && d.real.contains_key(i)
        })
        .collect();
    let mut expected = expected;
    expected.sort();
    assert_eq!(own, expected);
    assert_eq!(own.len(), 80);
    for (code, c) in &d.imputed {
        let t = &c.table;
        assert!(t.io.iter().flatten().all(|x| x.is_finite()), "{code}");
        assert!((0..N_GOODS).all(|g| col_sum(&t.io, g) < 1.0), "{code}");
        assert!(
            (total(&t.final_demand_shares) - 1.0).abs() <= 1e-12,
            "{code}"
        );
        assert!(t.va.iter().all(|v| v.is_finite() && *v >= 0.0), "{code}");
        assert!(closes(t, 1e-9, 1e-9), "{code}");
    }
}

#[test]
fn real_donors_stay_inside_the_resolved_region() {
    let Some(d) = data() else { return };
    let mut placed = 0;
    for (code, c) in &d.imputed {
        if let (Some(donor), Some(region)) = (&c.donor, &c.region)
            && c.va_basis != Basis::Parent
            && !region.is_empty()
        {
            assert_eq!(d.obs.region.get(donor), Some(region), "{code}");
            placed += 1;
        }
    }
    assert!(placed > 140);
    for c in ["SAH", "CYN", "SOL"] {
        assert!(d.imputed[c].region.as_ref().is_some_and(|r| !r.is_empty()));
    }
}

#[test]
fn real_populated_territories_have_value_added() {
    let Some(d) = data() else { return };
    let populated = [
        "SOL", "SAH", "CYN", "JEY", "GGY", "ALD", "AIA", "BLM", "COK", "WLF", "SPM", "MSR", "SHN",
    ];
    for code in populated {
        assert!(total(&d.imputed[code].table.va) > 0.0, "{code}");
    }
    for (code, c) in &d.imputed {
        if c.va_basis == Basis::None {
            assert!(UNINHABITED.contains(&code.as_str()), "{code}");
        }
    }
    let region = |c: &str| d.imputed[c].region.clone().unwrap_or_default();
    assert_eq!(region("SAH"), "Middle East & North Africa");
    assert_eq!(region("CYN"), "Europe & Central Asia");
    assert_eq!(region("SOL"), "Sub-Saharan Africa");
    assert_eq!(d.imputed["VEN"].income_source, IncomeSource::WdiNotPwt);
    let set_aside = d
        .imputed
        .values()
        .filter(|c| c.income_source == IncomeSource::WdiNotPwt)
        .count();
    assert_eq!(set_aside, 1);
    assert_eq!(region("AIA"), "Latin America & Caribbean");
    assert_eq!(region("MSR"), "Latin America & Caribbean");
}

#[test]
fn real_territories_inside_a_parent_take_their_share_of_it() {
    let Some(d) = data() else { return };
    for (territory, parent) in INSIDE {
        let (t, p) = (&d.imputed[territory], &d.imputed[parent]);
        let tv = total(&t.table.va);
        assert_eq!(t.va_basis, Basis::Parent, "{territory}");
        assert!(0.0 < p.shed && p.shed < 0.5 && tv > 0.0, "{territory}");
        assert!(
            approx(tv / (tv + total(&p.table.va)), p.shed, 1e-9),
            "{territory}"
        );
    }
    for c in ["FIN", "CYP"] {
        assert!(total(&d.imputed[c].table.va) < total(&d.real[c].va));
    }
}

#[test]
fn real_wdi_basis_value_added_is_gdp_times_the_shares() {
    let Some(d) = data() else { return };
    let mut checked = 0;
    for (code, c) in &d.imputed {
        let own = d.iso.get(code).unwrap_or(code);
        // Split economies sum several.
        if c.va_basis == Basis::Wdi
            && let Some(gdp) = d.wdi.gdp.get(own)
        {
            let w = &d.wdi;
            let share = w.agriculture[own] + w.industry[own] + w.services[own];
            assert!(
                approx(total(&c.table.va) * 1e6, gdp * share, 1e-9),
                "{code}"
            );
            checked += 1;
        }
    }
    assert!(checked > 80);
}

#[test]
fn real_donor_basis_levels_are_scaled_by_income() {
    let Some(d) = data() else { return };
    let donor_basis: BTreeMap<&String, &Economy> = d
        .imputed
        .iter()
        .filter(|(_, v)| v.va_basis == Basis::Donor)
        .collect();
    let prk = donor_basis[&"PRK".to_owned()];
    assert_eq!(prk.income_source, IncomeSource::Maddison);
    // North Korea at Cambodia's Maddison income (1,657 and 1,659 dollars of 2011): about 7
    // billion USD, not the 43 billion of an unscaled Thai level.
    assert!((5e3..3e4).contains(&total(&prk.table.va)));
    assert_eq!(
        donor_basis[&"COK".to_owned()].income_source,
        IncomeSource::Adb
    );
}

const GDP_WITHOUT_SHARES: [&str; 17] = [
    "ARM", "ASM", "BHR", "CAF", "DJI", "GNQ", "GUM", "IMN", "LCA", "LIE", "MCO", "MNP", "NRU",
    "SMR", "SOM", "VEN", "VIR",
];

#[test]
fn real_gdp_without_shares_uses_the_icio_median_va_over_gdp() {
    let Some(d) = data() else { return };
    let ratio = median_va_over_gdp(&d.real, &d.wdi);
    assert!(0.90 < ratio && ratio < 0.96);
    let mut scope = Vec::new();
    for (code, c) in &d.imputed {
        let (gdp, s) = sector_shares(&d.wdi, &[d.iso.get(code).unwrap_or(code).clone()]);
        let none = s.agriculture.is_none() && s.industry.is_none() && s.services.is_none();
        if c.va_basis == Basis::WdiGdp && gdp > 0.0 && none {
            scope.push(code.as_str());
            // SOM gave 17.9 percent of its VA to Somaliland.
            assert!(
                approx(total(&c.table.va) * 1e6, gdp * ratio * (1.0 - c.shed), 1e-9),
                "{code}"
            );
        }
    }
    assert_eq!(scope, GDP_WITHOUT_SHARES);
    // Somalia is no longer scaled by Congo's 0.43 (it was 0.356 of GDP).
    let som = &d.imputed["SOM"];
    assert!(total(&som.table.va) / (1.0 - som.shed) * 1e6 / d.wdi.gdp["SOM"] > 0.9);
}

#[test]
fn real_venezuela_is_not_left_empty() {
    let Some(d) = data() else { return };
    // WDI reports zero sector shares for 2000, so GDP (117 billion USD) times the ICIO median VA
    // over GDP.
    let ven = &d.imputed["VEN"];
    assert_eq!(ven.va_basis, Basis::WdiGdp);
    assert!((80e3..130e3).contains(&total(&ven.table.va)));
}

/// `econ_home_shares.arrow`: one row per pack country and traded good; an ICIO economy (by its ISO
/// code) carries its domestic over total use and its total use, a country outside ICIO is flagged
/// and carries NaN.
#[test]
fn home_share_rows_follow_the_pack_countries() {
    use arrow_array::cast::AsArray;
    use arrow_array::types::{Float64Type, UInt8Type, UInt32Type};

    let mut home = [0.0; N_GOODS];
    let mut total = [0.0; N_GOODS];
    for g in 0..7 {
        home[g] = 1.0 + g as f64;
        total[g] = 4.0 + 2.0 * g as f64;
    }
    total[2] = 0.0;
    home[2] = 0.0;
    let used = map(&[("AAA", HomeUse { home, total })]);
    let iso = map(&[("XAA", "AAA".to_owned())]);
    let codes = ["XAA".to_owned(), "BBB".to_owned()];
    let batch =
        arrow_array::RecordBatch::try_from_iter(home_share_columns(&codes, &used, &iso).unwrap())
            .unwrap();
    assert_eq!(batch.num_rows(), 14);
    let col = |n: &str| batch.column_by_name(n).unwrap().clone();
    let country = col("country")
        .as_primitive::<UInt32Type>()
        .values()
        .to_vec();
    let good = col("good").as_primitive::<UInt8Type>().values().to_vec();
    let share = col("home_share")
        .as_primitive::<Float64Type>()
        .values()
        .to_vec();
    let absorption = col("absorption")
        .as_primitive::<Float64Type>()
        .values()
        .to_vec();
    let icio: Vec<bool> = col("icio")
        .as_boolean()
        .iter()
        .map(|b| b.unwrap())
        .collect();
    for r in 0..14 {
        let (c, g) = (r / 7, r % 7);
        assert_eq!((country[r] as usize, usize::from(good[r])), (c, g));
        if c == 0 && g != 2 {
            assert!(icio[r]);
            assert!(approx(share[r], home[g] / total[g], 1e-12), "row {r}");
            assert_eq!(absorption[r], total[g]);
        } else {
            // outside ICIO, or an ICIO economy that uses none of the good
            assert_eq!(icio[r], c == 0);
            assert!(share[r].is_nan(), "row {r}");
            assert!(
                c == 0 && absorption[r] == 0.0 || absorption[r].is_nan(),
                "row {r}"
            );
        }
    }
}

/// Economies whose WDI 2000 lacks one of agriculture, industry or services.
const PARTIAL_SHARES: [&str; 9] = [
    "BMU", "ERI", "KEN", "KWT", "LBY", "MAC", "MDG", "PNG", "TUV",
];

#[test]
fn real_partial_shares_reach_the_icio_median_va_over_gdp() {
    let Some(d) = data() else { return };
    let ratio = median_va_over_gdp(&d.real, &d.wdi);
    let mut scope = Vec::new();
    for (code, c) in &d.imputed {
        let (gdp, s) = sector_shares(&d.wdi, &[d.iso.get(code).unwrap_or(code).clone()]);
        let have = [s.agriculture, s.industry, s.services];
        let partial = have.iter().any(Option::is_some) && have.iter().any(Option::is_none);
        if c.va_basis != Basis::Icio && gdp > 0.0 && partial {
            scope.push(code.as_str());
            // All nine have less than the median in the shares there are.
            assert!(
                approx(total(&c.table.va) * 1e6, gdp * ratio, 1e-9),
                "{code}"
            );
        }
    }
    assert_eq!(scope, PARTIAL_SHARES);
    // Kuwait was 135 million USD against GDP 37.7 billion.
    assert!(total(&d.imputed["KWT"].table.va) > 30e3);
}

#[test]
fn real_oil_exporters_outside_icio_have_their_oil_and_gas() {
    let Some(d) = data() else { return };
    // Extraction VA over rents (ADR 0281). Below: a rent, (price - unit cost) times output, is
    // at most the output's value, so k is at least the extraction column's a_VA, 0.666 in the
    // world totals of ADR 0270. Above: good 1's VA holds refining's as well, which is not
    // negative, so k is below the same sum over good 1's VA. 2000: 0.991 against 1.430.
    let k = oil_gas_va_over_rents(&d.real, &d.wdi);
    let (va_1, rents): (Vec<f64>, Vec<f64>) = d
        .real
        .iter()
        .filter_map(|(code, c)| {
            let r = match (d.wdi.oil_rents.get(code), d.wdi.gas_rents.get(code)) {
                (None, None) => return None,
                (oil, gas) => oil.unwrap_or(&0.0) + gas.unwrap_or(&0.0),
            };
            Some((c.va[1] * 1e6, d.wdi.gdp.get(code)? * r))
        })
        .unzip();
    let with_refining = va_1.iter().sum::<f64>() / rents.iter().sum::<f64>();
    let a_va = 1.0 - world_oil_gas_column(&d.real).iter().sum::<f64>();
    assert!(a_va < k && k < with_refining, "{a_va} {k} {with_refining}");
    // Billion USD of crude oil and gas VA, against about 21, 14, 20 and 33 of 2000 crude output.
    // Crude oil and gas from rents produce with the world column, not the donor's (Sudan's
    // donor Senegal has a value-added share of 0.07 in it).
    let world = world_oil_gas_column(&d.real);
    for code in ["SDN", "TKM", "COG", "AZE"] {
        let io = &d.imputed[code].table.io;
        assert!(io.iter().zip(world).all(|(row, w)| row[1] == w), "{code}");
    }
    // Extraction only (ADR 0270): 0.666 on 2000 data, against 0.418 with refining.
    assert!((0.6..0.7).contains(&(1.0 - world.iter().sum::<f64>())));
    for (code, low) in [("KWT", 10.0), ("LBY", 5.0), ("DZA", 8.0), ("VEN", 15.0)] {
        let va = d.imputed[code].table.va[1] / 1e3;
        assert!(va > low, "{code} {va}");
    }
}
