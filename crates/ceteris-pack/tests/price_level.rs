//! Price level p_c of each pack country (ADR 0098 as amended by ADRs 0282 and 0294): PWT 11 pl_da
//! over the US's, else the level WDI's GDP at PPP implies over the US's (ADR 0289), else the
//! donor's, each screened by Tukey's far-out fences on the residuals of the Penn effect fitted on
//! the PWT levels.

mod common;

use std::collections::BTreeMap;

use ceteris_pack::econ::price_level::{
    FAR_OUT, Level, Screen, Source, fences, price_levels, relative, wdi_implied,
};
use ceteris_pack::read::ByYear;
use ceteris_pack::read::people::read_pwt;
use common::{V, approx, write_rows};

fn map(v: &[(&str, f64)]) -> BTreeMap<String, f64> {
    v.iter().map(|(k, x)| ((*k).to_owned(), *x)).collect()
}

fn by_year(v: &[(&str, &[(i64, f64)])]) -> BTreeMap<String, ByYear> {
    v.iter()
        .map(|(k, ys)| ((*k).to_owned(), ys.iter().copied().collect()))
        .collect()
}

/// RFC 0282 D7: the price level of domestic absorption pl_da, not the output-side pl_gdpo.
#[test]
fn pwt_reader_keeps_the_domestic_absorption_price_level() {
    let dir = tempfile::tempdir().unwrap();
    let row = |code: &'static str, year: f64, pl: f64| {
        vec![
            ("countrycode", V::T(code)),
            ("year", V::N(year)),
            ("pl_da", V::N(pl)),
            ("pl_gdpo", V::N(10.0 * pl)),
        ]
    };
    let path = write_rows(
        &dir.path().join("pwt.xlsx"),
        &[(
            "Data",
            vec![
                row("USA", 2000.0, 0.66),
                row("AAA", 1999.0, 0.3),
                row("AAA", 2000.0, 0.33),
            ],
        )],
    );
    let pwt = read_pwt(&path).unwrap();
    assert_eq!(pwt.price_level["USA"][&2000], 0.66);
    assert_eq!(pwt.price_level["AAA"], [(1999, 0.3), (2000, 0.33)].into());
}

/// RFC 0282 D8: PWT's i_cig of 2000, how its price level of that year was made.
#[test]
fn pwt_reader_keeps_the_price_basis_of_2000() {
    let dir = tempfile::tempdir().unwrap();
    let row = |code: &'static str, year: f64, basis: &'static str| {
        vec![
            ("countrycode", V::T(code)),
            ("year", V::N(year)),
            ("i_cig", V::T(basis)),
        ]
    };
    let path = write_rows(
        &dir.path().join("pwt.xlsx"),
        &[(
            "Data",
            vec![
                row("USA", 2000.0, "Benchmark"),
                row("AAA", 1999.0, "Benchmark"),
                row("AAA", 2000.0, "Interpolated"),
                row("BBB", 2001.0, "Extrapolated"),
            ],
        )],
    );
    let pwt = read_pwt(&path).unwrap();
    assert_eq!(
        pwt.price_basis,
        [
            ("AAA".to_owned(), "Interpolated".to_owned()),
            ("USA".to_owned(), "Benchmark".to_owned()),
        ]
        .into()
    );
}

/// RFC 0282 D9: GDP at market rates over GDP per person at PPP times population, by year where
/// all three are reported.
#[test]
fn wdi_level_is_gdp_over_gdp_at_ppp() {
    let got = wdi_implied(
        &by_year(&[
            ("USA", &[(2000, 100.0), (2001, 110.0)]),
            ("AAA", &[(2000, 30.0)]),
            ("BBB", &[(2000, 5.0)]),
        ]),
        &by_year(&[
            ("USA", &[(2000, 50.0), (2001, 55.0)]),
            ("AAA", &[(2000, 20.0), (2001, 21.0)]),
            ("CCC", &[(2000, 1.0)]),
        ]),
        &by_year(&[
            ("USA", &[(2000, 2.0), (2001, 2.0)]),
            ("AAA", &[(2000, 3.0), (2001, 3.0)]),
            ("BBB", &[(2000, 1.0)]),
            ("CCC", &[(2000, 1.0)]),
        ]),
    );
    assert_eq!(
        got,
        by_year(&[
            ("USA", &[(2000, 1.0), (2001, 1.0)]),
            ("AAA", &[(2000, 0.5)])
        ])
    );
}

