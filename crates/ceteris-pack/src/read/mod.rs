//! Readers of the econ sources in the fetch cache: each turns one file into typed rows, as the
//! Python readers of `packs/econ` did, value for value.

pub mod baci;
pub mod downscale;
pub mod geojson;
pub mod icio;
pub mod natural_earth;
pub mod people;
pub mod tariffs;
pub mod wdi;
pub mod xlsx;

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use csv::StringRecord;
use zip::ZipArchive;

/// The reference year of the econ pack.
pub const YEAR: i64 = 2000;
/// A value from another year is used when it lies within this many years of `YEAR`.
pub const WINDOW: i64 = 5;

/// Values by year.
pub type ByYear = BTreeMap<i64, f64>;

/// Value of the year nearest `year` within `window` years (the earlier year on a tie).
pub fn nearest_year(series: &ByYear, year: i64, window: i64) -> Option<f64> {
    series
        .iter()
        .filter(|(y, _)| (**y - year).abs() <= window)
        .min_by_key(|(y, _)| ((**y - year).abs(), **y))
        .map(|(_, v)| *v)
}

/// The nearest-year value of every series that has one.
pub fn nearest(series: &BTreeMap<String, ByYear>) -> BTreeMap<String, f64> {
    series
        .iter()
        .filter_map(|(k, v)| Some((k.clone(), nearest_year(v, YEAR, WINDOW)?)))
        .collect()
}

pub fn err(path: &Path, e: impl std::fmt::Display) -> String {
    format!("{}: {e}", path.display())
}

/// Runs `f` on the first member of the zip at `path` (in directory order) whose name `pick` accepts.
pub fn with_member<T>(
    path: &Path,
    pick: impl Fn(&str) -> bool,
    f: impl FnOnce(&mut dyn Read) -> Result<T, String>,
) -> Result<T, String> {
    let file = File::open(path).map_err(|e| err(path, e))?;
    let mut zip = ZipArchive::new(BufReader::new(file)).map_err(|e| err(path, e))?;
    for i in 0..zip.len() {
        let mut member = zip.by_index(i).map_err(|e| err(path, e))?;
        let name = member.name().map_err(|e| err(path, e))?.into_owned();
        if pick(&name) {
            return f(&mut member);
        }
    }
    Err(err(path, "no matching member"))
}

/// The bytes of the first member of the zip at `path` whose name `pick` accepts.
pub fn read_member(path: &Path, pick: impl Fn(&str) -> bool) -> Result<Vec<u8>, String> {
    with_member(path, pick, |r| {
        let mut out = Vec::new();
        r.read_to_end(&mut out).map_err(|e| err(path, e))?;
        Ok(out)
    })
}

/// A CSV file read as Python's `csv.DictReader` reads it: a header row naming the columns (the
/// last of a repeated name wins), rows of any length, a missing cell read as empty.
pub(crate) struct Csv<R: Read> {
    reader: csv::Reader<R>,
    header: Vec<String>,
    columns: BTreeMap<String, usize>,
    origin: String,
}

impl<R: Read> Csv<R> {
    pub fn new(input: R, origin: &Path) -> Result<Self, String> {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_reader(input);
        let mut header = StringRecord::new();
        reader
            .read_record(&mut header)
            .map_err(|e| err(origin, e))?;
        let header: Vec<String> = header
            .iter()
            .enumerate()
            .map(|(i, name)| {
                if i == 0 {
                    name.trim_start_matches('\u{feff}')
                } else {
                    name
                }
                .to_owned()
            })
            .collect();
        let columns = header
            .iter()
            .enumerate()
            .map(|(i, name)| (name.clone(), i))
            .collect();
        Ok(Csv {
            reader,
            header,
            columns,
            origin: origin.display().to_string(),
        })
    }

    /// Index of a column; a missing column is an error, as the KeyError of the Python reader.
    pub fn col(&self, name: &str) -> Result<usize, String> {
        self.columns
            .get(name)
            .copied()
            .ok_or_else(|| format!("{}: no column {name}", self.origin))
    }

    /// Byte offset just past the row read last.
    pub fn offset(&self) -> u64 {
        self.reader.position().byte()
    }

    /// Reads the next row into `row`; false at the end.
    pub fn next(&mut self, row: &mut StringRecord) -> Result<bool, String> {
        self.reader
            .read_record(row)
            .map_err(|e| format!("{}: {e}", self.origin))
    }
}

/// Cell `i` of a row, empty when the row is short.
pub(crate) fn cell(row: &StringRecord, i: usize) -> &str {
    row.get(i).unwrap_or("")
}

/// Python's `float(text)`, an error where it raises.
pub(crate) fn float(text: &str, origin: &str) -> Result<f64, String> {
    crate::numeric::parse_f64(text)
        .ok_or_else(|| format!("{origin}: could not convert '{text}' to float"))
}

/// Python's `int(text)` of a decimal integer.
pub(crate) fn int(text: &str, origin: &str) -> Result<i64, String> {
    let t = text.trim();
    let t = t.strip_prefix('+').unwrap_or(t);
    if t.is_empty() || t.starts_with('_') || t.ends_with('_') || t.contains("__") {
        return Err(format!("{origin}: invalid literal for int(): '{text}'"));
    }
    t.replace('_', "")
        .parse()
        .map_err(|_| format!("{origin}: invalid literal for int(): '{text}'"))
}

/// Opens a plain CSV file in the cache.
pub(crate) fn open_csv(path: &Path) -> Result<Csv<BufReader<File>>, String> {
    let file = File::open(path).map_err(|e| err(path, e))?;
    Csv::new(BufReader::new(file), path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_year_prefers_the_closest_and_the_earlier_on_a_tie() {
        let s: ByYear = [(1998, 1.0), (2002, 2.0), (2010, 3.0)].into();
        assert_eq!(nearest_year(&s, 2000, 5), Some(1.0));
        let s: ByYear = [(2003, 2.0), (2010, 3.0)].into();
        assert_eq!(nearest_year(&s, 2000, 5), Some(2.0));
        let s: ByYear = [(2010, 3.0)].into();
        assert_eq!(nearest_year(&s, 2000, 5), None);
    }

    #[test]
    fn csv_skips_blank_lines_and_reads_short_rows_as_empty() {
        let text = "\u{feff}a,b,a\n1,2,3\n\n4\n";
        let mut csv = Csv::new(text.as_bytes(), Path::new("t.csv")).unwrap();
        let (a, b) = (csv.col("a").unwrap(), csv.col("b").unwrap());
        let mut row = StringRecord::new();
        let mut got = Vec::new();
        while csv.next(&mut row).unwrap() {
            got.push((cell(&row, a).to_owned(), cell(&row, b).to_owned()));
        }
        assert_eq!(
            got,
            [("3".into(), "2".into()), (String::new(), String::new())]
        );
    }
}
