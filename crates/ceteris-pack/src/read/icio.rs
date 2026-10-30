//! OECD ICIO 2025 (small version) country input-output structure by good, and the income series
//! (WDI, ADB Key Indicators, Maddison Project) the imputation of other economies uses.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use csv::StringRecord;

use super::wdi::read_wdi;
use super::xlsx::{self, Cell};
use super::{
    ByYear, Csv, WINDOW, YEAR, cell, err, float, int, nearest_year, open_csv, with_member,
};
use crate::numeric::pairwise_sum;

/// Number of goods.
pub const N_GOODS: usize = 10;
/// Final demand columns that enter the shares.
pub const FINAL_DEMAND: [&str; 5] = ["HFCE", "NPISH", "GGFC", "GFCF", "INVNT"];
const SPECIAL_ROWS: [&str; 3] = ["TLS", "VA", "OUT"];
/// ADB Key Indicators file of GDP per capita (Cook Islands).
pub const ADB_INCOME: &str = "adb_kidb_NY_GDP_PCAP_CD_COO.csv";
/// Maddison Project Database (North Korea).
pub const MADDISON: &str = "mpd2023_web.xlsx";
/// ADB economy codes that differ from ISO 3166-1 alpha-3.
pub const ADB_ISO: [(&str, &str); 1] = [("COO", "COK")];
/// GDP per person is taken from the year nearest 2000 within these years.
pub const INCOME_YEARS: (i64, i64) = (1995, 2010);

/// Good of an ICIO industry. The 2025 edition has 50 industries; the older names stay mapped,
/// except the combined coal, oil and gas industry B05_06, which this reader refuses.
pub fn icio_good(industry: &str) -> Result<usize, String> {
    Ok(match industry {
        "A01" | "A02" | "A03" | "A01_02" => 0,
        "B06" | "B09" => 1,
        "B05" | "B07" | "B08" | "B07_08" => 2,
        "C10T12" => 3,
        "C13T15" | "C16" | "C17_18" | "C31T33" => 4,
        "C19" | "C20" | "C21" | "C22" | "C23" | "C24" | "C24A" | "C24B" | "C25" => 5,
        "C26" | "C27" | "C28" | "C29" | "C30" | "C301" | "C302T309" => 6,
        "D" | "E" => 7,
        "F" => 8,
        "G" | "H49" | "H50" | "H51" | "H52" | "H53" | "I" | "J58T60" | "J61" | "J62_63" | "K"
        | "L" | "M" | "N" | "O" | "P" | "Q" | "R" | "S" | "T" => 9,
        "B05_06" => {
            return Err(format!(
                "{industry} combines coal with oil and gas; use the 2025 edition"
            ));
        }
        _ => return Err(format!("unknown ICIO industry {industry}")),
    })
}

/// One economy's 2000 structure by good, read from ICIO.
#[derive(Debug, Clone, PartialEq)]
pub struct CountryIo {
    /// Million USD.
    pub output: [f64; N_GOODS],
    /// Million USD: output less intermediate inputs, so it includes taxes less subsidies on products.
    pub va: [f64; N_GOODS],
    /// `io[k][g]` is units of k per unit of g, all source economies.
    pub io: [[f64; N_GOODS]; N_GOODS],
    pub final_demand_shares: [f64; N_GOODS],
    /// Coal (B05) in the value added of good 2.
    pub coal_share: f64,
}

/// One year of ICIO: row labels, column names and the numeric columns (without the last, OUT).
struct Table {
    labels: Vec<String>,
    names: Vec<String>,
    columns: Vec<Vec<f64>>,
}

fn read_table(input: impl Read, origin: &Path) -> Result<Table, String> {
    let mut csv = Csv::new(input, origin)?;
    let width = csv.header.len();
    if width < 2 {
        return Err(err(origin, "no numeric columns"));
    }
    let names = csv.header[1..width - 1].to_vec();
    let origin_text = origin.display().to_string();
    let (mut labels, mut columns) = (Vec::new(), vec![Vec::new(); names.len()]);
    let mut row = StringRecord::new();
    while csv.next(&mut row)? {
        labels.push(cell(&row, 0).to_owned());
        for (j, column) in columns.iter_mut().enumerate() {
            let v = cell(&row, j + 1);
            if v == "NA" {
                return Err(format!("{} has NA cells", names[j]));
            }
            column.push(float(v, &origin_text)?);
        }
    }
    Ok(Table {
        labels,
        names,
        columns,
    })
}

