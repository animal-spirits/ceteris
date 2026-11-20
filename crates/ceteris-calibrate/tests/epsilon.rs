//! The dark distance cost fit (ADR 0272) on the P2 fixture.
#![allow(clippy::disallowed_methods)] // offline fitting test, as the crate

use std::path::PathBuf;

use ceteris_calibrate::border;
use ceteris_calibrate::epsilon::{self, Gravity, Targets};
use ceteris_calibrate::steady::{self, CalibrationError, Limits};
use ceteris_reference::io::{self, Inputs};

fn fixture() -> PathBuf {
    std::env::var_os("CETERIS_FIXTURE_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/p2-fixture"),
        PathBuf::from,
    )
}

fn quiet() -> impl FnMut(&str) {
    |_| {}
}

/// Pair distances for the fixture's countries: spread on a line, so ln distance varies.
fn ln_distance(n: usize) -> Vec<f64> {
    (0..n * n)
        .map(|k| {
            let (a, b) = (k / n, k % n);
            if a == b {
                f64::NAN
            } else {
                (500.0 * (a.abs_diff(b) as f64 + 0.3 * ((a + b) % 3) as f64)).ln()
            }
        })
        .collect()
}

struct World {
    inp: Inputs,
    class: Vec<usize>,
    groups: Vec<Vec<usize>>,
    icio: Vec<bool>,
    ln_d: Vec<f64>,
    gravity: Gravity,
}

fn world() -> World {
    let inp = io::load(fixture()).unwrap();
    let n_traded = inp.traded.iter().filter(|&&t| t).count();
    let class = inp.theta_class[..n_traded].to_vec();
    let n = inp.n_countries();
    let ln_d = ln_distance(n);
    let distw: Vec<f64> = ln_d.iter().map(|l| l.exp()).collect();
    let gravity = Gravity::new(n, &distw, &vec![0.0; n * n]);
    World {
        groups: border::friction_groups(&class),
        icio: vec![true; n],
        class,
        ln_d,
        gravity,
        inp,
    }
}

/// Targets as the model makes them at a known epsilon and nu: the nu targets and the data flows.
fn known(w: &World, epsilon: f64, nu_group: &[f64]) -> (Vec<f64>, Vec<f64>) {
    known_scaled(w, epsilon, &[1.0; border::N_CLASSES], nu_group)
}

fn known_scaled(w: &World, epsilon: f64, scale: &[f64], nu_group: &[f64]) -> (Vec<f64>, Vec<f64>) {
    known_weighted(w, epsilon, scale, nu_group, &trade_weight(&w.inp))
}

fn known_weighted(
    w: &World,
    epsilon: f64,
    scale: &[f64],
    nu_group: &[f64],
    weight: &[f64],
) -> (Vec<f64>, Vec<f64>) {
    let (mut mean, min) = epsilon::class_offsets(&w.inp, &w.class, &w.ln_d, weight);
    // a scaled class is a floored one: d_0 at its smallest linked distance
    // the per-good class at nu 0 is floored likewise
    for c in (0..scale.len()).filter(|&c| scale[c] < 1.0 || c == border::PER_GOOD_CLASS) {
        mean[c] = min[c];
    }
    let at = epsilon::at_epsilon(&w.inp, &w.class, &w.ln_d, &mean, scale, epsilon).unwrap();
    let nu = border::nu_of_goods(&w.groups, nu_group, &at.border_nu);
    let target =
        border::data_state_shares(&at, &nu, &w.groups, &w.icio, Limits::default()).unwrap();
    let (spend, supply) = border::spending_at(&at, &nu, Limits::default()).unwrap();
    let pairs = border::data_state_pair_values(
        &epsilon::at_epsilon(&w.inp, &w.class, &w.ln_d, &mean, scale, epsilon).unwrap(),
        &nu,
        &spend,
        &supply,
        Limits::default(),
    )
    .unwrap();
    let (n, g, t) = (w.inp.n_countries(), w.inp.n_goods(), w.class.len());
    let flows = (0..n * n * t).map(|i| pairs[(i / t) * g + i % t]).collect();
    (target, flows)
}

