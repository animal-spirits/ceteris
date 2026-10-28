//! ADR 0089: the calibrated data state is the steady state, on the fixture and, with
//! `CETERIS_PACK_EXPORT` set (a `ceteris export --pack` directory of the pack in `CETERIS_PACK`,
//! default `data/packs/2000`), on the full pack.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use arrow_array::RecordBatch;
use arrow_array::cast::AsArray;
use arrow_array::types::{Float64Type, UInt8Type, UInt32Type};
use arrow_ipc::reader::FileReader;
use arrow_schema::DataType;
use ceteris_calibrate::steady::{self, COVER, CalibrationError, Limits, PASS_TOL, SLOTS, Steady};
use ceteris_calibrate::write;
use ceteris_pack::compare::{self, Tolerance};
use ceteris_reference::demand;
use ceteris_reference::io::{self, Inputs, Mat};
use ceteris_reference::model::{self, Model, max_abs};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture() -> PathBuf {
    std::env::var_os("CETERIS_FIXTURE_DIR")
        .map_or_else(|| root().join("tests/golden/p2-fixture"), PathBuf::from)
}

fn pack() -> PathBuf {
    std::env::var_os("CETERIS_PACK").map_or_else(|| root().join("data/packs/2000"), PathBuf::from)
}

struct World {
    name: &'static str,
    inp: Inputs,
    s: Steady,
    /// The pack province of each province, for a world from a pack.
    pack_province: Option<Vec<usize>>,
}

fn quiet() -> impl FnMut(&str) {
    |_: &str| {}
}

/// The fixture and, when `CETERIS_PACK_EXPORT` is set, the pack, each calibrated once.
fn worlds() -> &'static [World] {
    static WORLDS: OnceLock<Vec<World>> = OnceLock::new();
    WORLDS.get_or_init(|| {
        let inp = io::load(fixture()).unwrap();
        let s = steady::calibrate(&inp, &inp.u_star.clone(), &mut quiet()).unwrap();
        let mut out = vec![World {
            name: "fixture",
            inp,
            s,
            pack_province: None,
        }];
        match std::env::var_os("CETERIS_PACK_EXPORT") {
            Some(dir) => {
                let dir = PathBuf::from(dir);
                let inp = io::load(&dir).unwrap();
                let u_star = write::read_u_star(&pack()).unwrap();
                let s = steady::calibrate(&inp, &u_star, &mut quiet()).unwrap();
                out.push(World {
                    name: "pack",
                    inp,
                    s,
                    pack_province: Some(write::read_pack_provinces(&dir).unwrap()),
                });
            }
            None => eprintln!(
                "skipping the pack: CETERIS_PACK_EXPORT unset (a full-pack export directory)"
            ),
        }
        out
    })
}

/// numpy's `assert_allclose`: |actual - desired| <= atol + rtol |desired|, NaN equal to NaN.
fn allclose(what: &str, actual: &[f64], desired: &[f64], rtol: f64, atol: f64) {
    assert_eq!(actual.len(), desired.len(), "{what}: length");
    for (i, (&a, &d)) in actual.iter().zip(desired).enumerate() {
        let ok = (a.is_nan() && d.is_nan()) || a == d || (a - d).abs() <= atol + rtol * d.abs();
        assert!(ok, "{what}[{i}]: {a} vs {d}");
    }
}

