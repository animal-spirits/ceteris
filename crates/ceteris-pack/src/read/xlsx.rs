//! Worksheet rows as maps from the header row's names to cell values.

use std::collections::BTreeMap;
use std::path::Path;

use calamine::{Data, Reader, Xlsx, open_workbook};

use crate::numeric::repr_f64;

use super::err;

/// A cell with a value. Numbers stay `f64` whatever their format: a date is its serial number
/// (1 January 2000 is 36526) and a boolean is 1 or 0.
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    Number(f64),
    Text(String),
}

impl Cell {
    pub fn number(&self) -> Option<f64> {
        match self {
            Cell::Number(x) => Some(*x),
            Cell::Text(_) => None,
        }
    }

    pub fn text(&self) -> Option<&str> {
        match self {
            Cell::Text(s) => Some(s),
            Cell::Number(_) => None,
        }
    }

    /// Python's `str` of the value.
    pub fn to_text(&self) -> String {
        match self {
            Cell::Number(x) => repr_f64(*x),
            Cell::Text(s) => s.clone(),
        }
    }

    /// Python's `float` of the value: a number, or text that parses as one.
    pub fn to_number(&self) -> Option<f64> {
        match self {
            Cell::Number(x) => Some(*x),
            Cell::Text(s) => crate::numeric::parse_f64(s),
        }
    }
}

/// One row: header name to value, cells without a value (empty or an error such as `#REF!`)
/// left out.
pub type Row = BTreeMap<String, Cell>;

fn value(data: &Data) -> Option<Cell> {
    match data {
        Data::Float(x) => Some(Cell::Number(*x)),
        Data::Int(i) => Some(Cell::Number(*i as f64)),
        Data::DateTime(d) => Some(Cell::Number(d.as_f64())),
        Data::Bool(b) => Some(Cell::Number(if *b { 1.0 } else { 0.0 })),
        Data::String(s) | Data::DateTimeIso(s) | Data::DurationIso(s) => {
            Some(Cell::Text(s.clone()))
        }
        Data::Error(_) | Data::Empty => None,
    }
}

/// The rows of worksheet `name` below its first row with a value, which names the columns
/// (numbers by their Python `str`); a row without any value is skipped, and cells in a column
/// without a name are left out.
pub fn sheet(path: &Path, name: &str) -> Result<impl Iterator<Item = Row>, String> {
    let mut book: Xlsx<_> = open_workbook(path).map_err(|e| err(path, e))?;
    let range = book
        .worksheet_range(name)
        .map_err(|e| err(path, format!("{name}: {e}")))?;
    let mut rows = range.rows().map(|row| {
        row.iter()
            .enumerate()
            .filter_map(|(j, d)| Some((j, value(d)?)))
            .collect::<Vec<_>>()
    });
    let header: BTreeMap<usize, String> = rows
        .by_ref()
        .find(|cells| !cells.is_empty())
        .unwrap_or_default()
        .into_iter()
        .map(|(j, c)| (j, c.to_text()))
        .collect();
    let out: Vec<Row> = rows
        .filter(|cells| !cells.is_empty())
        .map(|cells| {
            cells
                .into_iter()
                .filter_map(|(j, c)| Some((header.get(&j)?.clone(), c)))
                .collect()
        })
        .collect();
    Ok(out.into_iter())
}
