//! Proxies of the province downscaling: DOSE regional manufacturing, EIA SEDS state oil and
//! gas production, and the GEM coal mine and oil and gas field trackers.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use csv::StringRecord;

use super::xlsx::{self, Cell, Row};
use super::{ByYear, YEAR, cell, float, int, nearest_year, open_csv};
use crate::numeric::median;

/// Cubic metres of natural gas per barrel of oil equivalent (5,800 cubic feet).
pub const M3_PER_BOE: f64 = 5800.0 * 0.0283168466;
/// DOSE regional accounts.
pub const DOSE: &str = "DOSE_V2.14.csv";
/// EIA State Energy Data System.
pub const SEDS: &str = "Complete_SEDS.csv";
/// SEDS series: crude oil production and marketed natural gas production, billion Btu.
pub const SEDS_SERIES: [&str; 2] = ["PAPRB", "NGMPB"];
/// Statuses of coal mines that may have produced in 2000 (closed mines are judged by their closing year).
pub const COAL_OPEN: [&str; 2] = ["Operating", "Mothballed"];
/// Statuses of oil and gas fields that may have produced in 2000.
pub const FIELD_PRODUCED: [&str; 4] = ["operating", "mothballed", "abandoned", "decommissioning"];

/// One country's DOSE regions in the year nearest 2000.
#[derive(Debug, Clone, PartialEq)]
pub struct Dose {
    /// Manufacturing GVA (local currency) by region name.
    pub values: BTreeMap<String, f64>,
    /// Population of these regions over the country's, at most 1.
    pub coverage: f64,
}

/// DOSE manufacturing GVA by region for each ISO country, of the year nearest `year` within
/// `window` years. `coverage` is the regions' people over the national `population` (2000),
/// 1 where that is unknown.
pub fn read_dose(
    path: &Path,
    population: &BTreeMap<String, f64>,
    year: i64,
    window: i64,
) -> Result<BTreeMap<String, Dose>, String> {
    let mut csv = open_csv(path)?;
    let (gid, yr, region, man, pop) = (
        csv.col("GID_0")?,
        csv.col("year")?,
        csv.col("region")?,
        csv.col("man_grp_pc_lcu")?,
        csv.col("pop")?,
    );
    let origin = path.display().to_string();
    type Regions = BTreeMap<String, f64>;
    let mut values: BTreeMap<String, BTreeMap<i64, Regions>> = BTreeMap::new();
    let mut people: BTreeMap<String, ByYear> = BTreeMap::new();
    let mut row = StringRecord::new();
    while csv.next(&mut row)? {
        let (m, p) = (cell(&row, man), cell(&row, pop));
        if !m.is_empty() && !p.is_empty() {
            let (m, p, y) = (
                float(m, &origin)?,
                float(p, &origin)?,
                int(cell(&row, yr), &origin)?,
            );
            let code = cell(&row, gid);
            *values
                .entry(code.to_owned())
                .or_default()
                .entry(y)
                .or_default()
                .entry(cell(&row, region).to_owned())
                .or_insert(0.0) += m * p;
            *people
                .entry(code.to_owned())
                .or_default()
                .entry(y)
                .or_insert(0.0) += p;
        }
    }
    let mut out = BTreeMap::new();
    for (code, mut years) in values {
        let index: ByYear = years.keys().map(|y| (*y, *y as f64)).collect();
        let Some(found) = nearest_year(&index, year, window) else {
            continue;
        };
        let found = found as i64;
        let coverage = match population.get(&code) {
            Some(total) if *total != 0.0 => {
                let share = people[&code][&found] / total;
                if 1.0 < share { 1.0 } else { share }
            }
            _ => 1.0,
        };
        let values = years.remove(&found).unwrap_or_default();
        out.insert(code, Dose { values, coverage });
    }
    Ok(out)
}

/// EIA SEDS crude oil plus marketed natural gas production in `year` (billion Btu) by state
/// code, with the federal offshore areas X3 (Gulf of Mexico) and X5 (Pacific); the US total is
/// left out.
pub fn read_seds(path: &Path, year: i64) -> Result<BTreeMap<String, f64>, String> {
    let mut csv = open_csv(path)?;
    let (msn, yr, state, data) = (
        csv.col("MSN")?,
        csv.col("Year")?,
        csv.col("StateCode")?,
        csv.col("Data")?,
    );
    let origin = path.display().to_string();
    let year = year.to_string();
    let mut out = BTreeMap::new();
    let mut row = StringRecord::new();
    while csv.next(&mut row)? {
        let d = cell(&row, data);
        if SEDS_SERIES.contains(&cell(&row, msn))
            && cell(&row, yr) == year
            && cell(&row, state) != "US"
            && !d.is_empty()
        {
            *out.entry(cell(&row, state).to_owned()).or_insert(0.0) += float(d, &origin)?;
        }
    }
    Ok(out)
}

/// A coal mine or an oil and gas field.
#[derive(Debug, Clone, PartialEq)]
pub struct Asset {
    pub lat: f64,
    pub lon: f64,
    pub weight: f64,
    /// A start (or discovery, or opening) year is known.
    pub dated: bool,
    pub offshore: bool,
    /// As GEM writes it.
    pub country: String,
}

/// An asset row before its coordinates are checked and its missing weight filled.
struct Located {
    country: String,
    lat: Option<f64>,
    lon: Option<f64>,
    weight: Option<f64>,
    dated: bool,
    offshore: bool,
}

fn number(r: &Row, name: &str) -> Option<f64> {
    r.get(name).and_then(Cell::to_number)
}

/// Python's `str` of a cell that may be missing.
fn text(r: &Row, name: &str) -> String {
    r.get(name).map_or_else(|| "None".to_owned(), Cell::to_text)
}

