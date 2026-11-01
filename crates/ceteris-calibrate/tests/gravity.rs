//! Direct estimation of trade costs (ADR 0022): the PPML estimator reproduces Yotov et al.
//! (2016) on their sample, and each estimator recovers known values on synthetic gravity.

use std::fs::File;
use std::path::{Path, PathBuf};

use arrow_array::RecordBatch;
use arrow_array::cast::AsArray;
use arrow_array::types::Float64Type;
use arrow_cast::cast;
use arrow_ipc::reader::FileReader;
use arrow_schema::DataType;
use ceteris_calibrate::dta::{Column, read_dta};
use ceteris_calibrate::gravity::{
    self, AIR, CLASSES, Graph, N_Q, Q_FEE, Q_HOURS, Quantities, RAIL, ROAD, SEA, Sample,
    THETA_DEFAULT, TIME_COST, Transport, YOTOV, YOTOV_PUBLISHED,
};

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

/// `round(x, 3)`.
fn round3(x: f64) -> f64 {
    format!("{x:.3}").parse().unwrap()
}

/// Every published PPML estimate and pair-clustered standard error of Tables 1 to 3, to the
/// printed three decimals, and the observation counts.
#[test]
fn gravity_reproduces_yotov_2016() {
    let zip = root().join("data/cache").join(YOTOV);
    if !zip.exists() {
        eprintln!("skipping: no {YOTOV} in data/cache; fetch it with `ceteris pack fetch`");
        return;
    }
    let fits = gravity::yotov(&zip).unwrap();
    assert_eq!(fits.len(), YOTOV_PUBLISHED.len());
    for (published, (column, fit)) in YOTOV_PUBLISHED.iter().zip(&fits) {
        assert_eq!(published.column, column);
        assert_eq!(fit.n, published.n, "{column}");
        for &(term, estimate, se) in published.terms {
            let (got, got_se) = (fit.coef(term), fit.se(term));
            assert!(
                (round3(got) - estimate).abs() <= 1e-9,
                "{column} {term} {got}"
            );
            assert!(
                (round3(got_se) - se).abs() <= 1e-9,
                "{column} {term} {got_se}"
            );
        }
    }
}

#[test]
fn read_dta_round_trips_numbers_strings_and_missing() {
    let names = ["exporter", "year", "trade", "flag"];
    let types = [3u8, 252, 255, 251];
    let mut data = vec![113u8, 2, 1, 0];
    data.extend((names.len() as u16).to_le_bytes());
    data.extend(3i32.to_le_bytes());
    data.extend([0u8; 81 + 18]);
    data.extend(types);
    for n in names {
        let mut b = n.as_bytes().to_vec();
        b.resize(33, 0);
        data.extend(b);
    }
    data.extend(vec![
        0u8;
        2 * (names.len() + 1)
            + 12 * names.len()
            + 33 * names.len()
    ]);
    data.extend(vec![0u8; 81 * names.len()]);
    data.extend([0u8; 5]);
    let rows: [(&str, i16, f64, i8); 3] = [
        ("USA", 1986, 1.5, 1),
        ("CA", 2006, -2.0, 0),
        ("DEU", 1990, 8.988_465_674_311_58e307, 101),
    ];
    for (e, y, t, f) in rows {
        let mut b = e.as_bytes().to_vec();
        b.resize(3, 0);
        data.extend(b);
        data.extend(y.to_le_bytes());
        data.extend(t.to_le_bytes());
        data.extend(f.to_le_bytes());
    }
    let out = read_dta(&data).unwrap();
    assert_eq!(
        out["exporter"],
        Column::Strings(vec!["USA".into(), "CA".into(), "DEU".into()])
    );
    assert_eq!(out["year"], Column::Numbers(vec![1986.0, 2006.0, 1990.0]));
    let trade = out["trade"].numbers().unwrap();
    assert_eq!(&trade[..2], &[1.5, -2.0]);
    assert!(trade[2].is_nan());
    let flag = out["flag"].numbers().unwrap();
    assert_eq!(&flag[..2], &[1.0, 0.0]);
    assert!(flag[2].is_nan());
}

