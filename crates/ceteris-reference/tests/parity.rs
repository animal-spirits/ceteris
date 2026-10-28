//! Compares the reference with the values the Python reference derives from the same export,
//! dumped as JSON by `scripts/parity/step4_dump.py` (`scripts/parity/step4.sh` runs both). Skips
//! without `CETERIS_REFERENCE_PARITY`, the path of the dump.

use serde_json::Value;
use statrs::function::erf::erf_inv;

use ceteris_reference::demand::{self, Weights};
use ceteris_reference::io::{self, Mat};
use ceteris_reference::sinkhorn;
use ceteris_reference::uniqueness;

/// ADR 0113: floats agree to a relative difference of 1e-12 per element.
const TOL: f64 = 1e-12;

fn relative(a: f64, b: f64) -> f64 {
    if a == b || (a.is_nan() && b.is_nan()) {
        0.0
    } else if !a.is_finite() || !b.is_finite() {
        f64::INFINITY
    } else {
        (a - b).abs() / a.abs().max(b.abs())
    }
}

fn float(v: &Value) -> f64 {
    match v {
        Value::String(s) => match s.as_str() {
            "inf" => f64::INFINITY,
            "-inf" => f64::NEG_INFINITY,
            "nan" => f64::NAN,
            other => panic!("not a float: {other}"),
        },
        v => v.as_f64().unwrap(),
    }
}

fn floats(v: &Value) -> Vec<f64> {
    v["data"].as_array().unwrap().iter().map(float).collect()
}

fn mat(v: &Value) -> Mat {
    let shape: Vec<usize> = v["shape"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_u64().unwrap() as usize)
        .collect();
    let [rows, cols] = shape[..] else {
        panic!("not a matrix: {shape:?}")
    };
    Mat {
        rows,
        cols,
        data: floats(v),
    }
}

fn ints(v: &Value) -> Vec<usize> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|i| i.as_u64().unwrap() as usize)
        .collect()
}

struct Report {
    worst: f64,
    failures: Vec<String>,
}

impl Report {
    fn check(&mut self, name: &str, got: &[f64], want: &[f64], tol: f64) {
        assert_eq!(got.len(), want.len(), "{name}: length");
        let max = got
            .iter()
            .zip(want)
            .map(|(&a, &b)| relative(a, b))
            .fold(0.0, f64::max);
        println!("{name}: max_rel={max:e}");
        if max > tol {
            self.failures.push(format!("{name}: {max:e} > {tol:e}"));
        }
        self.worst = self.worst.max(max);
    }

    fn weights(&mut self, name: &str, got: &Weights, want: &Value) {
        self.check(
            &format!("{name}.scale"),
            &got.scale,
            &floats(&want["scale"]),
            TOL,
        );
        self.check(
            &format!("{name}.omega"),
            &got.omega.data,
            &floats(&want["omega"]),
            TOL,
        );
        self.check(
            &format!("{name}.within"),
            &got.within.data,
            &floats(&want["within"]),
            TOL,
        );
    }
}