fn open_table(path: &Path, year: i64) -> Result<Table, String> {
    if path.extension().is_some_and(|e| e == "zip") {
        let suffix = format!("{year}_SML.csv");
        with_member(
            path,
            |n| n.ends_with(&suffix),
            |member| read_table(member, path),
        )
    } else {
        let file = File::open(path).map_err(|e| err(path, e))?;
        read_table(BufReader::new(file), path)
    }
}

fn split_once(name: &str) -> (&str, Option<&str>) {
    match name.split_once('_') {
        Some((a, b)) => (a, Some(b)),
        None => (name, None),
    }
}

/// `np.bincount(good, weights, N_GOODS)`.
fn bincount(good: &[usize], weights: impl Iterator<Item = f64>) -> [f64; N_GOODS] {
    let mut out = [0.0; N_GOODS];
    for (&g, w) in good.iter().zip(weights) {
        out[g] += w;
    }
    out
}

/// Economy code to `CountryIo`, from an ICIO CSV (rows: industries, TLS, VA, OUT) or a package
/// zip of yearly CSVs.
///
/// Output is the OUT row; input coefficients divide the intermediate use of a good by the output
/// of the buying good, summing the sellers of every source economy. Final demand shares sum the
/// `FINAL_DEMAND` columns over all sellers. A negative share is an error. The coal share is
/// B05's part of good 2's value added.
pub fn aggregate(path: &Path, year: i64) -> Result<BTreeMap<String, CountryIo>, String> {
    let table = open_table(path, year)?;
    let mut rows = Vec::new();
    let mut row_good = Vec::new();
    for (i, label) in table.labels.iter().enumerate() {
        if !SPECIAL_ROWS.contains(&label.as_str()) {
            let industry = split_once(label)
                .1
                .ok_or_else(|| format!("row label {label} has no industry"))?;
            row_good.push(icio_good(industry)?);
            rows.push(i);
        }
    }
    let out_row = table
        .labels
        .iter()
        .position(|l| l == "OUT")
        .ok_or_else(|| err(path, "no OUT row"))?;

    type Matrix = [[f64; N_GOODS]; N_GOODS];
    let mut inputs: BTreeMap<&str, Matrix> = BTreeMap::new();
    let mut output: BTreeMap<&str, [f64; N_GOODS]> = BTreeMap::new();
    let mut fin: BTreeMap<&str, [f64; N_GOODS]> = BTreeMap::new();
    let mut coal: BTreeMap<&str, f64> = BTreeMap::new();
    let mut mining: BTreeMap<&str, f64> = BTreeMap::new();
    let mut picked = vec![0.0; rows.len()];
    for (name, x) in table.names.iter().zip(&table.columns) {
        let (code, tail) = split_once(name);
        let tail = tail.ok_or_else(|| format!("column {name} has no industry"))?;
        for (p, &i) in picked.iter_mut().zip(&rows) {
            *p = x[i];
        }
        if FINAL_DEMAND.contains(&tail) {
            let add = bincount(&row_good, picked.iter().copied());
            let f = fin.entry(code).or_insert([0.0; N_GOODS]);
            for (a, b) in f.iter_mut().zip(add) {
                *a += b;
            }
        } else if tail != "DPABR" {
            let g = icio_good(tail)?;
            let add = bincount(&row_good, picked.iter().copied());
            let m = inputs.entry(code).or_insert([[0.0; N_GOODS]; N_GOODS]);
            for (k, b) in add.into_iter().enumerate() {
                m[k][g] += b;
            }
            output.entry(code).or_insert([0.0; N_GOODS])[g] += x[out_row];
            if g == 2 {
                let va = x[out_row] - pairwise_sum(&picked);
                *mining.entry(code).or_insert(0.0) += va;
                *coal.entry(code).or_insert(0.0) += if tail == "B05" { va } else { 0.0 };
            }
        }
    }

    let mut result = BTreeMap::new();
    for (code, x) in output {
        let f = fin
            .get(code)
            .ok_or_else(|| format!("{code} has no final demand"))?;
        let total = pairwise_sum(f);
        let shares = f.map(|v| v / total);
        if shares.iter().any(|s| *s < 0.0) {
            return Err(format!(
                "negative final demand share for {code}: {shares:?}"
            ));
        }
        let m = &inputs[code];
        let mut io = [[0.0; N_GOODS]; N_GOODS];
        let mut used = [0.0; N_GOODS];
        for k in 0..N_GOODS {
            for g in 0..N_GOODS {
                io[k][g] = if x[g] > 0.0 { m[k][g] / x[g] } else { 0.0 };
                used[g] += m[k][g];
            }
        }
        let mining = mining.get(code).copied().unwrap_or(0.0);
        let coal_share = if mining > 0.0 {
            (coal.get(code).copied().unwrap_or(0.0) / mining).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let va = std::array::from_fn(|g| x[g] - used[g]);
        result.insert(
            code.to_owned(),
            CountryIo {
                output: x,
                va,
                io,
                final_demand_shares: shares,
                coal_share,
            },
        );
    }
    Ok(result)
}

/// Economy code to sales of its industries to every other economy by good (million USD):
/// intermediate use and final demand of the foreign columns.
pub fn exports(path: &Path, year: i64) -> Result<BTreeMap<String, [f64; N_GOODS]>, String> {
    let table = open_table(path, year)?;
    let (mut rows, mut seller, mut good) = (Vec::new(), Vec::new(), Vec::new());
    for (i, label) in table.labels.iter().enumerate() {
        if !SPECIAL_ROWS.contains(&label.as_str()) {
            let (code, industry) = split_once(label);
            rows.push(i);
            seller.push(code);
            good.push(icio_good(
                industry.ok_or_else(|| format!("row label {label} has no industry"))?,
            )?);
        }
    }
    let mut abroad = vec![0.0; rows.len()];
    for (name, x) in table.names.iter().zip(&table.columns) {
        let buyer = split_once(name).0;
        for ((a, &i), s) in abroad.iter_mut().zip(&rows).zip(&seller) {
            *a += if *s != buyer { x[i] } else { 0.0 };
        }
    }
    let mut out: BTreeMap<String, [f64; N_GOODS]> = BTreeMap::new();
    for ((s, g), a) in seller.iter().zip(&good).zip(&abroad) {
        out.entry((*s).to_owned()).or_insert([0.0; N_GOODS])[*g] += a;
    }
    Ok(out)
}

/// Value of the year nearest 2000 within `INCOME_YEARS` (the earlier year on a tie).
pub fn nearest_income(series: &ByYear) -> Option<f64> {
    series
        .iter()
        .filter(|(y, _)| (INCOME_YEARS.0..=INCOME_YEARS.1).contains(*y))
        .min_by_key(|(y, _)| ((**y - YEAR).abs(), **y))
        .map(|(_, v)| *v)
}

/// ADB Key Indicators (SDMX CSV) GDP per capita in current USD: value by year for each ISO code.
pub fn read_adb_income(path: &Path) -> Result<BTreeMap<String, ByYear>, String> {
    let mut csv = open_csv(path)?;
    let (indicator, economy, period, value) = (
        csv.col("INDICATOR")?,
        csv.col("ECONOMY_CODE")?,
        csv.col("TIME_PERIOD")?,
        csv.col("OBS_VALUE")?,
    );
    let origin = path.display().to_string();
    let mut out: BTreeMap<String, ByYear> = BTreeMap::new();
    let mut row = StringRecord::new();
    while csv.next(&mut row)? {
        let v = cell(&row, value);
        if cell(&row, indicator) == "NY_GDP_PCAP_CD" && !v.trim().is_empty() {
            let e = cell(&row, economy);
            let iso = ADB_ISO.iter().find(|(a, _)| *a == e).map_or(e, |(_, i)| i);
            let v = float(v, &origin)?;
            out.entry(iso.to_owned())
                .or_default()
                .insert(int(cell(&row, period), &origin)?, v);
        }
    }
    Ok(out)
}

/// Maddison Project Database GDP per capita (2011 international dollars): value by year for each ISO code.
pub fn read_maddison(path: &Path) -> Result<BTreeMap<String, ByYear>, String> {
    let origin = path.display().to_string();
    let mut out: BTreeMap<String, ByYear> = BTreeMap::new();
    for r in xlsx::sheet(path, "Full data")? {
        if let Some(Cell::Number(gdppc)) = r.get("gdppc") {
            let code = r.get("countrycode").map_or("None".into(), Cell::to_text);
            let year = match r.get("year") {
                Some(Cell::Number(y)) => y.trunc() as i64,
                Some(Cell::Text(y)) => int(y, &origin)?,
                None => return Err(format!("{origin}: row without year")),
            };
            out.entry(code).or_default().insert(year, *gdppc);
        }
    }
    Ok(out)
}

/// GDP per person (current USD) of the year nearest 2000 within `INCOME_YEARS`, for each economy that has one.
pub fn income_by_year(
    gdp: &BTreeMap<String, ByYear>,
    population: &BTreeMap<String, ByYear>,
) -> BTreeMap<String, f64> {
    let empty = ByYear::new();
    gdp.iter()
        .filter_map(|(code, g)| {
            let pop = population.get(code).unwrap_or(&empty);
            let per_person: ByYear = g
                .iter()
                .filter_map(|(y, v)| Some((*y, v / pop.get(y)?)))
                .collect();
            Some((code.clone(), nearest_income(&per_person)?))
        })
        .collect()
}

/// WDI series by ISO code, year nearest 2000: GDP in current USD, sector value added as fractions of GDP.
#[derive(Debug, Default)]
pub struct Wdi {
    pub gdp: BTreeMap<String, f64>,
    /// GDP per person in current USD, nearest year in `INCOME_YEARS`.
    pub income: BTreeMap<String, f64>,
    /// wdi, or adb where WDI has no value.
    pub income_source: BTreeMap<String, String>,
    /// Maddison Project GDP per person, same years.
    pub maddison: BTreeMap<String, f64>,
    pub agriculture: BTreeMap<String, f64>,
    pub industry: BTreeMap<String, f64>,
    pub manufacturing: BTreeMap<String, f64>,
    pub services: BTreeMap<String, f64>,
    /// Oil and natural gas rents, shares of GDP.
    pub oil_rents: BTreeMap<String, f64>,
    pub gas_rents: BTreeMap<String, f64>,
}

/// The WDI series the imputation needs, with ADB and Maddison Project income where WDI has none.
pub fn load_wdi(cache: &Path) -> Result<Wdi, String> {
    let wdi = |indicator: &str| read_wdi(&cache.join(format!("wdi_{indicator}.zip")));
    let series = |indicator: &str, scale: f64| -> Result<BTreeMap<String, f64>, String> {
        Ok(wdi(indicator)?
            .iter()
            .filter_map(|(k, v)| Some((k.clone(), nearest_year(v, YEAR, WINDOW)? / scale)))
            .collect())
    };
    let mut income = income_by_year(&wdi("NY.GDP.MKTP.CD")?, &wdi("SP.POP.TOTL")?);
    let mut source: BTreeMap<String, String> =
        income.keys().map(|k| (k.clone(), "wdi".into())).collect();
    for (code, v) in read_adb_income(&cache.join(ADB_INCOME))? {
        if !income.contains_key(&code)
            && let Some(found) = nearest_income(&v)
        {
            income.insert(code.clone(), found);
            source.insert(code, "adb".into());
        }
    }
    let maddison = read_maddison(&cache.join(MADDISON))?
        .iter()
        .filter_map(|(k, v)| Some((k.clone(), nearest_income(v)?)))
        .collect();
    Ok(Wdi {
        gdp: series("NY.GDP.MKTP.CD", 1.0)?,
        income,
        income_source: source,
        maddison,
        agriculture: series("NV.AGR.TOTL.ZS", 100.0)?,
        industry: series("NV.IND.TOTL.ZS", 100.0)?,
        manufacturing: series("NV.IND.MANF.ZS", 100.0)?,
        services: series("NV.SRV.TOTL.ZS", 100.0)?,
        oil_rents: series("NY.GDP.PETR.RT.ZS", 100.0)?,
        gas_rents: series("NY.GDP.NGAS.RT.ZS", 100.0)?,
    })
}