#[test]
fn levels_are_relative_to_the_united_states_in_the_same_year() {
    // PWT's levels are not based on the US (0.66 for it in 2000): each value is over the US's of
    // the same year, the year nearest 2000 within five.
    let got = relative(
        &by_year(&[
            ("USA", &[(2000, 122.0), (2003, 100.0)]),
            ("AAA", &[(2000, 61.0)]),
            ("BBB", &[(2003, 150.0), (2010, 1.0)]),
            ("CCC", &[(2010, 50.0)]),
        ]),
        "WDI",
    )
    .unwrap();
    assert_eq!(got, map(&[("USA", 1.0), ("AAA", 0.5), ("BBB", 1.5)]));
    let err = relative(&by_year(&[("AAA", &[(2000, 0.3)])]), "PWT").unwrap_err();
    assert!(err.contains("PWT"), "{err}");
}

#[test]
fn fences_are_tukeys_far_out_fences_on_the_logs() {
    // Logs 0, 1, 2, 3, 100: quartiles 1 and 3 by linear interpolation, IQR 2.
    let levels = [0.0, 1.0, 2.0, 3.0, 100.0].map(f64::exp);
    let (lo, hi) = fences(levels).unwrap();
    assert_eq!(FAR_OUT, 3.0);
    assert!(approx(lo.ln(), 1.0 - 3.0 * 2.0, 1e-12));
    assert!(approx(hi.ln(), 3.0 + 3.0 * 2.0, 1e-12));
    assert!(fences([]).is_err());
}

type Pairs = Vec<(String, f64)>;

const INTERCEPT: f64 = -2.7;
const SLOPE: f64 = 0.25;

/// The level at income `y` whose residual from the Penn line ln p = -2.7 + 0.25 ln y is `r`.
fn on_line(y: f64, r: f64) -> f64 {
    (INTERCEPT + SLOPE * y.ln() + r).exp()
}

/// PWT levels and incomes of R0..R7 (ln y = ln 1000 + i) at residuals +-0.1, which sum to zero
/// and are orthogonal to ln y, and of the US on the line at level 1: their OLS fit is the line.
fn reference() -> (Pairs, Pairs) {
    let r = [0.1, -0.1, -0.1, 0.1, 0.1, -0.1, -0.1, 0.1];
    let mut levels = vec![("USA".to_owned(), 1.0)];
    let mut income = vec![("USA".to_owned(), (-INTERCEPT / SLOPE).exp())];
    for (i, r) in r.iter().enumerate() {
        let y = 1000.0 * (i as f64).exp();
        levels.push((format!("R{i}"), on_line(y, *r)));
        income.push((format!("R{i}"), y));
    }
    (levels, income)
}

fn owned(v: &[(&str, f64)]) -> Pairs {
    v.iter().map(|(k, x)| ((*k).to_owned(), *x)).collect()
}

fn level_of(code: &str) -> f64 {
    reference()
        .0
        .into_iter()
        .find(|(c, _)| c == code)
        .unwrap()
        .1
}

#[test]
fn the_screen_fits_the_penn_effect_and_fences_its_residuals() {
    let (levels, income) = reference();
    let screen = Screen::fit(&levels.into_iter().collect(), &income.into_iter().collect()).unwrap();
    assert!(approx(screen.intercept, INTERCEPT, 1e-12));
    assert!(approx(screen.slope, SLOPE, 1e-12));
    assert_eq!(screen.fitted, 9);
    // Residual quartiles -0.1 and 0.1 (four each side of the US's zero): fences at +-0.7.
    assert!(approx(screen.residuals.0, -0.7, 1e-12));
    assert!(approx(screen.residuals.1, 0.7, 1e-12));
    assert!(screen.keeps(on_line(5e4, 0.69), Some(5e4)));
    assert!(!screen.keeps(on_line(5e4, 0.71), Some(5e4)));
    assert!(!screen.keeps(on_line(5e4, -0.71), Some(5e4)));
    // With no income, the level fences.
    assert!(screen.keeps(screen.levels.1, None));
    assert!(!screen.keeps(screen.levels.1 * 1.01, None));
}

