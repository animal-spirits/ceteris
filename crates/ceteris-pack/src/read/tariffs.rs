//! Trade agreements in force in 2000: the CEPII gravity RTA flags and the World Bank Deep Trade
//! Agreements pair list.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use csv::StringRecord;

use super::xlsx::{self, Cell};
use super::{Csv, YEAR, cell, float, int, with_member};

/// CEPII gravity package.
pub const GRAVITY: &str = "Gravity_csv_V202211.zip";
/// World Bank DTA 1.0 horizontal content workbook.
pub const DTA: &str = "DTA 1.0 - Horizontal Content (v2).xlsx";
/// CEPII rta_type codes that remove tariffs on goods: customs unions and free trade agreements,
/// with or without an EIA.
pub const CEPII_FREE: [i64; 4] = [1, 2, 4, 5];
/// DTA agreement types that remove tariffs on goods.
pub const DTA_FREE: [&str; 4] = ["CU", "CU & EIA", "FTA", "FTA & EIA"];
/// 1 January 2000 as an Excel serial date (the DTA workbook's dates).
pub const NEW_YEAR_SERIAL: f64 = 36526.0;

/// Pairs of source codes.
pub type Pairs = BTreeSet<(String, String)>;

/// pyarrow's default null spellings, which its float columns read as null.
const NULLS: [&str; 17] = [
    "", "#N/A", "#N/A N/A", "#NA", "-1.#IND", "-1.#QNAN", "-NaN", "-nan", "1.#IND", "1.#QNAN",
    "N/A", "NA", "NULL", "NaN", "n/a", "nan", "null",
];

/// Source code pairs with a goods FTA or customs union flagged in force in both 1999 and 2000.
pub fn read_cepii(path: &Path) -> Result<Pairs, String> {
    let origin = path.display().to_string();
    let (mut before, mut now) = (Pairs::new(), Pairs::new());
    with_member(
        path,
        |n| n.starts_with("Gravity_V"),
        |member| {
            let mut csv = Csv::new(member, path)?;
            let [year, o, d, fta, rta] =
                ["year", "iso3_o", "iso3_d", "fta_wto", "rta_type"].map(|c| csv.col(c));
            let (year, o, d, fta, rta) = (year?, o?, d?, fta?, rta?);
            let number = |text: &str| -> Result<Option<f64>, String> {
                if NULLS.contains(&text) {
                    Ok(None)
                } else {
                    float(text, &origin).map(Some)
                }
            };
            let mut row = StringRecord::new();
            while csv.next(&mut row)? {
                let y = cell(&row, year);
                let set = if y == "1999" {
                    &mut before
                } else if y == "2000" {
                    &mut now
                } else {
                    continue;
                };
                if number(cell(&row, fta))? != Some(1.0) {
                    continue;
                }
                if let Some(t) = number(cell(&row, rta))?
                    && CEPII_FREE.contains(&(t.trunc() as i64))
                {
                    set.insert((cell(&row, o).to_owned(), cell(&row, d).to_owned()));
                }
            }
            Ok(())
        },
    )?;
    Ok(before.intersection(&now).cloned().collect())
}

/// Source code pairs in a goods FTA or customs union in force on 1 January 2000 (World Bank DTA
/// pair list).
///
/// A pair listed for 2000 is in force on 1 January when it is also listed for an earlier year,
/// or when its agreement entered into force on 1 January 2000.
pub fn read_dta(path: &Path) -> Result<Pairs, String> {
    let origin = path.display().to_string();
    let mut entry: BTreeMap<i64, f64> = BTreeMap::new();
    for r in xlsx::sheet(path, "Agreements")? {
        if let (Some(Cell::Number(id)), Some(Cell::Number(date))) =
            (r.get("WB ID"), r.get("Date of Entry into Force (G)"))
        {
            entry.insert(id.trunc() as i64, *date);
        }
    }
    let mut first: BTreeMap<(String, String, i64), f64> = BTreeMap::new();
    let mut listed = BTreeSet::new();
    for r in xlsx::sheet(path, "Bilateral Information")? {
        let Some(Cell::Number(year)) = r.get("year") else {
            continue;
        };
        let free = r
            .get("type")
            .and_then(Cell::text)
            .is_some_and(|t| DTA_FREE.contains(&t));
        if !free || *year > YEAR as f64 {
            continue;
        }
        let text = |name: &str| {
            r.get(name)
                .map(Cell::to_text)
                .ok_or_else(|| format!("{origin}: row without {name}"))
        };
        let wbid = match r.get("WBID") {
            Some(Cell::Number(x)) => x.trunc() as i64,
            Some(Cell::Text(t)) => int(t, &origin)?,
            None => return Err(format!("{origin}: row without WBID")),
        };
        let key = (text("iso1")?, text("iso2")?, wbid);
        let f = first.entry(key.clone()).or_insert(f64::INFINITY);
        *f = f.min(*year);
        if *year == YEAR as f64 {
            listed.insert(key);
        }
    }
    Ok(listed
        .into_iter()
        .filter(|key| first[key] < YEAR as f64 || entry.get(&key.2) == Some(&NEW_YEAR_SERIAL))
        .map(|(a, b, _)| (a, b))
        .collect())
}
