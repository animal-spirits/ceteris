//! Province VA downscaling on a hand-made world and, when the pack and cache are present, on the
//! 2000 sources.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::sync::OnceLock;

use ceteris_pack::econ::build::{ADMIN0, WORLDPOP, read_geography};
use ceteris_pack::econ::downscale::{
    DoseMatch, Downscaled, GEM, Geography, KUMMU, LEVELS, Level, MANUFACTURING, MIN_WORKING_AGE,
    Provinces, Proxies, allocate, coverage, eia_oil_gas, gridded_gdp, load_proxies, match_dose,
    mineral_sites, point_sums, report,
};
use ceteris_pack::econ::icio::{Basis, Economy, IncomeSource, impute_pack};
use ceteris_pack::econ::people::{country_labour, national_population, scale_to_national};
use ceteris_pack::raster::geotiff;
use ceteris_pack::raster::population::{BLOCK_ROWS, CellOwner, cell_of, province_population};
use ceteris_pack::read::downscale::{Asset, Dose};
use ceteris_pack::read::icio::{CountryIo, N_GOODS, aggregate, load_wdi};
use ceteris_pack::read::natural_earth::read_codes;
use ceteris_pack::read::people::load_observed;
use common::{Uniform, approx, cached, pack_table, strings};
use h3o::Resolution;

// Hand-made world: AAA has four provinces (the second below 1,000 working-age people), BBB three,
// CCC one tiny one.
const COUNTRY: [&str; 8] = ["AAA", "AAA", "AAA", "AAA", "BBB", "BBB", "BBB", "CCC"];
const WORKING_AGE: [f64; 8] = [
    5000.0, 500.0, 20000.0, 8000.0, 3000.0, 4000.0, 1500.0, 300.0,
];
const AAA: [usize; 4] = [0, 1, 2, 3];
const BBB: [usize; 3] = [4, 5, 6];
const AAA_ELIGIBLE: [usize; 3] = [0, 2, 3];

fn population() -> Vec<f64> {
    WORKING_AGE.iter().map(|w| w * 1.5).collect()
}

fn io(va: [f64; N_GOODS], basis: Basis) -> Economy {
    Economy {
        table: CountryIo {
            output: va.map(|v| v * 2.0),
            va,
            io: [[0.0; N_GOODS]; N_GOODS],
            final_demand_shares: [1.0 / N_GOODS as f64; N_GOODS],
            coal_share: 0.4,
            extraction: [0.0; N_GOODS],
            extraction_output: 0.0,
        },
        donor: None,
        va_basis: basis,
        region: None,
        income_source: IncomeSource::None,
        shed: 0.0,
    }
}