fn trade_weight(inp: &Inputs) -> Vec<f64> {
    vec![1.0; inp.n_countries().pow(2)]
}

/// epsilon 0 leaves the inputs as they are.
#[test]
fn epsilon_zero_is_the_inputs() {
    let w = world();
    let at = epsilon::at_epsilon(&w.inp, &w.class, &w.ln_d, &[0.0; 4], &[1.0; 4], 0.0).unwrap();
    assert_eq!(at.crossing.k, w.inp.crossing.k);
}

/// Targets made at a known epsilon and nu come back from the fit: epsilon to its tolerance, the
/// shares to the nu fit's, the pooled coefficient to the data's.
#[test]
fn fit_recovers_a_known_epsilon() {
    let w = world();
    let nu_group = [0.15, 0.6, 0.35, 0.0, 0.0];
    let e0 = 0.4;
    let (nu_target, data_flows) = known(&w, e0, &nu_group);
    let t = Targets {
        nu_target: &nu_target,
        icio: &w.icio,
        theta_class: &w.class,
        ln_d: &w.ln_d,
        weight: &trade_weight(&w.inp),
        gravity: &w.gravity,
        data_flows: &data_flows,
    };
    let fit = epsilon::fit_epsilon(&w.inp, &t, Limits::default(), &mut quiet()).unwrap();
    println!("{fit:?}");
    let json = fit.json().compact();
    assert!(
        json.contains("\"epsilon\"") && json.contains("\"held_out\""),
        "{json}"
    );
    assert!(!fit.at_zero);
    let file = tempfile::tempdir().unwrap().keep().join("epsilon.json");
    std::fs::write(&file, fit.json().pretty()).unwrap();
    let (e, d0, scale, nu) = epsilon::read_fit(&file).unwrap();
    assert_eq!((e, nu), (fit.trial.epsilon, fit.trial.fit.nu.clone()));
    assert_eq!(d0.len(), fit.trial.ln_d0.len());
    assert_eq!(scale, fit.trial.scale);
    assert!((fit.trial.epsilon - e0).abs() <= 2.0 * epsilon::EPSILON_TOL);
    assert!(fit.held_out.iter().all(|c| c.within), "{:?}", fit.held_out);
}

/// A model whose coefficient is already at the data's keeps epsilon 0.
#[test]
fn data_at_epsilon_zero_keeps_zero() {
    let w = world();
    let nu_group = [0.15, 0.6, 0.35, 0.0, 0.0];
    let (nu_target, mut data_flows) = known(&w, 0.0, &nu_group);
    // The data's coefficient a hair weaker than the model's own: the refitted nu is the known one
    // only to the fit's tolerance, so equal coefficients would leave the sign to rounding.
    let (n, n_t) = (w.inp.n_countries(), w.class.len());
    for (i, v) in data_flows.iter_mut().enumerate() {
        let l = w.ln_d[i / n_t];
        if l.is_finite() {
            *v *= (1e-3 * l).exp();
        }
    }
    debug_assert_eq!(data_flows.len(), n * n * n_t);
    let t = Targets {
        nu_target: &nu_target,
        icio: &w.icio,
        theta_class: &w.class,
        ln_d: &w.ln_d,
        weight: &trade_weight(&w.inp),
        gravity: &w.gravity,
        data_flows: &data_flows,
    };
    let fit = epsilon::fit_epsilon(&w.inp, &t, Limits::default(), &mut quiet()).unwrap();
    assert!(fit.at_zero && fit.trial.epsilon == 0.0);
}

