#![allow(clippy::disallowed_methods)] // tests are not hashed
//! A calibration solved under the approved storage values (`TradeParams::with_storage`, the
//! storage-on half of the RFC 0286 screen): its static digest records them, so the calibrated
//! inputs run with storage on, and only under those values.

use std::path::Path;
use std::sync::Arc;

use ceteris_calibrate::{steady, write};
use ceteris_core::sim::Sim;
use ceteris_econ::export::write_inputs;
use ceteris_econ::fixture::small_world;
use ceteris_econ::systems::{SEED, schedule};
use ceteris_econ::{EconInputs, EconParams, EconWorld};
use ceteris_reference::io;
use ceteris_trade::{Kernel, TradeParams};

/// The fixture exported under `trade` and calibrated, its tables written into `dir`.
fn calibrate_under(trade: &TradeParams, dir: &Path) {
    let export = tempfile::tempdir().unwrap();
    let inputs = small_world();
    let kernel = Kernel::build(&inputs.network, &inputs.weight, &inputs.wedges, trade);
    write_inputs(
        export.path(),
        &inputs,
        &kernel,
        &EconParams::default(),
        trade,
    )
    .unwrap();
    let inp = io::load(export.path()).unwrap();
    let s = steady::calibrate(&inp, &inp.u_star.clone(), &mut |_: &str| {}).unwrap();
    let provinces: Vec<usize> = (0..inp.n_provinces()).collect();
    write::write(dir, &inp, &s, &provinces).unwrap();
}

#[test]
fn storage_calibration_runs_with_storage_on() {
    let dir = tempfile::tempdir().unwrap();
    let on = TradeParams::default().with_storage();
    calibrate_under(&on, dir.path());
    let mut inputs = small_world();
    inputs.calibrate(dir.path()).unwrap();
    assert!(inputs.storage);
    let trade = inputs.trade_params();
    assert!((0..trade.goods.n()).any(|g| trade.stores(g)));
    assert_eq!(trade.speculation, on.speculation);
    assert_eq!(trade.storage_loss, on.storage_loss);
    let inputs = Arc::new(inputs);
    let mut sim = Sim::new(
        EconWorld::new(inputs.clone(), EconParams::default(), trade),
        schedule(),
        SEED,
    );
    sim.run_until(24).unwrap();
    // the storage-off parameters are not the ones the steady state was solved at
    let off = TradeParams {
        goods: inputs.goods.clone(),
        border_nu: inputs.border_nu.clone().unwrap(),
        ..TradeParams::default()
    };
    let refused =
        std::panic::catch_unwind(|| EconWorld::new(inputs.clone(), EconParams::default(), off));
    assert!(refused.is_err());
}

#[test]
fn default_calibration_runs_with_storage_off() {
    let mut inputs: EconInputs = small_world();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/p2-fixture");
    inputs.calibrate(&fixture.join("calibration")).unwrap();
    assert!(!inputs.storage);
    let trade = inputs.trade_params();
    assert!((0..trade.goods.n()).all(|g| !trade.stores(g)));
}
