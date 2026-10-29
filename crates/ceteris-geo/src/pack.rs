//! Reading the world pack built by `ceteris pack build` (Section 16.1): Arrow IPC files, one
//! record batch per table.

use std::fs::File;
use std::path::{Path, PathBuf};

use arrow_array::cast::AsArray;
use arrow_array::types::{Float64Type, UInt8Type, UInt32Type};
use arrow_array::{Array, RecordBatch};
use arrow_ipc::reader::FileReader;

use crate::graph::{EdgeList, Mode, NodeKind, NodeList, TransportGraph, TransportParams};

/// Bumped with `PACK_FORMAT` in `ceteris_pack::table`.
pub const PACK_FORMAT: &str = "3";

#[derive(Debug)]
pub struct PackError(pub String);

impl std::fmt::Display for PackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "pack: {}", self.0)
    }
}

impl std::error::Error for PackError {}

fn err(e: impl std::fmt::Display) -> PackError {
    PackError(e.to_string())
}

pub fn read_table(path: &Path) -> Result<RecordBatch, PackError> {
    let file = File::open(path).map_err(|e| err(format!("{}: {e}", path.display())))?;
    let mut batches: Vec<RecordBatch> = FileReader::try_new(file, None)
        .map_err(err)?
        .collect::<Result<_, _>>()
        .map_err(err)?;
    match batches.len() {
        1 => Ok(batches.pop().unwrap()),
        n => Err(err(format!(
            "{}: expected one record batch, found {n}",
            path.display()
        ))),
    }
}

fn column<'a>(batch: &'a RecordBatch, name: &str) -> Result<&'a dyn Array, PackError> {
    batch
        .column_by_name(name)
        .map(|c| c.as_ref())
        .ok_or_else(|| err(format!("missing column '{name}'")))
}

pub fn f64s(batch: &RecordBatch, name: &str) -> Result<Vec<f64>, PackError> {
    Ok(column(batch, name)?
        .as_primitive_opt::<Float64Type>()
        .ok_or_else(|| err(format!("'{name}' is not f64")))?
        .values()
        .to_vec())
}

pub fn u32s(batch: &RecordBatch, name: &str) -> Result<Vec<u32>, PackError> {
    Ok(column(batch, name)?
        .as_primitive_opt::<UInt32Type>()
        .ok_or_else(|| err(format!("'{name}' is not u32")))?
        .values()
        .to_vec())
}

pub fn u8s(batch: &RecordBatch, name: &str) -> Result<Vec<u8>, PackError> {
    Ok(column(batch, name)?
        .as_primitive_opt::<UInt8Type>()
        .ok_or_else(|| err(format!("'{name}' is not u8")))?
        .values()
        .to_vec())
}

pub fn strings(batch: &RecordBatch, name: &str) -> Result<Vec<String>, PackError> {
    let a = column(batch, name)?;
    if let Some(s) = a.as_string_opt::<i32>() {
        return Ok(s.iter().map(|v| v.unwrap_or_default().to_owned()).collect());
    }
    let s = a
        .as_string_opt::<i64>()
        .ok_or_else(|| err(format!("'{name}' is not a string")))?;
    Ok(s.iter().map(|v| v.unwrap_or_default().to_owned()).collect())
}

/// Province attributes the core needs; the map reads the full table from `web/`.
#[derive(Clone, Debug)]
pub struct Provinces {
    pub id: Vec<String>,
    pub name: Vec<String>,
    pub country: Vec<String>,
    pub lon: Vec<f64>,
    pub lat: Vec<f64>,
}

impl Provinces {
    pub fn len(&self) -> usize {
        self.id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.id.is_empty()
    }

    pub fn index_of(&self, id: &str) -> Option<u32> {
        self.id.iter().position(|x| x == id).map(|i| i as u32)
    }
}

pub struct Pack {
    pub dir: PathBuf,
    pub provinces: Provinces,
    pub chokepoints: Vec<String>,
    pub graph: TransportGraph,
}

impl Pack {
    pub fn load(dir: &Path, params: &TransportParams) -> Result<Self, PackError> {
        let p = read_table(&dir.join("provinces.arrow"))?;
        let format = p
            .schema()
            .metadata()
            .get("pack_format")
            .cloned()
            .unwrap_or_default();
        if format != PACK_FORMAT {
            return Err(err(format!(
                "pack format '{format}', expected '{PACK_FORMAT}'; rebuild the pack"
            )));
        }
        let provinces = Provinces {
            id: strings(&p, "id")?,
            name: strings(&p, "name")?,
            country: strings(&p, "country")?,
            lon: f64s(&p, "lon")?,
            lat: f64s(&p, "lat")?,
        };
        let n = read_table(&dir.join("nodes.arrow"))?;
        let kind = u8s(&n, "kind")?
            .into_iter()
            .map(|k| NodeKind::from_u8(k).ok_or_else(|| err(format!("unknown node kind {k}"))))
            .collect::<Result<_, _>>()?;
        let nodes = NodeList {
            kind,
            province: u32s(&n, "province")?,
            lon: f64s(&n, "lon")?,
            lat: f64s(&n, "lat")?,
        };
        let e = read_table(&dir.join("edges.arrow"))?;
        let mode = u8s(&e, "mode")?
            .into_iter()
            .map(|m| Mode::from_u8(m).ok_or_else(|| err(format!("unknown mode {m}"))))
            .collect::<Result<_, _>>()?;
        let edges = EdgeList {
            a: u32s(&e, "a")?,
            b: u32s(&e, "b")?,
            mode,
            length_km: f64s(&e, "length_km")?,
            tri_m: f64s(&e, "tri_m")?,
            chokepoint: u8s(&e, "chokepoint")?,
        };
        let chokepoints = strings(&read_table(&dir.join("chokepoints.arrow"))?, "name")?;
        let graph = TransportGraph::new(provinces.len(), nodes, edges, params);
        Ok(Self {
            dir: dir.to_owned(),
            provinces,
            chokepoints,
            graph,
        })
    }

    /// Chokepoint id (1-based, as stored on edges) by name.
    pub fn chokepoint(&self, name: &str) -> Option<u8> {
        self.chokepoints
            .iter()
            .position(|c| c == name)
            .map(|i| i as u8 + 1)
    }
}

/// The default pack location: `$CETERIS_PACK`, else `data/packs/2000` in the repository.
pub fn default_dir() -> PathBuf {
    std::env::var_os("CETERIS_PACK")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/packs/2000"))
}
