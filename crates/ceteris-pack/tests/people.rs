//! Labour inputs, donors, national scaling and pops of the people module, on hand-made series
//! and, when the cache and the pack are present, on the 2000 sources.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use arrow_array::cast::AsArray;
use arrow_array::types::{Float64Type, UInt8Type, UInt32Type};
use arrow_array::{Array, ArrayRef, RecordBatch};
use ceteris_pack::econ::people::{
    GiniSource, Labour, country_columns, country_labour, national_population, pops,
    province_columns, scale_to_national,
};
use ceteris_pack::raster::geotiff;
use ceteris_pack::raster::population::{
    BLOCK_ROWS, CellOwner, Grid, cell_of, pixel_centre, province_population,
};
use ceteris_pack::read::natural_earth::read_codes;
use ceteris_pack::read::people::{Observed, PARTS, load_observed};
use ceteris_pack::read::wdi::Names;
use common::{approx, cached, pack_table, strings};
use h3o::{CellIndex, LatLng, Resolution};

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| (*x).to_owned()).collect()
}

fn map<T: Copy>(v: &[(&str, T)]) -> BTreeMap<String, T> {
    v.iter().map(|(k, x)| ((*k).to_owned(), *x)).collect()
}

fn same(codes: &[&str]) -> Vec<(String, String)> {
    codes
        .iter()
        .map(|c| ((*c).to_owned(), (*c).to_owned()))
        .collect()
}

/// Eight economies in two regions: income, then labour variables for some of them.
fn observed() -> Observed {
    let income = [
        ("AAA", 1000.0),
        ("BBB", 2000.0),
        ("CCC", 9000.0),
        ("DDD", 10000.0),
        ("EEE", 1500.0),
        ("FFF", 1100.0),
    ];
    let region = ["R1", "R1", "R2", "R2", "R1", "R1"];
    let (low, mid) = ([0.7, 0.25, 0.05], [0.2, 0.5, 0.3]);
    Observed {
        participation: map(&[("AAA", 0.6), ("BBB", 0.7), ("CCC", 0.8), ("DDD", 0.75)]),
        unemployment: map(&[("AAA", 0.1), ("BBB", 0.2), ("CCC", 0.05), ("DDD", 0.06)]),
        natural_unemployment: map(&[("AAA", 0.08), ("CCC", 0.04)]),
        working_age: income.iter().map(|(k, _)| ((*k).to_owned(), 0.6)).collect(),
        edu_wic: map(&[("AAA", low), ("CCC", mid)]),
        edu_barro_lee: map(&[("BBB", [0.5, 0.4, 0.1])]),
        labour_share: map(&[("AAA", 0.5), ("BBB", 0.6), ("CCC", 0.65), ("DDD", 0.7)]),
        gini_wdi: map(&[("AAA", 0.3), ("CCC", 0.4)]),
        gini_swiid: map(&[("AAA", 0.9), ("BBB", 0.35)]),
        gdp_per_capita: map(&income),
        population: map(&[("AAA", 1.0), ("BBB", 3.0)]),
        population_wpp: BTreeMap::new(),
        region: income
            .iter()
            .zip(region)
            .map(|((k, _), r)| ((*k).to_owned(), r.to_owned()))
            .collect(),
    }
}

fn close3(a: [f64; 3], b: [f64; 3]) -> bool {
    a.iter().zip(&b).all(|(x, y)| approx(*x, *y, 1e-12))
}

