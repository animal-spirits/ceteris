//! WID.world bulk data: the 2000 shares of net personal wealth (adults, equal split,
//! `shweal992j`) of the bottom 50%, middle 40% and top 10%, nearest year within the window
//! (ADR 0308).

use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use csv::StringRecord;
use zip::ZipArchive;

use super::{ByYear, Csv, WINDOW, YEAR, cell, err, float, nearest_year};

/// The bulk download in the fetch cache.
pub const WID: &str = "wid_all_data.zip";
pub const VARIABLE: &str = "shweal992j";
pub const PERCENTILES: [&str; 3] = ["p0p50", "p50p90", "p90p100"];

/// Shares of the three brackets by WID country code (ISO 3166 alpha-2), each the value of the
/// year nearest 2000 within the window; a country missing any bracket is left out.
pub fn read_wid(path: &Path) -> Result<BTreeMap<String, [f64; 3]>, String> {
    let file = File::open(path).map_err(|e| err(path, e))?;
    let mut zip = ZipArchive::new(BufReader::new(file)).map_err(|e| err(path, e))?;
    let mut series: BTreeMap<(String, usize), ByYear> = BTreeMap::new();
    for i in 0..zip.len() {
        let member = zip.by_index(i).map_err(|e| err(path, e))?;
        let name = member.name().map_err(|e| err(path, e))?.into_owned();
        let base = name.rsplit('/').next().unwrap_or(&name);
        if !base.starts_with("WID_data_") || !base.ends_with(".csv") {
            continue;
        }
        let mut csv = Csv::new_delimited(member, path, b';')?;
        let cols = ["country", "variable", "percentile", "year", "value"].map(|c| csv.col(c));
        let [country, variable, percentile, year, value] = cols;
        let (country, variable, percentile, year, value) =
            (country?, variable?, percentile?, year?, value?);
        let mut row = StringRecord::new();
        let origin = format!("{}:{name}", path.display());
        while csv.next(&mut row)? {
            if cell(&row, variable) != VARIABLE {
                continue;
            }
            let Some(b) = PERCENTILES
                .iter()
                .position(|&p| p == cell(&row, percentile))
            else {
                continue;
            };
            let (y, v) = (cell(&row, year), cell(&row, value));
            if v.is_empty() {
                continue;
            }
            let y = float(y, &origin)? as i64;
            series
                .entry((cell(&row, country).to_owned(), b))
                .or_default()
                .insert(y, float(v, &origin)?);
        }
    }
    let mut out: BTreeMap<String, [Option<f64>; 3]> = BTreeMap::new();
    for ((country, b), years) in &series {
        out.entry(country.clone()).or_default()[*b] = nearest_year(years, YEAR, WINDOW);
    }
    Ok(out
        .into_iter()
        .filter_map(|(c, s)| Some((c, [s[0]?, s[1]?, s[2]?])))
        .collect())
}
