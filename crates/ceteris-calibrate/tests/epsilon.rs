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
    for c in (0..scale.len()).filter(|&c| scale[c] < 1.0) {
        mean[c] = min[c];
    }
    // and so is any class the trial would floor at the truth's nu (ADR 0272, Decision 2): the same
    // rule, `product_misses`, so the truth is a state the trial can recover
    loop {
        let at = epsilon::at_epsilon(&w.inp, &w.class, &w.ln_d, &mean, scale, epsilon).unwrap();
        let nu = border::nu_of_goods(&w.groups, nu_group, &at.border_nu);
        let bad = epsilon::product_misses(&w.inp, &w.class, &w.ln_d, &mean, scale, epsilon, &nu);
        let new: Vec<usize> = bad.into_iter().filter(|&c| mean[c] != min[c]).collect();
        if new.is_empty() {
            break;
        }
        for c in new {
            mean[c] = min[c];
        }
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
    let at = epsilon::at_epsilon(
        &w.inp,
        &w.class,
        &w.ln_d,
        &[0.0; border::N_CLASSES],
        &[1.0; border::N_CLASSES],
        0.0,
    )
    .unwrap();
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
        theta_fit: false,
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
    let read = epsilon::read_fit(&file).unwrap();
    let e = read.epsilon;
    assert_eq!((e, read.nu), (fit.trial.epsilon, fit.trial.fit.nu.clone()));
    assert_eq!(read.ln_d0.len(), fit.trial.ln_d0.len());
    assert_eq!(read.scale, fit.trial.scale);
    assert_eq!(read.theta2, Some(fit.trial.theta2));
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
        theta_fit: false,
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
        ln_d0: vec![6.0, 6.2, 6.4, f64::NAN, f64::NAN],
        scale: vec![1.0, 0.7, 1.0, 1.0, 1.0],
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
            theta_fit: false,
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
        theta_fit: false,
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

/// The Newton phase moves a trial within the scalings' tolerance (RFC 0362 D2): nu and s_c agree
/// with the sweep-only trial to 1e-13, and the trial is bit-identical at any thread count.
#[test]
fn newton_trial_agrees_with_the_sweep_and_threads() {
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
        theta_fit: false,
    };
    let trial = |newton| {
        let limits = Limits {
            newton,
            ..Limits::default()
        };
        epsilon::fit_at_epsilon(&w.inp, &t, e0, limits, &mut quiet()).unwrap()
    };
    let (on, off) = (trial(true), trial(false));
    let worst = |a: &[f64], b: &[f64]| {
        a.iter()
            .zip(b)
            .fold(0.0f64, |m, (x, y)| m.max((x - y).abs()))
    };
    eprintln!(
        "nu {:e} scale {:e} ln_d0 {:e} pooled {:e}",
        worst(&on.fit.nu, &off.fit.nu),
        worst(&on.scale, &off.scale),
        worst(&on.ln_d0, &off.ln_d0),
        (on.pooled - off.pooled).abs()
    );
    assert!(worst(&on.fit.nu, &off.fit.nu) <= 1e-13);
    assert!(worst(&on.scale, &off.scale) <= 1e-13);
    assert!(worst(&on.ln_d0, &off.ln_d0) <= 1e-13);
    let bits = |t: &epsilon::Trial| -> Vec<u64> {
        [&t.fit.nu, &t.scale, &t.ln_d0, &t.classes]
            .into_iter()
            .flatten()
            .chain([&t.pooled])
            .map(|x| x.to_bits())
            .collect()
    };
    for threads in [1, 2, 8] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        let again = pool.install(|| trial(true));
        assert_eq!(bits(&again), bits(&on), "{threads} threads");
    }
}

