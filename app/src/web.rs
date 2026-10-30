//! Attributes from the pack's `web/provinces.json` and `web/counties.json` (columnar; index =
//! feature).

use std::path::Path;

use serde::Deserialize;
use serde::de::DeserializeOwned;

fn read<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

#[derive(Clone, Default, Deserialize)]
pub struct Countries {
    pub code: Vec<String>,
    pub name: Vec<String>,
    pub sovereign: Vec<String>,
    /// Slot of a neighbour-separating palette (the pack's `PALETTE_SIZE`).
    pub colour: Vec<u8>,
}

pub const TERRAIN: [&str; 4] = ["Plains", "Plateau", "Hills", "Mountains"];

/// USGS MRDS commodity groups, as the pack's `res_<group>` columns.
pub const RESOURCE_GROUPS: [&str; 5] = ["precious", "base", "ferrous", "energy", "industrial"];

#[derive(Clone, Default, Deserialize)]
pub struct Provinces {
    pub id: Vec<String>,
    pub name: Vec<String>,
    /// Index into `countries`.
    pub country: Vec<u32>,
    pub area_km2: Vec<f64>,
    pub elev_mean_m: Vec<f64>,
    pub tri_m: Vec<f64>,
    /// Index into `TERRAIN`.
    pub terrain: Vec<u8>,
    pub coastal: Vec<bool>,
    pub dev_slice: Vec<bool>,
    pub ports: Vec<u32>,
    pub airports: Vec<u32>,
    pub rail: Vec<bool>,
    pub res_precious: Vec<u32>,
    pub res_base: Vec<u32>,
    pub res_ferrous: Vec<u32>,
    pub res_energy: Vec<u32>,
    pub res_industrial: Vec<u32>,
    pub countries: Countries,
}

impl Provinces {
    pub fn read(path: &Path) -> Result<Self, String> {
        read(path)
    }

    pub fn len(&self) -> usize {
        self.id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.id.is_empty()
    }

    /// Mineral sites of one group (an index into `RESOURCE_GROUPS`), or of all with `None`.
    pub fn sites(&self, group: Option<usize>, i: usize) -> u32 {
        let columns = [
            &self.res_precious,
            &self.res_base,
            &self.res_ferrous,
            &self.res_energy,
            &self.res_industrial,
        ];
        match group {
            Some(g) => columns[g][i],
            None => columns.iter().map(|c| c[i]).sum(),
        }
    }
}

#[derive(Deserialize)]
pub struct Counties {
    pub name: Vec<String>,
    pub level: Vec<String>,
    /// Index into the provinces.
    pub province: Vec<u32>,
    pub area_km2: Vec<f64>,
    pub elev_mean_m: Vec<f64>,
    pub tri_m: Vec<f64>,
}

impl Counties {
    pub fn read(path: &Path) -> Result<Self, String> {
        read(path)
    }
}
