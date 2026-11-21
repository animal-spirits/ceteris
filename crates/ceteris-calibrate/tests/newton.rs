//! The scaling's Newton phase (RFC 0361, D3 amended by RFC 0362). The oracle of every test is the
//! same call with `newton` off, which is the sweep alone and main's code path.
#![allow(clippy::disallowed_methods)] // tests are not hashed

use std::path::Path;

use ceteris_calibrate::newton::NEWTON_AFTER;
use ceteris_calibrate::steady::{
    self, Accel, Limits, MAX_SWEEPS, NewtonOutcome, SINKHORN_TOL, Scaling, Steady,
};
use ceteris_reference::io::{self, Inputs, Mat};
use ceteris_reference::model::Model;

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn fixture() -> Inputs {
    io::load(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/p2-fixture")).unwrap()
}

/// A deterministic start: ln w of the active origins spread over [-3, 3), so the scaling takes
/// many sweeps.
fn start(inp: &Inputs) -> Mat {
    let (n_p, n_g) = (inp.n_provinces(), inp.n_goods());
    Mat::from_fn(n_p, n_g, |p, g| {
        let mut x = ((p * n_g + g) as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        x ^= x >> 29;
        x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        x ^= x >> 32;
        if inp.traded[g] && inp.active[p][g] {
            (x >> 11) as f64 / (1u64 << 53) as f64 * 6.0 - 3.0
        } else {
            0.0
        }
    })
}

/// The stage 2 inputs of a calibrated steady state: spending and sales of the traded goods.
fn stage2(inp: &Inputs, s: &Steady) -> (Mat, Mat) {
    let (n_p, n_g) = (inp.n_provinces(), inp.n_goods());
    let mut v = Mat::zeros(n_p, n_g);
    for (e, (&p, &g)) in inp.ent_province.iter().zip(&inp.ent_good).enumerate() {
        if inp.traded[g] {
            v[(p, g)] += s.value[e] / (1.0 + s.spoilage[g]);
        }
    }
    let spend = Mat::from_fn(n_p, n_g, |p, g| s.spend_hh[(p, g)] + s.spend_int[(p, g)]);
    (spend, v)
}

fn scale(m: &Model, spend: &Mat, v: &Mat, from: Option<&Mat>, newton: bool) -> Scaling {
    steady::sinkhorn_to(m, spend, v, from, MAX_SWEEPS, SINKHORN_TOL, newton).unwrap()
}

/// `max |a - b|` of two log-weight matrices, and `max |a - b| / b` of two r vectors.
fn differences(a: &Scaling, b: &Scaling) -> (f64, f64) {
    let dlw = a
        .log_weight
        .data
        .iter()
        .zip(&b.log_weight.data)
        .fold(0.0f64, |m, (x, y)| m.max((x - y).abs()));
    let dr =
        a.r.iter()
            .zip(&b.r)
            .fold(0.0f64, |m, (x, y)| m.max(((x - y) / y).abs()));
    (dlw, dr)
}

struct Case {
    m_inp: Inputs,
    spend: Mat,
    v: Mat,
}

fn case() -> Case {
    let inp = fixture();
    let quiet = &mut |_: &str| {};
    let s = steady::calibrate_with(&inp, &inp.u_star.clone(), Limits::default(), quiet).unwrap();
    let (spend, v) = stage2(&inp, &s);
    Case {
        m_inp: inp,
        spend,
        v,
    }
}

/// Switch-on: the sweep needs more than `NEWTON_AFTER` sweeps, so the phase runs, takes steps and
/// ends the scaling at the next sweep; its result meets the stop rule and sits within the
/// pre-registered bounds of the sweep-only result (RFC 0362 D1). Fails without the phase: the
/// sweep alone takes over a hundred sweeps.
#[test]
fn newton_solves_a_slow_scaling_and_matches_the_sweep() {
    let c = case();
    let m = Model::new(&c.m_inp).unwrap();
    let from = start(&c.m_inp);
    for from in [None, Some(&from)] {
        let sweep = scale(&m, &c.spend, &c.v, from, false);
        let newton = scale(&m, &c.spend, &c.v, from, true);
        assert!(sweep.sweeps > NEWTON_AFTER + 1, "{}", sweep.sweeps);
        assert_eq!(newton.newton.outcome, NewtonOutcome::Solved);
        assert!(newton.newton.steps > 0 && newton.newton.goods > 0);
        assert!(newton.sweeps <= NEWTON_AFTER + 1, "{}", newton.sweeps);
        assert!(newton.residual <= SINKHORN_TOL, "{:e}", newton.residual);
        let (dlw, dr) = differences(&newton, &sweep);
        eprintln!("dlw {dlw:e} dr {dr:e} {:?}", newton.newton);
        assert!(dlw <= 1e-8, "{dlw:e}");
        assert!(dr <= 1e-12, "{dr:e}");
    }
}

/// A scaling that stops within `NEWTON_AFTER` sweeps never enters the phase and returns main's
/// bits: here a restart from the solution.
#[test]
fn scalings_below_the_switch_are_bit_identical_to_the_sweep() {
    let c = case();
    let m = Model::new(&c.m_inp).unwrap();
    let solved = scale(&m, &c.spend, &c.v, None, false);
    let (on, off) = (
        scale(&m, &c.spend, &c.v, Some(&solved.log_weight), true),
        scale(&m, &c.spend, &c.v, Some(&solved.log_weight), false),
    );
    assert!(off.sweeps <= NEWTON_AFTER, "{}", off.sweeps);
    assert_eq!(on.newton.outcome, NewtonOutcome::NotRun);
    assert_eq!(bits(&on.log_weight.data), bits(&off.log_weight.data));
    assert_eq!(bits(&on.r), bits(&off.r));
    assert_eq!(
        (on.sweeps, on.residual.to_bits()),
        (off.sweeps, off.residual.to_bits())
    );
}

/// A good whose legs are all closed has no balancing (the origins serve only themselves, so
/// received is spending whatever the weights): the reduced Jacobian is singular, Newton fails
/// and the sweep resumes where it stood, so the error is main's bit for bit, the residual and
/// the worst good included.
#[test]
fn an_unbalanceable_good_falls_back_to_the_sweeps_error() {
    let mut c = case();
    let g = (0..c.m_inp.n_goods())
        .find(|&g| {
            c.m_inp.traded[g]
                && (0..c.m_inp.n_provinces())
                    .filter(|&p| c.m_inp.active[p][g])
                    .count()
                    > 1
        })
        .expect("a traded good with two origins");
    let inp = &mut c.m_inp;
    for (k, &good) in inp.domestic.k.iter_mut().zip(&inp.domestic.good) {
        if good == g {
            *k = 0.0;
        }
    }
    for (k, &good) in inp.access.k.iter_mut().zip(&inp.access.good) {
        if good == g {
            *k = 0.0;
        }
    }
    for (k, &good) in inp.crossing.k.iter_mut().zip(&inp.crossing.good) {
        if good == g {
            *k = 0.0;
        }
    }
    let m = Model::new(&c.m_inp).unwrap();
    let run = |newton| {
        let (out, stats) = steady::sinkhorn_traced(
            &m,
            &c.spend,
            &c.v,
            None,
            80,
            steady::Stop {
                tol: SINKHORN_TOL,
                plateau: None,
            },
            Accel::Joint,
            newton,
        );
        (out.unwrap_err(), stats)
    };
    let ((on, stats), (off, _)) = (run(true), run(false));
    assert!(
        matches!(&stats.outcome, NewtonOutcome::Failed(why) if why.starts_with(&format!("good {g}:"))),
        "{:?}",
        stats.outcome
    );
    assert_eq!(on, off);
}

fn pool(threads: usize) -> rayon::ThreadPool {
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .unwrap()
}

/// The Newton path is bit-identical at any thread count, goods being solved in parallel into
/// index order (ADR 0097).
#[test]
fn newton_scaling_is_bit_identical_at_any_thread_count() {
    let c = case();
    let m = Model::new(&c.m_inp).unwrap();
    let from = start(&c.m_inp);
    let mut reference = None;
    for t in [1, 2, 4, 8] {
        let r = pool(t).install(|| scale(&m, &c.spend, &c.v, Some(&from), true));
        assert_eq!(r.newton.outcome, NewtonOutcome::Solved);
        let got = (
            bits(&r.log_weight.data),
            bits(&r.r),
            r.sweeps,
            r.residual.to_bits(),
            r.newton,
        );
        match &reference {
            None => reference = Some(got),
            Some(want) => assert!(*want == got, "{t} threads differ from the first"),
        }
    }
}
