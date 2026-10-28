//! Fits the PPML designs dumped by `scripts/parity/step4_ppml_dump.py` and compares them with the
//! Python reference's fits (`scripts/parity/step4.sh` runs both). Skips without
//! `CETERIS_PPML_PARITY`, the dump directory: it reads DIR/designs and DIR/python, writes the
//! Rust fits to DIR/rust in the same layout (for `ceteris pack compare`) and checks every column
//! to the 1e-9 of an iterative solve, relative, except where a value is rounding noise around 0.
//! There the mixed form |a - b| <= 1e-9 max(1, |a|, |b|) applies: to every column of a design
//! whose regressor the fixed effects absorb (its coefficient, standard error and covariances are
//! noise), and to the coefficients of the `d_` regressors of `mode_costs`, the Gauss-Newton steps
//! of the mode cost ratios, which tend to 0 as the ratios converge.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use arrow_array::cast::AsArray;
use arrow_array::types::{Float64Type, Int64Type};
use arrow_array::{
    Array, ArrayRef, BooleanArray, Float64Array, Int64Array, RecordBatch, StringArray,
};
use arrow_ipc::reader::FileReader;
use arrow_ipc::writer::FileWriter;
use arrow_schema::{Field, Schema};

use ceteris_reference::ppml;

const ITERATIVE: f64 = 1e-9;
/// Designs with a regressor the fixed effects absorb.
const ABSORBED: [&str; 1] = ["hdfe_absorbed"];

/// Whether the value of `design`'s column `col` at regressor `row` is compared in the mixed form.
fn mixed(design: &str, col: &str, row: &str) -> bool {
    ABSORBED.contains(&design)
        || (design.ends_with("_mode_costs") && col == "coef" && row.starts_with("d_"))
}

fn read(path: &Path) -> (Arc<Schema>, Vec<RecordBatch>) {
    let reader = FileReader::try_new(File::open(path).unwrap(), None).unwrap();
    let schema = reader.schema();
    (schema, reader.collect::<Result<_, _>>().unwrap())
}

fn f64s(batches: &[RecordBatch], name: &str) -> Vec<f64> {
    batches
        .iter()
        .flat_map(|b| {
            b.column_by_name(name)
                .unwrap()
                .as_primitive::<Float64Type>()
                .values()
                .to_vec()
        })
        .collect()
}

fn i64s(batches: &[RecordBatch], name: &str) -> Vec<i64> {
    batches
        .iter()
        .flat_map(|b| {
            b.column_by_name(name)
                .unwrap()
                .as_primitive::<Int64Type>()
                .values()
                .to_vec()
        })
        .collect()
}

fn write(path: &Path, columns: Vec<(&str, ArrayRef)>) {
    let fields: Vec<Field> = columns
        .iter()
        .map(|(name, a)| Field::new(*name, a.data_type().clone(), true))
        .collect();
    let schema = Arc::new(Schema::new(fields));
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        columns.into_iter().map(|(_, a)| a).collect(),
    )
    .unwrap();
    let mut w = FileWriter::try_new(BufWriter::new(File::create(path).unwrap()), &schema).unwrap();
    w.write(&batch).unwrap();
    w.finish().unwrap();
    w.into_inner().unwrap().flush().unwrap();
}

fn floats(v: Vec<f64>) -> ArrayRef {
    Arc::new(Float64Array::from(v))
}

struct Summary {
    design: String,
    n: usize,
    n_separated: usize,
    exporters: Option<usize>,
    importers: Option<usize>,
    pseudo_r2: f64,
}

/// One design fitted: the fit table's columns and the estimated rows of an hdfe fit.
struct Fitted {
    columns: Vec<(String, Vec<f64>)>,
    names: Vec<String>,
    keep: Option<Vec<bool>>,
    summary: Summary,
    seconds: f64,
}