fn resolve(
    codes: &[&str],
    pwt: &[(&str, f64)],
    wdi: &[(&str, f64)],
    income: &[(&str, f64)],
    donor: &[(&str, &str)],
    split: &[(&str, &[&str])],
) -> BTreeMap<String, Level> {
    let (levels, incomes) = reference();
    let pwt: BTreeMap<String, f64> = [levels, owned(pwt)].concat().into_iter().collect();
    let income: BTreeMap<String, f64> = [incomes, owned(income)].concat().into_iter().collect();
    let wdi = map(wdi);
    let parts = |c: &str| -> Vec<String> {
        split.iter().find(|(k, _)| *k == c).map_or_else(
            || vec![c.to_owned()],
            |(_, ps)| ps.iter().map(|p| (*p).to_owned()).collect(),
        )
    };
    let donor_of = |c: &str| {
        donor
            .iter()
            .find(|(k, _)| *k == c)
            .map(|(_, d)| (*d).to_owned())
    };
    let codes: Vec<String> = codes.iter().map(|c| (*c).to_owned()).collect();
    let screen = Screen::fit(&pwt, &income).unwrap();
    let got = price_levels(&codes, &parts, &donor_of, &pwt, &wdi, &income, &screen).unwrap();
    codes.into_iter().zip(got).collect()
}

fn level(value: f64, source: Source) -> Level {
    Level { value, source }
}

#[test]
fn pwt_first_then_wdi_then_the_donor() {
    let (aaa, bbb) = (on_line(5000.0, 0.3), on_line(2e4, 0.0));
    let got = resolve(
        &["USA", "AAA", "BBB", "CCC", "NOY"],
        &[("AAA", aaa)],
        &[("AAA", 0.9), ("BBB", bbb), ("NOY", 0.5)],
        &[("AAA", 5000.0), ("BBB", 2e4)],
        &[("CCC", "R2")],
        &[],
    );
    assert_eq!(got["USA"], level(1.0, Source::Pwt));
    assert_eq!(got["AAA"], level(aaa, Source::Pwt));
    assert_eq!(got["BBB"], level(bbb, Source::Wdi));
    assert_eq!(got["CCC"], level(level_of("R2"), Source::Donor));
    // No income: the level fences alone.
    assert_eq!(got["NOY"], level(0.5, Source::Wdi));
    assert_eq!(
        [Source::Pwt, Source::Wdi, Source::Donor].map(Source::as_str),
        ["pwt", "wdi", "donor"]
    );
}

#[test]
fn values_the_screen_rejects_fall_to_the_next_source() {
    // PWT 11 pl_gdpo had Bermuda at 1,059 times the US; the WDI archive file of RFC 0282 D9 had
    // Turkey at 3,712 times, East Timor at 0.003 and the US Virgin Islands, on the dollar, at
    // 0.117: inside the level fences but far below the level of their income.
    let bmu = on_line(3e4, 0.05);
    let got = resolve(
        &["BMU", "TUR", "TLS", "VIR"],
        &[("BMU", 1059.0)],
        &[
            ("BMU", bmu),
            ("TUR", 3712.0),
            ("TLS", 0.003),
            ("VIR", 0.117),
        ],
        &[("BMU", 3e4), ("TUR", 3000.0), ("TLS", 500.0), ("VIR", 3e4)],
        &[("TUR", "R4"), ("TLS", "R1"), ("VIR", "R7")],
        &[],
    );
    assert_eq!(got["BMU"], level(bmu, Source::Wdi));
    assert_eq!(got["TUR"], level(level_of("R4"), Source::Donor));
    assert_eq!(got["TLS"], level(level_of("R1"), Source::Donor));
    let (levels, _) = reference();
    let (lo, hi) = fences(levels.iter().map(|(_, p)| *p).chain([1059.0])).unwrap();
    assert!(lo < 0.117 && 0.117 < hi);
    assert_eq!(got["VIR"], level(level_of("R7"), Source::Donor));
}

#[test]
fn a_country_of_several_economies_takes_their_mean() {
    let (srb, mne) = (on_line(3000.0, 0.1), on_line(3000.0, -0.1));
    let got = resolve(
        &["YUG"],
        &[("SRB", srb), ("MNE", mne)],
        &[],
        &[("SRB", 3000.0), ("MNE", 3000.0)],
        &[],
        &[("YUG", &["SRB", "MNE"])],
    );
    assert!(approx(got["YUG"].value, (srb + mne) / 2.0, 1e-15));
    assert_eq!(got["YUG"].source, Source::Pwt);
}

#[test]
fn no_level_and_no_donor_is_an_error() {
    let (levels, income) = reference();
    let (pwt, income): (BTreeMap<_, _>, BTreeMap<_, _>) =
        (levels.into_iter().collect(), income.into_iter().collect());
    let screen = Screen::fit(&pwt, &income).unwrap();
    let codes = vec!["XXX".to_owned()];
    let parts = |c: &str| vec![c.to_owned()];
    let none = |_: &str| None;
    let err = price_levels(
        &codes,
        &parts,
        &none,
        &pwt,
        &BTreeMap::new(),
        &income,
        &screen,
    )
    .unwrap_err();
    assert!(err.contains("XXX"), "{err}");
}