fn world() -> BTreeMap<String, Economy> {
    let mut rng = Uniform::new(3);
    let mut draw = |lo: f64, hi: f64| std::array::from_fn(|_| rng.next(lo, hi));
    [
        ("AAA", io(draw(10.0, 1e6), Basis::Icio)),
        ("BBB", io(draw(10.0, 1e4), Basis::Wdi)),
        ("CCC", io(draw(1.0, 5.0), Basis::Icio)),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect()
}

fn provinces() -> Provinces {
    Provinces {
        country: COUNTRY.map(str::to_owned).to_vec(),
        population: population(),
        working_age: WORKING_AGE.to_vec(),
    }
}

fn proxies() -> Proxies {
    let mut rng = Uniform::new(7);
    let crops = (0..8).map(|_| rng.next(0.0, 10.0)).collect();
    let gdp = (0..8).map(|_| rng.next(1.0, 10.0)).collect();
    let nan = f64::NAN;
    Proxies {
        crops: Some(crops),
        oil_gas: Some(vec![0.0, 3.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0]),
        coal: Some(vec![2.0, 0.0, 0.0, 6.0, 0.0, 1.0, 1.0, 0.0]),
        sites: Some(vec![1.0, 5.0, 3.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
        dose: Some(vec![4.0, nan, 2.0, 2.0, nan, nan, nan, nan]),
        dose_rest: [("AAA".to_owned(), 0.0)].into(),
        gdp: Some(gdp),
        ..Proxies::default()
    }
}

fn place(p: &Proxies) -> Downscaled {
    allocate(&world(), &provinces(), p).unwrap()
}

fn shares(x: &[f64], idx: &[usize]) -> Vec<f64> {
    let total: f64 = idx.iter().map(|&i| x[i]).sum();
    idx.iter().map(|&i| x[i] / total).collect()
}

fn column(out: &Downscaled, g: usize) -> Vec<f64> {
    out.va.iter().map(|v| v[g]).collect()
}

fn assert_close(got: &[f64], want: &[f64], rel: f64) {
    assert_eq!(got.len(), want.len());
    for (a, b) in got.iter().zip(want) {
        assert!(approx(*a, *b, rel), "{got:?} vs {want:?}");
    }
}

fn levels(out: &Downscaled, idx: &[usize], g: usize) -> BTreeSet<Level> {
    idx.iter().map(|&i| out.level[i][g]).collect()
}

#[test]
fn downscaling_preserves_national_va() {
    let world = world();
    let mut bare = proxies();
    (bare.coal, bare.dose, bare.gdp, bare.oil_gas) = (None, None, None, None);
    for p in [proxies(), Proxies::default(), bare] {
        let out = allocate(&world, &provinces(), &p).unwrap();
        assert_eq!((out.va.len(), out.imputed.len()), (8, 8));
        assert!(out.va.iter().flatten().all(|v| *v >= 0.0));
        for (code, idx) in [("AAA", &AAA[..]), ("BBB", &BBB[..]), ("CCC", &[7][..])] {
            for g in 0..N_GOODS {
                let sum: f64 = idx.iter().map(|&i| out.va[i][g]).sum();
                let want = world[code].table.va[g];
                assert!((sum - want).abs() <= 1e-9 + 1e-9 * want.abs(), "{code} {g}");
            }
        }
    }
}

#[test]
fn empty_provinces_get_no_va() {
    let out = place(&proxies());
    assert!(out.va[1].iter().all(|v| *v == 0.0));
    // Its share goes to the other provinces by the same proxy: crops for agriculture, population
    // for services.
    let crops = proxies().crops.unwrap();
    assert_close(
        &shares(&column(&out, 0), &AAA_ELIGIBLE),
        &shares(&crops, &AAA_ELIGIBLE),
        1e-12,
    );
    assert_close(
        &shares(&column(&out, 9), &AAA_ELIGIBLE),
        &shares(&population(), &AAA_ELIGIBLE),
        1e-12,
    );
}

#[test]
fn a_country_without_any_eligible_province_puts_its_va_in_its_largest() {
    let world = world();
    let out = allocate(&world, &provinces(), &proxies()).unwrap();
    assert_close(&out.va[7], &world["CCC"].table.va, 1e-12);
    assert!(out.imputed[7].iter().all(|x| *x));
    assert!(out.level[7].iter().all(|l| *l == Level::Largest));
}

#[test]
fn proxy_levels_follow_the_spec_order() {
    let out = place(&proxies());
    let one = |l: Level| BTreeSet::from([l]);
    assert_eq!(levels(&out, &AAA_ELIGIBLE, 0), one(Level::Crops));
    // Province 1 holds oil but is too small.
    assert_eq!(levels(&out, &AAA_ELIGIBLE, 1), one(Level::Gem));
    assert_eq!(levels(&out, &AAA_ELIGIBLE, 3), one(Level::Dose));
    assert_eq!(levels(&out, &AAA_ELIGIBLE, 7), one(Level::Population));
    assert_eq!(levels(&out, &AAA_ELIGIBLE, 9), one(Level::Population));
    // BBB has no oil fields and no DOSE regions: only manufacturing falls back to gridded GDP;
    // oil and gas go straight to population.
    assert_eq!(levels(&out, &BBB, 4), one(Level::Gdp));
    assert_eq!(levels(&out, &BBB, 1), one(Level::Population));
    assert_eq!(levels(&out, &BBB, 2), one(Level::Mining));
}

#[test]
fn fallbacks_are_flagged_imputed_and_data_levels_are_not() {
    let out = place(&proxies());
    // ICIO country, every good from its first proxy.
    assert!(
        AAA_ELIGIBLE
            .iter()
            .all(|&i| out.imputed[i].iter().all(|x| !x))
    );
    // National VA is imputed (wdi basis).
    assert!(BBB.iter().all(|&i| out.imputed[i].iter().all(|x| *x)));
    let mut world = world();
    world.insert("BBB".into(), io(world["BBB"].table.va, Basis::Icio));
    let out = allocate(&world, &provinces(), &proxies()).unwrap();
    // Population stands in for GEM, gridded GDP for DOSE.
    for &i in &BBB {
        assert!([1, 3, 4, 5, 6].iter().all(|&g| out.imputed[i][g]));
        assert!([0, 2, 7, 8, 9].iter().all(|&g| !out.imputed[i][g]));
    }
}

#[test]
fn without_crops_agriculture_follows_population() {
    let out = place(&Proxies {
        crops: Some(vec![0.0; 8]),
        ..proxies()
    });
    assert_close(
        &shares(&column(&out, 0), &AAA_ELIGIBLE),
        &shares(&population(), &AAA_ELIGIBLE),
        1e-12,
    );
    assert!(AAA_ELIGIBLE.iter().all(|&i| out.imputed[i][0]));
}

#[test]
fn oil_gas_and_mining_fall_back_to_population_not_gridded_gdp() {
    let out = place(&Proxies {
        oil_gas: None,
        coal: None,
        sites: None,
        ..proxies()
    });
    for g in [1, 2] {
        assert_close(
            &shares(&column(&out, g), &AAA_ELIGIBLE),
            &shares(&population(), &AAA_ELIGIBLE),
            1e-12,
        );
        assert!(AAA_ELIGIBLE.iter().all(|&i| out.imputed[i][g]));
    }
}

#[test]
fn eia_state_production_comes_before_gem() {
    let out = place(&Proxies {
        eia: Some(vec![0.0, 0.0, 1.0, 3.0, 0.0, 0.0, 0.0, 0.0]),
        ..proxies()
    });
    let total: f64 = AAA.iter().map(|&i| out.va[i][1]).sum();
    let got: Vec<f64> = AAA_ELIGIBLE.iter().map(|&i| out.va[i][1] / total).collect();
    assert_close(&got, &[0.0, 0.25, 0.75], 1e-12);
    assert_eq!(levels(&out, &AAA_ELIGIBLE, 1), BTreeSet::from([Level::Eia]));
    assert!(AAA_ELIGIBLE.iter().all(|&i| !out.imputed[i][1]));
    // BBB has no EIA data: its own chain (no fields there, so population).
    assert_eq!(levels(&out, &BBB, 1), BTreeSet::from([Level::Population]));
}

#[test]
fn gem_oil_and_gas_is_imputed_where_undated_fields_carry_most_weight() {
    // AAA eligible weight: 1 (province 3).
    let out = place(&Proxies {
        oil_gas_undated: Some(vec![0.0, 3.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.0]),
        ..proxies()
    });
    assert!(AAA_ELIGIBLE.iter().all(|&i| !out.imputed[i][1]));
    let oil = proxies().oil_gas.unwrap();
    let out = place(&Proxies {
        oil_gas_undated: Some(oil.iter().map(|x| x * 0.6).collect()),
        ..proxies()
    });
    assert!(AAA_ELIGIBLE.iter().all(|&i| out.imputed[i][1]));
    assert_eq!(levels(&out, &AAA_ELIGIBLE, 1), BTreeSet::from([Level::Gem]));
}

#[test]
fn mining_weights_coal_mines_and_mineral_sites_by_the_coal_share() {
    let p = proxies();
    let out = place(&p);
    let (coal, sites) = (
        shares(p.coal.as_ref().unwrap(), &AAA_ELIGIBLE),
        shares(p.sites.as_ref().unwrap(), &AAA_ELIGIBLE),
    );
    let want: Vec<f64> = coal
        .iter()
        .zip(&sites)
        .map(|(c, s)| 0.4 * c + 0.6 * s)
        .collect();
    assert_close(&shares(&column(&out, 2), &AAA_ELIGIBLE), &want, 1e-12);
    assert!(AAA_ELIGIBLE.iter().all(|&i| !out.imputed[i][2]));
}

#[test]
fn without_gem_mining_follows_mineral_sites_flagged_imputed() {
    let p = Proxies {
        coal: None,
        ..proxies()
    };
    let out = place(&p);
    assert_close(
        &shares(&column(&out, 2), &AAA_ELIGIBLE),
        &shares(p.sites.as_ref().unwrap(), &AAA_ELIGIBLE),
        1e-12,
    );
    assert!(AAA_ELIGIBLE.iter().all(|&i| out.imputed[i][2]));
    assert_eq!(
        levels(&out, &AAA_ELIGIBLE, 2),
        BTreeSet::from([Level::Sites])
    );
}

#[test]
fn dose_regions_without_a_province_leave_their_share_to_the_unmatched_by_gridded_gdp() {
    let nan = f64::NAN;
    let out = place(&Proxies {
        dose: Some(vec![6.0, nan, 2.0, nan, nan, nan, nan, nan]),
        dose_rest: [("AAA".to_owned(), 0.25)].into(),
        ..proxies()
    });
    let total: f64 = AAA.iter().map(|&i| out.va[i][3]).sum();
    // Matched provinces 0 and 2 split 75% by DOSE (6:2); unmatched province 3 takes the other
    // 25% alone.
    let got: Vec<f64> = [0, 2, 3].iter().map(|&i| out.va[i][3] / total).collect();
    assert_close(&got, &[0.5625, 0.1875, 0.25], 1e-12);
    assert!(!out.imputed[0][3] && !out.imputed[2][3] && out.imputed[3][3]);
    assert_eq!(out.level[3][3], Level::DoseRest);
}

#[test]
fn local_goods_split_by_population() {
    let out = place(&proxies());
    for g in [7, 8, 9] {
        assert_close(
            &shares(&column(&out, g), &BBB),
            &shares(&population(), &BBB),
            1e-12,
        );
    }
}

#[test]
fn coverage_shares_sum_to_one_per_good() {
    let out = place(&proxies());
    let cover = coverage(&out);
    for shares in &cover {
        let total: f64 = shares.iter().map(|(_, v)| v).sum();
        assert!((total - 1.0).abs() <= 1e-12);
    }
    let services = column(&out, 9);
    let largest = services[7] / services.iter().sum::<f64>();
    let got: BTreeMap<Level, f64> = cover[9].iter().copied().collect();
    assert_eq!(
        got.keys().copied().collect::<Vec<_>>(),
        [Level::Population, Level::Largest]
    );
    assert!(approx(got[&Level::Population], 1.0 - largest, 1e-6));
    assert!(approx(got[&Level::Largest], largest, 1e-6));
}

#[test]
fn level_values_are_the_python_levels_order() {
    let names: Vec<&str> = LEVELS.iter().map(|l| l.name()).collect();
    assert_eq!(
        names,
        [
            "crops",
            "gem",
            "mining",
            "sites",
            "dose",
            "dose_rest",
            "gdp",
            "population",
            "largest",
            "eia"
        ]
    );
    assert!(LEVELS.iter().enumerate().all(|(k, l)| *l as usize == k));
}

// DOSE

fn dose(values: &[(&str, f64)], coverage: f64) -> Dose {
    Dose {
        values: values.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect(),
        coverage,
    }
}

fn owned(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|x| (*x).to_owned()).collect()
}

fn names(pairs: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), owned(v)))
        .collect()
}

