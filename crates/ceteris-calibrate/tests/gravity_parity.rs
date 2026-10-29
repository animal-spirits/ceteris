//! Compares the gravity estimation with the Python estimation of the same pack:
//! `CETERIS_GRAVITY_PARITY` names a directory with `python/` (what `step4_gravity_dump.py`
//! dumps) and `rust/gravity.txt` (what `ceteris gravity` printed), as `scripts/parity/step4.sh`
//! writes them. The test estimates again in process and compares every field of the estimates,
//! the TOML and the Yotov fits (1e-9 relative, counts equal), the country costs of the
//! least-cost trees (1e-12 relative) and the printed text (equal); it prints the worst gaps and
//! both timings. `CETERIS_PACK` names the pack (default `data/packs/2000`). Skips without the
//! variable.

use std::path::{Path, PathBuf};
use std::time::Instant;

use arrow_array::RecordBatch;
use arrow_array::cast::AsArray;
use arrow_array::types::Float64Type;
use arrow_cast::cast;
use arrow_ipc::reader::FileReader;
use arrow_schema::DataType;
use ceteris_calibrate::gravity::{self, Estimates, N_Q, N_TRADED, Transport, YOTOV};
use serde_json::{Value, json};

/// ADR 0113: values an iterative solve sets agree to 1e-9 relative.
const ITERATIVE: f64 = 1e-9;
/// Sums along least-cost paths and their weighted means.
const COSTS: f64 = 1e-12;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn pack() -> PathBuf {
    std::env::var_os("CETERIS_PACK").map_or_else(|| root().join("data/packs/2000"), PathBuf::from)
}

fn relative(a: f64, b: f64) -> f64 {
    if a == b || (a.is_nan() && b.is_nan()) {
        0.0
    } else {
        (a - b).abs() / a.abs().max(b.abs())
    }
}

#[derive(Default)]
struct Worst {
    gap: f64,
    what: String,
}

impl Worst {
    fn number(&mut self, what: &str, a: f64, b: f64) {
        let gap = relative(a, b);
        if gap > self.gap || gap.is_nan() {
            self.gap = gap;
            self.what = format!("{what}: {a:e} vs {b:e}");
        }
    }

    /// Integers, booleans and strings equal; floats (null for NaN) into the worst gap.
    fn value(&mut self, what: &str, a: &Value, b: &Value) {
        match (a, b) {
            (Value::Object(x), Value::Object(y)) => {
                let keys =
                    |o: &serde_json::Map<String, Value>| o.keys().cloned().collect::<Vec<_>>();
                assert_eq!(keys(x), keys(y), "{what}: keys");
                for (k, v) in x {
                    self.value(&format!("{what}.{k}"), v, &y[k]);
                }
            }
            (Value::Array(x), Value::Array(y)) => {
                assert_eq!(x.len(), y.len(), "{what}: length");
                for (i, (v, w)) in x.iter().zip(y).enumerate() {
                    self.value(&format!("{what}[{i}]"), v, w);
                }
            }
            (Value::Number(x), Value::Number(y)) if x.is_f64() || y.is_f64() => {
                self.number(what, x.as_f64().unwrap(), y.as_f64().unwrap());
            }
            (Value::Null, Value::Number(y)) => self.number(what, f64::NAN, y.as_f64().unwrap()),
            (Value::Number(x), Value::Null) => self.number(what, x.as_f64().unwrap(), f64::NAN),
            _ => assert_eq!(a, b, "{what}"),
        }
    }
}

/// JSON as Python writes it, NaN read as null.
fn read_json(path: &Path) -> Value {
    let text = std::fs::read_to_string(path).unwrap();
    serde_json::from_str(&text.replace("NaN", "null")).unwrap()
}

fn float(x: f64) -> Value {
    serde_json::Number::from_f64(x).map_or(Value::Null, Value::Number)
}

fn floats(v: &[f64]) -> Value {
    Value::Array(v.iter().map(|&x| float(x)).collect())
}

/// The estimates with the field names of the Python dataclasses.
fn estimates_json(e: &Estimates) -> Value {
    let theta: Vec<Value> = e
        .theta
        .iter()
        .map(|t| {
            json!({
                "goods": t.goods, "theta": float(t.theta), "se": float(t.se),
                "tau": float(t.tau), "se_tau": float(t.se_tau), "n": t.n,
                "pseudo_r2": float(t.pseudo_r2),
            })
        })
        .collect();
    let b = &e.border;
    let m = &e.modes;
    let notes: Value =
        serde_json::from_str(&e.notes.json().compact().replace("NaN", "null")).unwrap();
    json!({
        "theta": theta,
        "border": {
            "nu": float(b.nu), "se": float(b.se), "nu_by_class": floats(&b.nu_by_class),
            "se_by_class": floats(&b.se_by_class), "theta_by_class": floats(&b.theta_by_class),
            "border_by_class": floats(&b.border_by_class), "n": b.n,
            "pseudo_r2": float(b.pseudo_r2),
        },
        "modes": {
            "ratio": floats(&m.ratio), "se": floats(&m.se), "identified": m.identified,
            "theta_by_class": floats(&m.theta_by_class), "tau_by_class": floats(&m.tau_by_class),
            "n": m.n, "pseudo_r2": float(m.pseudo_r2), "iterations": m.iterations,
        },
        "class_shares": floats(&e.class_shares),
        "n_countries": e.n_countries,
        "n_domestic_countries": e.n_domestic_countries,
        "notes": notes,
    })
}