/// A big-endian file (byte order 1) of two rows, one of each numeric width.
#[test]
fn read_dta_reads_big_endian_files() {
    let names = ["code", "b", "h", "l", "f", "d"];
    let types = [2u8, 251, 252, 253, 254, 255];
    let mut data = vec![114u8, 1, 1, 0];
    data.extend((names.len() as u16).to_be_bytes());
    data.extend(2i32.to_be_bytes());
    data.extend([0u8; 81 + 18]);
    data.extend(types);
    for n in names {
        let mut b = n.as_bytes().to_vec();
        b.resize(33, 0);
        data.extend(b);
    }
    data.extend(vec![
        0u8;
        2 * (names.len() + 1)
            + 49 * names.len()
            + 33 * names.len()
    ]);
    data.extend(vec![0u8; 81 * names.len()]);
    data.extend([0u8; 5]);
    for (code, b, h, l, f, d) in [
        ("AB", -3i8, -300i16, 70_000i32, 1.5f32, -2.25f64),
        (
            "Z",
            101,
            32_741,
            2_147_483_621,
            1.701_411_8e38,
            8.988_465_674_311_58e307,
        ),
    ] {
        let mut c = code.as_bytes().to_vec();
        c.resize(2, 0);
        data.extend(c);
        data.extend(b.to_be_bytes());
        data.extend(h.to_be_bytes());
        data.extend(l.to_be_bytes());
        data.extend(f.to_be_bytes());
        data.extend(d.to_be_bytes());
    }
    let out = read_dta(&data).unwrap();
    assert_eq!(out["code"], Column::Strings(vec!["AB".into(), "Z".into()]));
    for (name, want) in [
        ("b", -3.0),
        ("h", -300.0),
        ("l", 70_000.0),
        ("f", 1.5),
        ("d", -2.25),
    ] {
        let v = out[name].numbers().unwrap();
        assert_eq!(v[0], want, "{name}");
        assert!(v[1].is_nan(), "{name}: missing code read as {}", v[1]);
    }
}

/// splitmix64 draws without a dependency.
struct Draws(u64);