#[test]
fn dose_match_rate_reported() {
    let mut regions: BTreeMap<String, Dose> = [
        (
            "AAA".to_owned(),
            dose(
                &[
                    ("Rio de Janeiro", 30.0),
                    ("Nowhere", 10.0),
                    ("Ostland", 60.0),
                ],
                1.0,
            ),
        ),
        ("BBB".to_owned(), dose(&[("X", 1.0)], 1.0)),
    ]
    .into();
    // Provinces: two parts of one Natural Earth region (".1", ".2"), one matched by an alternate
    // name.
    let ids = owned(&["AAA-1.1", "AAA-1.2", "AAA-2", "AAA-3", "CCC-1"]);
    let names = names(&[
        ("AAA-1", &["Rio De Janeiro"]),
        ("AAA-2", &["Westland", "Ostland"]),
        ("AAA-3", &["Southland"]),
        ("CCC-1", &["X"]),
    ]);
    let country = owned(&["AAA", "AAA", "AAA", "AAA", "CCC"]);
    let weight = [1.0, 3.0, 1.0, 1.0, 1.0];
    let iso = BTreeMap::new();
    let (proxy, rest, matched) = match_dose(&regions, &ids, &country, &names, &weight, &iso);
    assert_close(&proxy[..3], &[7.5, 22.5, 60.0], 1e-12);
    assert!(proxy[3].is_nan() && proxy[4].is_nan());
    assert_eq!(rest.keys().collect::<Vec<_>>(), ["AAA"]);
    assert!(approx(rest["AAA"], 0.1, 1e-12));
    assert_eq!((matched.matched, matched.regions), (2, 4));
    assert!(approx(matched.rate(), 0.5, 1e-12));
    // BBB has no provinces; AAA's matched regions hold 90% of its DOSE manufacturing.
    assert!(approx(matched.gva_share, 90.0 / 101.0, 1e-12));
    let text = report(&place(&proxies()), &matched);
    assert!(
        text.contains("DOSE regions matched: 2 of 4 (50.0%)"),
        "{text}"
    );
    // Regions DOSE does not list count at their population share: 80% coverage leaves
    // 1 - 0.8 x 0.9 unplaced.
    regions.get_mut("AAA").unwrap().coverage = 0.8;
    let (_, rest, _) = match_dose(&regions, &ids, &country, &names, &weight, &iso);
    assert!(approx(rest["AAA"], 0.28, 1e-12));
}