fn fit_design(name: &str, path: &Path) -> Fitted {
    let (schema, batches) = read(path);
    let kind = schema.metadata().get("kind").unwrap().clone();
    let flow = f64s(&batches, "flow");
    let fields: Vec<String> = schema.fields().iter().map(|f| f.name().clone()).collect();
    let fes: Vec<Vec<i64>> = fields
        .iter()
        .filter(|f| f.starts_with("fe_"))
        .map(|f| i64s(&batches, f))
        .collect();
    let cluster = fields
        .iter()
        .any(|f| f == "cluster")
        .then(|| i64s(&batches, "cluster"));
    let regs: Vec<(String, Vec<f64>)> = fields
        .iter()
        .filter_map(|f| {
            f.strip_prefix("x.")
                .map(|n| (n.to_owned(), f64s(&batches, f)))
        })
        .collect();
    let regressors: Vec<(&str, &[f64])> = regs
        .iter()
        .map(|(n, v)| (n.as_str(), v.as_slice()))
        .collect();
    let start = Instant::now();
    if kind == "fit" {
        let index = |v: &[i64]| -> Vec<usize> { v.iter().map(|&i| i as usize).collect() };
        let r = ppml::fit_robust(&flow, &index(&fes[0]), &index(&fes[1]), &regressors).unwrap();
        let seconds = start.elapsed().as_secs_f64();
        let s = r.separation;
        Fitted {
            columns: vec![("coef".into(), r.coef), ("se".into(), r.se)],
            names: r.names,
            keep: None,
            summary: Summary {
                design: name.into(),
                n: flow.len() - s.n_rows,
                n_separated: s.n_rows,
                exporters: Some(s.n_exporters),
                importers: Some(s.n_importers),
                pseudo_r2: r.pseudo_r2,
            },
            seconds,
        }
    } else {
        let fe: Vec<&[i64]> = fes.iter().map(Vec::as_slice).collect();
        let fit = ppml::hdfe(&flow, &fe, &regressors, cluster.as_deref()).unwrap();
        let seconds = start.elapsed().as_secs_f64();
        let k = fit.names.len();
        let mut columns = vec![
            ("coef".into(), fit.coef.clone()),
            (
                "se".into(),
                (0..k).map(|i| fit.cov[(i, i)].sqrt()).collect(),
            ),
        ];
        for j in 0..k {
            columns.push((
                format!("cov_{j}"),
                (0..k).map(|i| fit.cov[(i, j)]).collect(),
            ));
        }
        Fitted {
            columns,
            names: fit.names,
            keep: Some(fit.keep),
            summary: Summary {
                design: name.into(),
                n: fit.n,
                n_separated: fit.n_separated,
                exporters: None,
                importers: None,
                pseudo_r2: fit.pseudo_r2,
            },
            seconds,
        }
    }
}

fn gap(a: f64, b: f64, mixed: bool) -> f64 {
    if a == b || (a.is_nan() && b.is_nan()) {
        0.0
    } else if !a.is_finite() || !b.is_finite() {
        f64::INFINITY
    } else if mixed {
        (a - b).abs() / 1f64.max(a.abs()).max(b.abs())
    } else {
        (a - b).abs() / a.abs().max(b.abs())
    }
}

