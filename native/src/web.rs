//! Attributes from the pack's `web/provinces.json` and `web/counties.json` (columnar; index =
//! feature).

use std::path::Path;

use serde::Deserialize;

#[derive(Deserialize)]
pub struct Countries {
    pub code: Vec<String>,
    pub name: Vec<String>,
    /// Slot of a neighbour-separating palette (the pack's `PALETTE_SIZE`).
    pub colour: Vec<u8>,
}

#[derive(Deserialize)]
pub struct Provinces {
    pub id: Vec<String>,
    pub name: Vec<String>,
    /// Index into `countries`.
    pub country: Vec<u32>,
    pub countries: Countries,
}

impl Provinces {
    pub fn read(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
    }
}

#[derive(Deserialize)]
pub struct Counties {
    pub name: Vec<String>,
    /// Index into the provinces.
    pub province: Vec<u32>,
}

impl Counties {
    pub fn read(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
    }
}