#[test]
fn reference_matches_python_dump() {
    let Some(path) = std::env::var_os("CETERIS_REFERENCE_PARITY") else {
        eprintln!("skipping: CETERIS_REFERENCE_PARITY unset (a scripts/parity/step4_dump.py dump)");
        return;
    };
    let d: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let inp = io::load(d["inputs"].as_str().unwrap()).unwrap();
    let mut r = Report {
        worst: 0.0,
        failures: Vec::new(),
    };

    let derived: [(&str, Vec<f64>); 12] = [
        ("labour_supply", inp.labour_supply.clone()),
        ("population", inp.population.clone()),
        ("pop_effective", inp.pop_effective.clone()),
        ("ent_inputs", inp.ent_inputs().data),
        ("ent_a_va", inp.ent_a_va()),
        ("base_income", inp.base_income()),
        ("base_spend", inp.base_spend()),
        ("sigma_income", inp.sigma_income()),
        ("employed_supply", inp.employed_supply()),
        ("index_weights", inp.index_weights().data),
        ("base_output", inp.base_output().data),
        ("base_demand", inp.base_demand().data),
    ];
    for (name, got) in derived {
        r.check(name, &got, &floats(&d[name]), TOL);
    }
    assert_eq!(inp.active_cells(), ints(&d["active_cells"]), "active_cells");
    let w = inp.pigl_weights().unwrap();
    r.weights("pigl_weights", &w, &d["pigl_weights"]);

    let at = &d["invert_at"];
    let got = demand::invert_at(
        &inp.omega,
        &mat(&at["price"]),
        &floats(&at["spend"]),
        &inp.sigma_income(),
        &inp.category,
        inp.sigma_demand,
        &inp.pigl,
    )
    .unwrap();
    r.weights("invert_at", &got, &at["weights"]);

    let draws = &d["draws"];
    let price = mat(&draws["price"]);
    let money = floats(&draws["money"]);
    let share = demand::shares(
        &w,
        &ints(&draws["country"]),
        &price,
        &money,
        &floats(&draws["sigma"]),
        &inp.category,
        inp.sigma_demand,
        &inp.pigl,
    );
    r.check("draws.share", &share.data, &floats(&draws["share"]), TOL);
    let quantity = demand::quantities(&share, &price, &money);
    r.check(
        "draws.quantity",
        &quantity.data,
        &floats(&draws["quantity"]),
        TOL,
    );

    let e = &d["erfinv"];
    let x = floats(&e["x"]);
    let y: Vec<f64> = x.iter().map(|&x| erf_inv(x)).collect();
    r.check("erfinv", &y, &floats(&e["y"]), 1e-15);

    let u = &d["uniqueness"];
    let va = uniqueness::value_added_share(&inp);
    r.check(
        "value_added_share",
        &va,
        &floats(&u["value_added_share"]),
        TOL,
    );
    let traded: Vec<usize> = (0..inp.n_goods()).filter(|&g| inp.theta[g] > 0.0).collect();
    let mut radius = Vec::new();
    for (i, &g) in traded.iter().enumerate() {
        let m = uniqueness::trade_with_intermediates(inp.theta[g], va[g]).unwrap();
        r.check(
            &format!("trade_with_intermediates[{g}]"),
            &m.data,
            &floats(&u["matrix"][i]),
            TOL,
        );
        radius.push(uniqueness::spectral_radius(&m));
    }
    r.check("spectral_radius", &radius, &floats(&u["radius"]), TOL);
    let bound = uniqueness::spectral_bound(&inp.theta, 0.0, None, &va).unwrap();
    r.check("spectral_bound", &[bound], &floats(&u["bound"]), TOL);
    let labour_only =
        uniqueness::spectral_bound(&inp.theta, 0.0, None, &vec![1.0; inp.n_goods()]).unwrap();
    r.check(
        "spectral_bound_labour_only",
        &[labour_only],
        &floats(&u["bound_labour_only"]),
        TOL,
    );

    let random = &d["random_radius"];
    let radius: Vec<f64> = random["matrix"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| uniqueness::spectral_radius(&mat(m)))
        .collect();
    r.check("random_radius", &radius, &floats(&random["radius"]), TOL);

    if let Some(kernels) = d["kernels"].as_array() {
        for k in kernels {
            let g = k["good"].as_u64().unwrap() as usize;
            let kern = sinkhorn::dense_kernel(&inp, g, false);
            r.check(
                &format!("dense_kernel[{g}]"),
                &kern.data,
                &floats(&k["kernel"]),
                TOL,
            );
            let net = sinkhorn::dense_kernel(&inp, g, true);
            r.check(
                &format!("dense_kernel_net[{g}]"),
                &net.data,
                &floats(&k["net"]),
                TOL,
            );
            let flows =
                sinkhorn::balanced(&net, &floats(&k["supply"]), &floats(&k["demand"])).unwrap();
            r.check(
                &format!("balanced[{g}]"),
                &flows.data,
                &floats(&k["balanced"]),
                TOL,
            );
        }
    }
    println!("worst max_rel={:e}", r.worst);
    assert!(r.failures.is_empty(), "{:#?}", r.failures);
}