/// The economy's kernel with a `PairDistance` and the reference inputs at the same epsilon give
/// the same crossings, the tables round-trip, and the economy reads them back from them: the
/// calibration and the run apply one distance cost.
#[test]
fn run_and_calibration_apply_the_same_distance_cost() {
    use ceteris_calibrate::write::{self, Distance};
    use ceteris_econ::EconParams;
    use ceteris_econ::distance::read_pair_distance;
    use ceteris_econ::export::write_inputs;
    use ceteris_econ::fixture::small_world;
    use ceteris_trade::{Kernel, PairDistance, TradeParams};

    let mut econ = small_world();
    let n = econ.n_countries();
    let ln_d = ln_distance(n);
    let d = Distance {
        epsilon: 0.4,
        ln_d0: vec![6.0, 6.2, 6.4, f64::NAN],
        scale: vec![1.0, 0.7, 1.0, 1.0],
        ln_d: ln_d.clone(),
    };
    let dir = tempfile::tempdir().unwrap();
    write::write_distance(dir.path(), &d).unwrap();
    let read = write::read_distance(dir.path(), n).unwrap().unwrap();
    assert_eq!(read.epsilon, 0.4);
    assert_eq!(&read.ln_d0[..3], &d.ln_d0[..3]);
    assert_eq!(read.scale, d.scale);
    assert!(
        read.ln_d
            .iter()
            .zip(&ln_d)
            .all(|(a, b)| a == b || (a.is_nan() && b.is_nan()))
    );

    let export = |econ: &ceteris_econ::EconInputs| {
        let trade = TradeParams::default();
        let kernel = Kernel::build(&econ.network, &econ.weight, &econ.wedges, &trade);
        let tmp = tempfile::tempdir().unwrap();
        write_inputs(tmp.path(), econ, &kernel, &EconParams::default(), &trade).unwrap();
        io::load(tmp.path()).unwrap()
    };
    let plain = export(&econ);
    let pair: PairDistance = read_pair_distance(dir.path(), n).unwrap().unwrap();
    econ.network.pair_distance = Some(pair);
    let run = export(&econ);
    let cal = epsilon::at_distance(&plain, &read).unwrap();
    assert_ne!(run.crossing.k, plain.crossing.k);
    for (j, (a, b)) in run.crossing.k.iter().zip(&cal.crossing.k).enumerate() {
        assert!(
            (a - b).abs() <= 1e-12 * a.abs().max(b.abs()),
            "crossing {j}: run {a}, calibration {b}"
        );
    }
    for (a, b) in run.crossing.inv_loss.iter().zip(&cal.crossing.inv_loss) {
        assert!((a - b).abs() <= 1e-12 * a.abs().max(b.abs()));
    }
}

/// A cached fit is bit-identical to an uncached one, a second run replays every trial from the
/// cache, and a changed target misses it.
#[test]
fn cached_fit_is_bit_identical() {
    let w = world();
    let (nu_target, data_flows) = known(&w, 0.4, &[0.15, 0.6, 0.35, 0.0, 0.0]);
    let weight = trade_weight(&w.inp);
    let targets = |nu: &[f64]| -> Vec<f64> { nu.to_vec() };
    let run = |nu: &[f64], cache: Option<&std::path::Path>| {
        let nu = targets(nu);
        let t = Targets {
            nu_target: &nu,
            icio: &w.icio,
            theta_class: &w.class,
            ln_d: &w.ln_d,
            weight: &weight,
            gravity: &w.gravity,
            data_flows: &data_flows,
        };
        let mut lines = vec![];
        let fit = epsilon::fit_epsilon_cached(&w.inp, &t, Limits::default(), cache, &mut |l| {
            lines.push(l.to_owned())
        })
        .unwrap();
        (
            format!("{:?}", fit.json().compact()),
            fit.trial.epsilon,
            lines,
        )
    };
    let dir = tempfile::tempdir().unwrap();
    let (plain, e_plain, _) = run(&nu_target, None);
    let (first, e_first, _) = run(&nu_target, Some(dir.path()));
    let (second, e_second, lines) = run(&nu_target, Some(dir.path()));
    assert_eq!(plain, first);
    assert_eq!(plain, second);
    assert_eq!(e_plain.to_bits(), e_first.to_bits());
    assert_eq!(e_plain.to_bits(), e_second.to_bits());
    assert!(
        lines.iter().all(|l| l.ends_with("from the cache")),
        "{lines:?}"
    );
    // a different target is a different key
    let mut other = nu_target.clone();
    other[0] *= 0.9;
    let (_, _, lines) = run(&other, Some(dir.path()));
    assert!(lines.iter().any(|l| !l.ends_with("from the cache")));
}

