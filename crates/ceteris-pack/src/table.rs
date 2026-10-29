//! Writing one pack table as an Arrow IPC file, and reading its columns back.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::sync::Arc;

use arrow_array::types::ArrowPrimitiveType;
use arrow_array::{Array, ArrayRef, PrimitiveArray, RecordBatch};
use arrow_ipc::reader::FileReader;
use arrow_ipc::writer::FileWriter;
use arrow_schema::{ArrowError, Field, Schema};
use sha2::{Digest, Sha256};

pub const PACK_FORMAT: &str = "3";

/// Lowercase hex of a digest, as `hexdigest()` writes it.
pub fn hex(digest: impl AsRef<[u8]>) -> String {
    digest.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

/// One column of a table as arrays of type `A`, one per batch in order; a null is an error.
pub fn arrays<A: Array + Clone + 'static>(path: &Path, name: &str) -> Result<Vec<A>, String> {
    let fail = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    let reader =
        FileReader::try_new(File::open(path).map_err(|e| fail(&e))?, None).map_err(|e| fail(&e))?;
    let mut out = Vec::new();
    for batch in reader {
        let batch = batch.map_err(|e| fail(&e))?;
        let a = batch
            .column_by_name(name)
            .ok_or_else(|| fail(&format!("no column {name}")))?;
        let a = a.as_any().downcast_ref::<A>().ok_or_else(|| {
            fail(&format!(
                "{name} is {}, not the type asked for",
                a.data_type()
            ))
        })?;
        if a.null_count() > 0 {
            return Err(fail(&format!("null in {name}")));
        }
        out.push(a.clone());
    }
    Ok(out)
}

/// One primitive column of a table, every batch in order.
pub fn values<T: ArrowPrimitiveType>(path: &Path, name: &str) -> Result<Vec<T::Native>, String> {
    Ok(arrays::<PrimitiveArray<T>>(path, name)?
        .iter()
        .flat_map(|a| a.values().iter().copied())
        .collect())
}

fn io_error(e: ArrowError) -> io::Error {
    io::Error::other(e)
}

/// Writes `batch` to `path` with `pack_format` in the schema metadata and returns the SHA-256
/// of the file's bytes.
///
/// Arrow serialises schema metadata in hash-map order, which differs between runs, so any
/// entry beyond `pack_format` would make the bytes non-deterministic and is refused.
pub fn write(
    path: &Path,
    batch: &RecordBatch,
    metadata: &BTreeMap<String, String>,
) -> io::Result<[u8; 32]> {
    if !metadata.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "schema metadata beyond pack_format is not byte-deterministic",
        ));
    }
    let schema = Arc::new(
        Schema::new(batch.schema().fields().clone())
            .with_metadata([("pack_format".to_owned(), PACK_FORMAT.to_owned())]),
    );
    let batch = batch
        .clone()
        .with_schema(Arc::clone(&schema))
        .map_err(io_error)?;
    let mut writer =
        FileWriter::try_new(BufWriter::new(File::create(path)?), &schema).map_err(io_error)?;
    writer.write(&batch).map_err(io_error)?;
    writer.finish().map_err(io_error)?;
    writer.into_inner().map_err(io_error)?.flush()?;
    Ok(Sha256::digest(std::fs::read(path)?).into())
}

/// A record batch of `columns`, every field nullable as pyarrow writes them.
pub fn batch(columns: Vec<(String, ArrayRef)>) -> Result<RecordBatch, String> {
    let fields: Vec<Field> = columns
        .iter()
        .map(|(name, a)| Field::new(name, a.data_type().clone(), true))
        .collect();
    RecordBatch::try_new(
        Arc::new(Schema::new(fields)),
        columns.into_iter().map(|(_, a)| a).collect(),
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::{Float64Array, Int64Array};

    fn batch() -> RecordBatch {
        let cols: Vec<(&str, ArrayRef)> = vec![
            ("id", Arc::new(Int64Array::from(vec![1, 2, 3]))),
            ("x", Arc::new(Float64Array::from(vec![0.5, 1.5, 2.5]))),
        ];
        RecordBatch::try_from_iter(cols).unwrap()
    }

    #[test]
    fn table_write_is_byte_deterministic() {
        let dir = tempfile::tempdir().unwrap();
        let none = BTreeMap::new();
        let (p, q) = (dir.path().join("p.arrow"), dir.path().join("q.arrow"));
        let (hp, hq) = (
            write(&p, &batch(), &none).unwrap(),
            write(&q, &batch(), &none).unwrap(),
        );
        assert_eq!(hp, hq);
        assert_eq!(std::fs::read(&p).unwrap(), std::fs::read(&q).unwrap());
        let back = arrow_ipc::reader::FileReader::try_new(File::open(&p).unwrap(), None).unwrap();
        assert_eq!(back.schema().metadata()["pack_format"], "3");
    }

    #[test]
    fn extra_metadata_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let extra: BTreeMap<String, String> = [("a".to_owned(), "1".to_owned())].into();
        let err = write(&dir.path().join("p.arrow"), &batch(), &extra).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }
}
