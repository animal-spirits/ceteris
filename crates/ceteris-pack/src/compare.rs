//! Comparison of two directories of Arrow tables with the tolerances of ADR 0113: integer,
//! string, boolean and categorical columns must be equal, float columns agree to a relative
//! difference per element.

use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;

use arrow_array::cast::AsArray;
use arrow_array::types::{Float32Type, Float64Type};
use arrow_array::{Array, ArrayRef, RecordBatch};
use arrow_cast::cast;
use arrow_ipc::reader::FileReader;
use arrow_schema::DataType;

#[derive(Clone, Copy, Debug)]
pub struct Tolerance {
    pub rel: f64,
}

impl Tolerance {
    pub const DEFAULT: Tolerance = Tolerance { rel: 1e-12 };
    /// For values set by an iterative solve.
    pub const ITERATIVE: Tolerance = Tolerance { rel: 1e-9 };
}

impl Default for Tolerance {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, PartialEq)]
pub struct ColumnDiff {
    pub name: String,
    /// Largest relative difference of a float column; 0 for an equal column of any type.
    pub max_rel: f64,
    pub ok: bool,
    pub note: String,
}

#[derive(Debug)]
pub struct TableDiff {
    pub name: String,
    pub columns: Vec<ColumnDiff>,
    /// Problems with the table as a whole: missing on one side, differing row counts.
    pub issues: Vec<String>,
}

impl TableDiff {
    pub fn ok(&self) -> bool {
        self.issues.is_empty() && self.columns.iter().all(|c| c.ok)
    }
}

#[derive(Debug)]
pub struct Report {
    pub tables: Vec<TableDiff>,
    pub ok: bool,
}

impl Report {
    /// One line per column, and one per table-level issue.
    pub fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        for t in &self.tables {
            for issue in &t.issues {
                out.push(format!("{}: FAIL {issue}", t.name));
            }
            for c in &t.columns {
                let status = if c.ok { "ok" } else { "FAIL" };
                let note = if c.note.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", c.note)
                };
                out.push(format!(
                    "{}.{}: {status} max_rel={:e}{note}",
                    t.name, c.name, c.max_rel
                ));
            }
        }
        out
    }
}

/// Reads every `*.arrow` file of `dir` as one concatenated batch per table, keyed by file stem.
fn read_dir(dir: &Path) -> Result<BTreeMap<String, RecordBatch>, String> {
    let mut tables = BTreeMap::new();
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().is_none_or(|e| e != "arrow") {
            continue;
        }
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let file = File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let reader = FileReader::try_new(file, None).map_err(|e| format!("{name}: {e}"))?;
        let schema = reader.schema();
        let batches: Vec<RecordBatch> = reader
            .collect::<Result<_, _>>()
            .map_err(|e| format!("{name}: {e}"))?;
        let batch = arrow_select::concat::concat_batches(&schema, &batches)
            .map_err(|e| format!("{name}: {e}"))?;
        tables.insert(name, batch);
    }
    Ok(tables)
}

fn relative(a: f64, b: f64) -> f64 {
    if a == b || (a.is_nan() && b.is_nan()) {
        0.0
    } else if !a.is_finite() || !b.is_finite() {
        f64::INFINITY
    } else {
        (a - b).abs() / a.abs().max(b.abs())
    }
}

fn floats(a: &ArrayRef) -> Option<Vec<Option<f64>>> {
    match a.data_type() {
        DataType::Float64 => Some(a.as_primitive::<Float64Type>().iter().collect()),
        DataType::Float32 => Some(
            a.as_primitive::<Float32Type>()
                .iter()
                .map(|v| v.map(f64::from))
                .collect(),
        ),
        _ => None,
    }
}

fn column(name: &str, a: &ArrayRef, b: &ArrayRef, tol: Tolerance) -> ColumnDiff {
    let fail = |note: String| ColumnDiff {
        name: name.to_owned(),
        max_rel: f64::INFINITY,
        ok: false,
        note,
    };
    if a.data_type() != b.data_type() {
        return fail(format!("type {} vs {}", a.data_type(), b.data_type()));
    }
    if let (Some(x), Some(y)) = (floats(a), floats(b)) {
        let mut max_rel = 0.0f64;
        for (p, q) in x.iter().zip(&y) {
            match (p, q) {
                (Some(p), Some(q)) => max_rel = max_rel.max(relative(*p, *q)),
                (None, None) => {}
                _ => return fail("null in one column only".into()),
            }
        }
        return ColumnDiff {
            name: name.to_owned(),
            max_rel,
            ok: max_rel <= tol.rel,
            note: String::new(),
        };
    }
    // Dictionary keys are not part of the value, so compare categoricals by their labels.
    let decode = |x: &ArrayRef| match x.data_type() {
        DataType::Dictionary(_, value) => cast(x, value).unwrap_or_else(|_| x.clone()),
        _ => x.clone(),
    };
    let equal = decode(a) == decode(b);
    ColumnDiff {
        name: name.to_owned(),
        max_rel: if equal { 0.0 } else { f64::INFINITY },
        ok: equal,
        note: if equal {
            String::new()
        } else {
            "values differ".into()
        },
    }
}

