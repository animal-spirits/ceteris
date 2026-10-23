use ceteris_geo::graph::TransportParams;
use ceteris_geo::pack::{Pack, default_dir};

/// The built pack, or None (with a notice) where it has not been built, as in CI.
pub fn pack() -> Option<Pack> {
    let dir = default_dir();
    if !dir.join("provinces.arrow").exists() {
        eprintln!(
            "skipping: no pack at {}; build it with `uv run python -m ceteris.packs`",
            dir.display()
        );
        return None;
    }
    Some(Pack::load(&dir, &TransportParams::default()).expect("pack loads"))
}

pub fn province(pack: &Pack, id: &str) -> u32 {
    pack.provinces
        .index_of(id)
        .unwrap_or_else(|| panic!("province {id} not in pack"))
}

/// Rotterdam, Hamburg, Shanghai, Mumbai, Ulaanbaatar, Nairobi, Sao Paulo, New York.
pub const PROBES: [&str; 8] = [
    "NLD-3485",
    "DEU-1578",
    "CHN-1819",
    "IND-2447.1",
    "MNG-3331",
    "KEN-1196",
    "BRA-1311",
    "USA-3559",
];
