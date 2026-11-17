//! The dark distance cost fit (ADR 0272) on the P2 fixture.
#![allow(clippy::disallowed_methods)] // offline fitting test, as the crate

use std::path::PathBuf;

use ceteris_calibrate::border;
use ceteris_calibrate::epsilon::{self, Gravity, Targets};
use ceteris_calibrate::steady::{self, Limits};
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
    let (mut mean, min) = epsilon::class_offsets(&w.inp, &w.class, &w.ln_d, &trade_weight(&w.inp));
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