/// A class whose target share lies above what its distance cost allows (nu clamps at 0) takes
/// s_c < 1 and meets the target with nu = 0; the other classes keep s_c = 1 (ADR 0375, 0376).
#[test]
fn clamped_class_takes_an_epsilon_scale() {
    let w = world();
    let (e0, c, s0) = (0.4, 1, 0.5);
    let mut scale = [1.0; border::N_CLASSES];
    scale[c] = s0;
    let nu_group = [0.15, 0.0, 0.35, 0.0, 0.0];
    let (nu_target, data_flows) = known_scaled(&w, e0, &scale, &nu_group);
    let t = Targets {
        nu_target: &nu_target,
        icio: &w.icio,
        theta_class: &w.class,
        ln_d: &w.ln_d,
        weight: &trade_weight(&w.inp),
        gravity: &w.gravity,
        data_flows: &data_flows,
    };
    let tr = epsilon::fit_at_epsilon(&w.inp, &t, e0, Limits::default(), &mut quiet()).unwrap();
    assert!(tr.scale[c] < 1.0, "{:?}", tr.scale);
    assert!(tr.fit.nu[c] < 1e-4, "{:?}", tr.fit.nu);
    assert!(
        (tr.fit.share[c] - nu_target[c]).abs() <= 1e-4,
        "share {} target {}",
        tr.fit.share[c],
        nu_target[c]
    );
    assert!((tr.scale[c] - s0).abs() <= 1e-3, "{:?}", tr.scale);
    for k in (0..border::N_CLASSES).filter(|&k| k != c) {
        assert_eq!(tr.scale[k], 1.0);
    }
}

/// The per-good class (3) takes one s_c on its pooled share (ADR 0376): both of its goods, held at
/// nu 0, recover the scale the targets were made at.
#[test]
fn per_good_class_takes_one_epsilon_scale() {
    let w = world();
    let (e0, c, s0) = (0.4, border::PER_GOOD_CLASS, 0.5);
    let mut scale = [1.0; border::N_CLASSES];
    scale[c] = s0;
    let nu_group = [0.15, 0.6, 0.35, 0.0, 0.0];
    let (nu_target, data_flows) = known_scaled(&w, e0, &scale, &nu_group);
    let t = Targets {
        nu_target: &nu_target,
        icio: &w.icio,
        theta_class: &w.class,
        ln_d: &w.ln_d,
        weight: &trade_weight(&w.inp),
        gravity: &w.gravity,
        data_flows: &data_flows,
    };
    let tr = epsilon::fit_at_epsilon(&w.inp, &t, e0, Limits::default(), &mut quiet()).unwrap();
    assert!((tr.scale[c] - s0).abs() <= 1e-3, "{:?}", tr.scale);
    for (k, target) in nu_target.iter().enumerate().skip(3) {
        assert!(tr.fit.nu[k] < 1e-4, "{:?}", tr.fit.nu);
        assert!((tr.fit.share[k] - target).abs() <= 1e-4);
    }
    for k in (0..border::N_CLASSES).filter(|&k| k != c) {
        assert_eq!(tr.scale[k], 1.0);
    }
}

/// A fit at a given epsilon is one trial at exactly that epsilon.
#[test]
fn fit_at_a_given_epsilon_is_one_trial() {
    let w = world();
    let nu_group = [0.15, 0.6, 0.35, 0.0, 0.0];
    let (nu_target, data_flows) = known(&w, 0.4, &nu_group);
    let t = Targets {
        nu_target: &nu_target,
        icio: &w.icio,
        theta_class: &w.class,
        ln_d: &w.ln_d,
        weight: &trade_weight(&w.inp),
        gravity: &w.gravity,
        data_flows: &data_flows,
    };
    let fit =
        epsilon::fit_epsilon_at(&w.inp, &t, 0.37, Limits::default(), None, &mut quiet()).unwrap();
    assert_eq!(
        (fit.trial.epsilon, fit.evaluations, fit.at_zero),
        (0.37, 1, false)
    );
    assert!(fit.json().compact().contains("\"scale\""));
}