fn table(name: &str, a: &RecordBatch, b: &RecordBatch, tol: Tolerance) -> TableDiff {
    let mut diff = TableDiff {
        name: name.to_owned(),
        columns: Vec::new(),
        issues: Vec::new(),
    };
    if a.num_rows() != b.num_rows() {
        diff.issues
            .push(format!("rows {} vs {}", a.num_rows(), b.num_rows()));
        return diff;
    }
    let (sa, sb) = (a.schema(), b.schema());
    for field in sa.fields() {
        match sb.index_of(field.name()) {
            Ok(j) => diff.columns.push(column(
                field.name(),
                a.column_by_name(field.name()).unwrap(),
                b.column(j),
                tol,
            )),
            Err(_) => diff
                .issues
                .push(format!("column {} missing in B", field.name())),
        }
    }
    for field in sb.fields() {
        if sa.index_of(field.name()).is_err() {
            diff.issues
                .push(format!("column {} missing in A", field.name()));
        }
    }
    diff
}

/// Compares the tables of `a` against those of `b`, row by row in file order.
pub fn tables(a: &Path, b: &Path, tol: Tolerance) -> Result<Report, String> {
    tables_named(a, b, tol, None)
}

/// As [`tables`], restricted to one table when `only` is given.
pub fn tables_named(
    a: &Path,
    b: &Path,
    tol: Tolerance,
    only: Option<&str>,
) -> Result<Report, String> {
    let (ta, tb) = (read_dir(a)?, read_dir(b)?);
    let mut names: Vec<&String> = ta.keys().chain(tb.keys()).collect();
    names.sort();
    names.dedup();
    let mut out = Vec::new();
    for name in names
        .into_iter()
        .filter(|n| only.is_none_or(|o| o == n.as_str()))
    {
        out.push(match (ta.get(name), tb.get(name)) {
            (Some(x), Some(y)) => table(name, x, y, tol),
            (Some(_), None) => missing(name, "B"),
            _ => missing(name, "A"),
        });
    }
    if let Some(o) = only
        && out.is_empty()
    {
        return Err(format!("no table named {o}"));
    }
    let ok = out.iter().all(TableDiff::ok);
    Ok(Report { tables: out, ok })
}

fn missing(name: &str, side: &str) -> TableDiff {
    TableDiff {
        name: name.to_owned(),
        columns: Vec::new(),
        issues: vec![format!("table missing in {side}")],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::write;
    use arrow_array::{Float64Array, Int64Array, StringArray};
    use std::sync::Arc;

    fn put(dir: &Path, name: &str, cols: Vec<(&str, ArrayRef)>) {
        let batch = RecordBatch::try_from_iter(cols).unwrap();
        write(&dir.join(format!("{name}.arrow")), &batch, &BTreeMap::new()).unwrap();
    }

    fn f(v: &[f64]) -> ArrayRef {
        Arc::new(Float64Array::from(v.to_vec()))
    }

    fn pair() -> (tempfile::TempDir, tempfile::TempDir) {
        (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap())
    }

    #[test]
    fn compare_flags_float_beyond_tolerance() {
        let (a, b) = pair();
        put(a.path(), "t", vec![("x", f(&[1.0, 2.0]))]);
        put(b.path(), "t", vec![("x", f(&[1.0, 2.0 * (1.0 + 5e-12)]))]);
        let r = tables(a.path(), b.path(), Tolerance::DEFAULT).unwrap();
        assert!(!r.ok);
        assert!(r.tables[0].columns[0].max_rel > 1e-12);
        let r = tables(a.path(), b.path(), Tolerance::ITERATIVE).unwrap();
        assert!(r.ok);
        put(b.path(), "t", vec![("x", f(&[1.0, 2.0 * (1.0 + 1e-13)]))]);
        assert!(tables(a.path(), b.path(), Tolerance::DEFAULT).unwrap().ok);
    }

    #[test]
    fn compare_requires_equal_ints_and_strings() {
        let (a, b) = pair();
        let ints = |v: Vec<i64>| -> ArrayRef { Arc::new(Int64Array::from(v)) };
        let strs = |v: Vec<&str>| -> ArrayRef { Arc::new(StringArray::from(v)) };
        put(
            a.path(),
            "t",
            vec![("n", ints(vec![1, 2])), ("s", strs(vec!["a", "b"]))],
        );
        put(
            b.path(),
            "t",
            vec![("n", ints(vec![1, 3])), ("s", strs(vec!["a", "c"]))],
        );
        let r = tables(a.path(), b.path(), Tolerance::ITERATIVE).unwrap();
        assert!(!r.ok);
        assert!(r.tables[0].columns.iter().all(|c| !c.ok));
        put(
            b.path(),
            "t",
            vec![("n", ints(vec![1, 2])), ("s", strs(vec!["a", "b"]))],
        );
        assert!(tables(a.path(), b.path(), Tolerance::DEFAULT).unwrap().ok);
    }

    #[test]
    fn compare_reports_missing_column() {
        let (a, b) = pair();
        put(a.path(), "t", vec![("x", f(&[1.0])), ("y", f(&[2.0]))]);
        put(b.path(), "t", vec![("x", f(&[1.0]))]);
        put(b.path(), "extra", vec![("x", f(&[1.0]))]);
        let r = tables(a.path(), b.path(), Tolerance::DEFAULT).unwrap();
        assert!(!r.ok);
        let t = r.tables.iter().find(|t| t.name == "t").unwrap();
        assert_eq!(t.issues, ["column y missing in B"]);
        let e = r.tables.iter().find(|t| t.name == "extra").unwrap();
        assert_eq!(e.issues, ["table missing in A"]);
    }
}