#[test]
fn country_labour_prefers_data_then_fallback_source_then_donor() {
    let obs = observed();
    let out = country_labour(
        &same(&["AAA", "BBB", "CCC", "DDD", "EEE", "FFF"]),
        &obs,
        &Names::new(),
    )
    .unwrap();
    let (a, b) = (&out["AAA"], &out["BBB"]);
    // WDI wins over SWIID.
    assert_eq!(
        (a.gini, a.gini_source, a.gini_imputed),
        (0.3, GiniSource::Wdi, false)
    );
    assert_eq!(
        (b.gini, b.gini_source, b.gini_imputed),
        (0.35, GiniSource::Swiid, true)
    );
    // Barro-Lee fills a WIC gap.
    assert!(!b.edu_imputed && close3(b.edu_shares, [0.5, 0.4, 0.1]));
    // DDD has no Gini or education from any source: the donor is the same-region economy nearest in income.
    let d = &out["DDD"];
    assert_eq!(
        (d.gini, d.gini_source, d.gini_imputed),
        (0.4, GiniSource::Donor, true)
    );
    assert!(d.edu_imputed && close3(d.edu_shares, [0.2, 0.5, 0.3]));
    // EEE (1500) is nearer BBB (2000) than AAA (1000) in logs: 0.288 against 0.405.
    let e = &out["EEE"];
    assert!(e.participation_imputed && e.participation == 0.7);
    assert!(e.labour_share == 0.6 && e.labour_share_imputed);
    // Observed values are never flagged.
    assert!(
        !(a.participation_imputed
            || a.unemployment_imputed
            || a.labour_share_imputed
            || a.working_age_imputed)
    );
}

#[test]
fn natural_rate_comes_from_the_window_mean_else_the_donor() {
    let obs = observed();
    let out = country_labour(&same(&["AAA", "BBB", "DDD"]), &obs, &Names::new()).unwrap();
    assert_eq!(
        (out["AAA"].u_star, out["AAA"].u_star_imputed),
        (0.08, false)
    );
    // BBB (R1) takes AAA's, DDD (R2) takes CCC's: the donor rule of every series.
    assert_eq!((out["BBB"].u_star, out["BBB"].u_star_imputed), (0.08, true));
    assert_eq!((out["DDD"].u_star, out["DDD"].u_star_imputed), (0.04, true));
}

#[test]
fn country_labour_donor_uses_the_region_median_without_income() {
    let mut obs = observed();
    let z = &country_labour(&same(&["ZZZ"]), &obs, &Names::new()).unwrap()["ZZZ"];
    // No region and no income: the donor is the economy nearest the median log income of all donors.
    assert!(z.participation_imputed && [0.6, 0.7, 0.8, 0.75].contains(&z.participation));
    obs.region.insert("ZZZ".into(), "R2".into());
    let z = &country_labour(&same(&["ZZZ"]), &obs, &Names::new()).unwrap()["ZZZ"];
    // R2 donors only.
    assert!([0.8, 0.75].contains(&z.participation));
}

#[test]
fn country_labour_combines_split_economies_by_population_and_follows_the_sovereign_region() {
    let mut obs = observed();
    let countries = vec![
        ("YUG".to_owned(), "YUG".to_owned()),
        ("TER".to_owned(), "CCC".to_owned()),
    ];
    let out = country_labour(&countries, &obs, &Names::new()).unwrap();
    // YUG = SRB + MNE (PARTS): neither is in the series, both missing, so a donor; give them data and combine.
    obs.participation.extend(map(&[("SRB", 0.5), ("MNE", 0.9)]));
    obs.population.extend(map(&[("SRB", 3.0), ("MNE", 1.0)]));
    let combined = &country_labour(&same(&["YUG"]), &obs, &Names::new()).unwrap()["YUG"];
    assert!(approx(combined.participation, 0.6, 1e-12) && !combined.participation_imputed);
    // A territory with no region of its own takes its sovereign's region for the donor search.
    assert!([0.8, 0.75].contains(&out["TER"].participation));
}

fn f64s(a: &ArrayRef) -> Vec<f64> {
    a.as_primitive::<Float64Type>().values().to_vec()
}

fn bools(a: &ArrayRef) -> Vec<bool> {
    a.as_boolean().iter().map(Option::unwrap).collect()
}

fn named<'a>(cols: &'a [(String, ArrayRef)], name: &str) -> &'a ArrayRef {
    &cols.iter().find(|(n, _)| n == name).unwrap().1
}