#[test]
fn dose_names_must_be_unique_within_the_country() {
    let regions = [("AAA".to_owned(), dose(&[("Twin", 1.0)], 1.0))].into();
    let names = names(&[("AAA-1", &["Twin"]), ("AAA-2", &["Twin"])]);
    let (proxy, rest, matched) = match_dose(
        &regions,
        &owned(&["AAA-1", "AAA-2"]),
        &owned(&["AAA", "AAA"]),
        &names,
        &[1.0, 1.0],
        &BTreeMap::new(),
    );
    assert!(proxy.iter().all(|x| x.is_nan()));
    assert_eq!(matched.matched, 0);
    assert_eq!(rest, [("AAA".to_owned(), 1.0)].into());
}

#[test]
fn dose_names_match_after_unicode_normalisation_and_case_folding() {
    // A decomposed accent, a sharp s and surrounding spaces in DOSE; the NFC capitals in Natural
    // Earth.
    let regions = [(
        "AAA".to_owned(),
        dose(&[("  Cora\u{0301}o ", 2.0), ("Gro\u{df}feld", 1.0)], 1.0),
    )]
    .into();
    let names = names(&[("AAA-1", &["CORÁO"]), ("AAA-2", &["GROSSFELD"])]);
    let (proxy, _, matched) = match_dose(
        &regions,
        &owned(&["AAA-1", "AAA-2"]),
        &owned(&["AAA", "AAA"]),
        &names,
        &[1.0, 1.0],
        &BTreeMap::new(),
    );
    assert_eq!(matched.matched, 2);
    assert_eq!(proxy, [2.0, 1.0]);
}