/// SplitMix64 with Box-Muller normals: reproducible draws for the tests.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn uniform(&mut self) -> f64 {
        ((self.next() >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }

    fn normal(&mut self, mean: f64, sd: f64) -> f64 {
        let (u, v) = (self.uniform(), self.uniform());
        mean + sd * (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }
}

/// Stage 4: weights inverted at buyer prices q and spending m give the final demand shares there
/// exactly, and at prices 1 the inversion is the one of ADR 0021.
#[test]
fn inversion_at_given_prices_reproduces_the_shares() {
    let inp = io::load(fixture()).unwrap();
    let (n_c, n_g) = (inp.n_countries(), inp.n_goods());
    let mut rng = Rng(4);
    let draws: Vec<f64> = (0..n_c * n_g).map(|_| rng.normal(0.0, 0.3).exp()).collect();
    let price = Mat {
        rows: n_c,
        cols: n_g,
        data: draws,
    };
    let m: Vec<f64> = inp
        .base_spend()
        .iter()
        .map(|b| b * rng.normal(0.0, 0.2).exp())
        .collect();
    let sigma = inp.sigma_income();
    let w = demand::invert_at(
        &inp.omega,
        &price,
        &m,
        &sigma,
        &inp.category,
        inp.sigma_demand,
        &inp.pigl,
    )
    .unwrap();
    let countries: Vec<usize> = (0..n_c).collect();
    let share = demand::shares(
        &w,
        &countries,
        &price,
        &m,
        &sigma,
        &inp.category,
        inp.sigma_demand,
        &inp.pigl,
    );
    let total = inp.omega.row_sums();
    let want = Mat::from_fn(n_c, n_g, |c, g| inp.omega[(c, g)] / total[c]);
    allclose("shares", &share.data, &want.data, 0.0, 1e-12);
    let ones = Mat::filled(n_c, n_g, 1.0);
    let at_one = demand::invert_at(
        &inp.omega,
        &ones,
        &inp.base_spend(),
        &sigma,
        &inp.category,
        inp.sigma_demand,
        &inp.pigl,
    )
    .unwrap();
    let base = inp.pigl_weights().unwrap();
    allclose("scale", &at_one.scale, &base.scale, 1e-13, 1e-15);
    allclose("omega", &at_one.omega.data, &base.omega.data, 1e-13, 1e-15);
    allclose(
        "within",
        &at_one.within.data,
        &base.within.data,
        1e-13,
        1e-15,
    );
}

/// Stage 2: from two starts the scaling reaches the same weights up to one scale per good.
#[test]
fn sinkhorn_is_unique_up_to_scale() {
    for w in worlds() {
        let (inp, s) = (&w.inp, &w.s);
        let (n_p, n_g) = (inp.n_provinces(), inp.n_goods());
        let m = Model::new(inp).unwrap();
        let mut v = Mat::zeros(n_p, n_g);
        for (e, (&p, &g)) in inp.ent_province.iter().zip(&inp.ent_good).enumerate() {
            v[(p, g)] += if inp.traded[g] {
                s.value[e] / s.r[g]
            } else {
                0.0
            };
        }
        let spend = Mat::from_fn(n_p, n_g, |p, g| s.spend_hh[(p, g)] + s.spend_int[(p, g)]);
        let origin = |p: usize, g: usize| inp.active[p][g] && inp.traded[g];
        let mut rng = Rng(9);
        let mut start = Mat::zeros(n_p, n_g);
        for p in 0..n_p {
            for g in (0..n_g).filter(|&g| origin(p, g)) {
                start[(p, g)] = rng.normal(0.0, 3.0);
            }
        }
        let one = steady::sinkhorn(&m, &spend, &v, None).unwrap();
        let other = steady::sinkhorn(&m, &spend, &v, Some(&start)).unwrap();
        for g in (0..n_g).filter(|&g| inp.traded[g]) {
            let d: Vec<f64> = (0..n_p)
                .filter(|&p| origin(p, g))
                .map(|p| one.log_weight[(p, g)] - other.log_weight[(p, g)])
                .collect();
            let mean = d.iter().sum::<f64>() / d.len() as f64;
            let spread = d.iter().fold(0.0f64, |m, x| m.max((x - mean).abs()));
            assert!(spread <= 1e-10, "{}: good {g} spread {spread}", w.name);
        }
        allclose(w.name, &one.r, &other.r, 1e-12, 0.0);
        let traded: Vec<usize> = (0..n_g).filter(|&g| inp.traded[g]).collect();
        let pick = |r: &[f64]| traded.iter().map(|&g| r[g]).collect::<Vec<_>>();
        allclose(w.name, &pick(&one.r), &pick(&s.r), 1e-12, 0.0);
    }
}

/// Stage 3: at the calibrated prices and base wages, with A = 1 / p_VA, coef = a_kg p_o / q_ik
/// and L0 = s_L VA0 / w0, the reference model's residual vanishes (every market clears, labour
/// at (1 - u*) L^s, the world price index 1), and VA, employment and output are the data's.
#[test]
fn closed_forms_zero_the_reference_residual() {
    for w in worlds() {
        let (inp, s) = (&w.inp, &w.s);
        let reference = steady::reference_inputs(inp, s);
        let x = steady::reference_point(inp, s);
        let m = Model::new(&reference).unwrap();
        let residual = max_abs(&model::residual(&x, &m));
        assert!(residual <= 1e-10, "{}: residual {residual}", w.name);
        let e = model::evaluate(&x, &m, &reference.ent_a);
        allclose(w.name, &e.value_added, &s.va0, 1e-10, 0.0);
        allclose(w.name, &e.employment, &s.l0, 1e-10, 0.0);
        allclose(w.name, &e.output, &s.output, 1e-10, 0.0);
        let target = inp.ent_va_target.as_ref().unwrap();
        let traded: Vec<usize> = (0..inp.ent_good.len())
            .filter(|&e| inp.traded[inp.ent_good[e]])
            .collect();
        let va0: Vec<f64> = traded.iter().map(|&e| s.va0[e]).collect();
        let want: Vec<f64> = traded
            .iter()
            .map(|&e| s.r[inp.ent_good[e]] * target[e])
            .collect();
        allclose(w.name, &va0, &want, 1e-13, 0.0);
        assert!(s.a.iter().all(|a| a.is_finite() && *a > 0.0), "{}", w.name);
        assert!(s.a_va.iter().all(|a| *a > 0.0), "{}", w.name);
    }
}

/// Local goods at p = 1: output solves each province's Leontief system against household and
/// intermediate local demand.
#[test]
fn local_output_is_nonnegative_and_clears_locally() {
    for w in worlds() {
        let (inp, s) = (&w.inp, &w.s);
        let (n_p, n_g) = (inp.n_provinces(), inp.n_goods());
        let (ep, eg) = (&inp.ent_province, &inp.ent_good);
        for e in (0..eg.len()).filter(|&e| !inp.traded[eg[e]]) {
            assert!(s.output[e] >= 0.0, "{}: enterprise {e}", w.name);
        }
        let mut used = Mat::zeros(n_p, n_g);
        let mut out = Mat::zeros(n_p, n_g);
        for e in 0..eg.len() {
            for k in 0..n_g {
                used[(ep[e], k)] += s.coef[(e, k)] * s.output[e];
            }
            out[(ep[e], eg[e])] += s.output[e];
        }
        let (mut got, mut want) = (Vec::new(), Vec::new());
        for p in 0..n_p {
            for g in (0..n_g).filter(|&g| !inp.traded[g]) {
                got.push(out[(p, g)]);
                want.push(s.spend_hh[(p, g)] + used[(p, g)]);
            }
        }
        allclose(w.name, &got, &want, 1e-12, 0.0);
    }
}

/// With national profit ownership every province's households earn dividends, so every local
/// enterprise produces and every province has employment and a positive base wage.
#[test]
fn every_province_produces_and_pays_a_wage() {
    for w in worlds() {
        let (inp, s) = (&w.inp, &w.s);
        let eg = &inp.ent_good;
        assert!(
            (0..eg.len())
                .filter(|&e| !inp.traded[eg[e]])
                .all(|e| s.output[e] > 0.0),
            "{}",
            w.name
        );
        let mut va = vec![0.0; inp.n_provinces()];
        for (&p, v) in inp.ent_province.iter().zip(&s.va0) {
            va[p] += v;
        }
        assert!(va.iter().all(|&v| v > 0.0), "{}", w.name);
        assert!(s.w0.iter().all(|w| w.is_finite() && *w > 0.0), "{}", w.name);
        assert!(s.l0.iter().all(|l| l.is_finite() && *l > 0.0), "{}", w.name);
    }
}

/// Stage 5: at the start of a tick each input stock is (1 + COVER) months of use and the pipeline
/// holds, j ticks ahead, one month of shipments of every arrival slot j or later, so the
/// order-up-to rule of E3 orders exactly one month of use; merchant stocks are coverage times
/// output; pops hold one month of income less this tick's wages.
#[test]
fn stage5_state_is_stationary_under_the_stock_rules() {
    for w in worlds() {
        let (inp, s) = (&w.inp, &w.s);
        let (n_p, n_g) = (inp.n_provinces(), inp.n_goods());
        let n_e = inp.ent_good.len();
        let ep = &inp.ent_province;
        let traded = &inp.traded;
        let used = Mat::from_fn(n_e, n_g, |e, k| s.coef[(e, k)] * s.output[e]);
        let want_stock: Vec<f64> = used.data.iter().map(|u| (1.0 + COVER) * u).collect();
        let stock = s.stock();
        allclose(w.name, &stock.data, &want_stock, 1e-15, 0.0);
        assert_eq!(s.arrive.len(), SLOTS + 1);
        // arrive[e][g][slot]: units arriving per month in each slot.
        let arrive =
            |e: usize, g: usize, slot: usize| s.arrive[slot][(ep[e], g)] * s.ent_spend[(e, g)];
        let ahead = |e: usize, g: usize| s.ent_pipeline[e * n_g + g];
        let use_max = used.data.iter().fold(0.0f64, |m, &u| m.max(u));
        let (mut got, mut want, mut order, mut month, mut first, mut later) =
            (vec![], vec![], vec![], vec![], vec![], vec![]);
        for e in 0..n_e {
            for g in (0..n_g).filter(|&g| traded[g]) {
                let total: f64 = (0..=SLOTS).map(|j| arrive(e, g, j)).sum();
                got.push(total);
                want.push(used[(e, g)]);
                let target = (1.0 + COVER + s.lead[(ep[e], g)]) * used[(e, g)];
                let held = stock[(e, g)] - arrive(e, g, 0);
                let on_order: f64 = ahead(e, g)[1..].iter().sum();
                order.push(target - held - on_order);
                month.push(used[(e, g)]);
                first.push(ahead(e, g)[0]);
                later.push((1..=SLOTS).map(|j| arrive(e, g, j)).sum());
            }
            for g in (0..n_g).filter(|&g| !traded[g]) {
                assert!(ahead(e, g).iter().all(|&u| u == 0.0), "{}", w.name);
            }
        }
        allclose(w.name, &got, &want, 1e-12, 0.0);
        allclose(w.name, &order, &month, 1e-10, 1e-12 * use_max);
        allclose(w.name, &first, &later, 1e-12, 0.0);
        let inventory = s.inventory();
        for p in 0..n_p {
            for g in (0..n_g).filter(|&g| traded[g] && inp.active[p][g]) {
                let want = s.coverage[g] * s.supply[(p, g)];
                allclose(w.name, &[inventory[(p, g)]], &[want], 1e-15, 0.0);
            }
        }
        let money: Vec<f64> = (0..s.pop_income.len())
            .map(|k| s.pop_income[k] - s.pop_wage[k])
            .collect();
        allclose(w.name, &s.pop_money(), &money, 1e-15, 0.0);
        allclose(w.name, &s.pop_spend.row_sums(), &s.pop_income, 1e-12, 0.0);
    }
}

fn read(dir: &Path, name: &str) -> RecordBatch {
    let file = File::open(dir.join(name)).unwrap();
    let mut batches: Vec<RecordBatch> = FileReader::try_new(file, None)
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(batches.len(), 1, "{name}: one batch");
    batches.pop().unwrap()
}

fn f64s(t: &RecordBatch, col: &str) -> Vec<f64> {
    t.column_by_name(col)
        .unwrap()
        .as_primitive::<Float64Type>()
        .values()
        .to_vec()
}

fn u32s(t: &RecordBatch, col: &str) -> Vec<usize> {
    let a = t.column_by_name(col).unwrap();
    assert_eq!(a.data_type(), &DataType::UInt32, "{col}");
    a.as_primitive::<UInt32Type>()
        .values()
        .iter()
        .map(|&v| v as usize)
        .collect()
}

fn u8s(t: &RecordBatch, col: &str) -> Vec<usize> {
    let a = t.column_by_name(col).unwrap();
    assert_eq!(a.data_type(), &DataType::UInt8, "{col}");
    a.as_primitive::<UInt8Type>()
        .values()
        .iter()
        .map(|&v| usize::from(v))
        .collect()
}

#[test]
fn calibration_tables_have_the_documented_layout() {
    let w = &worlds()[0];
    let (inp, s) = (&w.inp, &w.s);
    let n_g = inp.n_goods();
    let tmp = tempfile::tempdir().unwrap();
    let pack: Vec<usize> = (0..inp.n_provinces()).map(|p| 2 * p + 1).collect();
    write::write(tmp.path(), inp, s, &pack).unwrap();

    let ent = read(tmp.path(), "econ_calib.arrow");
    let names: Vec<String> = ent
        .schema()
        .fields()
        .iter()
        .map(|f| f.name().clone())
        .collect();
    assert_eq!(names[..3], ["province", "good", "a"]);
    let key: Vec<(usize, usize)> = u32s(&ent, "province")
        .into_iter()
        .zip(u8s(&ent, "good"))
        .collect();
    let mut sorted = key.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(key, sorted);
    assert_eq!(key.len(), inp.ent_good.len());
    let want: Vec<usize> = inp.ent_province.iter().map(|&p| pack[p]).collect();
    assert_eq!(u32s(&ent, "province"), want);
    assert_eq!(f64s(&ent, "a"), s.a);
    for k in 0..n_g {
        let want: Vec<f64> = (0..s.coef.rows).map(|e| s.coef[(e, k)]).collect();
        assert_eq!(f64s(&ent, &format!("coef_{k}")), want);
    }
    let meta: serde_json::Value =
        serde_json::from_str(&ent.schema().metadata()["calibration"]).unwrap();
    assert_eq!(meta["format"], steady::FORMAT);
    assert_eq!(meta["r"].as_array().unwrap().len(), n_g);

    let cells = read(tmp.path(), "econ_calib_cells.arrow");
    assert_eq!(cells.num_rows(), inp.n_provinces() * n_g);
    assert_eq!(f64s(&cells, "consumer_price"), s.consumer.data);
    let pops = read(tmp.path(), "econ_calib_pops.arrow");
    let key: Vec<(usize, usize)> = u32s(&pops, "province")
        .into_iter()
        .zip(u8s(&pops, "edu"))
        .collect();
    let mut sorted: Vec<(usize, usize)> = inp
        .pop_province
        .iter()
        .map(|&p| pack[p])
        .zip(u8s(&pops, "edu"))
        .collect();
    sorted.sort_unstable();
    assert_eq!(key, sorted);
    let pipe = read(tmp.path(), "econ_calib_pipeline.arrow");
    assert!(u8s(&pipe, "ahead").iter().all(|a| (1..=4).contains(a)));
    let units = f64s(&pipe, "units");
    assert!(units.iter().all(|&u| u > 0.0));
    let total: f64 = s
        .pop_pipeline
        .iter()
        .chain(&s.ent_pipeline)
        .flatten()
        .sum::<f64>();
    let got: f64 = units.iter().sum();
    assert!(
        (got - total).abs() <= 1e-12 * total.abs(),
        "{got} vs {total}"
    );
    let countries = read(tmp.path(), "econ_calib_countries.arrow");
    assert_eq!(f64s(&countries, "u_star"), inp.u_star);
    let provinces = read(tmp.path(), "econ_calib_provinces.arrow");
    assert_eq!(f64s(&provinces, "w0"), s.w0);
    for name in [
        "econ_calib.arrow",
        "econ_calib_cells.arrow",
        "econ_calib_provinces.arrow",
        "econ_calib_pops.arrow",
        "econ_calib_pipeline.arrow",
        "econ_calib_countries.arrow",
    ] {
        let t = read(tmp.path(), name);
        assert_eq!(
            t.schema().metadata()["calibration_format"],
            steady::FORMAT,
            "{name}"
        );
        assert!(
            t.schema().fields().iter().all(|f| f.is_nullable()),
            "{name}"
        );
    }
}

/// The committed fixture calibration (`tests/golden/p2-fixture/calibration`, which the Rust
/// property tests read) is the one this crate writes, within the tolerance of an iterative solve.
#[test]
fn fixture_tables_match_the_committed_calibration() {
    let w = &worlds()[0];
    let tmp = tempfile::tempdir().unwrap();
    let provinces: Vec<usize> = (0..w.inp.n_provinces()).collect();
    write::write(tmp.path(), &w.inp, &w.s, &provinces).unwrap();
    let committed = fixture().join("calibration");
    let report = compare::tables(&committed, tmp.path(), Tolerance::ITERATIVE).unwrap();
    assert!(report.ok, "{:#?}", report.lines());
    let meta = |dir: &Path| -> serde_json::Value {
        let t = read(dir, "econ_calib.arrow");
        serde_json::from_str(&t.schema().metadata()["calibration"]).unwrap()
    };
    let (got, want) = (meta(tmp.path()), meta(&committed));
    for key in ["format", "passes", "static_digest"] {
        assert_eq!(got[key], want[key], "{key}");
    }
}

/// A calibration that does not settle reports the passes it ran and the last change, and
/// writes no table; so does a scaling or an income fixed point that does not converge.
#[test]
fn nonconverging_calibration_reports_and_writes_nothing() {
    let inp = &worlds()[0].inp;
    let provinces: Vec<usize> = (0..inp.n_provinces()).collect();
    let tmp = tempfile::tempdir().unwrap();
    let run = |limits: Limits| {
        let out = tmp.path().join("out");
        let mut lines = Vec::new();
        let result = ceteris_calibrate::run(
            inp,
            &inp.u_star,
            &provinces,
            &out,
            limits,
            &mut |l: &str| lines.push(l.to_owned()),
        );
        assert!(
            !out.exists(),
            "a failed calibration wrote {}",
            out.display()
        );
        (result.unwrap_err(), lines)
    };
    let (err, lines) = run(Limits {
        max_passes: 3,
        ..Limits::default()
    });
    let CalibrationError::NotSettled { passes, change } = err.clone() else {
        panic!("{err}");
    };
    assert_eq!(passes, 3);
    assert!(change > PASS_TOL);
    assert_eq!(lines.len(), 3);
    assert!(lines[2].starts_with("pass 3: change ") && lines[2].contains(", residual "));
    assert_eq!(
        err.to_string(),
        format!(
            "passes did not settle in 3, last change {}",
            steady::sci(change)
        )
    );
    let (err, _) = run(Limits {
        max_sweeps: 2,
        ..Limits::default()
    });
    assert!(
        err.to_string()
            .starts_with("Sinkhorn did not converge in 2 sweeps, residual "),
        "{err}"
    );
    let (err, _) = run(Limits {
        max_local: 1,
        ..Limits::default()
    });
    assert_eq!(
        err.to_string(),
        "income and local output did not settle in 1 iterations"
    );
}

/// `EconInputs::from_pack` loads the tables this crate writes for the pack: a copy of the pack
/// (its files linked) with the Rust tables in place of its calibration.
#[cfg(unix)]
#[test]
fn pack_world_loads_the_written_tables() {
    let Some(w) = worlds().iter().find(|w| w.name == "pack") else {
        eprintln!("skipping: CETERIS_PACK_EXPORT unset (a full-pack export directory)");
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    let copy = tmp.path().join("pack");
    std::fs::create_dir(&copy).unwrap();
    let pack = std::fs::canonicalize(pack()).unwrap();
    for entry in std::fs::read_dir(&pack).unwrap() {
        let name = entry.unwrap().file_name();
        if !name.to_string_lossy().starts_with("econ_calib") {
            std::os::unix::fs::symlink(pack.join(&name), copy.join(&name)).unwrap();
        }
    }
    write::write(&copy, &w.inp, &w.s, w.pack_province.as_ref().unwrap()).unwrap();
    let graph =
        ceteris_geo::pack::Pack::load(&copy, &ceteris_geo::graph::TransportParams::default())
            .unwrap();
    let inputs = ceteris_econ::EconInputs::from_pack(&graph, &copy).unwrap();
    let steady = inputs.steady.as_ref().expect("a calibrated steady state");
    assert_eq!(steady.output, w.s.output);
    assert_eq!(
        inputs.enterprises.a, w.s.a,
        "the enterprises' A are the calibrated ones"
    );
}
