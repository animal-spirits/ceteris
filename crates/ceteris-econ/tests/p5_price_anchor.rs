//! P5 Branch E1: the money-anchored price rule (ADR 0338, trade SPEC Money-anchored rule).
use std::path::Path;
use std::sync::Arc;

use ceteris_core::sim::Sim;
use ceteris_econ::equilibrium::Equilibrium;
use ceteris_econ::fixture::small_world;
use ceteris_econ::systems::{SEED, schedule};
use ceteris_econ::{EconParams, EconWorld};
use ceteris_trade::{PriceAnchor, TradeParams};

fn settled(params: EconParams) -> EconWorld {
    let trade = TradeParams {
        anchor: Some(PriceAnchor::default()),
        ..TradeParams::default()
    };
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/p2-fixture/reference");
    let mut w = EconWorld::new(Arc::new(small_world()), params, trade);
    w.settle(&Equilibrium::read(&dir).unwrap());
    w
}

#[test]
fn anchor_is_off_by_default() {
    assert!(TradeParams::default().anchor.is_none());
}

#[test]
fn fixture_steady_state_unchanged() {
    for params in [EconParams::frictionless(), EconParams::default()] {
        let mut sim = Sim::new(settled(params), schedule(), SEED);
        let i0 = sim.world().world_price_index();
        sim.run_until(120).unwrap();
        let drift = libm::log(sim.world().world_price_index() / i0).abs();
        assert!(
            drift < 1e-6,
            "index drifted {drift:e} from the settled state"
        );
    }
}

#[test]
fn nominal_shock_returns_within_30_ticks() {
    let mut w = settled(EconParams::frictionless());
    let (i0, m0) = (w.world_price_index(), w.money_issued());
    w.anchor_base = Some((i0, m0));
    let acc = w.layout().accounts.clone();
    for &k in &acc.pop {
        let b = w.ledger.money(k);
        if b > 0 {
            w.ledger.transfer(
                ceteris_econ::world::MINT,
                acc.mint,
                k,
                (b as f64 * 0.05) as i64,
            );
        }
    }
    let target = i0 * w.money_issued() / m0;
    let mut sim = Sim::new(w, schedule(), SEED);
    sim.run_until(30).unwrap();
    let off = libm::log(sim.world().world_price_index() / target).abs();
    assert!(
        off < 0.01,
        "index {off:.4} log points from the new level at tick 30"
    );
}

/// ADR 0340: with the Q-floor gap one origin at Q near 0 set the good and world means and every
/// price stepped at -c_max, the index reaching 1e-4 by tick 100. Output still collapses at x10
/// (task E3, ADR 0341); this checks only the nominal level.
#[test]
fn x10_border_shock_keeps_the_index_off_the_bound() {
    let mut w = settled(EconParams::frictionless());
    let i0 = w.world_price_index();
    let mut wedges = ceteris_trade::network::Wedges::none(w.inputs().n_countries());
    wedges.ntb = vec![10.0; wedges.tariff.len()];
    w.set_wedges(wedges);
    let mut sim = Sim::new(w, schedule(), SEED);
    for t in 1..=200 {
        sim.run_until(t).unwrap();
        let off = libm::log(sim.world().world_price_index() / i0).abs();
        assert!(off < 0.25, "index {off:.3} log points off I0 at tick {t}");
    }
}