fn column(batches: &[RecordBatch], name: &str) -> Vec<f64> {
    batches
        .iter()
        .flat_map(|b| {
            let a = cast(b.column_by_name(name).unwrap(), &DataType::Float64).unwrap();
            a.as_primitive::<Float64Type>().values().to_vec()
        })
        .collect()
}

#[test]
fn gravity_matches_python() {
    let Some(dir) = std::env::var_os("CETERIS_GRAVITY_PARITY").map(PathBuf::from) else {
        eprintln!("skipping: CETERIS_GRAVITY_PARITY unset (a step4.sh gravity directory)");
        return;
    };
    let python = dir.join("python");
    let pack = pack();
    let cache = root().join("data/cache");

    let (a, b) = (
        std::fs::read_to_string(python.join("gravity.txt")).unwrap(),
        std::fs::read_to_string(dir.join("rust/gravity.txt")).unwrap(),
    );
    assert_eq!(a, b, "printed report, notes and stop rules");
    println!("printed text: equal ({} lines)", a.lines().count());

    let start = Instant::now();
    let est = gravity::estimate(&pack, &cache).unwrap();
    let seconds = start.elapsed().as_secs_f64();
    let mut worst = Worst::default();
    worst.value(
        "estimates",
        &read_json(&python.join("estimates.json")),
        &estimates_json(&est),
    );
    println!("estimates: worst {:e} ({})", worst.gap, worst.what);
    assert!(worst.gap <= ITERATIVE, "{}", worst.what);

    let toml = gravity::to_toml(&est);
    let want = std::fs::read_to_string(python.join("gravity_2000.toml")).unwrap();
    let (got, want): (Vec<&str>, Vec<&str>) = (toml.lines().collect(), want.lines().collect());
    assert_eq!(got.len(), want.len(), "toml lines");
    let mut worst = Worst::default();
    for (i, (x, y)) in want.iter().zip(&got).enumerate() {
        let number = |line: &str| -> Option<f64> {
            let (key, v) = line.split_once(" = ")?;
            ["value", "se", "pseudo_r2", "default"]
                .contains(&key)
                .then(|| v.parse().unwrap())
        };
        // The comment names the command that writes the file.
        if x.starts_with('#') {
            continue;
        }
        match (number(x), number(y)) {
            (Some(p), Some(r)) => worst.number(&format!("toml line {}", i + 1), p, r),
            _ => assert_eq!(x, y, "toml line {}", i + 1),
        }
    }
    println!("toml: worst {:e} ({})", worst.gap, worst.what);
    assert!(worst.gap <= ITERATIVE, "{}", worst.what);

    let file = std::fs::File::open(python.join("country_costs.arrow")).unwrap();
    let batches: Vec<RecordBatch> = FileReader::try_new(file, None)
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let start = Instant::now();
    let costs = gravity::country_costs(&pack, &Transport::default()).unwrap();
    let costs_seconds = start.elapsed().as_secs_f64();
    let (o, d, g) = (
        column(&batches, "exporter"),
        column(&batches, "importer"),
        column(&batches, "good"),
    );
    let reach: Vec<bool> = batches
        .iter()
        .flat_map(|b| {
            let a = b.column_by_name("reach").unwrap().as_boolean();
            (0..a.len()).map(|i| a.value(i)).collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(o.len(), costs.n * costs.n * N_TRADED);
    let q: Vec<Vec<f64>> = (0..N_Q)
        .map(|k| column(&batches, &format!("q_{k}")))
        .collect();
    let mut worst = Worst::default();
    for i in 0..o.len() {
        let (a, b, g) = (o[i] as usize, d[i] as usize, g[i] as usize);
        assert_eq!(reach[i], costs.reach[a * costs.n + b], "reach {a} {b}");
        let got = costs.at(a, b, g);
        for k in 0..N_Q {
            worst.number(&format!("q[{a}, {b}, {g}, {k}]"), q[k][i], got[k]);
        }
    }
    println!(
        "country costs: {} pairs reached, worst {:e} ({})",
        reach.iter().filter(|&&r| r).count() / N_TRADED,
        worst.gap,
        worst.what
    );
    assert!(worst.gap <= COSTS, "{}", worst.what);

    let start = Instant::now();
    let fits = gravity::yotov(&cache.join(YOTOV)).unwrap();
    let yotov_seconds = start.elapsed().as_secs_f64();
    let want = read_json(&python.join("yotov.json"));
    let mut worst = Worst::default();
    for (name, fit) in &fits {
        let w = &want[name];
        let se: Vec<f64> = (0..fit.names.len())
            .map(|i| fit.cov[(i, i)].sqrt())
            .collect();
        let got = json!({
            "names": fit.names, "coef": floats(&fit.coef), "se": floats(&se), "n": fit.n,
            "n_separated": fit.n_separated, "pseudo_r2": float(fit.pseudo_r2),
        });
        worst.value(name, w, &got);
    }
    assert_eq!(want.as_object().unwrap().len(), fits.len(), "yotov columns");
    println!("yotov: worst {:e} ({})", worst.gap, worst.what);
    assert!(worst.gap <= ITERATIVE, "{}", worst.what);

    let python_seconds = read_json(&python.join("seconds.json"));
    println!(
        "seconds: estimate python {:.1}, rust {seconds:.1} (country costs {costs_seconds:.1}); \
         yotov python {:.1}, rust {yotov_seconds:.1}",
        python_seconds["estimate"].as_f64().unwrap(),
        python_seconds["yotov"].as_f64().unwrap()
    );
}