#[test]
fn labour_columns_and_flags() {
    let obs = observed();
    let labour = country_labour(&same(&["AAA", "DDD"]), &obs, &Names::new()).unwrap();
    let cols = country_columns(&s(&["DDD", "AAA"]), &labour).unwrap();
    assert_eq!(f64s(named(&cols, "gini")), [0.4, 0.3]);
    assert_eq!(bools(named(&cols, "gini_imputed")), [true, false]);
    assert_eq!(bools(named(&cols, "edu_imputed")), [true, false]);
    assert_eq!(f64s(named(&cols, "u_star")), [0.04, 0.08]);
    assert_eq!(bools(named(&cols, "u_star_imputed")), [true, false]);
    let levels = ["edu_low", "edu_secondary", "edu_tertiary"].map(|k| f64s(named(&cols, k)));
    for i in 0..2 {
        assert!(approx(levels.iter().map(|l| l[i]).sum::<f64>(), 1.0, 1e-12));
    }
}

fn labour(shares: [f64; 3], participation: f64, working_age: f64) -> Labour {
    Labour {
        participation,
        unemployment: 0.1,
        u_star: 0.1,
        edu_shares: shares,
        labour_share: 0.5,
        working_age_share: working_age,
        gini: 0.3,
        gini_source: GiniSource::Wdi,
        participation_imputed: false,
        unemployment_imputed: false,
        u_star_imputed: false,
        edu_imputed: false,
        labour_share_imputed: false,
        working_age_imputed: false,
        gini_imputed: false,
    }
}

fn column<'a>(t: &'a RecordBatch, name: &str) -> &'a ArrayRef {
    t.column_by_name(name).unwrap()
}

#[test]
fn pops_split_each_province_by_its_countrys_education_shares() {
    let labour = BTreeMap::from([
        ("AAA".to_owned(), labour([0.5, 0.3, 0.2], 0.5, 0.6)),
        (
            "BBB".to_owned(),
            Labour {
                edu_imputed: true,
                ..labour([0.1, 0.1, 0.8], 0.8, 0.6)
            },
        ),
    ]);
    let population = [1000.0, 200.0, 50.0];
    let t = pops(&s(&["AAA", "AAA", "BBB"]), &population, &labour).unwrap();
    assert_eq!(t.num_rows(), 9);
    let province: Vec<u32> = column(&t, "province")
        .as_primitive::<UInt32Type>()
        .values()
        .to_vec();
    assert_eq!(province, [0, 0, 0, 1, 1, 1, 2, 2, 2]);
    let edu: Vec<u8> = column(&t, "edu")
        .as_primitive::<UInt8Type>()
        .values()
        .to_vec();
    assert_eq!(edu, [0, 1, 2].repeat(3));
    let pop = f64s(column(&t, "population"));
    let wa = f64s(column(&t, "working_age"));
    let lf = f64s(column(&t, "labour_force"));
    for (i, p) in population.iter().enumerate() {
        let row = |v: &[f64]| v[3 * i..3 * i + 3].iter().sum::<f64>();
        assert!(approx(row(&pop), *p, 1e-12));
        assert!(approx(row(&wa), p * 0.6, 1e-12));
    }
    assert!(close3(pop[..3].try_into().unwrap(), [500.0, 300.0, 200.0]));
    for j in 0..3 {
        assert!(approx(lf[6 + j], wa[6 + j] * 0.8, 1e-12));
        assert!(approx(lf[j], wa[j] * 0.5, 1e-12));
    }
    let imputed = bools(column(&t, "imputed"));
    assert_eq!(imputed, [[false; 6].as_slice(), &[true; 3]].concat());
}

#[test]
fn province_columns_scale_by_working_age_share() {
    let labour = BTreeMap::from([
        ("AAA".to_owned(), labour([1.0, 0.0, 0.0], 0.5, 0.5)),
        (
            "BBB".to_owned(),
            Labour {
                working_age_imputed: true,
                ..labour([1.0, 0.0, 0.0], 0.5, 0.7)
            },
        ),
    ]);
    let cols = province_columns(&s(&["AAA", "BBB"]), &[10.0, 10.0], &labour).unwrap();
    assert_eq!(f64s(named(&cols, "working_age")), [5.0, 7.0]);
    assert_eq!(bools(named(&cols, "working_age_imputed")), [false, true]);
}

