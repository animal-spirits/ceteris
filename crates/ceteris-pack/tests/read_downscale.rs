//! Readers of the downscaling proxies: DOSE, EIA SEDS, the GEM trackers and Natural Earth
//! admin-1 names.

mod common;

use std::collections::BTreeMap;
use std::fs;

use ceteris_pack::read::downscale::{
    Dose, M3_PER_BOE, read_dose, read_gem_coal, read_gem_oil_gas, read_seds,
};
use ceteris_pack::read::natural_earth::{province_names, us_states};
use ceteris_pack::read::wdi::read_wdi;
use common::{Row, V, cached, write_rows};

const DOSE_HEADER: &str = "country,region,GID_0,GID_1,year,grp_lcu,pop,grp_pc_lcu,ag_grp_pc_lcu,man_grp_pc_lcu,serv_grp_pc_lcu\n";

fn regions(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
    pairs.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect()
}

#[test]
fn read_dose_takes_manufacturing_gva_of_the_nearest_year_and_its_coverage() {
    let rows = [
        "A,R1,AAA,AAA.1_1,2000,1,10,1,,2,",
        "A,R2,AAA,AAA.2_1,2000,1,10,1,,3,",
        "A,R3,AAA,AAA.3_1,2000,1,10,1,,,", // no manufacturing value: not a DOSE region of 2000
        "A,R1,AAA,AAA.1_1,2002,1,10,1,,4,",
        "A,R2,AAA,AAA.2_1,2002,1,20,1,,5,",
        "A,R3,AAA,AAA.3_1,2002,1,5,1,,6,",
        "B,Q1,BBB,BBB.1_1,2010,1,10,1,,2,", // outside the five-year window
        "C,S1,CCC,CCC.1_1,1999,1,10,1,,2,",
    ];
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dose.csv");
    fs::write(&path, format!("{DOSE_HEADER}{}\n", rows.join("\n"))).unwrap();
    let got = read_dose(&path, &[("AAA".to_owned(), 40.0)].into(), 2000, 5).unwrap();
    let want = [
        (
            "AAA".to_owned(),
            Dose {
                values: regions(&[("R1", 20.0), ("R2", 30.0)]),
                coverage: 0.5,
            },
        ),
        (
            "CCC".to_owned(),
            Dose {
                values: regions(&[("S1", 20.0)]),
                coverage: 1.0,
            },
        ),
    ];
    assert_eq!(got, want.into());
}

fn mine<'a>(
    name: &'a str,
    status: &'a str,
    opening: Option<f64>,
    production: Option<f64>,
    capacity: Option<f64>,
    country: &'a str,
) -> Row<'a> {
    let mut r = vec![
        ("Mine Name", V::T(name)),
        ("Country / Area", V::T(country)),
        ("Status", V::T(status)),
        ("Latitude", V::N(10.0)),
        ("Longitude", V::N(20.0)),
    ];
    for (k, v) in [
        ("Opening Year", opening),
        ("Production (Mtpa)", production),
        ("Capacity (Mtpa)", capacity),
    ] {
        if let Some(v) = v {
            r.push((k, V::N(v)));
        }
    }
    r
}

