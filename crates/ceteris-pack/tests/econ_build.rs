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
use ceteris_pack::econ::build::{BACI, HS_KEY};
use ceteris_pack::econ::build::{BuildReport, BuildStats, TABLES, WORLDPOP, build};
use ceteris_pack::econ::downscale::{GEM, KUMMU, Level};
use ceteris_pack::econ::factors::wealth_proxy;
use ceteris_pack::econ::goods::hs_goods;
use ceteris_pack::econ::icio::{ICIO, INSIDE};
use ceteris_pack::read::baci::{read_hs_cpa, trade_by_good};
use ceteris_pack::read::natural_earth::{ADMIN0, iso3_by_iso2, read_codes};
use ceteris_pack::read::people::read_pwt;
use ceteris_pack::read::tariffs::{DTA, GRAVITY};
use ceteris_pack::read::wdi::read_wdi;
use ceteris_pack::read::wid::{INCOME, WEALTH, read_wid};
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

/// The stats the build writes to the pack manifest (whole provinces, ADR 0251), with the imputed
/// VA share after refined petroleum moved into good 1 and ADRs 0249 and 0256, and two provinces
/// (Cook Islands 710, St Helena 3504) above the working-age floor at UN WPP totals (ADR 0098).
#[test]
fn build_stats_match_the_python_build() {
    let Some(f) = full() else { return };
    assert_eq!(
        f.report.stats,
        BuildStats {
            gateways: 600,
            gateway_kinds: [286, 171, 143],
            tariff_importers_own: 172,
            tariff_agreement_pairs: 1321,
            dose_match_rate: 0.8339,
            provinces_with_va: 4370,
            va_imputed_share: 0.0754,
        }
    );
    assert_eq!(
        f.report.stats.json().text(),
        r#"{"dose_match_rate":0.8339,"gateway_kinds":{"airport":171,"land crossing":143,"port":286},"gateways":600,"provinces_with_va":4370,"tariff_agreement_pairs":1321,"tariff_importers_own":172,"va_imputed_share":0.0754}"#
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

/// ADRs 0282, 0317 and 0289: p_c from PWT pl_da over the US's for 181 pack countries, the level
/// WDI's GDP at PPP implies for 21, the donor for 40; Somalia's PWT level (1.04) gives way to
/// WDI's (0.827), and economies WDI has no GDP at PPP for (New Caledonia, Kosovo) take the
/// donor's.
#[test]
fn price_levels_come_from_pwt_then_wdi_then_the_donor() {
    let Some(f) = full() else { return };
    let path = f.out.path().join("econ_countries.arrow");
    let level = values::<Float64Type>(&path, "price_level");
    let source = strings(&path, "price_level_source");
    let code = strings(&path, "code");
    let imputed: Vec<bool> = batches(&path)
        .iter()
        .flat_map(|b| {
            b.column_by_name("price_level_imputed")
                .unwrap()
                .as_boolean()
                .iter()
                .map(Option::unwrap)
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(level.len(), rows(&path));
    assert!(level.iter().all(|p| p.is_finite() && *p > 0.0));
    let count = |s: &str| source.iter().filter(|x| *x == s).count();
    assert_eq!((count("pwt"), count("wdi"), count("donor")), (181, 21, 40));
    for i in 0..code.len() {
        assert_eq!(imputed[i], source[i] != "pwt", "{}", code[i]);
    }
    let at = |c: &str| code.iter().position(|x| x == c).unwrap();
    assert_eq!((level[at("USA")], source[at("USA")].as_str()), (1.0, "pwt"));
    for (c, s) in [
        ("BMU", "pwt"),
        ("SOM", "wdi"),
        ("KOS", "donor"),
        ("NCL", "donor"),
        ("VIR", "wdi"),
        ("AFG", "wdi"),
        ("TUR", "pwt"),
    ] {
        assert_eq!(source[at(c)], s, "{c}");
    }
    let (lo, hi) = level
        .iter()
        .fold((f64::INFINITY, 0.0f64), |(a, b), p| (a.min(*p), b.max(*p)));
    // Tajikistan to Bermuda, both PWT.
    assert!(
        (lo - 0.1268).abs() < 5e-5 && (hi - 1.8004).abs() < 5e-5,
        "{lo} {hi}"
    );
}

/// P160: the real GDP check's tolerance on the economies whose PWT price level of 2000 is a
/// benchmark (RFC 0282 D8, ADR 0290).
const BENCHMARK_TOLERANCE: f64 = 0.10;

/// ADR 0098's real GDP check (ADRs 0285, 0290): GDP over p_c against WDI NY.GDP.MKTP.PP.CD 2000,
/// totals, so that no population enters (WDI counts all of Cyprus, its GDP the south only), for
/// the ICIO economies WDI reports. With the pack's GDP equal to WDI's, the ratio is WDI's price
/// level over PWT's. Gated at `BENCHMARK_TOLERANCE` where PWT's i_cig of 2000 is Benchmark,
/// reported with i_cig elsewhere.
#[test]
fn real_gdp_against_wdi_ppp() {
    let Some(f) = full() else { return };
    let Some(ppp) = cached("wdi_NY.GDP.MKTP.PP.CD.zip") else {
        return;
    };
    let ppp = read_wdi(&ppp).unwrap();
    let basis_of = read_pwt(&f.cache.join("pwt110.xlsx")).unwrap().price_basis;
    let path = f.out.path().join("econ_countries.arrow");
    let (code, va_basis) = (strings(&path, "code"), strings(&path, "va_basis"));
    let gdp = values::<Float64Type>(&path, "gdp");
    let level = values::<Float64Type>(&path, "price_level");
    let pop = read_wdi(&f.cache.join("wdi_SP.POP.TOTL.zip")).unwrap();
    let (mut ln_y, mut ln_r) = (Vec::new(), Vec::new());
    let (mut gated, mut reported) = (Vec::new(), Vec::new());
    for i in (0..code.len()).filter(|&i| va_basis[i] == "icio") {
        let Some(w) = ppp.get(&code[i]).and_then(|s| s.get(&2000)) else {
            continue;
        };
        let r = gdp[i] / level[i] / w;
        let basis = basis_of.get(&code[i]).map_or("none", String::as_str);
        if basis == "Benchmark" {
            gated.push((code[i].as_str(), r));
        } else {
            reported.push((code[i].as_str(), basis, r));
        }
        ln_r.push(r.ln());
        // Income for the tilt only: GDP per person at market rates.
        ln_y.push((gdp[i] / pop[&code[i]][&2000]).ln());
    }
    let n = ln_y.len() as f64;
    let mean = |v: &[f64]| v.iter().sum::<f64>() / n;
    let (my, mr) = (mean(&ln_y), mean(&ln_r));
    let sxy: f64 = ln_y
        .iter()
        .zip(&ln_r)
        .map(|(y, r)| (y - my) * (r - mr))
        .sum();
    let sxx: f64 = ln_y.iter().map(|y| (y - my).powi(2)).sum();
    let srr: f64 = ln_r.iter().map(|r| (r - mr).powi(2)).sum();
    let (slope, corr) = (sxy / sxx, sxy / (sxx * srr).sqrt());
    let within = |r: f64, t: f64| (r - 1.0).abs() <= t;
    let all: Vec<f64> = ln_r.iter().map(|r| r.exp()).collect();
    let mut sorted = all.clone();
    sorted.sort_by(f64::total_cmp);
    let median = sorted[sorted.len() / 2];
    eprintln!(
        "real GDP check: {} economies, {} within 5%, {} within 10%, median {median:.4}, {:.4} to \
         {:.4}, slope {slope:.4}, correlation {corr:.4}",
        all.len(),
        all.iter().filter(|r| within(**r, 0.05)).count(),
        all.iter().filter(|r| within(**r, 0.10)).count(),
        sorted[0],
        sorted[sorted.len() - 1],
    );
    let worst = gated
        .iter()
        .copied()
        .max_by(|a, b| (a.1 - 1.0).abs().total_cmp(&(b.1 - 1.0).abs()))
        .unwrap();
    eprintln!(
        "benchmark economies: {} of {} within 10%, {} within 5%, largest gap {} {:.4}",
        gated
            .iter()
            .filter(|g| within(g.1, BENCHMARK_TOLERANCE))
            .count(),
        gated.len(),
        gated.iter().filter(|g| within(g.1, 0.05)).count(),
        worst.0,
        worst.1,
    );
    for (c, r) in &gated {
        eprintln!("  gated {c} {r:.4}");
    }
    for (c, b, r) in &reported {
        eprintln!("  reported {c} {b} {r:.4}");
    }
    for (c, r) in &gated {
        assert!(within(*r, BENCHMARK_TOLERANCE), "{c} {r}");
    }
    assert_eq!((gated.len(), reported.len()), (31, 48));
    assert_eq!(gated.iter().filter(|g| within(g.1, 0.05)).count(), 23);
    assert_eq!(worst.0, "GBR");
    assert!((worst.1 - 1.093).abs() < 5e-4, "{}", worst.1);
    // All 79, reported only.
    assert_eq!(all.iter().filter(|r| within(**r, 0.05)).count(), 35);
    assert!((median - 0.9552).abs() < 5e-5, "{median}");
    assert!((slope - 0.0523).abs() < 5e-5, "{slope}");
    assert!((corr - 0.2674).abs() < 5e-5, "{corr}");
}

/// ADR 0312's check, reported only: the proxy's top-10% wealth share (ADR 0311) against WID's
/// `shweal992j` 2000, and the lognormal-from-Gini top-10% income share (tau = 0) against WID's
/// pre-tax `sptinc992j` (the Gini is disposable income, so the income ratio sits below 1). Needs
/// WID in the fetch cache, a research profile.
#[test]
fn wealth_proxy_against_wid() {
    let Some(f) = full() else { return };
    let Some(wid) = cached("wid_all_data.zip") else {
        return;
    };
    let wealth = read_wid(&wid, WEALTH).unwrap();
    let income = read_wid(&wid, INCOME).unwrap();
    let iso3: BTreeMap<String, String> = iso3_by_iso2(&f.cache.join(ADMIN0))
        .unwrap()
        .into_iter()
        .map(|(a2, a3)| (a3, a2))
        .collect();
    let path = f.out.path().join("econ_countries.arrow");
    let code = strings(&path, "code");
    let (top, gini) = (
        values::<Float64Type>(&path, "wealth_2"),
        values::<Float64Type>(&path, "gini"),
    );
    let summary = |name: &str, r: &[f64]| {
        if r.is_empty() {
            return eprintln!("{name}: no countries in WID");
        }
        let mut s = r.to_vec();
        s.sort_by(f64::total_cmp);
        eprintln!(
            "{name}: {} countries, median ratio {:.3}, {:.3} to {:.3}, {} within 20%",
            s.len(),
            s[s.len() / 2],
            s[0],
            s[s.len() - 1],
            s.iter().filter(|r| (**r - 1.0).abs() <= 0.2).count(),
        );
    };
    let (mut rw, mut ri) = (Vec::new(), Vec::new());
    eprintln!("code  proxy_w  wid_w  ratio  proxy_y  wid_y  ratio");
    for i in 0..code.len() {
        let Some(a2) = iso3.get(&code[i]) else {
            continue;
        };
        let w = wealth.get(a2).map(|s| s[2]);
        let y = income.get(a2).map(|s| s[2]);
        if w.is_none() && y.is_none() {
            continue;
        }
        let py = wealth_proxy(gini[i], 0.0)[2];
        let cell = |p: f64, v: Option<f64>, out: &mut Vec<f64>| match v {
            Some(v) => {
                out.push(p / v);
                format!("{p:.3}  {v:.3}  {:.3}", p / v)
            }
            None => format!("{p:.3}  -  -"),
        };
        let cw = cell(top[i], w, &mut rw);
        let cy = cell(py, y, &mut ri);
        eprintln!("{}  {cw}  {cy}", code[i]);
    }
    summary("top-10% wealth, proxy over WID", &rw);
    summary("top-10% income, Gini lognormal over WID pre-tax", &ri);
}

/// Donor tables balanced by RAS (ADR 0286): every `ras` economy's use table [Z | F] meets its
/// domestic use X + M - E by good, its intermediate inputs X - VA and final demand VA + M - E. A
/// parent is balanced with the territories counted inside it, which share its coefficients;
/// ICIO's 80 and the two territories inside ICIO economies (Aland, Northern Cyprus) are `icio`.
#[test]
fn donor_tables_are_balanced_to_domestic_use() {
    let Some(f) = full() else { return };
    let path = f.out.path().join("econ_countries.arrow");
    let (code, io_basis) = (strings(&path, "code"), strings(&path, "io_basis"));
    let count = |s: &str| io_basis.iter().filter(|x| *x == s).count();
    assert_eq!((count("icio"), count("ras"), count("donor")), (82, 88, 72));
    let (iso, _) = read_codes(&f.cache.join("ne_10m_admin_0_countries.zip")).unwrap();
    let hs = read_hs_cpa(&f.cache.join(HS_KEY)).unwrap();
    let trade = trade_by_good(&f.cache.join(BACI), &hs_goods(&hs), &iso, &code).unwrap();
    let a = values::<Float64Type>(&f.out.path().join("econ_io.arrow"), "a");
    let per_good = |name: &str| -> Vec<Vec<f64>> {
        (0..N_GOODS)
            .map(|g| values::<Float64Type>(&path, &format!("{name}_{g}")))
            .collect()
    };
    let (mut va, fd) = (per_good("va"), per_good("fd"));
    let at_code = |c: &str| code.iter().position(|x| x == c).unwrap();
    for (territory, parent) in INSIDE {
        let (t, p) = (at_code(territory), at_code(parent));
        for v in &mut va {
            v[p] += v[t];
        }
    }
    let mut worst = 0.0f64;
    let inside = |c: usize| INSIDE.iter().any(|(t, _)| *t == code[c]);
    for c in (0..code.len()).filter(|&c| io_basis[c] == "ras" && !inside(c)) {
        let t = &trade[&code[c]];
        let at = |k: usize, g: usize| a[(c * N_GOODS + k) * N_GOODS + g];
        // X_g = VA_g / (1 - sum_k a_kg).
        let x: Vec<f64> = (0..N_GOODS)
            .map(|g| va[g][c] / (1.0 - (0..N_GOODS).map(|k| at(k, g)).sum::<f64>()))
            .collect();
        let (m, e): (Vec<f64>, Vec<f64>) = (0..N_GOODS)
            .map(|k| (t.imports[k] / 1e6, t.exports[k] / 1e6))
            .unzip();
        let total_fd = (0..N_GOODS).map(|g| va[g][c] + m[g] - e[g]).sum::<f64>();
        for k in 0..N_GOODS {
            let used: f64 =
                (0..N_GOODS).map(|g| at(k, g) * x[g]).sum::<f64>() + fd[k][c] * total_fd;
            let want = x[k] + m[k] - e[k];
            let miss = (used - want).abs() / want.max(1.0);
            worst = worst.max(miss);
            // ADR 0098: imputed economies hit their trade totals within 1e-8.
            assert!(
                miss <= 1e-8,
                "{} good {k}: use {used} against {want}",
                code[c]
            );
        }
    }
    eprintln!("donor tables: worst domestic-use miss {worst:e}");
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