#[test]
fn scale_to_national_keeps_shares_and_leaves_unknown_countries_alone() {
    let population = [10.0, 30.0, 5.0, 0.0, 0.0, 7.0];
    let country = s(&["AAA", "AAA", "BBB", "CCC", "CCC", "DDD"]);
    let out = scale_to_national(&population, &country, &map(&[("AAA", 80.0), ("CCC", 50.0)]));
    // BBB has no total, CCC no people, DDD no total.
    assert_eq!(out, [20.0, 60.0, 5.0, 0.0, 0.0, 7.0]);
}

#[test]
fn national_population_sums_split_economies_and_drops_unknown_ones() {
    let obs = Observed {
        population: map(&[("SRB", 7.0), ("MNE", 0.6), ("FRA", 59.0)]),
        ..Observed::default()
    };
    let iso: Names = map(&[("FRA", "FRA")])
        .into_iter()
        .map(|(k, v)| (k, v.to_owned()))
        .collect();
    let out = national_population(&s(&["YUG", "FRA", "CLP"]), &obs, &iso);
    assert_eq!(out.len(), 2);
    assert!(approx(out["YUG"], 7.6, 1e-12) && out["FRA"] == 59.0);
}

#[test]
fn national_population_falls_back_to_wpp() {
    let obs = Observed {
        population: map(&[("AAA", 1.0)]),
        population_wpp: map(&[("CCC", 4.0), ("AAA", 9.0)]),
        ..Observed::default()
    };
    let totals = national_population(&s(&["AAA", "CCC", "DDD"]), &obs, &Names::new());
    // WDI's AAA wins over WPP's; CCC has WPP only; DDD has neither.
    assert_eq!(totals, map(&[("AAA", 1.0), ("CCC", 4.0)]));
}

/// A float32 band in memory with WorldPop's nodata, the largest float32 (which is positive).
struct Pixels {
    width: usize,
    transform: [f64; 6],
    values: Vec<f32>,
}

impl Grid for Pixels {
    type Value = f32;
    fn width(&self) -> usize {
        self.width
    }
    fn height(&self) -> usize {
        self.values.len() / self.width
    }
    fn transform(&self) -> [f64; 6] {
        self.transform
    }
    fn nodata(&self) -> Option<f32> {
        Some(f32::MAX)
    }
    fn rows(&mut self, row0: usize, rows: usize) -> Result<Vec<f32>, String> {
        Ok(self.values[row0 * self.width..(row0 + rows) * self.width].to_vec())
    }
}

/// 40 by 40 pixels of 0.05 degrees from (10 E, 50 N), all ones but `holes` (row, col, value).
fn grid(holes: &[(usize, usize, f32)]) -> Pixels {
    let mut values = vec![1.0f32; 1600];
    for &(r, c, v) in holes {
        values[r * 40 + c] = v;
    }
    Pixels {
        width: 40,
        transform: [0.05, 0.0, 10.0, 0.0, -0.05, 50.0],
        values,
    }
}

/// The centres of the counted pixels and their cells at `res`.
fn centres(g: &Pixels, res: u8) -> Vec<((f64, f64), u64)> {
    let res = Resolution::try_from(res).unwrap();
    (0..g.values.len())
        .filter(|&i| g.values[i] > 0.0 && g.values[i] != f32::MAX)
        .map(|i| {
            let (lat, lng) = pixel_centre(&g.transform, i / 40, i % 40);
            ((lat, lng), u64::from(cell_of(lat, lng, res).unwrap()))
        })
        .collect()
}

