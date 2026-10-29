//! The econ build on the real pack and cache: the pack's tables hold together, the build is
//! reproducible, its stats are those of the Python build, and a missing optional source falls
//! back as the Python build does without being downloaded.

mod common;

use std::collections::BTreeMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use arrow_array::cast::AsArray;
use arrow_array::types::{Float64Type, UInt8Type, UInt32Type};
use arrow_array::{ArrowPrimitiveType, RecordBatch};
use arrow_ipc::reader::FileReader;
use ceteris_pack::econ::build::{BuildReport, BuildStats, TABLES, WORLDPOP, build};
use ceteris_pack::econ::downscale::{GEM, KUMMU, Level};
use ceteris_pack::econ::icio::ICIO;
use ceteris_pack::read::tariffs::{DTA, GRAVITY};
use ceteris_pack::sources;
use common::{cached, pack_table, strings};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const N_GOODS: usize = 10;
const GOLDEN_P2: &str = "tests/golden/p2-pack.txt";
/// Written by `ceteris calibrate` and installed into the pack (ADR 0089).
const CALIBRATION_TABLES: [&str; 6] = [
    "econ_calib.arrow",
    "econ_calib_cells.arrow",
    "econ_calib_provinces.arrow",
    "econ_calib_pops.arrow",
    "econ_calib_pipeline.arrow",
    "econ_calib_countries.arrow",
];

fn batches(path: &Path) -> Vec<RecordBatch> {
    FileReader::try_new(File::open(path).unwrap(), None)
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn values<T: ArrowPrimitiveType>(path: &Path, column: &str) -> Vec<T::Native> {
    batches(path)
        .iter()
        .flat_map(|b| {
            b.column_by_name(column)
                .unwrap()
                .as_primitive::<T>()
                .values()
                .to_vec()
        })
        .collect()
}

fn rows(path: &Path) -> usize {
    batches(path).iter().map(RecordBatch::num_rows).sum()
}

fn hex(digest: &[u8]) -> String {
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// The real pack and cache, in optimised builds only (the WorldPop sum takes minutes otherwise).
fn inputs() -> Option<(PathBuf, PathBuf)> {
    if cfg!(debug_assertions) {
        eprintln!(
            "skipping: summing WorldPop takes over 10 minutes unoptimised; run with --release"
        );
        return None;
    }
    let pack = pack_table("cells.arrow")?.parent()?.to_owned();
    for name in [WORLDPOP, ICIO, GRAVITY, DTA, GEM[0], GEM[1], KUMMU] {
        cached(name)?;
    }
    let cache = cached(WORLDPOP)?.parent()?.to_owned();
    Some((pack, cache))
}

struct Full {
    pack: PathBuf,
    cache: PathBuf,
    out: TempDir,
    report: BuildReport,
}

fn full() -> Option<&'static Full> {
    static DATA: OnceLock<Option<Full>> = OnceLock::new();
    DATA.get_or_init(|| {
        let (pack, cache) = inputs()?;
        let out = TempDir::new().unwrap();
        let report = build(&pack, out.path(), &cache).unwrap();
        Some(Full {
            pack,
            cache,
            out,
            report,
        })
    })
    .as_ref()
}

fn levels(dir: &Path) -> Vec<u8> {
    let path = dir.join("econ_provinces.arrow");
    (0..N_GOODS)
        .flat_map(|g| values::<UInt8Type>(&path, &format!("va_level_{g}")))
        .collect()
}

/// A cache of links to every file of `cache` but `missing`.
fn cache_without(cache: &Path, missing: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    for entry in std::fs::read_dir(cache).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() != missing {
            std::os::unix::fs::symlink(entry.path(), dir.path().join(entry.file_name())).unwrap();
        }
    }
    dir
}

#[test]
fn the_build_writes_its_tables_in_order() {
    let Some(f) = full() else { return };
    let names: Vec<&str> = f.report.tables.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, TABLES);
}