// Points and EIA

fn cell(lat: f64, lon: f64) -> u64 {
    u64::from(cell_of(lat, lon, Resolution::Four).unwrap())
}

fn owner(points: &[(f64, f64)]) -> CellOwner {
    CellOwner::new(
        points
            .iter()
            .enumerate()
            .map(|(k, (lat, lon))| (cell(*lat, *lon), k as u32)),
    )
    .unwrap()
}

fn asset(lat: f64, lon: f64, weight: f64, offshore: bool, country: &str) -> Asset {
    Asset {
        lat,
        lon,
        weight,
        dated: true,
        offshore,
        country: country.to_owned(),
    }
}

#[test]
fn points_go_to_the_province_owning_their_cell() {
    let mut owner = owner(&[(10.0, 20.0), (-30.0, 140.0)]);
    let points = [
        asset(10.0, 20.0, 2.0, false, ""),
        asset(-30.0, 140.0, 1.0, false, ""),
        asset(10.0, 20.0, 0.5, false, ""),
        asset(60.0, -40.0, 9.0, false, ""),
    ];
    assert_eq!(point_sums(&points, &mut owner, 2).unwrap(), [2.5, 1.0]);
}

#[test]
fn eia_spreads_states_by_onshore_fields_and_offshore_over_the_nearest_coastal_province() {
    // Texas in two parts (the second coastal), Louisiana coastal, Kansas inland, and one Canadian
    // province.
    let points = [
        (31.0, -101.0),
        (28.5, -96.5),
        (30.5, -92.0),
        (38.5, -98.0),
        (53.0, -113.0),
    ];
    let mut owner = owner(&points);
    let geo = Geography {
        id: owned(&["USA-1.1", "USA-1.2", "USA-2", "USA-3", "CAN-1"]),
        country: owned(&["USA", "USA", "USA", "USA", "CAN"]),
        coastal: vec![false, true, true, false, false],
        lat: points.iter().map(|p| p.0).collect(),
        lon: points.iter().map(|p| p.1).collect(),
        res: Default::default(),
    };
    let states = [("USA-1", "TX"), ("USA-2", "LA"), ("USA-3", "KS")]
        .map(|(a, b)| (a.to_owned(), b.to_owned()))
        .into();
    let seds = [("TX", 40.0), ("LA", 10.0), ("KS", 6.0), ("X3", 100.0)]
        .map(|(a, b)| (a.to_owned(), b))
        .into();
    let us = "United States";
    let assets = [
        asset(31.0, -101.0, 3.0, false, us),
        asset(28.5, -96.5, 1.0, false, us),
        asset(28.0, -91.0, 9.0, true, us), // off Louisiana
        asset(27.5, -96.0, 1.0, true, us), // off Texas
        asset(53.0, -113.0, 50.0, false, "Canada"),
    ];
    let population = [1.0, 1.0, 1.0, 4.0, 1.0];
    let got = eia_oil_gas(&seds, &geo, &states, &mut owner, &population, Some(&assets)).unwrap();
    // TX 40 by onshore fields 3:1, plus 10 of offshore; LA 10 plus 90 of offshore; KS has no
    // fields: by population.
    assert_close(&got, &[30.0, 20.0, 100.0, 6.0, 0.0], 1e-12);
    let without = eia_oil_gas(&seds, &geo, &states, &mut owner, &population, None).unwrap();
    assert_close(&without, &[20.0, 70.0, 60.0, 6.0, 0.0], 1e-12);
}