impl Draws {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn uniform(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn normal(&mut self) -> f64 {
        let (u, v) = (self.uniform(f64::EPSILON, 1.0), self.uniform(0.0, 1.0));
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }

    /// Marsaglia and Tsang, boosted below shape 1.
    fn gamma(&mut self, shape: f64) -> f64 {
        if shape < 1.0 {
            let u = self.uniform(f64::EPSILON, 1.0);
            return self.gamma(shape + 1.0) * u.powf(1.0 / shape);
        }
        let d = shape - 1.0 / 3.0;
        let c = 1.0 / (9.0 * d).sqrt();
        loop {
            let x = self.normal();
            let v = (1.0 + c * x).powi(3);
            if v <= 0.0 {
                continue;
            }
            let u = self.uniform(f64::EPSILON, 1.0);
            if u.ln() < 0.5 * x * x + d - d * v + d * v.ln() {
                return d * v;
            }
        }
    }

    fn dirichlet<const N: usize>(&mut self, alpha: [f64; N]) -> [f64; N] {
        let g = alpha.map(|a| self.gamma(a));
        let total: f64 = g.iter().sum();
        g.map(|x| x / total)
    }

    /// Knuth's product of uniforms, over chunks of mean at most 30.
    fn poisson(&mut self, lambda: f64) -> f64 {
        let chunks = (lambda / 30.0).ceil().max(1.0);
        let limit = (-lambda / chunks).exp();
        let mut k = 0.0;
        for _ in 0..chunks as usize {
            let mut p = self.uniform(0.0, 1.0);
            while p > limit {
                k += 1.0;
                p *= self.uniform(0.0, 1.0);
            }
        }
        k
    }
}

/// Synthetic countries on a plane with a mode mix per pair, so that S05-like path quantities
/// exist: `q[o * n + d]` and the importer's tariff `rate[o * n + d]` (0 inside agreements).
fn world(n: usize, seed: u64) -> (Vec<Quantities>, Vec<f64>) {
    let mut r = Draws(seed);
    let xy: Vec<(f64, f64)> = (0..n)
        .map(|_| (r.uniform(0.0, 1.0), r.uniform(0.0, 1.0)))
        .collect();
    let mut q = vec![[0.0; N_Q]; n * n];
    for o in 0..n {
        for d in 0..n {
            let km = if o == d {
                200.0
            } else {
                3000.0 * (xy[o].0 - xy[d].0).hypot(xy[o].1 - xy[d].1) + 50.0
            };
            // Road, rail, sea and air km shares.
            let share = r.dirichlet([2.0, 1.0, 2.0, 0.3]);
            for m in 0..4 {
                q[o * n + d][m] = km * share[m];
            }
            q[o * n + d][Q_FEE] = r.uniform(0.0, 20.0);
            q[o * n + d][Q_HOURS] = km / 30.0;
        }
    }
    let mut agreement = vec![false; n * n];
    for a in agreement.iter_mut() {
        *a = r.uniform(0.0, 1.0) < 0.3;
    }
    let own: Vec<f64> = (0..n).map(|_| r.uniform(0.02, 0.3)).collect();
    let mut rate = vec![0.0; n * n];
    for o in 0..n {
        for d in (0..n).filter(|&d| d != o) {
            let agreed = agreement[o * n + d] || agreement[d * n + o];
            rate[o * n + d] = if agreed { 0.0 } else { own[d] };
        }
    }
    (q, rate)
}

/// Flows exp(FE - theta ln(1 + t) - theta ln tau - theta ln(1 + nu) border) with tau the S05
/// form.
fn synthetic_sample(theta: &[f64; 7], nu: f64, cost: [f64; 4], domestic: bool) -> Sample {
    // Light enough that every good trades.
    let weight = [0.0005, 0.0008, 0.002, 0.0016, 0.0003, 0.0008, 0.0001];
    weighted_sample(theta, nu, cost, domestic, weight, usize::MAX, false)
}

/// [`synthetic_sample`] at the given weights, with air only between the first `air_among`
/// countries (elsewhere its km go by road; `q` holds road, rail, sea and air km first), and
/// with `log_cost` flows on ln(tau - 1), the log of cost, in place of the S05 ln tau.
fn weighted_sample(
    theta: &[f64; 7],
    nu: f64,
    cost: [f64; 4],
    domestic: bool,
    weight: [f64; 7],
    air_among: usize,
    log_cost: bool,
) -> Sample {
    let n = 30;
    let (mut q, rate) = world(n, 11);
    for (i, q) in q.iter_mut().enumerate() {
        if i / n >= air_among || i % n >= air_among {
            q[0] += q[3];
            q[3] = 0.0;
        }
    }
    let mut r = Draws(5);
    let mut s = Sample::default();
    for g in 0..theta.len() {
        let a: Vec<f64> = (0..n).map(|_| r.normal()).collect();
        let b: Vec<f64> = (0..n).map(|_| r.normal()).collect();
        let mut tau = vec![0.0; n * n];
        let mut mu = vec![0.0; n * n];
        for (o, &ao) in a.iter().enumerate() {
            for (d, &bd) in b.iter().enumerate() {
                let i = o * n + d;
                let c: f64 = (0..4).map(|m| q[i][m] * cost[m]).sum::<f64>() + q[i][Q_FEE];
                tau[i] = 1.0 + weight[g] * c + TIME_COST[g] * q[i][Q_HOURS] / 24.0;
                if log_cost {
                    tau[i] -= 1.0;
                }
                let border = if o == d { 0.0 } else { 1.0 };
                mu[i] = (ao + bd
                    - theta[g] * (rate[i].ln_1p() + tau[i].ln() + border * nu.ln_1p()))
                .exp();
            }
        }
        let mean = mu.iter().sum::<f64>() / mu.len() as f64;
        for o in 0..n {
            for d in 0..n {
                let i = o * n + d;
                let flow = r.poisson(mu[i] * 500.0 / mean);
                if o == d && !domestic {
                    continue;
                }
                s.exporter.push(o);
                s.importer.push(d);
                s.good.push(g);
                s.flow.push(flow);
                s.tariff.push(rate[i]);
                s.tau.push(tau[i]);
                s.q.push(q[i]);
                s.weight.push(weight[g]);
                s.time_cost.push(TIME_COST[g]);
            }
        }
    }
    s
}

/// Deliberately the first partition: synthetic data for the estimators, independent of the
/// P01 defaults.
const SYNTHETIC_CLASSES: [&[usize]; 3] = [&[3], &[4, 6], &[0, 1, 2, 5]];
const THETA: [f64; 7] = [12.0, 12.0, 12.0, 3.0, 6.0, 12.0, 6.0];
const COST: [f64; 4] = [0.10, 0.04, 0.008, 0.8];

fn approx(got: f64, want: f64, rel: f64, what: &str) {
    assert!(
        (got - want).abs() <= rel * want.abs(),
        "{what}: {got} vs {want}"
    );
}

#[test]
fn theta_by_class_recovers_known_thetas() {
    let s = synthetic_sample(&THETA, 0.0, COST, false);
    let est = gravity::theta_by_class(&s, &SYNTHETIC_CLASSES).unwrap();
    for (c, e) in est.iter().enumerate() {
        approx(
            e.theta,
            THETA[SYNTHETIC_CLASSES[c][0]],
            0.05,
            &format!("{c}"),
        );
        assert!(0.0 < e.se && e.se < 0.25 * e.theta, "{c}: {e:?}");
        assert!(
            e.n > 0 && 0.0 < e.pseudo_r2 && e.pseudo_r2 <= 1.0,
            "{c}: {e:?}"
        );
    }
}

#[test]
fn border_friction_recovers_known_nu() {
    let s = synthetic_sample(&THETA, 0.4, COST, true);
    let est = gravity::border_friction(&s, &SYNTHETIC_CLASSES, Some(&[0.1, 0.4, 0.5])).unwrap();
    approx(est.nu, 0.4, 0.05, "nu");
    assert!(0.0 < est.se && est.se < 0.1, "{est:?}");
    for (c, &nu) in est.nu_by_class.iter().enumerate() {
        approx(nu, 0.4, 0.1, &format!("class {c}"));
    }
}

/// Road is the numeraire at its P04a default (0.10); rail, sea and air are estimated relative
/// to it.
#[test]
fn mode_costs_recover_known_ratios() {
    let cost = [0.10, 0.03, 0.01, 0.5];
    let s = synthetic_sample(&THETA, 0.0, cost, false);
    let est = gravity::mode_costs(&s, &SYNTHETIC_CLASSES, &Transport::default()).unwrap();
    for m in 0..3 {
        let want = cost[m + 1] / cost[0];
        approx(est.ratio[m], want, 0.1, MODE[m]);
        assert!(0.0 < est.se[m] && est.se[m] < 0.2 * want, "{est:?}");
    }
}

const MODE: [&str; 3] = ["rail", "sea", "air"];

/// Deliberately heavy goods and an air cost far below its start (8 times road): ln tau is
/// concave in the air ratio, so the undamped first Gauss-Newton step overshoots past the ratio
/// at which some tau reaches 0 (the crash recorded in ADR 0022 before its Amendment
/// 2026-10-29). The guarded step halves it instead and the fit still recovers the ratios.
#[test]
fn mode_costs_guard_keeps_tau_positive() {
    let cost = [0.10, 0.04, 0.008, 0.004];
    let s = weighted_sample(&THETA, 0.0, cost, false, [0.01; 7], usize::MAX, false);
    let p = Transport::default();
    let est = gravity::mode_costs(&s, &SYNTHETIC_CLASSES, &p).unwrap();
    let start: [f64; 3] = std::array::from_fn(|m| p.per_tkm[m + 1] / p.per_tkm[0]);
    let undamped: [f64; 3] = std::array::from_fn(|m| start[m] + est.steps[0][m]);
    let tau = gravity::mode_tau(&s, &undamped, &p);
    assert!(tau.iter().any(|&t| t <= 0.0), "{undamped:?}");
    assert!(est.halvings[0] > 0, "{est:?}");
    for m in 0..3 {
        approx(est.ratio[m], cost[m + 1] / cost[0], 0.1, MODE[m]);
    }
    assert!(
        gravity::mode_tau(&s, &est.ratio, &p)
            .iter()
            .all(|&t| t > 0.0)
    );
}

/// Flows on the log of cost (the form the 2000 data prefer, ADR 0022) and a start far above
/// the data: some undamped step keeps every tau positive but raises the profile deviance, and
/// the guard halves it. The profile deviance never rises from one step to the next.
#[test]
fn mode_costs_halve_a_step_that_raises_the_deviance() {
    let s = weighted_sample(
        &THETA,
        0.0,
        [0.10, 0.04, 0.008, 0.004],
        false,
        [0.01; 7],
        usize::MAX,
        true,
    );
    let p = Transport {
        per_tkm: [0.10, 0.5, 0.5, 5.0],
        ..Transport::default()
    };
    let est = gravity::mode_costs(&s, &SYNTHETIC_CLASSES, &p).unwrap();
    assert_eq!(est.deviance.len(), est.iterations, "{est:?}");
    for w in est.deviance.windows(2) {
        assert!(w[1] <= w[0], "{:?}", est.deviance);
    }
    let mut r: [f64; 3] = std::array::from_fn(|m| p.per_tkm[m + 1] / p.per_tkm[0]);
    let mut rises = 0;
    for (k, step) in est.steps.iter().enumerate() {
        let full: [f64; 3] = std::array::from_fn(|m| r[m] + step[m]);
        if gravity::mode_tau(&s, &full, &p).iter().all(|&t| t > 0.0) {
            let d = gravity::profile_deviance(&s, &SYNTHETIC_CLASSES, &p, &full).unwrap();
            if d > est.deviance[k] {
                assert!(est.halvings[k] > 0, "step {k}: {est:?}");
                rises += 1;
            }
        }
        let alpha = 0.5_f64.powi(est.halvings[k] as i32);
        r = std::array::from_fn(|m| r[m] + alpha * step[m]);
    }
    assert!(rises > 0, "{est:?}");
}

/// Started a relative 3e-9 off its own converged ratios (the first step is above `MODE_TOL`),
/// inside the band where the deviance moves by less than the PPML's resolution, the fit converges
/// back. With a strict deviance comparison it fails on that noise: the halved trials never lower
/// the deviance, the last one equals the ratios bit for bit, and the same step repeats until
/// `MODE_MAX_ITER`.
#[test]
fn mode_costs_converge_from_inside_the_deviance_band() {
    let s = synthetic_sample(&THETA, 0.0, [0.10, 0.03, 0.01, 0.5], false);
    let p = Transport::default();
    let first = gravity::mode_costs(&s, &SYNTHETIC_CLASSES, &p).unwrap();
    let mut near = p.clone();
    for m in 0..3 {
        near.per_tkm[m + 1] = p.per_tkm[0] * first.ratio[m] * (1.0 + 3e-9);
    }
    let again = gravity::mode_costs(&s, &SYNTHETIC_CLASSES, &near).unwrap();
    for (m, mode) in MODE.iter().enumerate() {
        approx(again.ratio[m], first.ratio[m], 1e-6, mode);
    }
}

/// At the cap the fit returns an error, not a panic: with no halving allowed, the heavy
/// sample's first step (which drives a tau below 0) cannot be taken.
#[test]
fn mode_costs_return_an_error_at_the_cap() {
    let s = weighted_sample(
        &THETA,
        0.0,
        [0.10, 0.04, 0.008, 0.004],
        false,
        [0.01; 7],
        usize::MAX,
        false,
    );
    let err =
        gravity::mode_costs_halving(&s, &SYNTHETIC_CLASSES, &Transport::default(), 0).unwrap_err();
    assert!(err.starts_with("step 1: within 0 halvings"), "{err}");
}

/// Air between 5 countries only: at most 10 country pairs (2000 BACI has 3 with a positive flow),
/// fewer than `MODE_MIN_PAIRS`. Air keeps its start and is reported as not identified; rail and sea are
/// still recovered.
#[test]
fn mode_below_the_support_threshold_is_unidentified() {
    let cost = [0.10, 0.03, 0.01, 0.5];
    let weight = [0.0005, 0.0008, 0.002, 0.0016, 0.0003, 0.0008, 0.0001];
    let s = weighted_sample(&THETA, 0.0, cost, false, weight, 5, false);
    let support = gravity::mode_support(&s);
    assert!(0 < support[2] && support[2] <= 10, "{support:?}");
    assert!(support[0] >= gravity::MODE_MIN_PAIRS && support[1] >= gravity::MODE_MIN_PAIRS);
    let est = gravity::mode_costs(&s, &SYNTHETIC_CLASSES, &Transport::default()).unwrap();
    assert_eq!(est.identified, [true, true, false]);
    assert_eq!(est.support, support);
    assert!(est.ratio[2].is_nan() && est.se[2].is_nan(), "{est:?}");
    for m in 0..2 {
        approx(est.ratio[m], cost[m + 1] / cost[0], 0.1, MODE[m]);
    }
    assert!(gravity::report(&estimates(Ok(est))).contains("air not identified"));
}

/// Estimates on the synthetic samples with the given P04 result.
fn estimates(modes: Result<gravity::Modes, String>) -> gravity::Estimates {
    let s = synthetic_sample(&THETA, 0.4, COST, true);
    let international: Vec<bool> = (0..s.len())
        .map(|i| s.exporter[i] != s.importer[i])
        .collect();
    let border = gravity::border_friction(&s, &SYNTHETIC_CLASSES, Some(&[0.1, 0.4, 0.5])).unwrap();
    gravity::Estimates {
        theta: gravity::theta_by_class(&s.rows(&international), &SYNTHETIC_CLASSES).unwrap(),
        border,
        modes,
        class_shares: vec![0.1, 0.4, 0.5],
        n_countries: 30,
        n_domestic_countries: 30,
        notes: gravity::Notes {
            countries: 30,
            countries_with_baci_trade: 30,
            unreached_pairs: 0,
            baci_international_usd: 0.0,
            baci_international_usd_in_sample: 0.0,
            domestic_countries: 30,
            domestic_cells_dropped: 0,
            class_trade_usd: vec![0.0; 3],
        },
    }
}

/// A failed P04 fit is a diagnostic: the other estimates stand, the report says why P04 has no
/// values, a stop rule fires and `gravity_2000.toml` is left as it was. Ratios that keep the
/// ordering are written.
#[test]
fn p04_failure_leaves_the_toml_unchanged() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("gravity_2000.toml");
    std::fs::write(&out, "kept\n").unwrap();
    let failed = estimates(Err("forced".to_owned()));
    assert!(gravity::report(&failed).contains("P04 did not converge: forced"));
    let fired = gravity::stop_rules(&failed);
    assert_eq!(fired.len(), 1, "{fired:?}");
    assert!(fired[0].starts_with("P04"), "{fired:?}");
    assert!(!gravity::to_toml(&failed).contains("P04b/P04a"));
    assert!(!gravity::write_toml(&failed, &out).unwrap());
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "kept\n");

    let s = synthetic_sample(&THETA, 0.0, [0.10, 0.03, 0.01, 0.5], false);
    let fitted = gravity::mode_costs(&s, &SYNTHETIC_CLASSES, &Transport::default());
    let ok = estimates(fitted);
    assert!(
        gravity::stop_rules(&ok).is_empty(),
        "{:?}",
        gravity::stop_rules(&ok)
    );
    assert!(gravity::write_toml(&ok, &out).unwrap());
    assert!(std::fs::read_to_string(&out).unwrap().contains("P04d/P04a"));
}