#[test]
fn province_population_sums_pixels_by_cell_owner() {
    let mut g = grid(&[(0, 0, f32::MAX), (5, 5, 0.0)]);
    let pixels = centres(&g, 4);
    let cells: Vec<u64> = pixels
        .iter()
        .map(|(_, c)| *c)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    assert!(cells.len() >= 4);
    let owner_of = |c: u64| (cells.iter().position(|x| *x == c).unwrap() % 2) as u32;
    let mut owner = CellOwner::new(cells.iter().map(|&c| (c, owner_of(c)))).unwrap();
    let got = province_population(&mut g, &mut owner, BLOCK_ROWS).unwrap();
    let mut expected = [0.0; 2];
    for (_, c) in &pixels {
        expected[owner_of(*c) as usize] += 1.0;
    }
    assert_eq!(got, expected);
    assert_eq!(got.iter().sum::<f64>(), (1600 - 2) as f64);
    assert_eq!(pixels.len(), 1600 - 2);
}

#[test]
fn province_population_sends_an_unowned_cell_to_the_nearest_owned_one() {
    let mut g = grid(&[]);
    let pixels = centres(&g, 4);
    let cells: BTreeSet<u64> = pixels.iter().map(|(_, c)| *c).collect();
    let drop = *cells.first().unwrap();
    let at = |c: u64| LatLng::from(CellIndex::try_from(c).unwrap());
    let near = cells
        .iter()
        .copied()
        .filter(|&c| c != drop)
        .min_by(|a, b| {
            at(*a)
                .distance_rads(at(drop))
                .total_cmp(&at(*b).distance_rads(at(drop)))
        })
        .unwrap();
    let kept = cells.iter().copied().filter(|&c| c != drop);
    let mut owner = CellOwner::new(kept.map(|c| (c, u32::from(c == near)))).unwrap();
    let got = province_population(&mut g, &mut owner, BLOCK_ROWS).unwrap();
    assert_eq!(got.iter().sum::<f64>(), 1600.0);
    let count = |cell: u64| pixels.iter().filter(|(_, c)| *c == cell).count() as f64;
    assert_eq!(got[1], count(near) + count(drop));
}

#[test]
fn province_population_prefers_the_finest_owned_cell() {
    let mut g = grid(&[]);
    let coarse: BTreeSet<u64> = centres(&g, 4).into_iter().map(|(_, c)| c).collect();
    let fine_pixels = centres(&g, 6);
    let fine = fine_pixels[100].1;
    let pairs = coarse.iter().map(|&c| (c, 0)).chain([(fine, 1)]);
    let mut owner = CellOwner::new(pairs).unwrap();
    let got = province_population(&mut g, &mut owner, BLOCK_ROWS).unwrap();
    let in_fine = fine_pixels.iter().filter(|(_, c)| *c == fine).count() as f64;
    assert!(in_fine > 0.0);
    assert_eq!(got[1], in_fine);
    assert_eq!(got.iter().sum::<f64>(), 1600.0);
}

// Tests below need the downloaded sources and the 2000 pack; CI is data-free.

const NAMES: [&str; 11] = [
    "BL_v3_MF1564.csv",
    "BPJresult_AGE_V01.csv",
    "ilostat_EAP_2WAP_SEX_AGE_RT_A_2000.csv",
    "ilostat_UNE_2EAP_SEX_AGE_RT_A_2000.csv",
    "ilostat_UNE_2EAP_SEX_AGE_RT_A_1995-2005.csv",
    "pwt110.xlsx",
    "wdi_SI.POV.GINI.zip",
    "wdi_SP.POP.TOTL.zip",
    "wdi_SP.POP.1564.TO.ZS.zip",
    "swiid9_92.zip",
    "ne_10m_admin_0_countries.zip",
];

struct Data {
    obs: Observed,
    iso: Names,
    labour: BTreeMap<String, Labour>,
    country: Vec<String>,
    pack_countries: usize,
}

