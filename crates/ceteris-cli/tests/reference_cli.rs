//! `ceteris reference solve`, and the reference's routing against the kernel the Rust core
//! exports, on the fixture and on the pack.

use std::collections::BTreeMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;

use arrow_array::RecordBatch;
use arrow_array::cast::AsArray;
use arrow_array::types::{Float64Type, UInt64Type};
use arrow_cast::cast;
use arrow_ipc::reader::FileReader;
use arrow_schema::DataType;
use ceteris_pack::compare::{self, Tolerance};
use ceteris_reference::io::{self, Inputs, Mat};
use ceteris_reference::model::Model;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture() -> PathBuf {
    root().join("tests/golden/p2-fixture")
}

fn ceteris(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ceteris"))
        .args(args)
        .output()
        .unwrap()
}

fn path(p: &Path) -> &str {
    p.to_str().unwrap()
}

/// The committed fixture reference, within the tolerance of an iterative solve.
#[test]
fn reference_solve_reproduces_the_fixture_reference() {
    let out = tempfile::tempdir().unwrap();
    let run = ceteris(&[
        "reference",
        "solve",
        "--inputs",
        path(&fixture()),
        "--out",
        path(out.path()),
    ]);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let summary: serde_json::Value = serde_json::from_slice(&run.stdout).unwrap();
    assert_eq!(summary["method"], "newton_krylov");
    assert!(summary["seconds"].is_f64());
    let committed = fixture().join("reference");
    let report = compare::tables(&committed, out.path(), Tolerance::ITERATIVE).unwrap();
    assert!(report.ok, "{:#?}", report.lines());
    let json = |dir: &Path| -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(dir.join("solution.json")).unwrap()).unwrap()
    };
    let (got, want) = (json(out.path()), json(&committed));
    let keys =
        |v: &serde_json::Value| -> Vec<String> { v.as_object().unwrap().keys().cloned().collect() };
    assert_eq!(keys(&got), keys(&want));
    for key in ["format", "static_digest", "method", "dropped_province"] {
        assert_eq!(got[key], want[key], "{key}");
    }
}

#[test]
fn reference_solve_needs_its_arguments() {
    let run = ceteris(&["reference", "solve", "--inputs", path(&fixture())]);
    assert!(!run.status.success());
    assert!(String::from_utf8_lossy(&run.stderr).contains("--out is required"));
    let run = ceteris(&["reference", "calibrate"]);
    assert!(!run.status.success());
    assert!(String::from_utf8_lossy(&run.stderr).contains("expected a reference command"));
}

fn table(dir: &Path, name: &str) -> RecordBatch {
    let file = File::open(dir.join(format!("{name}.arrow"))).unwrap();
    let batches: Vec<RecordBatch> = FileReader::try_new(file, None)
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    arrow_select::concat::concat_batches(&batches[0].schema(), &batches).unwrap()
}

fn floats(t: &RecordBatch, col: &str) -> Vec<f64> {
    cast(t.column_by_name(col).unwrap(), &DataType::Float64)
        .unwrap()
        .as_primitive::<Float64Type>()
        .values()
        .to_vec()
}

fn indices(t: &RecordBatch, col: &str) -> Vec<usize> {
    cast(t.column_by_name(col).unwrap(), &DataType::UInt64)
        .unwrap()
        .as_primitive::<UInt64Type>()
        .values()
        .iter()
        .map(|&v| v as usize)
        .collect()
}

fn relative(got: f64, want: f64) -> f64 {
    if got == want {
        0.0
    } else {
        (got - want).abs() / want.abs()
    }
}

