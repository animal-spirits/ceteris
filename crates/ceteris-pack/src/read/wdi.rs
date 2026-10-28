//! World Bank WDI indicator downloads (one zip per indicator).

use std::collections::BTreeMap;
use std::path::Path;

use csv::StringRecord;

use super::{ByYear, Csv, YEAR, cell, err, float, int, nearest_year, read_member};

/// WDI weighted mean applied tariff rates, percent: primary products, manufactured products.
pub const TARIFF_SERIES: [&str; 2] = ["TM.TAX.TCOM.WM.AR.ZS", "TM.TAX.MANF.WM.AR.ZS"];
/// A weighted mean applied rate of 100% or more is a data error; such years count as missing.
pub const MAX_RATE: f64 = 1.0;

fn text(bytes: &[u8], path: &Path) -> Result<String, String> {
    let text = std::str::from_utf8(bytes).map_err(|e| err(path, e))?;
    Ok(text.trim_start_matches('\u{feff}').to_owned())
}

/// Value by year for each economy, in the indicator's own units.
pub fn read_wdi(path: &Path) -> Result<BTreeMap<String, ByYear>, String> {
    let bytes = read_member(path, |n| n.starts_with("API_"))?;
    let text = text(&bytes, path)?;
    let start = text
        .split_inclusive('\n')
        .scan(0, |at, line| {
            let here = *at;
            *at += line.len();
            Some((here, line))
        })
        .find(|(_, line)| line.starts_with("\"Country Name\""))
        .map(|(at, _)| at)
        .ok_or_else(|| err(path, "no \"Country Name\" header"))?;
    let mut csv = Csv::new(&text.as_bytes()[start..], path)?;
    let code = csv.col("Country Code")?;
    let years: Vec<(usize, i64)> = csv
        .columns
        .iter()
        .filter(|(name, _)| !name.is_empty() && name.chars().all(|c| c.is_ascii_digit()))
        .map(|(name, &i)| Ok((i, int(name, "year")?)))
        .collect::<Result<_, String>>()?;
    let origin = path.display().to_string();
    let mut out = BTreeMap::new();
    let mut row = StringRecord::new();
    while csv.next(&mut row)? {
        let mut values = ByYear::new();
        for &(i, year) in &years {
            let v = cell(&row, i);
            if !v.is_empty() {
                values.insert(year, float(v, &origin)?);
            }
        }
        out.insert(cell(&row, code).to_owned(), values);
    }
    Ok(out)
}

/// Text by text key, such as a region by economy code.
pub type Names = BTreeMap<String, String>;

/// World Bank region by economy code (aggregates have none), and economy code by table name.
pub fn read_wdi_regions(path: &Path) -> Result<(Names, Names), String> {
    let bytes = read_member(path, |n| n.starts_with("Metadata_Country"))?;
    let mut csv = Csv::new(bytes.as_slice(), path)?;
    let (code, region, table) = (
        csv.col("Country Code")?,
        csv.col("Region")?,
        csv.col("TableName")?,
    );
    let (mut regions, mut names) = (BTreeMap::new(), BTreeMap::new());
    let mut row = StringRecord::new();
    while csv.next(&mut row)? {
        if !cell(&row, region).is_empty() {
            regions.insert(cell(&row, code).to_owned(), cell(&row, region).to_owned());
            names.insert(cell(&row, table).to_owned(), cell(&row, code).to_owned());
        }
    }
    Ok((regions, names))
}

/// The tariff rate per group as a fraction, the year nearest 2000 within five years, by source code.
pub fn read_rates(cache: &Path) -> Result<Vec<BTreeMap<String, f64>>, String> {
    TARIFF_SERIES
        .iter()
        .map(|s| {
            Ok(plausible_rates(&read_wdi(
                &cache.join(format!("wdi_{s}.zip")),
            )?))
        })
        .collect()
}

/// Rate as a fraction of the year nearest 2000 within five years, leaving out years at or above `MAX_RATE`.
pub fn plausible_rates(series: &BTreeMap<String, ByYear>) -> BTreeMap<String, f64> {
    series
        .iter()
        .filter_map(|(k, by_year)| {
            let kept: ByYear = by_year
                .iter()
                .filter(|(_, v)| **v < 100.0 * MAX_RATE)
                .map(|(y, v)| (*y, *v))
                .collect();
            Some((k.clone(), nearest_year(&kept, YEAR, super::WINDOW)? / 100.0))
        })
        .collect()
}