#[test]
fn gem_coal_keeps_mines_open_in_2000_weighted_by_output() {
    let open = vec![
        mine(
            "old",
            "Operating",
            Some(1980.0),
            Some(4.0),
            Some(9.0),
            "Aland",
        ),
        mine("cap", "Mothballed", Some(1995.0), None, Some(2.0), "Aland"),
        mine("undated", "Operating", None, Some(1.0), None, "Aland"),
        mine("guess", "Operating", Some(1990.0), None, None, "Aland"), // no figure while others in its country have one: 0
        mine(
            "idle",
            "Operating",
            Some(1990.0),
            Some(0.0),
            Some(5.0),
            "Aland",
        ), // reported 0 stays 0
        mine("b1", "Operating", Some(1990.0), None, None, "Bland"), // Bland has no figures: the median positive figure
        mine("b2", "Operating", Some(1990.0), None, None, "Bland"),
        mine("new", "Operating", Some(2005.0), Some(50.0), None, "Aland"),
        mine("plan", "Proposed", Some(1990.0), Some(50.0), None, "Aland"),
    ];
    let closed = vec![
        // GEM swapped its coordinates.
        vec![
            ("Mine Name", V::T("late")),
            ("Country / Area", V::T("Aland")),
            ("Opening Year", V::N(1950.0)),
            ("Closing Year", V::N(2004.0)),
            ("Production (Mtpa)", V::N(3.0)),
            ("Latitude", V::N(144.0)),
            ("Longitude", V::N(-38.0)),
        ],
        vec![
            ("Mine Name", V::T("early")),
            ("Country / Area", V::T("Aland")),
            ("Closing Year", V::N(1999.0)),
            ("Production (Mtpa)", V::N(3.0)),
            ("Latitude", V::N(1.0)),
            ("Longitude", V::N(1.0)),
        ],
        vec![
            ("Mine Name", V::T("unknown")),
            ("Country / Area", V::T("Aland")),
            ("Production (Mtpa)", V::N(3.0)),
            ("Latitude", V::N(1.0)),
            ("Longitude", V::N(1.0)),
        ],
    ];
    let dir = tempfile::tempdir().unwrap();
    let path = write_rows(
        &dir.path().join("coal.xlsx"),
        &[
            ("README", vec![vec![("x", V::T("y"))]]),
            ("Non-closed mines", open),
            ("Closed mines", closed),
        ],
    );
    let got = read_gem_coal(&path).unwrap();
    let last = got.last().unwrap();
    assert_eq!((last.lat, last.lon), (-38.0, 144.0));
    let mut aland: Vec<f64> = got
        .iter()
        .filter(|a| a.country == "Aland")
        .map(|a| a.weight)
        .collect();
    aland.sort_by(f64::total_cmp);
    assert_eq!(aland, [0.0, 0.0, 1.0, 2.0, 3.0, 4.0]);
    assert_eq!(
        got.iter()
            .filter(|a| a.country == "Bland")
            .map(|a| a.weight)
            .collect::<Vec<_>>(),
        [2.5, 2.5]
    );
    let dated: Vec<bool> = got
        .iter()
        .filter(|a| a.country == "Aland")
        .map(|a| a.dated)
        .collect();
    assert_eq!(dated, [true, true, false, true, true, true]);
}

fn field<'a>(
    uid: &'a str,
    status: &'a str,
    start: V<'a>,
    discovery: V<'a>,
    lat: V<'a>,
    country: &'a str,
) -> Row<'a> {
    vec![
        ("Unit ID", V::T(uid)),
        ("Country/Area", V::T(country)),
        ("Status", V::T(status)),
        ("Production start year", start),
        ("Discovery year", discovery),
        ("Latitude", lat),
        ("Longitude", V::T("2.0")),
    ]
}

#[test]
fn gem_oil_and_gas_keeps_producing_fields_weighted_by_boe() {
    let (none, one) = (V::T(""), V::T("1.0"));
    let mut offshore = field("S", "operating", V::N(1990.0), none, one, "Bland");
    offshore.push(("Onshore/Offshore", V::T("offshore")));
    let fields = vec![
        field("A", "operating", V::N(1970.0), none, one, "Aland"),
        field("B", "operating", none, V::N(1985.0), one, "Aland"),
        field("C", "abandoned", V::N(1960.0), none, one, "Aland"),
        field("D", "operating", V::N(2010.0), none, one, "Aland"),
        field("E", "discovered", V::N(1990.0), none, one, "Aland"),
        field("F", "operating", V::N(1990.0), none, none, "Aland"),
        field("G", "mothballed", none, none, one, "Aland"),
        field("H", "operating", V::N(1990.0), none, one, "Aland"),
        offshore,
        field("T", "operating", none, none, one, "Bland"),
    ];
    let prod = |uid: &'static str, q: f64, units: &'static str, year: f64| -> Row<'static> {
        vec![
            ("Unit ID", V::T(uid)),
            ("Quantity (converted)", V::N(q)),
            ("Units (converted)", V::T(units)),
            ("Data Year", V::N(year)),
        ]
    };
    let production = vec![
        prod("A", 10.0, "million bbl/y", 2023.0),
        prod("A", 8.0, "million bbl/y", 2023.0), // same year, another liquid: the larger counts
        prod("A", 1642.0, "million m³/y", 2023.0),
        prod("A", 99.0, "million bbl/y", 2015.0), // an older year is ignored
        prod("B", 3.0, "million boe/y", 2023.0),
        prod("C", 5.0, "million bbl/y", 2023.0),
        prod("H", 0.0, "million bbl/y", 2023.0), // latest production 0 stays 0
    ];
    let dir = tempfile::tempdir().unwrap();
    let path = write_rows(
        &dir.path().join("goget.xlsx"),
        &[
            ("Field-level main data", fields),
            ("Field-level production data", production),
        ],
    );
    let got = read_gem_oil_gas(&path).unwrap();
    let gas = 1642.0 / M3_PER_BOE;
    // A: 10 bbl + gas; B: 3; C: 5; G: no data while Aland has figures, 0; H: 0.
    let round = |x: f64| (x * 1e6).round() / 1e6;
    let mut weights: Vec<f64> = got
        .iter()
        .filter(|a| a.country == "Aland")
        .map(|a| round(a.weight))
        .collect();
    weights.sort_by(f64::total_cmp);
    let mut want: Vec<f64> = [10.0 + gas, 3.0, 5.0, 0.0, 0.0].map(round).into();
    want.sort_by(f64::total_cmp);
    assert_eq!(weights, want);
    // Bland has no figures at all: each field takes the median positive figure (A, B, C: 3, 5, 10 + gas), 5.
    let bland: Vec<_> = got.iter().filter(|a| a.country == "Bland").collect();
    assert_eq!(
        bland.iter().map(|a| a.weight).collect::<Vec<_>>(),
        [5.0, 5.0]
    );
    assert_eq!(
        bland.iter().map(|a| a.offshore).collect::<Vec<_>>(),
        [true, false]
    );
    assert_eq!(
        bland.iter().map(|a| a.dated).collect::<Vec<_>>(),
        [true, false]
    );
    // G has no year.
    let aland_dated: Vec<bool> = got
        .iter()
        .filter(|a| a.country == "Aland")
        .map(|a| a.dated)
        .collect();
    assert!(aland_dated.contains(&true) && aland_dated.contains(&false));
}