/// A fit at a given epsilon is one trial at exactly that epsilon.
#[test]
fn fit_at_a_given_epsilon_is_one_trial() {
    let w = world();
    let nu_group = [0.15, 0.6, 0.35, 0.0, 0.0];
    let (nu_target, data_flows) = known(&w, 0.4, &nu_group);
    let t = Targets {
        theta_fit: false,
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

/// RFC 0355 D3: with `theta_fit` on, the trial's theta of good 2 is the one whose cross-border
/// share at nu 0 meets the target: targets made at theta 11 (good 2 at nu 0 and no distance cost of its own) on a world built at
/// 9.27 return 11, and with the switch off the mismatch shows up as a good 2 border friction.
#[test]
fn good_2_theta_is_fitted_jointly_with_epsilon() {
    let (truth, start) = (11.0, world());
    assert_ne!(start.inp.theta[epsilon::THETA_GOOD], truth);
    let mut w = world();
    w.inp = epsilon::at_theta(&start.inp, epsilon::THETA_GOOD, truth).unwrap();
    let nu_group = [0.15, 0.6, 0.35, 0.0, 0.5];
    let (nu_target, data_flows) = known_scaled(&w, 0.4, &[1.0, 1.0, 1.0, 0.0, 1.0], &nu_group);
    let weight = trade_weight(&start.inp);
    let targets = |theta_fit| Targets {
        theta_fit,
        nu_target: &nu_target,
        icio: &start.icio,
        theta_class: &start.class,
        ln_d: &start.ln_d,
        weight: &weight,
        gravity: &start.gravity,
        data_flows: &data_flows,
    };
    let on = epsilon::fit_at_epsilon(
        &start.inp,
        &targets(true),
        0.4,
        Limits::default(),
        &mut quiet(),
    )
    .unwrap();
    assert!((on.theta2 - truth).abs() <= 1e-3, "theta {}", on.theta2);
    assert!(on.fit.nu[3] < 1e-4, "{:?}", on.fit.nu);
    // off, the starting theta absorbs the mismatch in a border friction the targets never had
    let off = epsilon::fit_at_epsilon(
        &start.inp,
        &targets(false),
        0.4,
        Limits::default(),
        &mut quiet(),
    )
    .unwrap();
    assert_eq!(off.theta2, start.inp.theta[epsilon::THETA_GOOD]);
    assert!(off.fit.nu[3] > 1e-2, "{:?}", off.fit.nu);
}

/// theta_2 is fitted at the other classes' settled s_c, nu and d_0, not at s_c = 1 and the mean
/// d_0: targets made at theta 11 with classes 0 and 1 clamped (s_c 0.5, d_0 floored) return 11, and
/// class 3 meets its share at nu 0 (the Mac refit missed it by 0.0019 with the other classes'
/// floors unseen by the theta fit).
#[test]
fn good_2_theta_is_refitted_at_the_settled_epsilon_scales() {
    let (truth, start) = (11.0, world());
    let mut w = world();
    w.inp = epsilon::at_theta(&start.inp, epsilon::THETA_GOOD, truth).unwrap();
    let nu_group = [0.0, 0.0, 0.35, 0.0, 0.5];
    let (nu_target, data_flows) = known_scaled(&w, 0.4, &[0.5, 0.5, 1.0, 0.0, 1.0], &nu_group);
    let weight = trade_weight(&start.inp);
    let t = Targets {
        theta_fit: true,
        nu_target: &nu_target,
        icio: &start.icio,
        theta_class: &start.class,
        ln_d: &start.ln_d,
        weight: &weight,
        gravity: &start.gravity,
        data_flows: &data_flows,
    };
    let tr = epsilon::fit_at_epsilon(&start.inp, &t, 0.4, Limits::default(), &mut quiet()).unwrap();
    assert!(tr.scale[0] < 1.0 && tr.scale[1] < 1.0, "{:?}", tr.scale);
    assert!((tr.theta2 - truth).abs() <= 1e-3, "theta {}", tr.theta2);
    assert!(tr.fit.nu[3] < 1e-4, "{:?}", tr.fit.nu);
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
    let at = epsilon::at_epsilon(
        &w.inp,
        &w.class,
        &w.ln_d,
        &d0,
        &[1.0; border::N_CLASSES],
        0.4,
    )
    .unwrap();
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
        theta_fit: false,
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
        theta_fit: false,
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
    // Classes 3 and 4 at nu 0 sit where (1 + nu) D < 1 at the weighted mean, and there d_0 and nu
    // are not separately identified (a floored d_0 with nu 0, or the mean with a small nu, fit
    // alike); a nu above that gap makes the truth the trial's only fit.
    let (e0, nu_group) = (0.4, [0.15, 0.6, 0.35, 0.3, 0.2]);
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
            theta_fit: false,
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

fn parts_at(w: &World, s: f64, start: Option<&border::Reference>) -> border::Parts {
    let (d0, _) = epsilon::class_offsets(&w.inp, &w.class, &w.ln_d, &trade_weight(&w.inp));
    let at =
        epsilon::at_epsilon(&w.inp, &w.class, &w.ln_d, &d0, &[s; border::N_CLASSES], 0.4).unwrap();
    let nu = border::nu_of_goods(&w.groups, &[0.15, 0.6, 0.35, 1.2, 0.5], &at.border_nu);
    border::data_state_parts_from(&at, &nu, &w.groups, &w.icio, Limits::default(), start).unwrap()
}

/// RFC 0361 D6: the data state's non-final scalings stop above `SINKHORN_TOL`, pass 1 at
/// `INNER_TOL_MAX`, and the returned pass's scaling ran at `SINKHORN_TOL`.
#[test]
fn data_state_scalings_tighten_to_the_last_pass() {
    let w = world();
    let d = steady::data_state_spending_from(&w.inp, Limits::default(), None).unwrap();
    assert_eq!(d.tols[0], steady::INNER_TOL_MAX);
    assert_eq!(*d.tols.last().unwrap(), steady::SINKHORN_TOL);
    assert!(
        d.tols[..d.tols.len() - 1]
            .iter()
            .any(|&t| t > steady::SINKHORN_TOL)
    );
    assert_eq!(d.tols.len(), d.sweeps.len());
}

/// RFC 0361 D5: the scalings take the reference as their start (the final scaling from its own
/// converged weights stops in far fewer sweeps than cold), an evaluation from the reference gives the cold one's
/// parts within tolerance, and its bits do not depend on what ran before it. How many sweeps a
/// reference saves at another s is a property of the pack, measured on the full pack.
#[test]
fn evaluations_start_from_the_reference() {
    let w = world();
    let cold0 = parts_at(&w, 0.0, None);
    let again0 = parts_at(&w, 0.0, Some(&cold0.reference));
    assert!(
        again0.reference.final_sweeps * 2 < cold0.reference.final_sweeps,
        "{} final sweeps from its own weights, {} cold",
        again0.reference.final_sweeps,
        cold0.reference.final_sweeps
    );
    assert!(again0.reference.data_sweeps[0] <= cold0.reference.data_sweeps[0]);
    let cold = parts_at(&w, 1.0, None);
    let warm = parts_at(&w, 1.0, Some(&cold0.reference));
    for (a, b) in warm.abroad.iter().zip(&cold.abroad) {
        assert!((a - b).abs() <= 1e-9 * b.abs().max(1.0), "{a} vs {b}");
    }
    let _unrelated = parts_at(&w, 0.5, Some(&cold0.reference));
    let again = parts_at(&w, 1.0, Some(&cold0.reference));
    assert_eq!(warm.abroad, again.abroad);
    assert_eq!(warm.total, again.total);
}

fn targets_of<'a>(
    w: &'a World,
    nu_target: &'a [f64],
    weight: &'a [f64],
    ln_d: &'a [f64],
    data_flows: &'a [f64],
) -> Targets<'a> {
    Targets {
        nu_target,
        icio: &w.icio,
        theta_class: &w.class,
        ln_d,
        weight,
        gravity: &w.gravity,
        data_flows,
        theta_fit: false,
    }
}

#[test]
fn preflight_accepts_the_fixture_and_reports_an_unlinked_pair() {
    let w = world();
    let nu_group = [0.15, 0.0, 0.35, 0.0, 0.0];
    let (nu_target, data_flows) = known(&w, 0.4, &nu_group);
    let weight = trade_weight(&w.inp);
    let t = targets_of(&w, &nu_target, &weight, &w.ln_d, &data_flows);
    let report = epsilon::preflight(&w.inp, &t).unwrap();
    assert!(report.len() >= 3, "{report:?}");

    // a linked pair with no distance: its crossings carry no distance cost (`ln_factor`) and stay
    // out of d_0, as on the real pack; preflight reports the pair instead of refusing the fit
    let n = w.inp.n_countries();
    let mut ln_d = w.ln_d.clone();
    let j = (0..w.inp.crossing.good.len())
        .find(|&j| w.inp.crossing.good[j] < w.class.len() && w.inp.crossing.k[j] > 0.0)
        .unwrap();
    let (a, b) = (
        w.inp.gateway_country[w.inp.crossing.row[j]],
        w.inp.gateway_country[w.inp.crossing.col[j]],
    );
    assert_ne!(a, b);
    ln_d[a * n + b] = f64::NAN;
    let bad = targets_of(&w, &nu_target, &weight, &ln_d, &data_flows);
    let report = epsilon::preflight(&w.inp, &bad).unwrap();
    let line = report
        .iter()
        .find(|l| l.contains("no finite ln distance"))
        .unwrap_or_else(|| panic!("{report:?}"));
    assert!(line.contains(&format!("({a}, {b})")), "{line}");
    // the bracket checks still run
    assert!(report.len() >= 4, "{report:?}");
}

#[test]
fn live_progress_is_emitted_per_line_with_stamp_and_epsilon_and_keeps_the_result() {
    use std::sync::Mutex;
    let w = world();
    let nu_group = [0.15, 0.6, 0.35, 0.0, 0.0];
    let (nu_target, data_flows) = known(&w, 0.4, &nu_group);
    let weight = trade_weight(&w.inp);
    let t = targets_of(&w, &nu_target, &weight, &w.ln_d, &data_flows);
    let dir = std::env::temp_dir().join(format!("ceteris-live-{}", std::process::id()));
    let log = ceteris_calibrate::run_log::RunLog::new(&dir).unwrap();
    let lines = Mutex::new(vec![]);
    let live = |l: &str| lines.lock().unwrap().push(l.to_string());
    let logged =
        epsilon::fit_epsilon_logged(&w.inp, &t, Limits::default(), None, Some(0.4), &log, &live)
            .unwrap();
    let plain =
        epsilon::fit_epsilon_at(&w.inp, &t, 0.4, Limits::default(), None, &mut quiet()).unwrap();
    assert_eq!(logged.json().compact(), plain.json().compact());
    let lines = lines.into_inner().unwrap();
    assert!(!lines.is_empty());
    assert!(
        lines
            .iter()
            .all(|l| l.starts_with('[') && l.contains("s] epsilon 0.4")),
        "{lines:?}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// The committed d5e2 fit is what a calibration applies (#667): its theta_2 is the P01 default the
/// run's static digest holds, and its epsilon switches the distance cost on, moving the
/// crossings of the fixture off the frictionless ones.
#[test]
fn committed_fit_theta_2_is_the_default_and_epsilon_switches_the_cost_on() {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fits/epsilon-d5e2.json");
    let fit = epsilon::read_fit(&file).unwrap();
    assert_eq!(fit.theta2, Some(ceteris_trade::params::GOOD2_THETA));
    assert_eq!(fit.epsilon, 0.390_032_013_037_640_57);
    assert_eq!(fit.ln_d0.len(), border::N_CLASSES);
    let w = world();
    let at = |epsilon: f64| {
        epsilon::at_epsilon(&w.inp, &w.class, &w.ln_d, &fit.ln_d0, &fit.scale, epsilon).unwrap()
    };
    assert_eq!(at(0.0).crossing.k, w.inp.crossing.k);
    assert_ne!(at(fit.epsilon).crossing.k, w.inp.crossing.k);
}
