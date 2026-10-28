//! Terrain and H3 cell areas of the pack's cells, on the ETOPO raster of the fetch cache.

mod common;

use std::fs::File;
use std::path::Path;

use arrow_array::cast::AsArray;
use arrow_array::types::{Float64Type, UInt32Type, UInt64Type};
use arrow_array::{ArrowPrimitiveType, RecordBatch};
use arrow_ipc::reader::FileReader;
use ceteris_pack::geo::raster_terrain::{
    Elevation, SAMPLE_DEPTH, cell_area_km2, province_means, terrain,
};
use ceteris_pack::raster::geotiff;
use common::{cached, pack_table};
use h3o::CellIndex;

const ETOPO: &str = "ETOPO_2022_v1_60s_N90W180_surface.tif";

fn column<T: ArrowPrimitiveType>(path: &Path, name: &str) -> Vec<T::Native> {
    let reader = FileReader::try_new(File::open(path).unwrap(), None).unwrap();
    let batches: Vec<RecordBatch> = reader.map(Result::unwrap).collect();
    batches
        .iter()
        .flat_map(|b| {
            b.column_by_name(name)
                .unwrap()
                .as_primitive::<T>()
                .values()
                .to_vec()
        })
        .collect()
}

fn cells(path: &Path) -> Vec<CellIndex> {
    column::<UInt64Type>(path, "cell")
        .into_iter()
        .map(|c| CellIndex::try_from(c).unwrap())
        .collect()
}

#[test]
fn etopo_reads_as_rasterio_reads_it() {
    // Values rasterio reads at (row, col): the corners, tile edges and the antimeridian column.
    let Some(path) = cached(ETOPO) else {
        return;
    };
    let mut r = geotiff::open(&path).unwrap();
    assert_eq!((r.width, r.height), (21600, 10800));
    assert_eq!(
        r.transform,
        [1.0 / 60.0, 0.0, -180.0, 0.0, -1.0 / 60.0, 90.0]
    );
    assert_eq!(r.nodata, Some(-99999.0));
    for (row, col, v) in [
        (0, 0, -4153.25),
        (255, 256, -2193.25),
        (3618, 15394, 238.2595977783203),
        (5400, 10800, -4934.4130859375),
        (4000, 21599, -5653.5625),
        (10799, 21599, 2831.8251953125),
    ] {
        assert_eq!(
            f64::from(r.window(row, 1).unwrap()[col]),
            v,
            "({row}, {col})"
        );
    }
}

#[test]
fn h3o_cell_areas_match_the_pack_within_the_measured_gap() {
    // h3 and h3o sum the same Cagnoli terms over boundary vertices that differ in the last bits;
    // at resolution 4 the sums differ by up to 3.5e-11 relative.
    let Some(path) = pack_table("cells.arrow") else {
        return;
    };
    let ours = cell_area_km2(&cells(&path));
    let pack = column::<Float64Type>(&path, "area_km2");
    let worst = ours
        .iter()
        .zip(&pack)
        .map(|(a, b)| (a / b - 1.0).abs())
        .fold(0.0, f64::max);
    assert!(worst < 1e-10, "{worst}");
}

#[test]
fn terrain_of_the_pack_cells_matches_the_pack() {
    if cfg!(debug_assertions) {
        eprintln!("skipping: decoding all of ETOPO takes minutes unoptimised; run with --release");
        return;
    }
    let (Some(etopo), Some(path), Some(provinces)) = (
        cached(ETOPO),
        pack_table("cells.arrow"),
        pack_table("provinces.arrow"),
    ) else {
        return;
    };
    let z = Elevation::open(&etopo).unwrap();
    let ids = cells(&path);
    let t = terrain(&ids, &z, SAMPLE_DEPTH).unwrap();
    assert_eq!(t.elev_mean_m, column::<Float64Type>(&path, "elev_mean_m"));
    assert_eq!(t.elev_std_m, column::<Float64Type>(&path, "elev_std_m"));
    assert_eq!(t.tri_m, column::<Float64Type>(&path, "tri_m"));
    // Weighted by the pack's own cell areas, the province means are exact too.
    let owner = column::<UInt32Type>(&path, "province");
    let area = column::<Float64Type>(&path, "area_km2");
    let n = column::<Float64Type>(&provinces, "elev_mean_m").len();
    for (name, values) in [
        ("elev_mean_m", &t.elev_mean_m),
        ("elev_std_m", &t.elev_std_m),
        ("tri_m", &t.tri_m),
    ] {
        let means = province_means(&owner, &area, values, n);
        assert_eq!(means, column::<Float64Type>(&provinces, name), "{name}");
    }
}
