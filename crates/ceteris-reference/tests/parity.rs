//! Compares the reference with the values the Python reference derives from the same export,
//! dumped as JSON by `scripts/parity/step4_dump.py` (`scripts/parity/step4.sh` runs both). Skips
//! without `CETERIS_REFERENCE_PARITY`, the path of the dump.

use serde_json::Value;
use statrs::function::erf::erf_inv;

use ceteris_reference::demand::{self, Weights};
use ceteris_reference::io::{self, Mat};
use ceteris_reference::model::{self, Model};
use ceteris_reference::sinkhorn;
use ceteris_reference::solve;
use ceteris_reference::uniqueness;

/// ADR 0113: floats agree to a relative difference of 1e-12 per element, 1e-9 where an iterative
/// solve sets the value.
const TOL: f64 = 1e-12;
const ITERATIVE: f64 = 1e-9;

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

    /// Largest absolute difference, for values that are rounding noise around 0 (a residual at a
    /// root) or logs of values compared relatively (ln p, ln w).
    fn check_abs(&mut self, name: &str, got: &[f64], want: &[f64], tol: f64) {
        assert_eq!(got.len(), want.len(), "{name}: length");
        let max = got
            .iter()
            .zip(want)
            .map(|(&a, &b)| if a == b { 0.0 } else { (a - b).abs() })
            .fold(0.0, f64::max);
        println!("{name}: max_abs={max:e}");
        if max.is_nan() || max > tol {
            self.failures
                .push(format!("{name}: {max:e} > {tol:e} absolute"));
        }
    }

    /// Every quantity of `model.evaluate` at the point's x, to 1e-12; at a `root` the residual
    /// and the excess demand are rounding noise and are compared absolutely.
    fn point(&mut self, prefix: &str, m: &Model, point: &Value, root: bool) {
        let x = floats(&point["x"]);
        let s = model::evaluate(&x, m, &m.inp.ent_a);
        let f = &s.flows;
        let got: [(&str, &[f64]); 23] = [
            ("residual", &m.market_residual(&s)),
            ("excess", &m.excess_of(&s)),
            ("price", &s.price.data),
            ("wage", &s.wage),
            ("employment", &s.employment),
            ("output", &s.output),
            ("value_added", &s.value_added),
            ("supply", &s.supply.data),
            ("consumer_price", &s.consumer_price.data),
            ("spend", &s.spend.data),
            ("demand", &s.demand.data),
            ("q", &s.q.data),
            ("labour_demand", &s.labour_demand),
            ("income", &s.income),
            ("index", &[s.index]),
            ("flows.orders", &f.orders.data),
            ("flows.home", &f.home.data),
            ("flows.arrive", &f.arrive.data),
            ("flows.paid", &f.paid.data),
            ("flows.received", &f.received.data),
            ("flows.tariff", &f.tariff.data),
            ("flows.pair_value", &f.pair_value),
            ("flows.pair_units", &f.pair_units),
        ];
        for (key, values) in got {
            let name = format!("{prefix}.{key}");
            if root && (key == "residual" || key == "excess") {
                self.check_abs(&name, values, &floats(&point[key]), TOL);
            } else {
                self.check(&name, values, &floats(&point[key]), TOL);
            }
        }
    }

    /// The equilibrium from the Python solve's start (default the data state) against its
    /// solution, to the 1e-9 of an iterative solve.
    fn solution(&mut self, prefix: &str, m: &Model, want: &Value) {
        let start = (!want["start"].is_null()).then(|| floats(&want["start"]));
        let eq = solve::equilibrium(m, start.as_deref(), solve::TOL, None).unwrap();
        let info = &want["info"];
        println!(
            "{prefix}: rust {} in {} steps, {:.2}s, residual {:e}; python {} in {} steps, {:.2}s, residual {:e}",
            eq.info.method,
            eq.info.iterations,
            eq.info.seconds,
            eq.info.residual,
            info["method"].as_str().unwrap(),
            info["iterations"],
            info["seconds"].as_f64().unwrap(),
            info["residual"].as_f64().unwrap(),
        );
        // ln p and ln w: an absolute difference is the relative one of p and w.
        self.check_abs(
            &format!("{prefix}.x"),
            &eq.x,
            &floats(&want["x"]),
            ITERATIVE,
        );
        let got: [(&str, &[f64]); 13] = [
            ("price", &eq.price.data),
            ("wage", &eq.wage),
            ("employment", &eq.employment),
            ("output", &eq.output),
            ("supply", &eq.supply.data),
            ("consumer_price", &eq.consumer_price.data),
            ("expenditure", &eq.expenditure.data),
            ("home_value", &eq.home_value.data),
            ("home_units", &eq.home_units.data),
            ("flows_by_country_pair_good", &eq.flows_by_country_pair_good),
            ("units_by_country_pair_good", &eq.units_by_country_pair_good),
            ("income", &eq.income),
            ("price_index", &[eq.price_index]),
        ];
        for (key, values) in got {
            self.check(
                &format!("{prefix}.{key}"),
                values,
                &floats(&want[key]),
                ITERATIVE,
            );
        }
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
        radius.push(uniqueness::spectral_radius(&m).unwrap());
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
        .map(|m| uniqueness::spectral_radius(&mat(m)).unwrap())
        .collect();
    r.check("random_radius", &radius, &floats(&random["radius"]), TOL);

    let m = Model::new(&inp).unwrap();
    // The first point is the data state, the second the data state plus a draw.
    r.check(
        "data_state",
        &m.data_state(),
        &floats(&d["model"][0]["x"]),
        TOL,
    );
    for (i, point) in d["model"].as_array().unwrap().iter().enumerate() {
        r.point(&format!("model[{i}]"), &m, point, false);
    }
    if !d["solution"].is_null() {
        r.solution("solution", &m, &d["solution"]);
    }
    let rf = &d["reference"];
    if !rf.is_null() {
        let mut calibrated = inp.clone();
        calibrated.ent_a = floats(&rf["ent_a"]);
        calibrated.ent_va0 = floats(&rf["ent_va0"]);
        calibrated.ent_l0 = floats(&rf["ent_l0"]);
        calibrated.w0 = floats(&rf["w0"]);
        calibrated.u_star = floats(&rf["u_star"]);
        calibrated.calibrated_coef = Some(mat(&rf["coef"]));
        calibrated.calibrated_a_va = Some(floats(&rf["a_va"]));
        let w = &rf["weights"];
        calibrated.calibrated_weights = Some(Weights {
            scale: floats(&w["scale"]),
            omega: mat(&w["omega"]),
            within: mat(&w["within"]),
        });
        let m = Model::new(&calibrated).unwrap();
        r.point("reference.point", &m, &rf["point"], true);
        let at_point = model::max_abs(&model::residual(&floats(&rf["point"]["x"]), &m));
        println!("reference.point: residual {at_point:e}");
        r.solution("reference.solution", &m, &rf["solution"]);
    }

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