/// Issues #619, #622: with the other classes' distance cost on (their buyer prices move), good 2
/// at nu 0 and epsilon scale 0 buys the share it buys in the calibrated state at those inputs.
/// Stage 1 inverts the PIGL weights at the buyer prices it spends at, so no cost in a buyer price
/// moves the level of the weights; the others' cost still moves duties, rebates and the final
/// demand balance, and the share with them, as the calibration has it.
#[test]
fn a_class_without_distance_cost_has_the_calibrated_share_under_the_others_cost() {
    const GOOD: usize = 2;
    let w = world();
    let lim = Limits::default();
    let k = w.groups.iter().position(|g| g.contains(&GOOD)).unwrap();
    let c = w.class[GOOD];
    let (mut d0, min) = epsilon::class_offsets(&w.inp, &w.class, &w.ln_d, &trade_weight(&w.inp));
    d0[c] = min[c];
    let mut nu = w.inp.border_nu.clone();
    for &g in &w.groups[k] {
        nu[g] = 0.0;
    }
    let share =
        |inp: &Inputs| border::data_state_shares(inp, &nu, &w.groups, &w.icio, lim).unwrap()[k];
    let base = share(&w.inp);
    let mut scale = [1.0; border::N_CLASSES];
    scale[c] = 0.0;
    let at = epsilon::at_epsilon(&w.inp, &w.class, &w.ln_d, &d0, &scale, 2.0).unwrap();
    // the other classes' cost is on: their buyer prices moved
    assert!((0..at.n_provinces()).any(|p| {
        (0..w.class.len())
            .any(|g| (at.base_price[(p, g)] / w.inp.base_price[(p, g)] - 1.0).abs() > 1e-2)
    }));
    // The others' cost moves duties, rebates and the final demand balance, so the share is no
    // longer the one at epsilon 0 (-5.3e-4 here): it is the calibrated state's at these inputs.
    assert!((share(&at) - base).abs() > 1e-4);
    let at_nu = border::at_border_nu(&at, &nu).unwrap();
    let s = steady::calibrate(&at_nu, &at_nu.u_star.clone(), &mut quiet()).unwrap();
    let mut spend = s.spend_hh.clone();
    for (a, b) in spend.data.iter_mut().zip(&s.spend_int.data) {
        *a += b;
    }
    let calibrated =
        border::cross_border_shares(&at_nu, &s.price, &spend, &w.icio, &w.groups).unwrap()[k];
    let moved = share(&at) - calibrated;
    // what is left is the buyer prices held at posted prices 1 (-2.1e-5 here)
    assert!(
        moved.abs() <= 5e-5,
        "share off the calibrated state's by {moved}"
    );
    // household spending by country and good stays at the data's final demand
    let (s0, _) = steady::data_state_spending(&w.inp, lim).unwrap();
    let (s1, _) = steady::data_state_spending(&at, lim).unwrap();
    let total = |m: &ceteris_reference::io::Mat| (0..m.rows).map(|p| m[(p, GOOD)]).sum::<f64>();
    assert!((total(&s1) / total(&s0) - 1.0).abs() <= 1e-4);
}