/// A one-band Float32 GeoTIFF with GDAL's nodata tag.
fn write_tiff(path: &Path, width: u32, height: u32, origin: (f64, f64), size: f64, values: &[f32]) {
    use tiff::encoder::{TiffEncoder, colortype::Gray32Float};
    use tiff::tags::Tag;
    let mut bytes = Cursor::new(Vec::new());
    let mut tiff = TiffEncoder::new(&mut bytes).unwrap();
    let mut image = tiff.new_image::<Gray32Float>(width, height).unwrap();
    let dir = image.encoder();
    dir.write_tag(Tag::ModelPixelScaleTag, &[size, size, 0.0][..])
        .unwrap();
    dir.write_tag(
        Tag::ModelTiepointTag,
        &[0.0, 0.0, 0.0, origin.0, origin.1, 0.0][..],
    )
    .unwrap();
    dir.write_tag(Tag::GdalNodata, "-9").unwrap();
    image.write_data(values).unwrap();
    fs::write(path, bytes.into_inner()).unwrap();
}

#[test]
fn gridded_gdp_sums_the_derived_geotiff_and_skips_nodata() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gdp.tif");
    write_tiff(&path, 2, 1, (19.0, 11.0), 2.0, &[20.0, -9.0]);
    let got = gridded_gdp(&path, &mut owner(&[(10.0, 20.0)])).unwrap();
    assert_eq!(got, [20.0]);
}

#[test]
fn mineral_sites_sum_the_resource_groups() {
    let res: [Vec<u32>; 5] = std::array::from_fn(|_| vec![1, 0, 2]);
    assert_eq!(mineral_sites(&res), [5.0, 0.0, 10.0]);
}