#[test]
fn ppml_matches_python_dump() {
    let Some(dir) = std::env::var_os("CETERIS_PPML_PARITY") else {
        eprintln!("skipping: CETERIS_PPML_PARITY unset (a scripts/parity/step4_ppml_dump.py dump)");
        return;
    };
    let dir = Path::new(&dir);
    let out = dir.join("rust");
    std::fs::create_dir(&out).unwrap();
    let python_seconds: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("python/seconds.json")).unwrap())
            .unwrap();
    let mut names: Vec<String> = std::fs::read_dir(dir.join("designs"))
        .unwrap()
        .map(|e| {
            let p = e.unwrap().path();
            p.file_stem().unwrap().to_string_lossy().into_owned()
        })
        .collect();
    names.sort();

    let mut failures = Vec::new();
    let (mut worst, mut worst_mixed) = (0.0f64, 0.0f64);
    let mut summaries = Vec::new();
    for name in &names {
        let fitted = fit_design(name, &dir.join("designs").join(format!("{name}.arrow")));
        println!(
            "{name}: rust {:.3}s, python {:.3}s",
            fitted.seconds,
            python_seconds[name].as_f64().unwrap()
        );
        let (_, want) = read(&dir.join("python").join(format!("{name}.arrow")));
        let want_names: Vec<String> = want
            .iter()
            .flat_map(|b| {
                let a = b.column_by_name("name").unwrap().as_string::<i32>();
                (0..a.len())
                    .map(|i| a.value(i).to_owned())
                    .collect::<Vec<_>>()
            })
            .collect();
        assert_eq!(fitted.names, want_names, "{name}: names");
        for (col, got) in &fitted.columns {
            let want = f64s(&want, col);
            let (mut rel, mut mix) = (0.0f64, None::<f64>);
            for ((&a, &b), row) in got.iter().zip(&want).zip(&fitted.names) {
                if mixed(name, col, row) {
                    mix = Some(mix.unwrap_or(0.0).max(gap(a, b, true)));
                } else {
                    rel = rel.max(gap(a, b, false));
                }
            }
            match mix {
                Some(m) => println!("{name}.{col}: max_rel={rel:e} max_mixed={m:e}"),
                None => println!("{name}.{col}: max_rel={rel:e}"),
            }
            for max in [rel, mix.unwrap_or(0.0)] {
                if max.is_nan() || max > ITERATIVE {
                    failures.push(format!("{name}.{col}: {max:e}"));
                }
            }
            worst = worst.max(rel);
            worst_mixed = worst_mixed.max(mix.unwrap_or(0.0));
        }
        let mut columns: Vec<(&str, ArrayRef)> = vec![(
            "name",
            Arc::new(StringArray::from_iter_values(&fitted.names)),
        )];
        for (col, v) in &fitted.columns {
            columns.push((col.as_str(), floats(v.clone())));
        }
        write(&out.join(format!("{name}.arrow")), columns);
        if let Some(keep) = &fitted.keep {
            let (_, want) = read(&dir.join("python").join(format!("{name}.keep.arrow")));
            let want: Vec<bool> = want
                .iter()
                .flat_map(|b| {
                    let a = b.column_by_name("keep").unwrap().as_boolean();
                    (0..a.len()).map(|i| a.value(i)).collect::<Vec<_>>()
                })
                .collect();
            if *keep != want {
                failures.push(format!("{name}: keep differs"));
            }
            write(
                &out.join(format!("{name}.keep.arrow")),
                vec![("keep", Arc::new(BooleanArray::from(keep.clone())))],
            );
        }
        summaries.push(fitted.summary);
    }

    let (_, want) = read(&dir.join("python/summary.arrow"));
    let (n, sep) = (i64s(&want, "n"), i64s(&want, "n_separated"));
    let r2 = f64s(&want, "pseudo_r2");
    for (i, s) in summaries.iter().enumerate() {
        if (s.n as i64, s.n_separated as i64) != (n[i], sep[i]) {
            failures.push(format!("{}: n, n_separated differ", s.design));
        }
        let g = gap(s.pseudo_r2, r2[i], false);
        println!("{}.pseudo_r2: max_rel={g:e}", s.design);
        if g > ITERATIVE {
            failures.push(format!("{}.pseudo_r2: {g:e}", s.design));
        }
        worst = worst.max(g);
    }
    let count = |v: fn(&Summary) -> Option<usize>| -> ArrayRef {
        Arc::new(Int64Array::from(
            summaries
                .iter()
                .map(|s| v(s).map(|x| x as i64))
                .collect::<Vec<_>>(),
        ))
    };
    write(
        &out.join("summary.arrow"),
        vec![
            (
                "design",
                Arc::new(StringArray::from_iter_values(
                    summaries.iter().map(|s| &s.design),
                )),
            ),
            ("n", count(|s| Some(s.n))),
            ("n_separated", count(|s| Some(s.n_separated))),
            ("n_separated_exporters", count(|s| s.exporters)),
            ("n_separated_importers", count(|s| s.importers)),
            (
                "pseudo_r2",
                floats(summaries.iter().map(|s| s.pseudo_r2).collect()),
            ),
        ],
    );
    println!("worst max_rel={worst:e} max_mixed={worst_mixed:e}");
    assert!(failures.is_empty(), "{failures:#?}");
}