/// Issue #622: the data state the fits evaluate at is the calibrated state the run starts from,
/// buyer prices aside: with a distance cost and a border friction on, every group's cross-border
/// share and the pooled distance coefficient are the steady state's. Without stage 1's rebates
/// and final demand balance in the data state the shares were off by up to 7e-4 here and the
/// coefficient by 2.5e-2.
#[test]
fn the_fits_data_state_is_the_calibrated_state() {
    let w = world();
    let lim = Limits::default();
    let (d0, _) = epsilon::class_offsets(&w.inp, &w.class, &w.ln_d, &trade_weight(&w.inp));
    let at = epsilon::at_epsilon(&w.inp, &w.class, &w.ln_d, &d0, &[1.0; 4], 0.4).unwrap();
    let nu = border::nu_of_goods(&w.groups, &[0.15, 0.6, 0.35, 1.2, 0.5], &at.border_nu);
    let at_nu = border::at_border_nu(&at, &nu).unwrap();
    let fitted = border::data_state_shares(&at, &nu, &w.groups, &w.icio, lim).unwrap();
    let (spend, supply) = border::spending_at(&at, &nu, lim).unwrap();
    let pairs = border::data_state_pair_values(&at, &nu, &spend, &supply, lim).unwrap();

    let s = steady::calibrate(&at_nu, &at_nu.u_star.clone(), &mut quiet()).unwrap();
    let mut steady_spend = s.spend_hh.clone();
    for (a, b) in steady_spend.data.iter_mut().zip(&s.spend_int.data) {
        *a += b;
    }
    let calibrated =
        border::cross_border_shares(&at_nu, &s.price, &steady_spend, &w.icio, &w.groups).unwrap();
    for k in 0..w.groups.len() {
        assert!(
            (fitted[k] - calibrated[k]).abs() <= 5e-6,
            "group {k}: {} at the fit's state, {} calibrated",
            fitted[k],
            calibrated[k]
        );
    }
    let m = ceteris_reference::model::Model::new(&at_nu).unwrap();
    let steady_pairs = m
        .flows(&s.price, &steady_spend, &m.sourcing(&s.price))
        .pair_value;
    let (n, n_g, n_t) = (at.n_countries(), at.n_goods(), w.class.len());
    let pooled = |pairs: &[f64]| {
        let v: Vec<f64> = (0..n * n)
            .map(|k| pairs[k * n_g..k * n_g + n_t].iter().sum())
            .collect();
        w.gravity.coefficient(&v).unwrap().0
    };
    let (a, b) = (pooled(&pairs), pooled(&steady_pairs));
    // the residual, 1.1e-4 here, is the buyer prices held at posted prices 1
    assert!(
        (a - b).abs() <= 5e-4,
        "pooled {a} at the fit's state, {b} calibrated"
    );
}

/// The bracket's two end trials run concurrently; the fit, its progress lines and its journal
/// are bit-identical to one run on a single thread, where they run in sequence.
#[test]
fn concurrent_end_trials_match_sequential() {
    let w = world();
    let nu_group = [0.15, 0.6, 0.35, 0.0, 0.0];
    let (nu_target, data_flows) = known(&w, 0.4, &nu_group);
    let t = Targets {
        nu_target: &nu_target,
        icio: &w.icio,
        theta_class: &w.class,
        ln_d: &w.ln_d,
        weight: &trade_weight(&w.inp),
        gravity: &w.gravity,
        data_flows: &data_flows,
    };
    let run = |threads: usize| {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        let mut lines = vec![];
        let fit = pool
            .install(|| {
                epsilon::fit_epsilon(&w.inp, &t, Limits::default(), &mut |l| {
                    lines.push(l.to_string());
                })
            })
            .unwrap();
        (fit.json().compact(), fit.evaluations, lines)
    };
    let sequential = run(1);
    assert_eq!(run(4), sequential);
    assert_eq!(run(2), sequential);
}

/// A class short at zero friction is infeasible at every epsilon: the fit returns the typed
/// error naming the epsilon, not "too little cost" scored as a positive sign (RFC 0360 D4).
#[test]
fn a_target_above_the_zero_friction_share_is_infeasible_not_too_little_cost() {
    let w = world();
    let (e0, c, s0) = (0.4, 1, 0.5);
    let mut scale = [1.0; border::N_CLASSES];
    scale[c] = s0;
    let nu_group = [0.15, 0.0, 0.35, 0.0, 0.0];
    let (mut nu_target, data_flows) = known_scaled(&w, e0, &scale, &nu_group);
    nu_target[c] = 0.5 + 0.5 * nu_target[c];
    let t = Targets {
        nu_target: &nu_target,
        icio: &w.icio,
        theta_class: &w.class,
        ln_d: &w.ln_d,
        weight: &trade_weight(&w.inp),
        gravity: &w.gravity,
        data_flows: &data_flows,
    };
    let err = epsilon::fit_epsilon(&w.inp, &t, Limits::default(), &mut quiet()).unwrap_err();
    let CalibrationError::Infeasible(e) = err else {
        panic!("{err}");
    };
    assert!(e.starts_with("epsilon "), "{e}");
}