/// A cost ratio at or below zero is not a cost, whatever the ordering: sea < 0 < rail < 1 < air
/// fires the P04 stop rule and nothing is written.
#[test]
fn nonpositive_mode_ratio_fires_the_stop_rule() {
    let s = synthetic_sample(&THETA, 0.0, [0.10, 0.03, 0.01, 0.5], false);
    let fitted = gravity::mode_costs(&s, &SYNTHETIC_CLASSES, &Transport::default()).unwrap();
    for sea in [-0.05, 0.0] {
        let mut m = fitted.clone();
        m.ratio[1] = sea;
        let est = estimates(Ok(m));
        let fired = gravity::stop_rules(&est);
        assert_eq!(fired.len(), 1, "sea {sea}: {fired:?}");
        assert!(fired[0].starts_with("P04"), "{fired:?}");
    }
}

#[test]
fn edge_prices_follow_the_geo_spec() {
    let p = Transport::default();
    let kinds = [ROAD, RAIL, SEA, AIR, 5, 8];
    let length = [100.0, 100.0, 100.0, 100.0, 10.0, 7.0];
    let tri = [200.0, 1000.0, 500.0, 500.0, 300.0, 0.0];
    let q = gravity::edge_quantities(&kinds, &length, &tri, &p);
    let cost: Vec<f64> = q.iter().map(|q| gravity::edge_cost(q, &p)).collect();
    // road: 0.10 * 130 km * min(1 + 0.5 * 2, 3); rail: 0.04 * 130 * 3 (capped); sea 0.008 * 100;
    // air 0.8 * 105
    for (got, want) in cost[..4].iter().zip([26.0, 15.6, 0.8, 84.0]) {
        approx(*got, want, 1e-6, "cost");
    }
    // Port access: road line, no terrain, plus the port fee; canal.
    approx(cost[4], 0.10 * 13.0 + 8.0, 1e-6, "port access");
    approx(cost[5], 0.008 * 7.0 + 5.0, 1e-6, "canal");
    let hours = [
        130.0 / 40.0,
        130.0 / 25.0,
        100.0 / 25.0,
        105.0 / 500.0,
        13.0 / 40.0 + 48.0,
        7.0 / 25.0,
    ];
    for (q, want) in q.iter().zip(hours) {
        approx(q[Q_HOURS], want, 1e-6, "hours");
    }
}