/// The stats the Python build wrote to the pack manifest, with the imputed VA share after refined
/// petroleum moved into good 1 (0.0743 before).
#[test]
fn build_stats_match_the_python_build() {
    let Some(f) = full() else { return };
    assert_eq!(
        f.report.stats,
        BuildStats {
            gateways: 600,
            gateway_kinds: [287, 168, 145],
            tariff_importers_own: 172,
            tariff_agreement_pairs: 1321,
            dose_match_rate: 0.8339,
            provinces_with_va: 4555,
            va_imputed_share: 0.0739,
        }
    );
    assert_eq!(
        f.report.stats.json().text(),
        r#"{"dose_match_rate":0.8339,"gateway_kinds":{"airport":168,"land crossing":145,"port":287},"gateways":600,"provinces_with_va":4555,"tariff_agreement_pairs":1321,"tariff_importers_own":172,"va_imputed_share":0.0739}"#
    );
}

/// Each good's theta class with the class's harmonic mean (the model's P01) and arithmetic mean
/// (ADR 0023, Amendment 2026-10-29).
#[test]
fn econ_goods_records_both_class_means() {
    let Some(f) = full() else { return };
    let path = f.out.path().join("econ_goods.arrow");
    assert_eq!(
        values::<UInt8Type>(&path, "theta_class"),
        [2, 3, 3, 0, 2, 0, 1, 0, 0, 0]
    );
    let harmonic = values::<Float64Type>(&path, "theta_harmonic");
    let arithmetic = values::<Float64Type>(&path, "theta_arithmetic");
    let want_h = [7.68, 16.91, 16.91, 2.91, 7.68, 2.91, 4.14, 0.0, 0.0, 0.0];
    let want_a = [9.15, 26.51, 26.51, 3.23, 9.15, 3.23, 7.20, 0.0, 0.0, 0.0];
    for g in 0..N_GOODS {
        assert!((harmonic[g] - want_h[g]).abs() <= 0.005, "good {g}");
        assert!((arithmetic[g] - want_a[g]).abs() <= 0.005, "good {g}");
    }
}

#[test]
fn econ_build_is_reproducible() {
    let Some(f) = full() else { return };
    let again = TempDir::new().unwrap();
    let second = build(&f.pack, again.path(), &f.cache).unwrap();
    assert_eq!(second.tables, f.report.tables);
}

/// Each optional (and manual) source the build reads: without it the build succeeds, its proxy
/// level disappears from the VA chains (GEM: coal and oil and gas fall to mineral sites, EIA and
/// population; gridded GDP: DOSE leaves fall to population), every other table is unchanged, and
/// nothing is downloaded into the cache.
#[test]
fn missing_optional_source_falls_back() {
    let Some(f) = full() else { return };
    let full_levels = levels(f.out.path());
    let cases: [(&str, &[Level]); 3] = [
        (GEM[0], &[Level::Gem, Level::Mining]),
        (GEM[1], &[Level::Gem, Level::Mining]),
        (KUMMU, &[Level::Gdp]),
    ];
    for (missing, gone) in cases {
        let cache = cache_without(&f.cache, missing);
        let out = TempDir::new().unwrap();
        let report = build(&f.pack, out.path(), cache.path()).unwrap();
        assert!(
            !cache.path().join(missing).exists(),
            "{missing} was fetched"
        );
        let got = levels(out.path());
        for level in gone {
            let l = *level as u8;
            assert!(full_levels.contains(&l), "{level:?} unused with {missing}");
            assert!(!got.contains(&l), "{level:?} used without {missing}");
        }
        for ((name, digest), (full_name, full_digest)) in report.tables.iter().zip(&f.report.tables)
        {
            assert_eq!(name, full_name);
            if name != "econ_provinces.arrow" {
                assert_eq!(digest, full_digest, "{name} changed without {missing}");
            }
        }
    }
}

/// The ICIO tables are a manual but required download: the build stops naming the file.
#[test]
fn missing_manual_icio_stops_the_build() {
    let Some(f) = full() else { return };
    let cache = cache_without(&f.cache, ICIO);
    let out = TempDir::new().unwrap();
    let err = build(&f.pack, out.path(), cache.path()).unwrap_err();
    assert!(err.contains(ICIO), "{err}");
    assert!(!cache.path().join(ICIO).exists());
}