/// d_0 is the trade-weighted mean of ln d (ADR 0272, Decision 2): targets made with uneven pair
/// weights come back from a trial given those weights, and the weights move d_0 off the
/// unit-weight mean.
#[test]
fn d0_is_the_trade_weighted_mean_of_ln_distance() {
    let w = world();
    let n = w.inp.n_countries();
    let uneven: Vec<f64> = (0..n * n)
        .map(|k| 1.0 + ((k / n) * 7 + (k % n) * 3 % 5) as f64)
        .collect();
    let (e0, nu_group) = (0.4, [0.15, 0.6, 0.35, 0.0, 0.0]);
    let (nu_target, data_flows) =
        known_weighted(&w, e0, &[1.0; border::N_CLASSES], &nu_group, &uneven);
    let trial = |weight: &[f64]| {
        let t = Targets {
            nu_target: &nu_target,
            icio: &w.icio,
            theta_class: &w.class,
            ln_d: &w.ln_d,
            weight,
            gravity: &w.gravity,
            data_flows: &data_flows,
        };
        epsilon::fit_at_epsilon(&w.inp, &t, e0, Limits::default(), &mut quiet()).unwrap()
    };
    let (weighted, unit) = (trial(&uneven), trial(&trade_weight(&w.inp)));
    let (mean, _) = epsilon::class_offsets(&w.inp, &w.class, &w.ln_d, &uneven);
    for c in (0..border::N_CLASSES).filter(|&c| mean[c].is_finite()) {
        if !weighted.floored.contains(&c) {
            assert_eq!(weighted.ln_d0[c], mean[c], "class {c}");
        }
    }
    assert!(
        (0..border::N_CLASSES).any(|c| (weighted.ln_d0[c] - unit.ln_d0[c]).abs() > 1e-6),
        "uneven weights left d_0 at the unit-weight mean: {:?}",
        weighted.ln_d0
    );
    for (k, nu) in nu_group.iter().enumerate() {
        assert!(
            (weighted.fit.nu[k] - nu).abs() <= 1e-4,
            "{:?}",
            weighted.fit.nu
        );
    }
}

