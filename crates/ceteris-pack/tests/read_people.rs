//! Readers of the people sources (ILOSTAT, WIC, Barro-Lee, WDI, SWIID, PWT, Natural Earth) and
//! of xlsx worksheets.

mod common;

use std::collections::BTreeMap;
use std::fs;

use ceteris_pack::read::natural_earth::read_codes;
use ceteris_pack::read::people::{
    load_observed, read_barro_lee, read_ilostat, read_ilostat_mean, read_swiid, read_wic,
};
use ceteris_pack::read::wdi::{read_wdi, read_wdi_regions};
use ceteris_pack::read::xlsx::{self, Cell};
use common::{approx, cached, write_xlsx, write_zip};

#[test]
fn read_ilostat_keeps_both_sexes_and_the_asked_age_class_as_fractions() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ilo.csv");
    fs::write(
        &path,
        "\u{feff}\"ref_area\",\"source\",\"indicator\",\"sex\",\"classif1\",\"time\",\"obs_value\",\"obs_status\"\n\
         \"AFG\",\"XA:1\",\"EAP\",\"SEX_T\",\"AGE_YTHADULT_YGE15\",\"2000\",46.5,\n\
         \"AFG\",\"XA:1\",\"EAP\",\"SEX_T\",\"AGE_YTHADULT_Y15-64\",\"2000\",47.5,\n\
         \"AFG\",\"XA:1\",\"EAP\",\"SEX_M\",\"AGE_YTHADULT_Y15-64\",\"2000\",80.0,\n\
         \"ALB\",\"XA:1\",\"EAP\",\"SEX_T\",\"AGE_YTHADULT_Y15-64\",\"2000\",,\n",
    )
    .unwrap();
    let got = read_ilostat(&path, "AGE_YTHADULT_Y15-64").unwrap();
    assert_eq!(got.len(), 1);
    assert!(approx(got["AFG"], 0.475, 1e-12));
}