#[test]
fn pack_econ_tables_are_consistent() {
    let (Some(countries), Some(provinces)) = (
        pack_table("econ_countries.arrow"),
        pack_table("econ_provinces.arrow"),
    ) else {
        return;
    };
    let dir = countries.parent().unwrap();
    let codes = strings(&countries, "code");
    assert_eq!(codes, strings(&dir.join("countries.arrow"), "code"));
    let n = codes.len();
    let p1 = dir.join("provinces.arrow");
    assert_eq!(rows(&provinces), rows(&p1));

    let fd: Vec<Vec<f64>> = (0..N_GOODS)
        .map(|g| values::<Float64Type>(&countries, &format!("fd_{g}")))
        .collect();
    for c in 0..n {
        let sum: f64 = fd.iter().map(|col| col[c]).sum();
        assert!((sum - 1.0).abs() <= 1e-12, "{}: fd sums to {sum}", codes[c]);
    }

    let a = values::<Float64Type>(&dir.join("econ_io.arrow"), "a");
    assert_eq!(a.len(), n * N_GOODS * N_GOODS);
    assert!(a.iter().all(|&x| x >= 0.0));
    for c in 0..n {
        for g in 0..N_GOODS {
            let used: f64 = (0..N_GOODS)
                .map(|k| a[(c * N_GOODS + k) * N_GOODS + g])
                .sum();
            assert!(used < 1.0, "{} good {g}: inputs {used}", codes[c]);
        }
    }

    let index: BTreeMap<&str, usize> = codes
        .iter()
        .enumerate()
        .map(|(i, c)| (c.as_str(), i))
        .collect();
    let country = strings(&p1, "country");
    for g in 0..N_GOODS {
        let va = values::<Float64Type>(&provinces, &format!("va_{g}"));
        let national = values::<Float64Type>(&countries, &format!("va_{g}"));
        let mut by_country = vec![0.0; n];
        for (c, v) in country.iter().zip(&va) {
            by_country[index[c.as_str()]] += v;
        }
        for c in 0..n {
            let (got, want) = (by_country[c], national[c]);
            assert!(
                (got - want).abs() <= 1e-6 + 1e-9 * want.abs(),
                "{} good {g}: provinces {got}, country {want}",
                codes[c]
            );
        }
    }

    let tariffs = dir.join("econ_tariffs.arrow");
    assert_eq!(rows(&tariffs), 2 * n * n);
    assert!(
        values::<Float64Type>(&tariffs, "rate")
            .iter()
            .all(|r| (0.0..1.0).contains(r))
    );

    let pops = dir.join("econ_pops.arrow");
    assert_eq!(rows(&pops), 3 * rows(&provinces));
    let lf = values::<Float64Type>(&pops, "labour_force");
    let pop = values::<Float64Type>(&pops, "population");
    assert!(lf.iter().zip(&pop).all(|(l, p)| l <= p));
    assert!(
        values::<UInt32Type>(&dir.join("gateways.arrow"), "country")
            .iter()
            .all(|&c| (c as usize) < n)
    );
}

/// The econ table digests the manifest records, then those of the calibration tables installed
/// in the pack. Regenerate deliberately with `CETERIS_BLESS=1` after rebuilding or recalibrating,
/// and say why in the commit.
#[test]
fn pack_econ_tables_match_golden() {
    let Some(manifest) = pack_table("manifest.json") else {
        return;
    };
    let dir = manifest.parent().unwrap();
    let text = std::fs::read_to_string(&manifest).unwrap();
    let recorded = |name: &str| {
        let key = format!("\"{name}\": \"");
        let start = text
            .find(&key)
            .unwrap_or_else(|| panic!("{name} not in the manifest"))
            + key.len();
        text[start..start + 64].to_owned()
    };
    let mut lines = vec!["# P2 pack 2000: econ table sha256 from manifest.json".to_owned()];
    lines.extend(
        TABLES
            .iter()
            .map(|name| format!("{name} {}", recorded(name))),
    );
    for name in CALIBRATION_TABLES {
        let path = dir.join(name);
        if path.exists() {
            let digest = Sha256::digest(std::fs::read(&path).unwrap());
            lines.push(format!("{name} {}", hex(&digest)));
        }
    }
    let actual = lines.join("\n") + "\n";
    let golden = sources::workspace_root().join(GOLDEN_P2);
    if std::env::var_os("CETERIS_BLESS").is_some() {
        std::fs::write(&golden, actual).unwrap();
        return;
    }
    assert_eq!(std::fs::read_to_string(golden).unwrap(), actual);
}
