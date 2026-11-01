//! Population, education, labour and Gini sources: ILOSTAT, WIC, Barro-Lee, SWIID, PWT, UN WPP
//! and the WDI series of the people module.

use std::collections::BTreeMap;
use std::path::Path;

use csv::StringRecord;

use super::natural_earth::{iso_by_name, read_codes};
use super::wdi::{Names, read_wdi, read_wdi_regions};
use super::xlsx::{self, Cell};
use super::{ByYear, Csv, YEAR, cell, float, int, nearest, open_csv, with_member};
use crate::numeric::{fsum, pairwise_sum};

/// UN World Population Prospects 2024 total population by sex and year (gzip CSV).
pub const WPP: &str = "WPP2024_TotalPopulationBySex.csv.gz";
/// Number of education levels: below secondary, secondary, tertiary.
pub const N_EDU: usize = 3;
/// Years whose mean unemployment rate is the natural rate u*.
pub const U_STAR_YEARS: (i64, i64) = (1995, 2005);
/// Names of the SWIID that neither the World Bank nor Natural Earth spells the same way.
pub const SWIID_NAMES: [(&str, &str); 7] = [
    ("Cape Verde", "CPV"),
    ("Congo-Brazzaville", "COG"),
    ("Congo-Kinshasa", "COD"),
    ("Korea", "KOR"),
    ("Palestinian Territories", "PSE"),
    ("St. Vincent and Grenadines", "VCT"),
    ("São Tomé and Príncipe", "STP"),
];
/// Countries of 1 January 2000 that the sources split or rename: the source economies they combine.
pub const PARTS: [(&str, &[&str]); 3] = [
    ("YUG", &["SRB", "MNE"]),
    ("SDN", &["SDN", "SSD"]),
    ("ANT", &["CUW", "SXM"]),
];
/// Barro-Lee uses older World Bank codes.
pub const BARRO_LEE_CODES: [(&str, &str); 3] = [("ROM", "ROU"), ("ZAR", "COD"), ("TMP", "TLS")];

/// Education shares by economy: below secondary, secondary, tertiary.
pub type EduShares = BTreeMap<String, [f64; N_EDU]>;

/// The source economies of pack country `code`: its `PARTS`, else its ISO code (`iso`, by Natural
/// Earth code), else the code itself.
pub fn parts(code: &str, iso: &Names) -> Vec<String> {
    match PARTS.iter().find(|(c, _)| *c == code) {
        Some((_, ps)) => ps.iter().map(|p| (*p).to_owned()).collect(),
        None => vec![iso.get(code).map_or(code, String::as_str).to_owned()],
    }
}

/// WIC education level of each WIC code.
fn wic_level(edu: &str) -> Option<usize> {
    match edu {
        "e1" | "e2" | "e3" => Some(0),
        "e4" | "e5" => Some(1),
        "e6" => Some(2),
        _ => None,
    }
}

fn is_working_age(age: i64) -> bool {
    (15..65).contains(&age) && (age - 15) % 5 == 0
}

/// ILOSTAT indicator rows of both sexes and one age class: (economy, year, rate as a fraction).
fn ilostat_rows(
    path: &Path,
    age: &str,
    mut f: impl FnMut(&str, &str, f64) -> Result<(), String>,
) -> Result<(), String> {
    let mut csv = open_csv(path)?;
    let (area, sex, class, value, time) = (
        csv.col("ref_area")?,
        csv.col("sex")?,
        csv.col("classif1")?,
        csv.col("obs_value")?,
        csv.col("time")?,
    );
    let origin = path.display().to_string();
    let mut row = StringRecord::new();
    while csv.next(&mut row)? {
        let v = cell(&row, value);
        if cell(&row, sex) == "SEX_T" && cell(&row, class) == age && !v.is_empty() {
            f(
                cell(&row, area),
                cell(&row, time),
                float(v, &origin)? / 100.0,
            )?;
        }
    }
    Ok(())
}

/// ILOSTAT indicator CSV (modelled estimates, 2000): rate as a fraction for both sexes and one age class.
pub fn read_ilostat(path: &Path, age: &str) -> Result<BTreeMap<String, f64>, String> {
    let mut out = BTreeMap::new();
    ilostat_rows(path, age, |area, _, v| {
        out.insert(area.to_owned(), v);
        Ok(())
    })?;
    Ok(out)
}