fn table(path: &Path) -> Vec<RecordBatch> {
    let file = File::open(path).unwrap();
    FileReader::try_new(file, None)
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn column(t: &[RecordBatch], name: &str) -> Vec<f64> {
    t.iter()
        .flat_map(|b| {
            let a = cast(b.column_by_name(name).unwrap(), &DataType::Float64).unwrap();
            a.as_primitive::<Float64Type>().values().to_vec()
        })
        .collect()
}

fn within(got: f64, want: f64) -> bool {
    (got - want).abs() <= (1e-12 * want.abs()).max(1e-12)
}

/// The edge prices and least-cost trees give the kernel's crossing legs (cost and hours between
/// every pair of gateways of different countries) of `ceteris export`.
#[test]
fn s05_costs_match_the_rust_crossing_legs() {
    let pack = pack();
    if !pack.join("gateways.arrow").exists() {
        eprintln!(
            "skipping: no pack at {}; build it with `ceteris pack build`",
            pack.display()
        );
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    ceteris_calibrate::export_pack(&pack, tmp.path()).unwrap();
    let (legs, nodes, edges) = (
        table(&tmp.path().join("legs.arrow")),
        table(&pack.join("nodes.arrow")),
        table(&pack.join("edges.arrow")),
    );
    let gnode: Vec<usize> = column(&table(&pack.join("gateways.arrow")), "node")
        .iter()
        .map(|&x| x as usize)
        .collect();
    let p = Transport::default();
    let mode: Vec<u8> = column(&edges, "mode").iter().map(|&m| m as u8).collect();
    let q = gravity::edge_quantities(
        &mode,
        &column(&edges, "length_km"),
        &column(&edges, "tri_m"),
        &p,
    );
    let ends =
        |name: &str| -> Vec<usize> { column(&edges, name).iter().map(|&x| x as usize).collect() };
    let rows = nodes.iter().map(RecordBatch::num_rows).sum();
    let graph = Graph::new(rows, &ends("a"), &ends("b"), &q, &p);
    let trees: Vec<(Vec<f64>, Vec<Quantities>)> = gnode.iter().map(|&g| graph.tree(g)).collect();
    let (kind, from, to) = (
        column(&legs, "kind"),
        column(&legs, "from"),
        column(&legs, "to"),
    );
    let (cost, hours) = (column(&legs, "cost_per_t"), column(&legs, "hours"));
    let mut n = 0;
    for i in (0..kind.len()).filter(|&i| kind[i] == 2.0) {
        n += 1;
        let (a, b) = (from[i] as usize, to[i] as usize);
        let (dist, acc) = &trees[a];
        let at = gnode[b];
        assert!(
            within(dist[at], cost[i]),
            "leg {a} {b}: {} vs {}",
            dist[at],
            cost[i]
        );
        let c = gravity::edge_cost(&acc[at], &p);
        assert!(within(c, cost[i]), "leg {a} {b}: {c} vs {}", cost[i]);
        assert!(within(acc[at][Q_HOURS], hours[i]), "leg {a} {b} hours");
    }
    assert!(n > 300_000, "{n} crossing legs");
}

/// ADR 0247: with the bulk freight rate on (class 3 by default), the country costs of a bulk
/// class are those of the kernel's bulk legs (`EconInputs::network_from_graph`: the same graph
/// repriced at `TransportParams::bulk`, shortest paths by `ceteris_geo::paths::shortest`), and
/// the other classes keep the base legs. Two countries: A with provinces 0 and 1 and port 3, B
/// with province 2 and port 4; between the ports a sea lane of 4,000 km and a rail line of
/// 600 km, the rail line cheaper at the base rate (31.2 against 32 per tonne) and the sea lane
/// at the bulk rate, so the route switches.
#[test]
fn bulk_class_country_costs_match_the_kernel() {
    use ceteris_geo::graph::{EdgeList, Mode, NodeKind, NodeList, TransportGraph, TransportParams};
    use ceteris_geo::paths::shortest;
    use ceteris_trade::TradeParams;

    let edges = [
        (0, 1, Mode::Road, 100.0),
        (0, 3, Mode::PortAccess, 10.0),
        (1, 3, Mode::Road, 50.0),
        (2, 4, Mode::PortAccess, 10.0),
        (3, 5, Mode::Sea, 2000.0),
        (5, 4, Mode::Sea, 2000.0),
        (3, 6, Mode::Rail, 300.0),
        (6, 4, Mode::Rail, 300.0),
    ];
    let inputs = gravity::CostInputs {
        nodes: 7,
        edge_a: edges.iter().map(|e| e.0).collect(),
        edge_b: edges.iter().map(|e| e.1).collect(),
        mode: edges.iter().map(|e| e.2 as u8).collect(),
        length_km: edges.iter().map(|e| e.3).collect(),
        tri_m: vec![0.0; edges.len()],
        gateway_node: vec![3, 4],
        gateway_country: vec![0, 1],
        province_country: vec![0, 0, 1],
        n_countries: 2,
        va: vec![[1.0; 10]; 3],
        weight: [0.5; 10],
    };
    let trade = TradeParams::default();
    let (classes, sea, port) = trade.bulk_rate().expect("the rate is on by default");
    let p = Transport::default();
    let costs = inputs.country_costs(&p, &trade);

    let kind = [0, 0, 0, 2, 2, 4, 1].map(|k| NodeKind::from_u8(k).unwrap());
    let base = TransportGraph::new(
        3,
        NodeList {
            kind: kind.to_vec(),
            province: vec![0, 1, 2, 0, 2, 0, 0],
            lon: vec![0.0; 7],
            lat: vec![0.0; 7],
        },
        EdgeList {
            a: edges.iter().map(|e| e.0 as u32).collect(),
            b: edges.iter().map(|e| e.1 as u32).collect(),
            mode: edges.iter().map(|e| e.2).collect(),
            length_km: inputs.length_km.clone(),
            tri_m: inputs.tri_m.clone(),
            chokepoint: vec![0; edges.len()],
        },
        &TransportParams::default(),
    );
    let mut bulk = base.clone();
    bulk.reprice(&TransportParams::default().bulk(sea, port));
    // the kernel's chain from A to B: access from the province to port 3, the crossing to port 4,
    // access from province 2; with VA 1 everywhere the country cost is the mean over A's provinces
    let chain = |g: &TransportGraph| {
        let (p3, p2) = (shortest(g, 3), shortest(g, 2));
        let legs = |o: u32| {
            let t = shortest(g, o);
            (
                t.cost[3] + p3.cost[4] + p2.cost[4],
                t.hours[3] + p3.hours[4] + p2.hours[4],
            )
        };
        let (a, b) = (legs(0), legs(1));
        ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0)
    };
    let close = |x: f64, y: f64| (x - y).abs() <= 1e-12 * y;
    let (on_base, on_bulk) = (chain(&base), chain(&bulk));
    assert!(
        on_bulk.0 < 0.9 * on_base.0 && on_bulk.1 > on_base.1,
        "{on_bulk:?} {on_base:?}"
    );
    for g in 0..7 {
        let want = if classes[usize::from(trade.theta_class[g])] {
            on_bulk
        } else {
            on_base
        };
        let q = costs.at(0, 1, g);
        let got = (gravity::edge_cost(q, &p), q[Q_HOURS]);
        assert!(
            close(got.0, want.0) && close(got.1, want.1),
            "good {g}: {got:?} {want:?}"
        );
    }
}

/// ADR 0279: under `PathChoice::Iceberg` the country costs of each good are its least-iceberg
/// chains as the kernel's good legs take them (`ceteris_geo::paths::shortest_by` at the good's
/// kappa, on the bulk graph for the bulk class): A with provinces 0 and 1, port 3 and airport 6,
/// B with province 2, port 4 and airport 7; 4,000 km by sea or by air. Good 6 weighs 2e-5 t per
/// USD and flies; the others weigh 0.5 and sail.
#[test]
fn iceberg_country_costs_follow_the_kernel() {
    use ceteris_geo::graph::{
        EdgeList, Mode, NodeKind, NodeList, PathChoice, TransportGraph, TransportParams,
    };
    use ceteris_geo::paths::shortest_by;
    use ceteris_trade::{TradeParams, kappa};

    let edges = [
        (0, 1, Mode::Road, 100.0),
        (0, 3, Mode::PortAccess, 10.0),
        (2, 4, Mode::PortAccess, 10.0),
        (3, 5, Mode::Sea, 2000.0),
        (5, 4, Mode::Sea, 2000.0),
        (0, 6, Mode::AirAccess, 10.0),
        (2, 7, Mode::AirAccess, 10.0),
        (6, 7, Mode::Air, 4000.0),
    ];
    let mut weight = [0.5; 10];
    weight[6] = 2e-5;
    let inputs = gravity::CostInputs {
        nodes: 8,
        edge_a: edges.iter().map(|e| e.0).collect(),
        edge_b: edges.iter().map(|e| e.1).collect(),
        mode: edges.iter().map(|e| e.2 as u8).collect(),
        length_km: edges.iter().map(|e| e.3).collect(),
        tri_m: vec![0.0; edges.len()],
        gateway_node: vec![3, 4, 6, 7],
        gateway_country: vec![0, 1, 0, 1],
        province_country: vec![0, 0, 1],
        n_countries: 2,
        va: vec![[1.0; 10]; 3],
        weight,
    };
    let trade = TradeParams::default();
    let (classes, sea, port) = trade.bulk_rate().expect("the rate is on by default");
    let p = Transport {
        path_choice: PathChoice::Iceberg,
        ..Transport::default()
    };
    let costs = inputs.country_costs(&p, &trade);
    let freight = inputs.country_costs(&Transport::default(), &trade);

    let kind = [0, 0, 0, 2, 2, 4, 3, 3].map(|k| NodeKind::from_u8(k).unwrap());
    let base = TransportGraph::new(
        3,
        NodeList {
            kind: kind.to_vec(),
            province: vec![0, 1, 2, 0, 2, 0, 0, 2],
            lon: vec![0.0; 8],
            lat: vec![0.0; 8],
        },
        EdgeList {
            a: edges.iter().map(|e| e.0 as u32).collect(),
            b: edges.iter().map(|e| e.1 as u32).collect(),
            mode: edges.iter().map(|e| e.2).collect(),
            length_km: inputs.length_km.clone(),
            tri_m: inputs.tri_m.clone(),
            chokepoint: vec![0; edges.len()],
        },
        &TransportParams::default(),
    );
    let mut bulk = base.clone();
    bulk.reprice(&TransportParams::default().bulk(sea, port));
    // the least chain over gateway pairs (a of A, b of B) by c + kappa h, as the mean over A's
    // provinces with VA 1 everywhere
    let chain = |g: &TransportGraph, k: f64| {
        let from_b = shortest_by(g, 2, k);
        let legs = |o: u32| {
            let t = shortest_by(g, o, k);
            let mut best = (f64::INFINITY, 0.0, 0.0);
            for a in [3, 6] {
                let x = shortest_by(g, a, k);
                for b in [4, 7] {
                    let c = t.cost[a as usize] + x.cost[b] + from_b.cost[b];
                    let h = t.hours[a as usize] + x.hours[b] + from_b.hours[b];
                    if c + k * h < best.0 {
                        best = (c + k * h, c, h);
                    }
                }
            }
            (best.1, best.2)
        };
        let (a, b) = (legs(0), legs(1));
        ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0)
    };
    let close = |x: f64, y: f64| (x - y).abs() <= 1e-12 * y;
    let hours = |c: &gravity::CountryCosts, g: usize| c.at(0, 1, g)[Q_HOURS];
    for g in 0..7 {
        let on_bulk = classes[usize::from(trade.theta_class[g])];
        let k = kappa(weight[g], trade.time_cost[g]);
        let want = chain(if on_bulk { &bulk } else { &base }, k);
        let q = costs.at(0, 1, g);
        let got = (gravity::edge_cost(q, &p), q[Q_HOURS]);
        assert!(
            close(got.0, want.0) && close(got.1, want.1),
            "good {g}: {got:?} {want:?}"
        );
        if g != 6 {
            assert!(
                close(hours(&costs, g), hours(&freight, g)),
                "good {g} sails"
            );
        }
    }
    assert!(hours(&costs, 6) < 0.2 * hours(&freight, 6), "good 6 flies");
    assert_eq!(costs.reach, freight.reach);
}

/// The literals of ADR 0023 as exported from `TradeParams::default()` (pinned to the export by
/// p2_acceptance), against `CLASSES` and `THETA_DEFAULT`.
#[test]
fn fixture_defaults_match_the_calibration_constants() {
    let path = fixture().join("params.json");
    let params: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let trade = &params["trade"];
    let of: Vec<u64> = trade["theta_class"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    assert_eq!(of, [2, 3, 3, 0, 2, 0, 1, 0, 0, 0]);
    let theta: Vec<f64> = trade["theta"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    assert_eq!(
        theta,
        [7.68, 16.91, 16.91, 2.91, 7.68, 2.91, 4.14, 0.0, 0.0, 0.0]
    );
    let n_classes = *of[..7].iter().max().unwrap() as usize + 1;
    let classes: Vec<Vec<usize>> = (0..n_classes)
        .map(|c| (0..7).filter(|&g| of[g] as usize == c).collect())
        .collect();
    let want: Vec<Vec<usize>> = CLASSES.iter().map(|c| c.to_vec()).collect();
    assert_eq!(classes, want);
    let defaults: Vec<f64> = CLASSES.iter().map(|m| theta[m[0]]).collect();
    assert_eq!(defaults, THETA_DEFAULT);
}