#[test]
fn coal_share_is_coal_value_added_over_mining() {
    // Industries A01, B05, B07, B08, C10T12 and G for AAA and BBB; one final demand column.
    let industries = ["A01", "B05", "B07", "B08", "C10T12", "G"];
    let cols: Vec<String> = ["AAA", "BBB"]
        .iter()
        .flat_map(|e| industries.map(|i| format!("{e}_{i}")))
        .collect();
    let n = cols.len();
    let mut rng = Uniform::new(1);
    let z: Vec<Vec<f64>> = (0..n)
        .map(|_| (0..n).map(|_| rng.next(1.0, 10.0)).collect())
        .collect();
    let fd: Vec<f64> = (0..n).map(|_| rng.next(1.0, 10.0)).collect();
    let tls: Vec<f64> = (0..n).map(|_| rng.next(1.0, 3.0)).collect();
    let va: Vec<f64> = (0..n).map(|_| rng.next(10.0, 30.0)).collect();
    let out: Vec<f64> = (0..n)
        .map(|j| (0..n).map(|i| z[i][j]).sum::<f64>() + tls[j] + va[j])
        .collect();
    let join = |xs: &[f64]| {
        xs.iter()
            .map(|x| format!("{x:?}"))
            .collect::<Vec<_>>()
            .join(",")
    };
    let mut lines = vec![format!("V1,{},AAA_HFCE,BBB_HFCE,OUT", cols.join(","))];
    for (i, c) in cols.iter().enumerate() {
        let total = z[i].iter().sum::<f64>() + fd[i];
        lines.push(format!("{c},{},{:?},0,{total:?}", join(&z[i]), fd[i]));
    }
    for (label, row) in [("TLS", &tls), ("VA", &va), ("OUT", &out)] {
        lines.push(format!(
            "{label},{},0,0,{:?}",
            join(row),
            row.iter().sum::<f64>()
        ));
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("2000_SML.csv");
    fs::write(&path, lines.join("\n") + "\n").unwrap();
    let got = aggregate(&path, 2000).unwrap();
    for (k, code) in ["AAA", "BBB"].iter().enumerate() {
        let industry_va: Vec<f64> = (0..n)
            .map(|j| out[j] - (0..n).map(|i| z[i][j]).sum::<f64>())
            .collect();
        let block = &industry_va[k * 6..k * 6 + 6];
        let want = block[1] / block[1..4].iter().sum::<f64>();
        assert!(approx(got[*code].coal_share, want, 1e-12), "{code}");
    }
}

// The 2000 pack and sources.

const NAMES: [&str; 6] = [
    "1995-2000_SML.zip",
    "ne_10m_admin_1_states_provinces.zip",
    "spam2000v3.0.7_global_physical-area.geotiff.zip",
    "DOSE_V2.14.csv",
    "Complete_SEDS.csv",
    WORLDPOP,
];

struct Real {
    world: BTreeMap<String, Economy>,
    provinces: Provinces,
    names: Vec<String>,
    proxies: Proxies,
    matched: DoseMatch,
    out: Downscaled,
}

/// The 2000 downscaling as the econ build runs it, in optimised builds only.
fn real() -> Option<&'static Real> {
    static DATA: OnceLock<Option<Real>> = OnceLock::new();
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
        let pack = pack_table("provinces.arrow")?.parent()?.to_owned();
        let countries = pack_table("countries.arrow")?;
        let admin0 = cached(ADMIN0)?;
        let cache = admin0.parent()?.to_owned();
        let (iso, _) = read_codes(&admin0).unwrap();
        let obs = load_observed(&cache, &admin0).unwrap();
        let codes = strings(&countries, "code");
        let pairs: Vec<(String, String)> = codes
            .iter()
            .cloned()
            .zip(strings(&countries, "sovereign"))
            .collect();
        let labour = country_labour(&pairs, &obs, &iso).unwrap();
        let geo = read_geography(&pack).unwrap();
        let mut owner = CellOwner::read(&pack.join("cells.arrow")).unwrap();
        let raw = province_population(
            &mut geotiff::open(&cache.join(WORLDPOP)).unwrap(),
            &mut owner,
            BLOCK_ROWS,
        )
        .unwrap();
        let mut worldpop: BTreeMap<String, f64> = BTreeMap::new();
        for (c, p) in geo.country.iter().zip(&raw) {
            *worldpop.entry(c.clone()).or_insert(0.0) += p;
        }
        let population =
            scale_to_national(&raw, &geo.country, &national_population(&codes, &obs, &iso));
        let working_age = population
            .iter()
            .zip(&geo.country)
            .map(|(p, c)| p * labour[c].working_age_share)
            .collect();
        let world = impute_pack(
            &pairs,
            &aggregate(&cache.join(NAMES[0]), 2000).unwrap(),
            &load_wdi(&cache).unwrap(),
            &obs,
            &iso,
            &worldpop,
        )
        .unwrap();
        let gem = GEM.map(|n| cache.join(n));
        let gem = gem
            .iter()
            .all(|p| p.exists())
            .then(|| (gem[0].clone(), gem[1].clone()));
        let (proxies, matched) = load_proxies(
            &cache,
            &geo,
            &mut owner,
            &population,
            &obs.population,
            &iso,
            gem,
        )
        .unwrap();
        let provinces = Provinces {
            country: geo.country.clone(),
            population,
            working_age,
        };
        let out = allocate(&world, &provinces, &proxies).unwrap();
        Some(Real {
            world,
            provinces,
            names: strings(&pack.join("provinces.arrow"), "name"),
            proxies,
            matched,
            out,
        })
    })
    .as_ref()
}

fn indices(r: &Real, code: &str) -> Vec<usize> {
    (0..r.provinces.country.len())
        .filter(|&i| r.provinces.country[i] == code)
        .collect()
}

fn short(name: &str) -> &str {
    name.split(" (").next().unwrap_or(name)
}

fn cover_of(shares: &[(Level, f64)], level: Level) -> f64 {
    shares
        .iter()
        .find(|(l, _)| *l == level)
        .map_or(0.0, |(_, v)| *v)
}

