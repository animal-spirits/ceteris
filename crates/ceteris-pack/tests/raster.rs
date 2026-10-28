//! Pixel to H3 cell to province mapping at raster edges, against the mapping rasterio and h3
//! give (`tests/data/pixel_cells.txt`, recorded from the Python pipeline).

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::Path;

use ceteris_pack::raster::geotiff;
use ceteris_pack::raster::population::{CellOwner, cell_of, pixel_centre};
use ceteris_pack::sources;
use common::cached;
use h3o::Resolution;

struct Window {
    raster: String,
    base: Resolution,
    row0: usize,
    col0: usize,
    cols: usize,
    transform: [f64; 6],
    /// `(cell, owner, centre)` per pixel, row-major.
    pixels: Vec<(u64, i64, (f64, f64))>,
}

fn windows() -> Vec<Window> {
    let text = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/pixel_cells.txt"),
    )
    .unwrap();
    let mut out: Vec<Window> = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split(' ').collect();
        if f[0] == "window" {
            let x = |i: usize| f[i].parse::<f64>().unwrap();
            let n = |i: usize| f[i].parse::<usize>().unwrap();
            out.push(Window {
                raster: f[1].to_owned(),
                base: Resolution::try_from(f[2].parse::<u8>().unwrap()).unwrap(),
                row0: n(3),
                col0: n(4),
                cols: n(6),
                transform: [x(7), 0.0, x(8), 0.0, x(9), x(10)],
                pixels: Vec::with_capacity(n(5) * n(6)),
            });
        } else {
            let w = out.last_mut().unwrap();
            w.pixels.push((
                f[0].parse().unwrap(),
                f[1].parse().unwrap(),
                (f[2].parse().unwrap(), f[3].parse().unwrap()),
            ));
        }
    }
    out
}

/// The last `*_A.tif` member by name, whose transform `spam_physical_area` keeps.
fn last_spam_member(path: &Path) -> geotiff::Raster<std::io::Cursor<Vec<u8>>> {
    let mut zip = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
    let name = (0..zip.len())
        .map(|i| zip.by_index(i).unwrap().name().unwrap().into_owned())
        .filter(|n| n.ends_with("_A.tif"))
        .max()
        .unwrap();
    let mut bytes = Vec::new();
    zip.by_name(&name).unwrap().read_to_end(&mut bytes).unwrap();
    geotiff::from_bytes(bytes, &name).unwrap()
}

fn centre(w: &Window, i: usize) -> (f64, f64) {
    pixel_centre(&w.transform, w.row0 + i / w.cols, w.col0 + i % w.cols)
}

#[test]
fn pixel_cells_match_python_on_border_windows() {
    let windows = windows();
    assert_eq!(windows.len(), 12);
    for w in &windows {
        for (i, &(cell, _, python)) in w.pixels.iter().enumerate() {
            let (lat, lng) = centre(w, i);
            assert_eq!(
                (lat.to_bits(), lng.to_bits()),
                (python.0.to_bits(), python.1.to_bits()),
                "{} pixel centre {i}",
                w.raster
            );
            assert_eq!(
                u64::from(cell_of(lat, lng, w.base).unwrap()),
                cell,
                "{} pixel ({}, {})",
                w.raster,
                w.row0 + i / w.cols,
                w.col0 + i % w.cols
            );
        }
    }

    let cells = sources::workspace_root().join("data/packs/2000/cells.arrow");
    if !cells.exists() {
        eprintln!("skipping owners: no {}", cells.display());
        return;
    }
    let mut owner = CellOwner::read(&cells).unwrap();
    let mut refined = 0;
    for w in &windows {
        for (i, &(cell, province, _)) in w.pixels.iter().enumerate() {
            let (lat, lng) = centre(w, i);
            assert_eq!(
                owner.of_point(lat, lng).unwrap(),
                province,
                "{} pixel ({}, {})",
                w.raster,
                w.row0 + i / w.cols,
                w.col0 + i % w.cols
            );
            let base = h3o::CellIndex::try_from(cell).unwrap();
            if owner.is_split(base) && owner.of_base(base) != province {
                refined += 1;
            }
        }
    }
    assert!(refined > 0, "no window reaches a finer cell");
}

#[test]
fn raster_transforms_match_rasterio() {
    let windows = windows();
    let want: BTreeMap<&str, [f64; 6]> = windows
        .iter()
        .map(|w| (w.raster.as_str(), w.transform))
        .collect();
    for (raster, want) in want {
        let got = match raster {
            "worldpop" => {
                cached("ppp_2000_1km_Aggregated.tif").map(|p| geotiff::open(&p).unwrap().transform)
            }
            "kummu" => cached("kummu_gdp_ppp_2000_5arcmin.tif")
                .map(|p| geotiff::open(&p).unwrap().transform),
            "mapspam" => cached("spam2000v3.0.7_global_physical-area.geotiff.zip")
                .map(|p| last_spam_member(&p).transform),
            other => panic!("unknown raster {other}"),
        };
        if let Some(got) = got {
            assert_eq!(got, want, "{raster}");
        }
    }
}