/// Spike instrumentation (RFC 0361): one fit at epsilon 0 and one at epsilon 0.4 with a clamped
/// class, with the span and scaling report. Run with `--ignored --nocapture`.
#[test]
#[ignore]
fn spike_profile() {
    let w = world();
    if let Some(n) = std::env::var("CETERIS_NEWTON_AFTER")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        steady::NEWTON_AFTER.store(n, std::sync::atomic::Ordering::Relaxed);
    }
    if std::env::var_os("CETERIS_NEWTON_PLAIN").is_some() {
        steady::NEWTON_FROM_PLAIN.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    if let Some(n) = std::env::var("CETERIS_NEWTON_CAP")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        steady::NEWTON_CAP.store(n, std::sync::atomic::Ordering::Relaxed);
    }
    if let Some(n) = std::env::var("CETERIS_NEWTON_MAX")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        steady::NEWTON_MAX_STEPS.store(n, std::sync::atomic::Ordering::Relaxed);
    }
    if std::env::var_os("CETERIS_NEWTON_L2").is_some() {
        steady::NEWTON_L2.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    if std::env::var_os("CETERIS_NEWTON_ORACLE").is_some() {
        steady::NEWTON_ORACLE.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    for (e0, scale0) in [(0.0, 1.0), (0.4, 0.5)] {
        let mut scale = [1.0; border::N_CLASSES];
        scale[1] = scale0;
        let nu_group = [0.15, 0.0, 0.35, 0.0, 0.0];
        let (nu_target, data_flows) = known_scaled(&w, e0, &scale, &nu_group);
        let t = Targets {
            nu_target: &nu_target,
            icio: &w.icio,
            theta_class: &w.class,
            ln_d: &w.ln_d,
            weight: &trade_weight(&w.inp),
            gravity: &w.gravity,
            data_flows: &data_flows,
        };
        ceteris_calibrate::prof::reset();
        let start = std::time::Instant::now();
        let tr = epsilon::fit_at_epsilon(&w.inp, &t, e0, Limits::default(), &mut quiet()).unwrap();
        println!(
            "epsilon {e0}: wall {:.3} s, nu {:?}, scale {:?}, ln_d0 {:?}",
            start.elapsed().as_secs_f64(),
            tr.fit.nu,
            tr.scale,
            tr.ln_d0
        );
        println!(
            "DIGEST {e0} pooled {:e} classes {:?} share {:?}",
            tr.pooled, tr.classes, tr.fit.share
        );
        println!("{}", ceteris_calibrate::prof::report());
        let mut slow = ceteris_calibrate::prof::scalings();
        slow.sort_by_key(|r| std::cmp::Reverse(r.sweeps));
        for r in slow.iter().take(5) {
            println!("{r:?}");
        }
        let nw: Vec<_> = slow.iter().filter(|r| !r.newton.is_empty()).collect();
        let ok = nw.iter().filter(|r| r.newton == "ok").count();
        println!(
            "newton engaged {} ok {} steps {} products {} evals {} newton s {:.3}",
            nw.len(),
            ok,
            nw.iter().map(|r| r.newton_steps).sum::<usize>(),
            nw.iter().map(|r| r.newton_products).sum::<usize>(),
            nw.iter().map(|r| r.newton_evals).sum::<usize>(),
            nw.iter().map(|r| r.newton_ns).sum::<u128>() as f64 / 1e9
        );
        for r in nw.iter().filter(|r| r.newton != "ok") {
            println!("  newton fell back: {} (sweeps {})", r.newton, r.sweeps);
        }
        let mut orc = ceteris_calibrate::prof::oracle();
        orc.sort_by_key(|r| std::cmp::Reverse(r.sweeps_main));
        for o in orc.iter().take(8) {
            println!("  oracle {o:?}");
        }
        let big: Vec<_> = orc.iter().filter(|o| o.sweeps_main > 500).collect();
        if !big.is_empty() {
            println!(
                "GO>500 sweeps: n {} max steps(sum over goods) {} max products {} max newton/main wall {:.4} (min {:.4}) main {:.2} s newton {:.3} s",
                big.len(),
                big.iter().map(|o| o.steps).max().unwrap(),
                big.iter().map(|o| o.products).max().unwrap(),
                big.iter()
                    .map(|o| o.ns_newton as f64 / o.ns_main as f64)
                    .fold(0.0, f64::max),
                big.iter()
                    .map(|o| o.ns_newton as f64 / o.ns_main as f64)
                    .fold(f64::INFINITY, f64::min),
                big.iter().map(|o| o.ns_main).sum::<u128>() as f64 / 1e9,
                big.iter().map(|o| o.ns_newton).sum::<u128>() as f64 / 1e9,
            );
        }
        if !orc.is_empty() {
            let mx = |f: fn(&ceteris_calibrate::prof::OracleRecord) -> f64| {
                orc.iter().map(f).fold(0.0, f64::max)
            };
            println!(
                "oracle n {} max dlw {:.3e} max dr {:.3e}; main {:.3} s vs newton {:.3} s over those scalings",
                orc.len(),
                mx(|o| o.dlw),
                mx(|o| o.dr),
                orc.iter().map(|o| o.ns_main).sum::<u128>() as f64 / 1e9,
                orc.iter().map(|o| o.ns_newton).sum::<u128>() as f64 / 1e9
            );
        }
    }
}
