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
    settled_with(params, PriceAnchor::default())
}

fn settled_with(params: EconParams, anchor: PriceAnchor) -> EconWorld {
    let trade = TradeParams {
        anchor: Some(anchor),
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

const E3: PriceAnchor = PriceAnchor {
    kappa_r_traded: 0.25,
    kappa_r_local: 0.05,
    kappa_b: 0.3,
    kappa_m: 0.2,
    e3: true,
};

#[test]
fn e3_is_off_by_default() {
    assert!(!PriceAnchor::default().e3);
}

#[test]
fn e3_steady_state_unchanged() {
    let mut sim = Sim::new(
        settled_with(EconParams::frictionless(), E3),
        schedule(),
        SEED,
    );
    let i0 = sim.world().world_price_index();
    sim.run_until(120).unwrap();
    let drift = libm::log(sim.world().world_price_index() / i0).abs();
    assert!(drift < 1e-6, "index drifted {drift:e}");
}

#[test]
fn e3_nominal_shock_returns_within_30_ticks() {
    let mut w = settled_with(EconParams::frictionless(), E3);
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
    assert!(off < 0.01, "index {off:.4} log points off the new level");
}

/// RFC 0342 go number: x10 survives 700 ticks, the index within 1.10 of base, output near base.
#[test]
fn e3_x10_border_shock_survives_700_ticks() {
    let mut w = settled_with(EconParams::frictionless(), E3);
    let (i0, y0) = (w.world_price_index(), w.output.iter().sum::<f64>());
    let mut wedges = ceteris_trade::network::Wedges::none(w.inputs().n_countries());
    wedges.ntb = vec![10.0; wedges.tariff.len()];
    w.set_wedges(wedges);
    let mut sim = Sim::new(w, schedule(), SEED);
    let (mut imax, mut ymin) = (1.0f64, f64::INFINITY);
    for t in 1..=700 {
        sim.run_until(t).unwrap();
        let w = sim.world();
        imax = imax.max(w.world_price_index() / i0);
        if t > 20 {
            ymin = ymin.min(w.output.iter().sum::<f64>() / y0);
        }
    }
    assert!(imax < 1.10, "index reached {imax:.3} of base");
    assert!(ymin > 0.9, "output fell to {ymin:.3} of base");
}

/// The chain, the consignments and the flag are state: a restored world replays the run.
#[test]
fn e3_snapshot_replays() {
    let mut w = settled_with(EconParams::frictionless(), E3);
    let mut wedges = ceteris_trade::network::Wedges::none(w.inputs().n_countries());
    wedges.ntb = vec![10.0; wedges.tariff.len()];
    w.set_wedges(wedges);
    let mut sim = Sim::new(w, schedule(), SEED);
    sim.run_until(40).unwrap();
    let h = sim.state_hash();
    sim.seek(14).unwrap();
    sim.run_until(40).unwrap();
    assert_eq!(sim.state_hash(), h);
}
