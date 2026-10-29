//! Map modes: one RGBA colour per feature, uploaded to the feature colour texture (ADR 0010).

use crate::web::Provinces;

/// Neighbour-separating slots, as in the Tauri map (`app/src/modes.ts`).
const COUNTRY: [[u8; 3]; 8] = [
    [0x2a, 0x78, 0xd6],
    [0xeb, 0x68, 0x34],
    [0x1b, 0xaf, 0x7a],
    [0xed, 0xa1, 0x00],
    [0xe8, 0x7b, 0xa4],
    [0x00, 0x83, 0x00],
    [0x62, 0x50, 0xd6],
    [0xe3, 0x49, 0x48],
];

pub fn political(p: &Provinces) -> Vec<[u8; 4]> {
    p.country
        .iter()
        .map(|&c| {
            let [r, g, b] = COUNTRY[p.countries.colour[c as usize] as usize % COUNTRY.len()];
            [r, g, b, 255]
        })
        .collect()
}
