//! Arrow outputs of the workbench, schema `workbench/v1` (`specs/workbench/SPEC.md`, Outputs).
//! [`TABLES`] lists every table and column; a test holds the Arrow schemas to it.

use std::collections::BTreeMap;
use std::sync::Arc;

use arrow_array::{ArrayRef, Float64Array, RecordBatch, StringArray, UInt32Array};
use arrow_ipc::writer::StreamWriter;
use arrow_schema::{ArrowError, DataType, Field, Schema};

pub const VERSION: &str = "workbench/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Col {
    Text,
    Tick,
    Real,
}

impl Col {
    fn arrow(self) -> DataType {
        match self {
            Col::Text => DataType::Utf8,
            Col::Tick => DataType::UInt32,
            Col::Real => DataType::Float64,
        }
    }
}

/// Every table of the schema with its columns in order.
pub const TABLES: &[(&str, &[(&str, Col)])] = &[
    (
        "outcomes",
        &[
            ("branch", Col::Text),
            ("member", Col::Tick),
            ("tick", Col::Tick),
            ("outcome", Col::Text),
            ("value", Col::Real),
        ],
    ),
    (
        "effects",
        &[
            ("branch", Col::Text),
            ("baseline", Col::Text),
            ("outcome", Col::Text),
            ("tick", Col::Tick),
            ("mean", Col::Real),
            ("lo", Col::Real),
            ("hi", Col::Real),
            ("p05", Col::Real),
            ("p50", Col::Real),
            ("p95", Col::Real),
            ("sign_share", Col::Real),
        ],
    ),
    (
        "channel_shares",
        &[
            ("branch", Col::Text),
            ("outcome", Col::Text),
            ("channel", Col::Text),
            ("share", Col::Real),
            ("lo", Col::Real),
            ("hi", Col::Real),
        ],
    ),
];

pub fn schema(table: &str) -> Option<Schema> {
    let (_, cols) = TABLES.iter().find(|(t, _)| *t == table)?;
    let fields: Vec<Field> = cols
        .iter()
        .map(|&(name, col)| Field::new(name, col.arrow(), false))
        .collect();
    Some(Schema::new(fields).with_metadata(BTreeMap::from([(
        "schema".to_string(),
        VERSION.to_string(),
    )])))
}

pub enum Column {
    Text(Vec<String>),
    Tick(Vec<u32>),
    Real(Vec<f64>),
}

/// A batch of `table` from columns given in the table's order.
pub fn batch(table: &str, columns: Vec<Column>) -> Result<RecordBatch, ArrowError> {
    let schema =
        schema(table).ok_or_else(|| ArrowError::SchemaError(format!("no table {table}")))?;
    let arrays: Vec<ArrayRef> = columns
        .into_iter()
        .map(|c| match c {
            Column::Text(v) => Arc::new(StringArray::from(v)) as ArrayRef,
            Column::Tick(v) => Arc::new(UInt32Array::from(v)),
            Column::Real(v) => Arc::new(Float64Array::from(v)),
        })
        .collect();
    RecordBatch::try_new(Arc::new(schema), arrays)
}

/// The batch as an Arrow IPC stream.
pub fn ipc(batch: &RecordBatch) -> Result<Vec<u8>, ArrowError> {
    let mut out = Vec::new();
    let mut w = StreamWriter::try_new(&mut out, &batch.schema())?;
    w.write(batch)?;
    w.finish()?;
    drop(w);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use arrow_ipc::reader::StreamReader;

    use super::*;

    #[test]
    fn schemas_match_the_listed_columns() {
        for &(table, cols) in TABLES {
            let s = schema(table).unwrap();
            assert_eq!(s.fields().len(), cols.len(), "{table}");
            for (f, &(name, col)) in s.fields().iter().zip(cols) {
                assert_eq!((f.name().as_str(), f.data_type()), (name, &col.arrow()));
            }
            assert_eq!(s.metadata()["schema"], VERSION);
        }
        assert!(schema("nope").is_none());
    }

    #[test]
    fn effects_round_trip_through_ipc() {
        let b = batch(
            "effects",
            vec![
                Column::Text(vec!["tariff".into()]),
                Column::Text(vec!["main".into()]),
                Column::Text(vec!["gdp".into()]),
                Column::Tick(vec![24]),
                Column::Real(vec![-0.5]),
                Column::Real(vec![-0.7]),
                Column::Real(vec![-0.3]),
                Column::Real(vec![-0.8]),
                Column::Real(vec![-0.5]),
                Column::Real(vec![-0.2]),
                Column::Real(vec![1.0]),
            ],
        )
        .unwrap();
        let bytes = ipc(&b).unwrap();
        let mut r = StreamReader::try_new(bytes.as_slice(), None).unwrap();
        assert_eq!(r.next().unwrap().unwrap(), b);
        assert_eq!(r.schema().metadata()["schema"], VERSION);
    }

    #[test]
    fn a_batch_off_the_schema_is_refused() {
        assert!(batch("effects", vec![Column::Tick(vec![1])]).is_err());
        assert!(batch("nope", vec![]).is_err());
    }
}
