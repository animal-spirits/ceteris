//! Compares a Rust calibration with the Python calibration of the same pack or export:
//! `CETERIS_CALIBRATION_PARITY` names a directory with `python/` and `rust/` (the tables) and
//! `python.txt` and `rust.txt` (what each `calibrate` printed), as `scripts/parity/step4.sh`
//! writes them. The tables' columns are compared by `ceteris pack compare --iterative`; this
//! test compares what that leaves out: the schema metadata (format, static digest, passes, r)
//! and the printed summary. Skips without the variable.

use std::collections::BTreeMap;
use std::fs::File;
use std::path::{Path, PathBuf};

use arrow_ipc::reader::FileReader;
use serde_json::Value;

/// ADR 0113: values an iterative solve sets agree to 1e-9 relative.
const ITERATIVE: f64 = 1e-9;
/// The Sinkhorn residual at convergence is rounding noise below its tolerance.
const SINKHORN_TOL: f64 = 1e-13;

const TABLES: [&str; 6] = [
    "econ_calib",
    "econ_calib_cells",
    "econ_calib_provinces",
    "econ_calib_pops",
    "econ_calib_pipeline",
    "econ_calib_countries",
];

fn relative(a: f64, b: f64) -> f64 {
    if a == b {
        0.0
    } else {
        (a - b).abs() / a.abs().max(b.abs())
    }
}

fn metadata(dir: &Path, table: &str) -> BTreeMap<String, String> {
    let file = File::open(dir.join(format!("{table}.arrow"))).unwrap();
    FileReader::try_new(file, None)
        .unwrap()
        .schema()
        .metadata()
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// The JSON summary in a `calibrate` transcript: between the pass lines and `wrote`.
fn summary(path: &Path) -> Value {
    let text = std::fs::read_to_string(path).unwrap();
    let start = text.find("\n{").unwrap() + 1;
    let end = text.rfind("\n}").unwrap() + 2;
    serde_json::from_str(&text[start..end]).unwrap()
}

#[derive(Default)]
struct Worst {
    gap: f64,
    what: String,
}

impl Worst {
    fn check(&mut self, what: &str, a: &Value, b: &Value) {
        match (a.as_f64(), b.as_f64()) {
            (Some(x), Some(y)) => {
                let gap = relative(x, y);
                if gap > self.gap || gap.is_nan() {
                    self.gap = gap;
                    self.what = format!("{what}: {x} vs {y}");
                }
            }
            _ => assert_eq!(a, b, "{what}"),
        }
    }

    fn list(&mut self, what: &str, a: &Value, b: &Value) {
        let (a, b) = (a.as_array().unwrap(), b.as_array().unwrap());
        assert_eq!(a.len(), b.len(), "{what}: length");
        for (i, (x, y)) in a.iter().zip(b).enumerate() {
            self.check(&format!("{what}[{i}]"), x, y);
        }
    }
}

#[test]
fn calibration_matches_python() {
    let Some(dir) = std::env::var_os("CETERIS_CALIBRATION_PARITY").map(PathBuf::from) else {
        eprintln!("skipping: CETERIS_CALIBRATION_PARITY unset");
        return;
    };
    let (python, rust) = (dir.join("python"), dir.join("rust"));
    for table in TABLES {
        let (a, b) = (metadata(&python, table), metadata(&rust, table));
        assert_eq!(
            a.get("calibration_format"),
            b.get("calibration_format"),
            "{table}"
        );
        assert_eq!(a.len(), b.len(), "{table}: metadata keys");
    }
    let meta = |d: &Path| -> Value {
        serde_json::from_str(&metadata(d, "econ_calib")["calibration"]).unwrap()
    };
    let (a, b) = (meta(&python), meta(&rust));
    let keys = |v: &Value| v.as_object().unwrap().keys().cloned().collect::<Vec<_>>();
    assert_eq!(keys(&a), keys(&b), "calibration metadata keys");
    for key in ["format", "static_digest", "passes"] {
        assert_eq!(a[key], b[key], "{key}");
    }
    for side in [&a, &b] {
        assert!(side["sinkhorn_residual"].as_f64().unwrap() <= SINKHORN_TOL);
    }
    let mut worst = Worst::default();
    worst.list("metadata r", &a["r"], &b["r"]);
    println!(
        "metadata: format {}, static_digest {}, passes {}: equal; r worst {:e}",
        a["format"], a["static_digest"], a["passes"], worst.gap
    );
    assert!(worst.gap <= ITERATIVE, "{}", worst.what);

    let (a, b) = (
        summary(&dir.join("python.txt")),
        summary(&dir.join("rust.txt")),
    );
    assert_eq!(keys(&a), keys(&b), "summary keys");
    assert_eq!(a["passes"], b["passes"], "passes");
    let mut worst = Worst::default();
    worst.list("r", &a["r"], &b["r"]);
    worst.list(
        "min_log10_relative_weight",
        &a["min_log10_relative_weight"],
        &b["min_log10_relative_weight"],
    );
    for key in [
        "local_va_gap_quantiles",
        "ln_a_quantiles",
        "ln_a_quantiles_traded",
    ] {
        assert_eq!(keys(&a[key]), keys(&b[key]), "{key}");
        for q in keys(&a[key]) {
            worst.check(&format!("{key}[{q}]"), &a[key][&q], &b[key][&q]);
        }
    }
    let (wa, wb) = (
        a["local_va_gap_worst"].as_array().unwrap(),
        b["local_va_gap_worst"].as_array().unwrap(),
    );
    assert_eq!(wa.len(), wb.len());
    for (i, (x, y)) in wa.iter().zip(wb).enumerate() {
        assert_eq!(x["country"], y["country"], "local_va_gap_worst[{i}]");
        worst.check(&format!("local_va_gap_worst[{i}]"), &x["gap"], &y["gap"]);
    }
    worst.check(
        "local_va_world_gap",
        &a["local_va_world_gap"],
        &b["local_va_world_gap"],
    );
    let sweeps = a["sinkhorn_sweeps"] == b["sinkhorn_sweeps"];
    println!(
        "summary: passes {} equal, Sinkhorn sweeps per pass {}; worst {:e} ({})",
        a["passes"],
        if sweeps { "equal" } else { "differ" },
        worst.gap,
        worst.what
    );
    assert!(worst.gap <= ITERATIVE, "{}", worst.what);
}
