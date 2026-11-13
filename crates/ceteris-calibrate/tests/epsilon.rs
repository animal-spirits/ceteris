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
    let (mean, _) = epsilon::class_offsets(&w.inp, &w.class, &w.ln_d, &trade_weight(&w.inp));
    let at = epsilon::at_epsilon(&w.inp, &w.class, &w.ln_d, &mean, epsilon).unwrap();
    let nu = border::nu_of_goods(&w.groups, nu_group, &at.border_nu);
    let target =
        border::data_state_shares(&at, &nu, &w.groups, &w.icio, Limits::default()).unwrap();
    let (spend, supply) = steady::data_state_spending(&at, Limits::default()).unwrap();
    let pairs = border::data_state_pair_values(
        &epsilon::at_epsilon(&w.inp, &w.class, &w.ln_d, &mean, epsilon).unwrap(),
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
    let at = epsilon::at_epsilon(&w.inp, &w.class, &w.ln_d, &[0.0; 4], 0.0).unwrap();
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
    assert!(!fit.at_zero);
    assert!((fit.trial.epsilon - e0).abs() <= 2.0 * epsilon::EPSILON_TOL);
    assert!(fit.held_out.iter().all(|c| c.within), "{:?}", fit.held_out);
}

/// A model whose coefficient is already at the data's keeps epsilon 0.
#[test]
fn data_at_epsilon_zero_keeps_zero() {
    let w = world();
    let nu_group = [0.15, 0.6, 0.35, 0.0, 0.0];
    let (nu_target, data_flows) = known(&w, 0.0, &nu_group);
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