/// The reference routes on the kernel Rust exports, and that kernel is the spec's: every crossing
/// factor rebuilt from the legs, tariffs and gateway masses as
/// K_ab = m_a m_b (tau (1 + nu_c) (1 + t))^-theta, nu_c of the good's theta class (ADR 0092), with
/// 1 / (tau (1 + nu)) and 1 / (1 + t), to 1e-12; and the reference's delivered prices at prices 1
/// are Rust's base prices to 1e-12. Returns the largest relative difference.
fn assert_routing_matches_rust(inp: &Inputs, export: &Path) -> f64 {
    let (legs, goods, tariffs) = (
        table(export, "legs"),
        table(export, "goods"),
        table(export, "tariffs"),
    );
    let params: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(export.join("params.json")).unwrap())
            .unwrap();
    let list = |key: &str| -> Vec<f64> {
        params["trade"][key]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect()
    };
    let (border_nu, class) = (list("border_nu"), list("theta_class"));
    let gc = &inp.gateway_country;
    let n_c = inp.n_countries();
    let mut rate = vec![0.0; n_c * n_c * 2];
    let (rate_value, rate_group) = (floats(&tariffs, "rate"), indices(&tariffs, "group"));
    for (i, (imp, exp)) in indices(&tariffs, "importer")
        .into_iter()
        .zip(indices(&tariffs, "exporter"))
        .enumerate()
    {
        rate[(imp * n_c + exp) * 2 + rate_group[i]] = rate_value[i];
    }
    let crossing: Vec<usize> = indices(&legs, "kind")
        .iter()
        .enumerate()
        .filter(|&(_, &k)| k == 2)
        .map(|(i, _)| i)
        .collect();
    let pick = |v: Vec<usize>| -> Vec<usize> { crossing.iter().map(|&i| v[i]).collect() };
    let pick_f = |v: Vec<f64>| -> Vec<f64> { crossing.iter().map(|&i| v[i]).collect() };
    let (a, b) = (pick(indices(&legs, "from")), pick(indices(&legs, "to")));
    let (cost, hours) = (
        pick_f(floats(&legs, "cost_per_t")),
        pick_f(floats(&legs, "hours")),
    );
    let leg_at: BTreeMap<(usize, usize), usize> = a
        .iter()
        .zip(&b)
        .enumerate()
        .map(|(i, (&x, &y))| ((x, y), i))
        .collect();
    let (weight, time_cost, group) = (
        floats(&goods, "weight"),
        floats(&goods, "time_cost"),
        indices(&goods, "tariff_group"),
    );
    let cro = &inp.crossing;
    let mut worst = 0.0_f64;
    let mut check = |what: &str, got: f64, want: f64| {
        let r = relative(got, want);
        assert!(r <= 1e-12, "{what}: {got} vs {want}");
        worst = worst.max(r);
    };
    for g in (0..inp.n_goods()).filter(|&g| inp.traded[g]) {
        let nu = border_nu[class[g] as usize];
        for j in (0..cro.good.len()).filter(|&j| cro.good[j] == g) {
            let i = leg_at[&(cro.row[j], cro.col[j])];
            let tau = 1.0 + cost[i] * weight[g] + time_cost[g] * hours[i] / 24.0;
            let duty = 1.0 + rate[(gc[b[i]] * n_c + gc[a[i]]) * 2 + group[g]];
            let m = inp.gateway_mass[a[i]] * inp.gateway_mass[b[i]];
            check(
                "k",
                cro.k[j],
                m * (tau * (1.0 + nu) * duty).powf(-inp.theta[g]),
            );
            check("inv_loss", cro.inv_loss[j], 1.0 / (tau * (1.0 + nu)));
            check("inv_duty", cro.inv_duty[j], 1.0 / duty);
        }
    }
    let m = Model::new(inp).unwrap();
    let q = m
        .sourcing(&Mat::filled(inp.n_provinces(), inp.n_goods(), 1.0))
        .index;
    for p in 0..inp.n_provinces() {
        for g in (0..inp.n_goods()).filter(|&g| inp.traded[g]) {
            check("base price", inp.base_price[(p, g)], q[(p, g)]);
        }
    }
    worst
}

#[test]
fn routing_matches_rust() {
    let dir = fixture();
    let worst = assert_routing_matches_rust(&io::load(&dir).unwrap(), &dir);
    println!("fixture routing: max_rel={worst:e}");
}

/// [`assert_routing_matches_rust`] on an export of the pack (`CETERIS_PACK`, default
/// `data/packs/2000`); skips when the default pack is absent.
#[test]
fn routing_matches_rust_on_the_pack() {
    let pack = std::env::var_os("CETERIS_PACK")
        .map_or_else(|| root().join("data/packs/2000"), PathBuf::from);
    if !pack.join("provinces.arrow").exists() {
        // a pack named explicitly must be testable; only the absent default pack skips (CI)
        assert!(
            std::env::var_os("CETERIS_PACK").is_none(),
            "no pack at {}",
            pack.display()
        );
        eprintln!("skipping: no pack at {}", pack.display());
        return;
    }
    let out = tempfile::tempdir().unwrap();
    let run = ceteris(&["export", "--pack", path(&pack), "--out", path(out.path())]);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let worst = assert_routing_matches_rust(&io::load(out.path()).unwrap(), out.path());
    println!("pack routing: max_rel={worst:e}");
}