#[test]
fn read_seds_sums_crude_and_marketed_gas_by_state() {
    let rows = [
        r#""2024F","PAPRB","TX","2000",10"#,
        r#""2024F","NGMPB","TX","2000",30"#,
        r#""2024F","PAPRB","TX","2001",99"#,
        r#""2024F","PAPRP","TX","2000",99"#, // thousand barrels: not the Btu series
        r#""2024F","PAPRB","X3","2000",50"#,
        r#""2024F","PAPRB","US","2000",90"#,
        r#""2024F","NGMPB","VT","2000","#,
    ];
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("seds.csv");
    fs::write(
        &path,
        format!(
            "\u{feff}Data_Status,MSN,StateCode,Year,Data\n{}\n",
            rows.join("\n")
        ),
    )
    .unwrap();
    assert_eq!(
        read_seds(&path, 2000).unwrap(),
        regions(&[("TX", 40.0), ("X3", 50.0)])
    );
}

#[test]
fn real_gem_trackers_have_the_main_producers() {
    let (Some(coal), Some(oil)) = (
        cached("Global Coal Mine Tracker, August 2026.xlsx"),
        cached("Global-Oil-and-Gas-Extraction-Tracker-March-2026.xlsx"),
    ) else {
        return;
    };
    let (coal, oil) = (
        read_gem_coal(&coal).unwrap(),
        read_gem_oil_gas(&oil).unwrap(),
    );
    assert!(coal.len() > 2000 && oil.len() > 4000);
    assert!(
        coal.iter()
            .chain(&oil)
            .all(|a| a.lat.abs() <= 90.0 && a.lon.abs() <= 180.0 && a.weight >= 0.0)
    );
}

#[test]
fn real_dose_covers_large_economies_near_2000() {
    let (Some(dose), Some(pop)) = (cached("DOSE_V2.14.csv"), cached("wdi_SP.POP.TOTL.zip")) else {
        return;
    };
    let population = read_wdi(&pop)
        .unwrap()
        .into_iter()
        .filter_map(|(k, v)| Some((k, *v.get(&2000)?)))
        .collect();
    let got = read_dose(&dose, &population, 2000, 5).unwrap();
    assert!(
        ["USA", "CHN", "IND", "BRA", "DEU", "JPN"]
            .iter()
            .all(|c| got.contains_key(*c))
    );
    assert_eq!(got["USA"].values.len(), 51);
    assert!(got["USA"].coverage > 0.99);
    assert!(got["ETH"].coverage < 0.5); // Oromia alone
}

#[test]
fn real_admin1_names_and_us_states() {
    let Some(admin1) = cached("ne_10m_admin_1_states_provinces.zip") else {
        return;
    };
    let names = province_names(&admin1).unwrap();
    assert!(names.len() > 4000);
    assert!(
        names
            .values()
            .all(|n| n.iter().all(|x| !x.is_empty() && x.trim() == x))
    );
    let states = us_states(&admin1).unwrap();
    assert_eq!(states.len(), 51);
    assert!(states.values().any(|s| s == "TX"));
}