#[test]
fn read_ilostat_mean_averages_the_years_of_the_natural_rate_window() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("une.csv");
    let mut rows = vec![
        r#""ref_area","source","indicator","sex","classif1","time","obs_value","obs_status""#
            .to_owned(),
    ];
    for (year, value) in [
        (1994, 90.0),
        (1995, 4.0),
        (2000, 6.0),
        (2005, 8.0),
        (2006, 90.0),
    ] {
        rows.push(format!(
            r#""AFG","XA:1","UNE","SEX_T","AGE_YTHADULT_YGE15","{year}",{value},"#
        ));
    }
    rows.push(r#""AFG","XA:1","UNE","SEX_F","AGE_YTHADULT_YGE15","2000",50.0,"#.into());
    rows.push(r#""AFG","XA:1","UNE","SEX_T","AGE_YTHADULT_Y15-24","2000",50.0,"#.into());
    rows.push(r#""ALB","XA:1","UNE","SEX_T","AGE_YTHADULT_YGE15","2000",,"#.into());
    rows.push(r#""ALB","XA:1","UNE","SEX_T","AGE_YTHADULT_YGE15","1990",3.0,"#.into());
    fs::write(&path, format!("\u{feff}{}\n", rows.join("\n"))).unwrap();
    let got = read_ilostat_mean(&path, "AGE_YTHADULT_YGE15", 1995, 2005).unwrap();
    assert_eq!(got.len(), 1);
    assert!(approx(got["AFG"], 0.06, 1e-12));
}

#[test]
fn read_wic_groups_working_ages_into_three_levels() {
    let mut rows = vec![r#""Time","sex","edu","agest","pop","region","source""#.to_owned()];
    for sex in ["f", "m"] {
        for (edu, pop) in ["e1", "e2", "e3", "e4", "e5", "e6"].iter().zip(1..) {
            for age in [10, 15, 60, 65] {
                rows.push(format!(
                    r#"2000,"{sex}","{edu}",{age},{pop},"reg4","Back Projection""#
                ));
            }
            rows.push(format!(
                r#"1995,"{sex}","{edu}",15,100,"reg4","Back Projection""#
            ));
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wic.csv");
    fs::write(&path, rows.join("\n") + "\n").unwrap();
    let shares = read_wic(&path, &[(4, "AFG".to_owned())].into()).unwrap()["AFG"];
    // Ages 15 and 60 only, both sexes: 2 sexes x 2 ages x (1+2+3, 4+5, 6).
    for (got, want) in shares.iter().zip([6.0 / 21.0, 9.0 / 21.0, 6.0 / 21.0]) {
        assert!(approx(*got, want, 1e-12));
    }
    assert!(
        read_wic(&path, &[(5, "XXX".to_owned())].into())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn read_barro_lee_uses_the_total_population_of_2000() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bl.csv");
    fs::write(
        &path,
        "BLcode,country,year,sex,agefrom,ageto,lu,lp,lpc,ls,lsc,lh,lhc,yr_sch,yr_sch_pri,yr_sch_sec,yr_sch_ter,WBcode,region_code,pop\n\
         1,\"Romania\",2000,\"MF\",15,64,10,30,5,50,9,10,4,9,5,3,1,\"ROM\",\"Europe\",1\n\
         1,\"Romania\",2005,\"MF\",15,64,0,0,0,100,0,0,0,9,5,3,1,\"ROM\",\"Europe\",1\n\
         1,\"Romania\",2000,\"F\",15,64,0,0,0,100,0,0,0,9,5,3,1,\"ROM\",\"Europe\",1\n",
    )
    .unwrap();
    let got = read_barro_lee(&path).unwrap()["ROU"];
    for (g, w) in got.iter().zip([0.4, 0.5, 0.1]) {
        assert!(approx(*g, w, 1e-12));
    }
}

#[test]
fn read_wdi_and_regions() {
    let api = "\"Data Source\",\"World Development Indicators\",\n\n\"Last Updated Date\",\"2026-10-08\",\n\n\
               \"Country Name\",\"Country Code\",\"Indicator Name\",\"Indicator Code\",\"1999\",\"2000\",\"2001\",\n\
               \"Aruba\",\"ABW\",\"Gini\",\"X\",\"\",\"41.5\",\"\",\n\"Albania\",\"ALB\",\"Gini\",\"X\",\"30\",\"\",\"33\",\n";
    let meta = "\"Country Code\",\"Region\",\"IncomeGroup\",\"SpecialNotes\",\"TableName\",\n\
                \"ALB\",\"Europe & Central Asia\",\"Upper\",\"\",\"Albania\",\n\"WLD\",\"\",\"\",\"\",\"World\",\n";
    let dir = tempfile::tempdir().unwrap();
    let path = write_zip(
        &dir.path().join("wdi.zip"),
        &[
            ("API_X_DS2_en_csv_v2_1.csv", api),
            ("Metadata_Country_API_X_DS2_en_csv_v2_1.csv", meta),
            ("Metadata_Indicator_API_X_DS2_en_csv_v2_1.csv", "junk"),
        ],
    );
    let want: BTreeMap<String, BTreeMap<i64, f64>> = [
        ("ABW".into(), [(2000, 41.5)].into()),
        ("ALB".into(), [(1999, 30.0), (2001, 33.0)].into()),
    ]
    .into();
    assert_eq!(read_wdi(&path).unwrap(), want);
    let (regions, names) = read_wdi_regions(&path).unwrap();
    assert_eq!(
        regions,
        [("ALB".to_owned(), "Europe & Central Asia".to_owned())].into()
    );
    assert_eq!(names, [("Albania".to_owned(), "ALB".to_owned())].into());
}

#[test]
fn read_swiid_places_countries_by_name() {
    let text = "country,year,gini_disp,gini_disp_se,gini_mkt,gini_mkt_se,abs_red,abs_red_se,rel_red,rel_red_se\n\
                Albania,2000,31.4,1,,,,,,\nAlbania,2001,,1,,,,,,\nCongo-Kinshasa,2000,40,1,,,,,,\nSoviet Union,1990,24,1,,,,,,\n";
    let dir = tempfile::tempdir().unwrap();
    let path = write_zip(
        &dir.path().join("swiid.zip"),
        &[("swiid/swiid_summary.csv", text)],
    );
    let got = read_swiid(&path, &[("Albania".to_owned(), "ALB".to_owned())].into()).unwrap();
    assert_eq!(got.keys().collect::<Vec<_>>(), ["ALB", "COD"]);
    assert!(approx(got["ALB"][&2000], 0.314, 1e-12));
    assert_eq!(got["COD"], [(2000, 0.4)].into());
}

#[test]
fn xlsx_sheet_reads_numbers_and_shared_strings() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_xlsx(
        &dir.path().join("t.xlsx"),
        &["code", "labsh", "AAA"],
        &[
            ("Info", ""),
            (
                "Data",
                r#"<row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="s"><v>1</v></c></row>
                <row r="2"><c r="A2" t="s"><v>2</v></c><c r="B2" t="n"><v>0.55</v></c></row>
                <row r="3"><c r="A3" t="s"><v>2</v></c><c r="B3"/></row>
                <row r="4"><c r="A4" t="s"><v>2</v></c><c r="B4" t="e"><v>#REF!</v></c></row>"#,
            ),
        ],
    );
    let aaa = || ("code".to_owned(), Cell::Text("AAA".into()));
    let got: Vec<_> = xlsx::sheet(&path, "Data").unwrap().collect();
    // An error cell (#REF!) reads as empty.
    assert_eq!(
        got,
        [
            [aaa(), ("labsh".to_owned(), Cell::Number(0.55))].into(),
            [aaa()].into(),
            [aaa()].into(),
        ]
    );
}

/// The cell semantics of the Python reader (`people._xlsx_sheet`): dates stay serial numbers,
/// whole numbers stay floats, booleans read as 1 and 0, formula text and shared strings as
/// text, errors and empty cells are left out, numeric headers are named by their Python `str`
/// and columns without a header are dropped. Inline strings are read as text; the sheets the
/// pack reads have none.
#[test]
fn xlsx_cells_match_python_reader() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_xlsx(
        &dir.path().join("cells.xlsx"),
        &["name", "date", "AAA", "rich"],
        &[(
            "Sheet",
            r#"<row r="2"><c r="B2" t="s"><v>0</v></c><c r="C2" t="s"><v>1</v></c><c r="D2"><v>2000</v></c>
                <c r="E2" t="inlineStr"><is><t>inline</t></is></c><c r="F2" t="b"><v>1</v></c><c r="G2" t="str"><v>formula</v></c></row>
            <row r="3"/>
            <row r="4"><c r="A4"><v>9</v></c><c r="B4" t="s"><v>2</v></c><c r="C4" s="1"><v>36526</v></c><c r="D4"><v>3</v></c>
                <c r="E4" t="inlineStr"><is><t>x</t></is></c><c r="F4" t="b"><v>0</v></c><c r="G4" t="str"><f>A1</f><v>y</v></c></row>
            <row r="5"><c r="A5"><v>1</v></c><c r="C5" t="e"><v>#N/A</v></c></row>
            <row r="6"><c r="B6" t="s"><v>3</v></c><c r="C6" s="1"><v>36526.5</v></c><c r="D6"><v>-0.000012</v></c></row>"#,
        )],
    );
    let rows: Vec<_> = xlsx::sheet(&path, "Sheet").unwrap().collect();
    let text = |s: &str| Cell::Text(s.into());
    let row = |cells: Vec<(&str, Cell)>| {
        cells
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect::<BTreeMap<_, _>>()
    };
    assert_eq!(
        rows,
        [
            row(vec![
                ("name", text("AAA")),
                ("date", Cell::Number(36526.0)),
                ("2000.0", Cell::Number(3.0)),
                ("inline", text("x")),
                ("1.0", Cell::Number(0.0)),
                ("formula", text("y")),
            ]),
            row(vec![]),
            row(vec![
                ("name", text("rich")),
                ("date", Cell::Number(36526.5)),
                ("2000.0", Cell::Number(-1.2e-5))
            ]),
        ]
    );
}

#[test]
fn codes_follow_the_sources_spelling() {
    let Some(admin0) = cached("ne_10m_admin_0_countries.zip") else {
        return;
    };
    let (iso, m49) = read_codes(&admin0).unwrap();
    assert_eq!(
        (
            iso["KOS"].as_str(),
            iso["PSX"].as_str(),
            iso["FRA"].as_str()
        ),
        ("XKX", "PSE", "FRA")
    );
    assert_eq!((m49["NOR"], m49["TWN"], m49["SSD"]), (578, 158, 728));
    assert!(!m49.contains_key("XKX"));
}

#[test]
fn wic_and_barro_lee_agree_on_education() {
    let names = [
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
    if names.iter().any(|n| cached(n).is_none()) {
        return;
    }
    let admin0 = cached("ne_10m_admin_0_countries.zip").unwrap();
    let obs = load_observed(admin0.parent().unwrap(), &admin0).unwrap();
    let both: Vec<&String> = obs
        .edu_wic
        .keys()
        .filter(|k| obs.edu_barro_lee.contains_key(*k))
        .collect();
    assert!(both.len() > 100);
    let level = |m: &BTreeMap<String, [f64; 3]>, l: usize| {
        both.iter().map(|k| m[*k][l]).collect::<Vec<_>>()
    };
    let gap: f64 = (0..3)
        .flat_map(|l| {
            level(&obs.edu_wic, l)
                .into_iter()
                .zip(level(&obs.edu_barro_lee, l))
        })
        .map(|(a, b)| (a - b).abs())
        .sum::<f64>()
        / (3 * both.len()) as f64;
    // Barro-Lee counts any secondary or tertiary attendance, WIC only completed levels: the means differ by a few points.
    assert!(gap < 0.10, "{gap}");
    assert!(corr(&level(&obs.edu_wic, 2), &level(&obs.edu_barro_lee, 2)) > 0.7);
    assert!(corr(&level(&obs.edu_wic, 0), &level(&obs.edu_barro_lee, 0)) > 0.9);
}

fn corr(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len() as f64;
    let (ma, mb) = (a.iter().sum::<f64>() / n, b.iter().sum::<f64>() / n);
    let cov: f64 = a.iter().zip(b).map(|(x, y)| (x - ma) * (y - mb)).sum();
    let va: f64 = a.iter().map(|x| (x - ma).powi(2)).sum();
    let vb: f64 = b.iter().map(|y| (y - mb).powi(2)).sum();
    cov / (va * vb).sqrt()
}