/// ILOSTAT indicator CSV: mean rate over the years `first` to `last` with a value, as a fraction,
/// for both sexes and one age class.
pub fn read_ilostat_mean(
    path: &Path,
    age: &str,
    first: i64,
    last: i64,
) -> Result<BTreeMap<String, f64>, String> {
    let mut values: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let origin = path.display().to_string();
    ilostat_rows(path, age, |area, time, v| {
        if (first..=last).contains(&int(time, &origin)?) {
            values.entry(area.to_owned()).or_default().push(v);
        }
        Ok(())
    })?;
    Ok(values
        .into_iter()
        .map(|(k, v)| (k, fsum(v.iter().copied()) / v.len() as f64))
        .collect())
}

/// WIC reconstruction CSV: shares of the 2000 population aged 15 to 64 in the three education
/// levels, by ISO code (`m49_to_iso` places the WIC regions).
pub fn read_wic(path: &Path, m49_to_iso: &BTreeMap<i64, String>) -> Result<EduShares, String> {
    let mut csv = open_csv(path)?;
    let (time, age, region, edu, pop) = (
        csv.col("Time")?,
        csv.col("agest")?,
        csv.col("region")?,
        csv.col("edu")?,
        csv.col("pop")?,
    );
    let origin = path.display().to_string();
    let year = YEAR.to_string();
    let mut counts: BTreeMap<i64, [f64; N_EDU]> = BTreeMap::new();
    let mut row = StringRecord::new();
    while csv.next(&mut row)? {
        if cell(&row, time) == year && is_working_age(int(cell(&row, age), &origin)?) {
            let r = cell(&row, region);
            let m = int(r.strip_prefix("reg").unwrap_or(r), &origin)?;
            let level = wic_level(cell(&row, edu))
                .ok_or_else(|| format!("{origin}: unknown edu {}", cell(&row, edu)))?;
            counts.entry(m).or_insert([0.0; N_EDU])[level] += float(cell(&row, pop), &origin)?;
        }
    }
    Ok(counts
        .into_iter()
        .filter_map(|(m, c)| {
            let total = pairwise_sum(&c);
            let iso = m49_to_iso.get(&m)?;
            (total > 0.0).then(|| (iso.clone(), c.map(|x| x / total)))
        })
        .collect())
}

/// Barro-Lee v3 (both sexes, aged 15 to 64, year 2000): shares with at most primary, secondary
/// and tertiary schooling.
pub fn read_barro_lee(path: &Path) -> Result<EduShares, String> {
    let mut csv = open_csv(path)?;
    let (year, sex, code) = (csv.col("year")?, csv.col("sex")?, csv.col("WBcode")?);
    let levels = [
        csv.col("lu")?,
        csv.col("lp")?,
        csv.col("ls")?,
        csv.col("lh")?,
    ];
    let origin = path.display().to_string();
    let y = YEAR.to_string();
    let mut out = BTreeMap::new();
    let mut row = StringRecord::new();
    while csv.next(&mut row)? {
        if cell(&row, year) == y && cell(&row, sex) == "MF" {
            let f = |i: usize| float(cell(&row, i), &origin);
            let (lu, lp, ls, lh) = (f(levels[0])?, f(levels[1])?, f(levels[2])?, f(levels[3])?);
            let total = lu + lp + ls + lh;
            let c = cell(&row, code);
            let iso = BARRO_LEE_CODES
                .iter()
                .find(|(old, _)| *old == c)
                .map_or(c, |(_, new)| new);
            out.insert(iso.to_owned(), [(lu + lp) / total, ls / total, lh / total]);
        }
    }
    Ok(out)
}

