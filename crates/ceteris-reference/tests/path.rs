#![allow(clippy::disallowed_methods)]
use std::path::PathBuf;

use ceteris_reference::io::{self, Inputs};
use ceteris_reference::model::Model;
use ceteris_reference::path;
use ceteris_reference::solve::{self, TOL};

fn inputs() -> Inputs {
    let dir = std::env::var_os("CETERIS_FIXTURE_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/p2-fixture"),
        PathBuf::from,
    );
    io::load(dir).unwrap()
}

#[test]
fn path_solver_at_zero_trends_equals_the_static_reference() {
    let inp = inputs();
    let m = Model::new(&inp).unwrap();
    let fixed = solve::equilibrium(&m, None, TOL, None).unwrap();
    let eqs = path::solve(&inp, &[0, 60]).unwrap();
    for e in &eqs {
        assert_eq!(e.wage, fixed.wage);
        assert_eq!(e.price.data, fixed.price.data);
    }
}

#[test]
fn growing_country_labour_moves_the_reference() {
    let mut inp = inputs();
    inp.trends.n[0] = 2e-3;
    let eqs = path::solve(&inp, &[0, 60]).unwrap();
    assert_ne!(eqs[0].wage, eqs[1].wage);
    assert!(eqs[1].info.residual <= TOL);
}