/// The observed series, labour inputs and province countries of the 2000 sources and pack.
fn data() -> Option<&'static Data> {
    static DATA: OnceLock<Option<Data>> = OnceLock::new();
    DATA.get_or_init(|| {
        if NAMES.iter().any(|n| cached(n).is_none()) {
            return None;
        }
        let (countries, provinces) = (
            pack_table("countries.arrow")?,
            pack_table("provinces.arrow")?,
        );
        let admin0 = cached("ne_10m_admin_0_countries.zip").unwrap();
        let obs = load_observed(admin0.parent().unwrap(), &admin0).unwrap();
        let (iso, _) = read_codes(&admin0).unwrap();
        let pairs: Vec<(String, String)> = strings(&countries, "code")
            .into_iter()
            .zip(strings(&countries, "sovereign"))
            .collect();
        let labour = country_labour(&pairs, &obs, &iso).unwrap();
        Some(Data {
            obs,
            iso,
            labour,
            pack_countries: pairs.len(),
            country: strings(&provinces, "country"),
        })
    })
    .as_ref()
}

/// Raw WorldPop 2000 people per pack province, in optimised builds only.
fn worldpop() -> Option<&'static Vec<f64>> {
    static RAW: OnceLock<Option<Vec<f64>>> = OnceLock::new();
    RAW.get_or_init(|| {
        if cfg!(debug_assertions) {
            eprintln!(
                "skipping: summing WorldPop takes over 10 minutes unoptimised; run with --release"
            );
            return None;
        }
        let path = cached("ppp_2000_1km_Aggregated.tif")?;
        let mut owner = CellOwner::read(&pack_table("cells.arrow")?).unwrap();
        let mut raster = geotiff::open(&path).unwrap();
        Some(province_population(&mut raster, &mut owner, BLOCK_ROWS).unwrap())
    })
    .as_ref()
}

#[test]
fn every_pack_country_has_labour_inputs() {
    let Some(d) = data() else { return };
    assert_eq!(d.labour.len(), d.pack_countries);
    assert_eq!(d.pack_countries, 242);
}

#[test]
fn edu_shares_sum_to_one() {
    let Some(d) = data() else { return };
    assert_eq!(d.labour.len(), 242);
    for l in d.labour.values() {
        assert!(l.edu_shares.iter().all(|x| *x >= 0.0));
        assert!((l.edu_shares.iter().sum::<f64>() - 1.0).abs() <= 1e-12);
    }
}

type Field = fn(&Labour) -> f64;

#[test]
fn labour_share_in_unit_interval() {
    let Some(d) = data() else { return };
    let fields: [(&str, Field); 5] = [
        ("labour_share", |l| l.labour_share),
        ("participation", |l| l.participation),
        ("unemployment", |l| l.unemployment),
        ("u_star", |l| l.u_star),
        ("working_age_share", |l| l.working_age_share),
    ];
    for (name, f) in fields {
        assert!(
            d.labour.values().all(|l| f(l) > 0.0 && f(l) < 1.0),
            "{name}"
        );
    }
    let mut shares: Vec<f64> = d.labour.values().map(|l| l.labour_share).collect();
    shares.sort_by(f64::total_cmp);
    let n = shares.len();
    let median = if n % 2 == 1 {
        shares[n / 2]
    } else {
        (shares[n / 2 - 1] + shares[n / 2]) / 2.0
    };
    assert!(0.3 < median && median < 0.8);
}

#[test]
fn u_star_covers_every_pack_country() {
    // ADR 0089: u* is the ILOSTAT modelled mean of 1995 to 2005; a country without it takes its
    // donor's, flagged, and only such countries are flagged.
    let Some(d) = data() else { return };
    let have = &d.obs.natural_unemployment;
    assert!(have.len() > 180);
    for (code, l) in &d.labour {
        let parts: Vec<String> = match PARTS.iter().find(|(c, _)| c == code) {
            Some((_, ps)) => ps.iter().map(|p| (*p).to_owned()).collect(),
            None => vec![d.iso.get(code).unwrap_or(code).clone()],
        };
        assert_eq!(
            l.u_star_imputed,
            !parts.iter().any(|p| have.contains_key(p)),
            "{code}"
        );
    }
    assert!(d.labour.values().filter(|l| !l.u_star_imputed).count() > 150);
}

