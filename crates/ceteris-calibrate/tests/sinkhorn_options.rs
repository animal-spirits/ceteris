//! The scaling's per-good mixing and the outer restart reach the default's fixed point in fewer
//! sweeps. Fixture benchmark, release: joint 26 passes 459 sweeps, per good 26 passes 282.
use ceteris_calibrate::steady::{self, Accel, Limits, Steady};
use ceteris_reference::io;
use std::path::Path;

fn run(limits: Limits) -> (Steady, usize, usize) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/p2-fixture");
    let inp = io::load(dir).unwrap();
    let (mut passes, mut sweeps) = (0, 0);
    let mut log = |l: &str| {
        if let Some(rest) = l.split("sinkhorn ").nth(1) {
            passes += 1;
            sweeps += rest.split(' ').next().unwrap().parse::<usize>().unwrap();
        }
    };
    let s = steady::calibrate_with(&inp, &inp.u_star.clone(), limits, &mut log).unwrap();
    eprintln!("passes {passes} sweeps {sweeps}");
    (s, passes, sweeps)
}

fn close(a: &[f64], b: &[f64]) {
    for (x, y) in a.iter().zip(b) {
        assert!(
            (x - y).abs() <= 1e-9 * y.abs().max(1e-300) + 1e-12,
            "{x} vs {y}"
        );
    }
}

#[test]
fn per_good_mixing_needs_fewer_sweeps_at_the_same_fixed_point() {
    let (base, _, joint) = run(Limits::default());
    let per_good = Limits {
        accel: Accel::PerGood,
        ..Limits::default()
    };
    let (s, _, sweeps) = run(per_good);
    assert!(sweeps < joint, "{sweeps} sweeps, joint {joint}");
    close(&s.price.data, &base.price.data);
    close(&s.rebate, &base.rebate);
}

#[test]
fn outer_restart_reaches_the_same_fixed_point() {
    let (base, ..) = run(Limits::default());
    let restart = Limits {
        outer_restart: Some(1.0),
        ..Limits::default()
    };
    let (s, ..) = run(restart);
    close(&s.price.data, &base.price.data);
    close(&s.rebate, &base.rebate);
}
