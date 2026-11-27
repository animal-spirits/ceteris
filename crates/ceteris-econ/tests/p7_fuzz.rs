#![allow(clippy::disallowed_methods)] // tests are not hashed
//! Fuzz of the edit path (DESIGN 15.7, ADR 0120, #612): random interventions and extreme shocks
//! at random ticks never halt the run, break the ledger invariants or leave a non-finite price.
//! Every generated edit is in range, so every edit applies.

use std::sync::Arc;

use ceteris_core::sim::Sim;
use ceteris_econ::edit::apply_due;
use ceteris_econ::fixture::small_world;
use ceteris_econ::systems::{self, SEED};
use ceteris_econ::{EconParams, EconWorld};
use ceteris_run::{Edit, EditKind, EditOp, EditSource};
use ceteris_trade::TradeParams;
use proptest::prelude::*;

const HORIZON: u32 = 40;

fn edit_strategy() -> impl Strategy<Value = Edit> {
    let tick = 0..HORIZON;
    let cell = (0usize..3, 0usize..3, 0usize..2);
    let tariff = (tick.clone(), cell.clone(), 0.0f64..5.0).prop_map(|(tick, (c, d, g), v)| Edit {
        tick,
        source: EditSource::User,
        god: false,
        kind: EditKind::Policy,
        target: format!("tariff.{c}.{d}.{g}"),
        value: v,
        op: EditOp::Set,
        query: None,
    });
    let ntb =
        (tick.clone(), cell, 1.0f64..100.0, any::<bool>()).prop_map(|(tick, (c, d, g), v, mul)| {
            Edit {
                tick,
                source: EditSource::User,
                god: false,
                kind: EditKind::Policy,
                target: format!("ntb.{c}.{d}.{g}"),
                value: v,
                op: if mul { EditOp::Mul } else { EditOp::Set },
                query: None,
            }
        });
    let param = (
        tick,
        prop_oneof![Just("econ.kappa_w"), Just("econ.eta")],
        0.001f64..1.0,
    )
        .prop_map(|(tick, target, v)| Edit {
            tick,
            source: EditSource::User,
            god: false,
            kind: EditKind::Param,
            target: target.to_string(),
            value: v,
            op: EditOp::Set,
            query: None,
        });
    prop_oneof![tariff, ntb, param]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn random_edits_keep_the_run_valid(
        edits in proptest::collection::vec(edit_strategy(), 1..12),
        frictionless in any::<bool>(),
    ) {
        let params = if frictionless { EconParams::frictionless() } else { EconParams::default() };
        let world = EconWorld::new(Arc::new(small_world()), params, TradeParams::default());
        let mut sim = Sim::new(world, systems::schedule(), SEED);
        let mut applied = 0;
        for tick in 0..HORIZON {
            applied += apply_due(sim.world_mut(), &edits, tick).unwrap();
            let step = sim.step();
            prop_assert!(step.is_ok(), "tick {tick}: {}", step.unwrap_err());
        }
        prop_assert_eq!(applied, edits.len());
        let w = sim.world();
        prop_assert!(w.markets.price.iter().all(|p| p.is_finite() && *p > 0.0));
        let acc = &w.layout().accounts;
        for a in &acc.enterprise {
            prop_assert!(w.ledger.money(*a) <= 0);
        }
        for a in acc.market.iter().chain(&acc.rebate).chain(&acc.dividend) {
            prop_assert_eq!(w.ledger.money(*a), 0);
        }
    }
}