#[test]
fn gini_in_unit_interval_with_source_flag() {
    let Some(d) = data() else { return };
    for l in d.labour.values() {
        assert!(l.gini > 0.15 && l.gini < 0.8);
        assert_eq!(l.gini_imputed, l.gini_source != GiniSource::Wdi);
    }
    // Most countries have a WDI Gini within 1995 to 2005 or a SWIID value.
    let data_backed = d
        .labour
        .values()
        .filter(|l| l.gini_source != GiniSource::Donor)
        .count();
    assert!(data_backed > 150);
}

#[test]
fn data_shares_follow_the_gini_rule() {
    let Some(d) = data() else { return };
    let (usa, bra) = (&d.labour["USA"], &d.labour["BRA"]);
    assert_eq!(usa.gini_source, GiniSource::Wdi);
    assert!(approx(usa.gini, d.obs.gini_wdi["USA"], 1e-12));
    assert!(0.35 < usa.gini && usa.gini < 0.45 && 0.5 < bra.gini && bra.gini < 0.65);
}

fn by_country(country: &[String], population: &[f64]) -> BTreeMap<String, f64> {
    let mut out = BTreeMap::new();
    for (c, p) in country.iter().zip(population) {
        *out.entry(c.clone()).or_insert(0.0) += p;
    }
    out
}

#[test]
fn worldpop_totals_follow_the_united_nations_within_the_known_source_gap() {
    // Raw WorldPop 2000 is not UN-adjusted: large countries differ from WDI by -12 to +38%, so
    // only gross errors fail here.
    let (Some(d), Some(raw)) = (data(), worldpop()) else {
        return;
    };
    let got = by_country(&d.country, raw);
    let codes: Vec<String> = got.keys().cloned().collect();
    let expected = national_population(&codes, &d.obs, &d.iso);
    let big: Vec<&String> = expected
        .iter()
        .filter(|(_, e)| **e >= 10e6)
        .map(|(c, _)| c)
        .collect();
    assert!(big.len() >= 50);
    for code in big {
        let ratio = got[code] / expected[code];
        assert!(1.0 / 1.5 < ratio && ratio < 1.5, "{code}");
    }
    assert!(approx(raw.iter().sum(), expected.values().sum(), 0.05));
}

#[test]
fn population_sums_match_country_totals_within_2pct() {
    let (Some(d), Some(raw)) = (data(), worldpop()) else {
        return;
    };
    let codes: Vec<String> = d
        .country
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let expected = national_population(&codes, &d.obs, &d.iso);
    let scaled = scale_to_national(raw, &d.country, &expected);
    let got = by_country(&d.country, &scaled);
    let big: Vec<&String> = expected
        .iter()
        .filter(|(_, e)| **e >= 10e6)
        .map(|(c, _)| c)
        .collect();
    assert!(big.len() >= 50);
    for code in big {
        assert!(approx(got[code], expected[code], 0.02), "{code}");
    }
    // Provinces keep their shares of the country.
    let raw_by = by_country(&d.country, raw);
    let first = d.country.iter().position(|c| c == "FRA").unwrap();
    assert!(approx(
        scaled[first] / got["FRA"],
        raw[first] / raw_by["FRA"],
        1e-12
    ));
}

#[test]
fn pops_from_the_pack_cover_every_province() {
    let (Some(d), Some(raw)) = (data(), worldpop()) else {
        return;
    };
    let t = pops(&d.country, raw, &d.labour).unwrap();
    assert_eq!(t.num_rows(), d.country.len() * 3);
    let total = |name: &str| f64s(column(&t, name)).iter().sum::<f64>();
    let people = raw.iter().sum::<f64>();
    assert!(approx(total("population"), people, 1e-9));
    assert!(total("labour_force") < total("working_age") && total("working_age") < people);
    assert_eq!(column(&t, "imputed").null_count(), 0);
}