/// SWIID summary (zip): disposable income Gini as a fraction by year, for the countries `names`
/// can place.
pub fn read_swiid(
    path: &Path,
    names: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, ByYear>, String> {
    with_member(
        path,
        |n| n.ends_with("_summary.csv"),
        |member| {
            let mut csv = Csv::new(member, path)?;
            let (country, year, gini) =
                (csv.col("country")?, csv.col("year")?, csv.col("gini_disp")?);
            let origin = path.display().to_string();
            let mut out: BTreeMap<String, ByYear> = BTreeMap::new();
            let mut row = StringRecord::new();
            while csv.next(&mut row)? {
                let c = cell(&row, country);
                let iso = SWIID_NAMES
                    .iter()
                    .find(|(n, _)| *n == c)
                    .map(|(_, i)| *i)
                    .or_else(|| names.get(c).map(String::as_str));
                let g = cell(&row, gini);
                if let Some(iso) = iso.filter(|i| !i.is_empty())
                    && !g.is_empty()
                {
                    let v = float(g, &origin)? / 100.0;
                    out.entry(iso.to_owned())
                        .or_default()
                        .insert(int(cell(&row, year), &origin)?, v);
                }
            }
            Ok(out)
        },
    )
}

/// UN WPP total population (gzip CSV): persons in `YEAR`, medium variant, by ISO code (rows
/// without one are aggregates).
pub fn read_wpp(path: &Path) -> Result<BTreeMap<String, f64>, String> {
    let file = std::fs::File::open(path).map_err(|e| super::err(path, e))?;
    // The file is several gzip members back to back.
    let mut csv = Csv::new(
        flate2::read::MultiGzDecoder::new(std::io::BufReader::new(file)),
        path,
    )?;
    let (iso, variant, time, total) = (
        csv.col("ISO3_code")?,
        csv.col("Variant")?,
        csv.col("Time")?,
        csv.col("PopTotal")?,
    );
    let origin = path.display().to_string();
    let year = YEAR.to_string();
    let mut out = BTreeMap::new();
    let mut row = StringRecord::new();
    while csv.next(&mut row)? {
        let code = cell(&row, iso);
        if !code.is_empty() && cell(&row, variant) == "Medium" && cell(&row, time) == year {
            // WPP counts in thousands.
            out.insert(code.to_owned(), float(cell(&row, total), &origin)? * 1000.0);
        }
    }
    Ok(out)
}

/// PWT 11 series by economy and year.
#[derive(Debug, Default)]
pub struct Pwt {
    /// Labour share (labsh).
    pub labour_share: BTreeMap<String, ByYear>,
    /// GDP per person (rgdpna / pop).
    pub income: BTreeMap<String, ByYear>,
    /// Population, millions.
    pub population: BTreeMap<String, ByYear>,
    /// Price level of domestic absorption, C + I + G (pl_da, PPP over the market rate; not 1 for
    /// the US; RFC 0282 D7).
    pub price_level: BTreeMap<String, ByYear>,
    /// How the price level of `YEAR` was made (i_cig: Benchmark, Interpolated, Extrapolated or an
    /// ICP time-series flag; RFC 0282 D8).
    pub price_basis: BTreeMap<String, String>,
}

/// PWT 11 Data sheet: labour share, GDP per person, population and the price level of domestic
/// absorption by year, and the price level's basis in `YEAR`.
pub fn read_pwt(path: &Path) -> Result<Pwt, String> {
    let origin = path.display().to_string();
    let number = |c: &Cell, name: &str| {
        c.to_number()
            .ok_or_else(|| format!("{origin}: {name} '{}' is not a number", c.to_text()))
    };
    let mut out = Pwt::default();
    for r in xlsx::sheet(path, "Data")? {
        let code = r.get("countrycode").map_or("None".into(), Cell::to_text);
        let year = r
            .get("year")
            .ok_or_else(|| format!("{origin}: row without year"))?;
        // int() of a float year truncates toward zero.
        let year = number(year, "year")?.trunc() as i64;
        if let Some(c) = r.get("labsh") {
            out.labour_share
                .entry(code.clone())
                .or_default()
                .insert(year, number(c, "labsh")?);
        }
        if let Some(c) = r.get("pl_da") {
            out.price_level
                .entry(code.clone())
                .or_default()
                .insert(year, number(c, "pl_da")?);
        }
        if let (Some(c), YEAR) = (r.get("i_cig"), year) {
            out.price_basis.insert(code.clone(), c.to_text());
        }
        if let Some(c) = r.get("pop") {
            let pop = number(c, "pop")?;
            out.population
                .entry(code.clone())
                .or_default()
                .insert(year, pop);
            let truthy = match c {
                Cell::Number(x) => *x != 0.0,
                Cell::Text(s) => !s.is_empty(),
            };
            if let (Some(g), true) = (r.get("rgdpna"), truthy) {
                out.income
                    .entry(code)
                    .or_default()
                    .insert(year, number(g, "rgdpna")? / pop);
            }
        }
    }
    Ok(out)
}

/// Series from the sources, keyed by ISO 3166-1 alpha-3; scalars are the year nearest 2000.
#[derive(Debug, Default)]
pub struct Observed {
    pub participation: BTreeMap<String, f64>,
    pub unemployment: BTreeMap<String, f64>,
    /// Mean over `U_STAR_YEARS`.
    pub natural_unemployment: BTreeMap<String, f64>,
    pub working_age: BTreeMap<String, f64>,
    pub edu_wic: EduShares,
    pub edu_barro_lee: EduShares,
    pub labour_share: BTreeMap<String, f64>,
    pub gini_wdi: BTreeMap<String, f64>,
    pub gini_swiid: BTreeMap<String, f64>,
    pub gdp_per_capita: BTreeMap<String, f64>,
    /// PWT price level of domestic absorption (pl_da) by year, not relative to the US.
    pub price_level_pwt: BTreeMap<String, ByYear>,
    /// WDI population in `YEAR`.
    pub population: BTreeMap<String, f64>,
    /// UN WPP population in `YEAR`, for economies WDI does not list.
    pub population_wpp: BTreeMap<String, f64>,
    pub region: BTreeMap<String, String>,
}

fn scaled(m: BTreeMap<String, f64>, by: f64) -> BTreeMap<String, f64> {
    m.into_iter().map(|(k, v)| (k, v / by)).collect()
}

/// Every people source from the fetch cache (`fetch.toml` names).
pub fn load_observed(cache: &Path, admin0: &Path) -> Result<Observed, String> {
    let (_, m49) = read_codes(admin0)?;
    let m49_to_iso: BTreeMap<i64, String> = m49.into_iter().map(|(i, m)| (m, i)).collect();
    let gini_zip = cache.join("wdi_SI.POV.GINI.zip");
    let (region, table_names) = read_wdi_regions(&gini_zip)?;
    let mut names = iso_by_name(admin0)?;
    names.extend(table_names);
    let mut pwt = read_pwt(&cache.join("pwt110.xlsx"))?;
    let pop = read_wdi(&cache.join("wdi_SP.POP.TOTL.zip"))?;
    Ok(Observed {
        participation: read_ilostat(
            &cache.join("ilostat_EAP_2WAP_SEX_AGE_RT_A_2000.csv"),
            "AGE_YTHADULT_Y15-64",
        )?,
        unemployment: read_ilostat(
            &cache.join("ilostat_UNE_2EAP_SEX_AGE_RT_A_2000.csv"),
            "AGE_YTHADULT_YGE15",
        )?,
        natural_unemployment: read_ilostat_mean(
            &cache.join("ilostat_UNE_2EAP_SEX_AGE_RT_A_1995-2005.csv"),
            "AGE_YTHADULT_YGE15",
            U_STAR_YEARS.0,
            U_STAR_YEARS.1,
        )?,
        working_age: scaled(
            nearest(&read_wdi(&cache.join("wdi_SP.POP.1564.TO.ZS.zip"))?),
            100.0,
        ),
        edu_wic: read_wic(&cache.join("BPJresult_AGE_V01.csv"), &m49_to_iso)?,
        edu_barro_lee: read_barro_lee(&cache.join("BL_v3_MF1564.csv"))?,
        labour_share: nearest(&pwt.labour_share),
        gini_wdi: scaled(nearest(&read_wdi(&gini_zip)?), 100.0),
        gini_swiid: nearest(&read_swiid(&cache.join("swiid9_92.zip"), &names)?),
        gdp_per_capita: nearest(&pwt.income),
        price_level_pwt: std::mem::take(&mut pwt.price_level),
        population: pop
            .into_iter()
            .filter_map(|(k, v)| Some((k, *v.get(&YEAR)?)))
            .collect(),
        population_wpp: read_wpp(&cache.join(WPP))?,
        region,
    })
}