fn is_text(r: &Row, name: &str, among: &[&str]) -> bool {
    r.get(name)
        .and_then(Cell::text)
        .is_some_and(|t| among.contains(&t))
}

/// Assets with valid coordinates. A missing weight is 0 in a country where any asset has a
/// figure; where none has, every asset takes the median positive figure of all assets.
fn located(rows: Vec<Located>) -> Vec<Asset> {
    let weighted: BTreeSet<&str> = rows
        .iter()
        .filter(|r| r.weight.is_some())
        .map(|r| r.country.as_str())
        .collect();
    let known: Vec<f64> = rows
        .iter()
        .filter_map(|r| r.weight)
        .filter(|w| *w > 0.0)
        .collect();
    let fill = median(known).unwrap_or(1.0);
    let mut out = Vec::new();
    for r in &rows {
        let (Some(mut lat), Some(mut lon)) = (r.lat, r.lon) else {
            continue;
        };
        if lat.abs() > 90.0 && 90.0 >= lon.abs() {
            // Some GEM rows have latitude and longitude swapped.
            std::mem::swap(&mut lat, &mut lon);
        }
        if lat.abs() > 90.0 || lon.abs() > 180.0 {
            continue;
        }
        let weight = r
            .weight
            .unwrap_or(if weighted.contains(&r.country.as_str()) {
                0.0
            } else {
                fill
            });
        out.push(Asset {
            lat,
            lon,
            weight,
            dated: r.dated,
            offshore: r.offshore,
            country: r.country.clone(),
        });
    }
    out
}

/// GEM coal mines open in 2000 (Mt per year): non-closed mines operating or mothballed and
/// opened by 2000 (or of unknown opening), and closed mines that closed in 2000 or later. Weight
/// is reported production as given (0 stays 0), capacity only where production is missing.
pub fn read_gem_coal(path: &Path) -> Result<Vec<Asset>, String> {
    let year = YEAR as f64;
    let mut rows = Vec::new();
    for sheet in ["Non-closed mines", "Closed mines"] {
        let closed = sheet == "Closed mines";
        for r in xlsx::sheet(path, sheet)? {
            let (opening, closing) = (number(&r, "Opening Year"), number(&r, "Closing Year"));
            if opening.is_some_and(|o| o > year) {
                continue;
            }
            if closed && closing.is_none_or(|c| c < year) {
                continue;
            }
            if !closed && !is_text(&r, "Status", &COAL_OPEN) {
                continue;
            }
            rows.push(Located {
                country: text(&r, "Country / Area"),
                lat: number(&r, "Latitude"),
                lon: number(&r, "Longitude"),
                weight: number(&r, "Production (Mtpa)").or_else(|| number(&r, "Capacity (Mtpa)")),
                dated: opening.is_some(),
                offshore: false,
            });
        }
    }
    Ok(located(rows))
}

/// A cell as a dictionary key, equal where Python's values are.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Number(String),
    Text(String),
}

fn key(c: &Cell) -> Key {
    match c {
        Cell::Number(x) => Key::Number(crate::numeric::repr_f64(if *x == 0.0 { 0.0 } else { *x })),
        Cell::Text(s) => Key::Text(s.clone()),
    }
}

/// GEM oil and gas fields that may have produced in 2000 (million boe per year).
///
/// A field counts when its status shows it has produced and its production start (else
/// discovery) year is 2000 or earlier, or unknown (`dated` false). Weight is its latest
/// reported production, 0 included: the largest liquids and the largest gas figure of that
/// year, gas at `M3_PER_BOE`, or a boe figure.
pub fn read_gem_oil_gas(path: &Path) -> Result<Vec<Asset>, String> {
    let origin = path.display().to_string();
    let mut latest: BTreeMap<Key, f64> = BTreeMap::new();
    let mut by_unit: BTreeMap<Key, BTreeMap<String, f64>> = BTreeMap::new();
    for r in xlsx::sheet(path, "Field-level production data")? {
        let (Some(uid), Some(year), Some(q)) = (
            r.get("Unit ID"),
            number(&r, "Data Year"),
            number(&r, "Quantity (converted)"),
        ) else {
            continue;
        };
        let uid = key(uid);
        if year > latest.get(&uid).copied().unwrap_or(-1.0) {
            latest.insert(uid.clone(), year);
            by_unit.insert(uid.clone(), BTreeMap::new());
        }
        let last = latest
            .get(&uid)
            .ok_or_else(|| format!("{origin}: no year for unit {uid:?}"))?;
        if year == *last {
            let units = by_unit.entry(uid).or_default();
            let best = units.entry(text(&r, "Units (converted)")).or_insert(0.0);
            if q > *best {
                *best = q;
            }
        }
    }
    let year = YEAR as f64;
    let mut rows = Vec::new();
    for r in xlsx::sheet(path, "Field-level main data")? {
        if !is_text(&r, "Status", &FIELD_PRODUCED) {
            continue;
        }
        let start = number(&r, "Production start year").or_else(|| number(&r, "Discovery year"));
        if start.is_some_and(|s| s > year) {
            continue;
        }
        let weight = r
            .get("Unit ID")
            .and_then(|u| by_unit.get(&key(u)))
            .map(|q| match q.get("million boe/y") {
                Some(boe) => *boe,
                None => {
                    q.get("million bbl/y").copied().unwrap_or(0.0)
                        + q.get("million m³/y").copied().unwrap_or(0.0) / M3_PER_BOE
                }
            });
        rows.push(Located {
            country: text(&r, "Country/Area"),
            lat: number(&r, "Latitude"),
            lon: number(&r, "Longitude"),
            weight,
            dated: start.is_some(),
            offshore: is_text(&r, "Onshore/Offshore", &["offshore"]),
        });
    }
    Ok(located(rows))
}