#[test]
fn real_downscaling_preserves_national_va() {
    let Some(r) = real() else { return };
    for (code, e) in &r.world {
        let here = indices(r, code);
        if here.is_empty() {
            continue;
        }
        for g in 0..N_GOODS {
            let sum: f64 = here.iter().map(|&i| r.out.va[i][g]).sum();
            let want = e.table.va[g];
            assert!((sum - want).abs() <= 1e-9 + 1e-9 * want.abs(), "{code} {g}");
        }
    }
}

#[test]
fn real_provinces_under_1000_working_age_have_no_va_unless_alone() {
    let Some(r) = real() else { return };
    let small: Vec<usize> = (0..r.provinces.working_age.len())
        .filter(|&i| r.provinces.working_age[i] < MIN_WORKING_AGE)
        .collect();
    assert!(small.len() > 100);
    for i in small {
        let alone = r.out.level[i].iter().all(|l| *l == Level::Largest);
        assert!(alone || r.out.va[i].iter().all(|v| *v == 0.0), "{i}");
    }
}

#[test]
fn real_dose_match_rate_reported() {
    let Some(r) = real() else { return };
    assert!(0.6 < r.matched.rate() && r.matched.rate() <= 1.0);
    assert!(report(&r.out, &r.matched).contains("DOSE regions matched"));
}

#[test]
fn real_gem_places_oil_and_coal_where_they_are() {
    let Some(r) = real() else { return };
    if GEM.iter().any(|n| cached(n).is_none()) {
        return;
    }
    // Oil: Saudi oil VA is placed by GEM fields, and the Eastern Province leads.
    let sau = indices(r, "SAU");
    assert!(sau.iter().all(|&i| r.out.level[i][1] == Level::Gem));
    let total: f64 = sau.iter().map(|&i| r.out.va[i][1]).sum();
    let leading: BTreeSet<&str> = sau
        .iter()
        .filter(|&&i| r.out.va[i][1] > 0.05 * total)
        .map(|&i| short(&r.names[i]))
        .collect();
    assert_eq!(leading, BTreeSet::from(["Ash Sharqiyah"]));
    // Coal: Shanxi leads China and Wyoming the United States (production as reported, no median
    // fill).
    for (code, top) in [("CHN", "Shanxi"), ("USA", "Wyoming")] {
        let here = indices(r, code);
        let mut best = here[0];
        for &i in &here {
            if r.out.va[i][2] > r.out.va[best][2] {
                best = i;
            }
        }
        assert_eq!(short(&r.names[best]), top);
    }
    // US oil and gas follows EIA 2000 state production: Texas and Louisiana (with the Gulf
    // offshore) hold most.
    let usa = indices(r, "USA");
    assert!(usa.iter().all(|&i| r.out.level[i][1] == Level::Eia));
    let total: f64 = usa.iter().map(|&i| r.out.va[i][1]).sum();
    let mut share: BTreeMap<&str, f64> = BTreeMap::new();
    for &i in &usa {
        *share.entry(short(&r.names[i])).or_insert(0.0) += r.out.va[i][1] / total;
    }
    assert!(share["Texas"] + share["Louisiana"] > 0.5);
    assert!(share.get("Pennsylvania").copied().unwrap_or(0.0) < 0.02);
    assert!(r.proxies.coal.is_some() && r.proxies.oil_gas.is_some());
    let cover = coverage(&r.out);
    assert!(cover_of(&cover[1], Level::Gem) + cover_of(&cover[1], Level::Eia) > 0.8);
    assert!(cover_of(&cover[2], Level::Mining) > 0.8);
}

#[test]
fn real_gridded_gdp_places_manufacturing_dose_leaves() {
    let Some(r) = real() else { return };
    if cached(KUMMU).is_none() {
        return;
    }
    let gdp = r.proxies.gdp.as_ref().expect("gridded GDP loaded");
    // Kummu 2000, constant 2011 international dollars: about 62 trillion over the pack's
    // provinces.
    let total: f64 = gdp.iter().sum();
    assert!(5e13 < total && total < 7.5e13);
    let cover = coverage(&r.out);
    for g in MANUFACTURING {
        assert!(cover_of(&cover[g], Level::Gdp) > 0.1);
        assert!(cover_of(&cover[g], Level::Population) < 0.01);
    }
    for g in [1, 2] {
        assert_eq!(cover_of(&cover[g], Level::Gdp), 0.0);
    }
}
