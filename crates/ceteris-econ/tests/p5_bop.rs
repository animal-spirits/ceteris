//! The ADR 0228 closure's border conversion (RFC 0366 build step 3): payments across a currency
//! border go through the financiers.

use std::sync::Arc;

use ceteris_core::ledger::AccountId;
use ceteris_core::rows::Rows;
use ceteris_core::sim::Sim;
use ceteris_econ::fixture::small_world;
use ceteris_econ::inputs::{EconInputs, TRANSFER_YEARS};
use ceteris_econ::params::Transfers;
use ceteris_econ::systems::{self, SEED};
use ceteris_econ::{EconParams, EconWorld};
use ceteris_trade::TradeParams;
use ceteris_trade::goods::p2::N as N_GOODS;

/// The fixture with transfers (two payers, two receivers) and foreign owners, so every border
/// payment of the closure runs: goods legs, the transfer and the foreign dividends.
fn inputs() -> EconInputs {
    let mut inp = small_world();
    let nc = inp.n_countries();
    let share = vec![0.02, -0.012, -0.013, 0.005];
    inp.countries.transfer_path = Rows::from_rows(
        TRANSFER_YEARS,
        share.iter().map(|s| vec![*s; TRANSFER_YEARS]),
    );
    inp.countries.transfer = share;
    let f = |c: usize, g: usize| match (c, (1..=6).contains(&g)) {
        (_, false) => 0.0,
        (0, true) => 0.3,
        _ => 0.1,
    };
    inp.countries.foreign = Rows::from_flat(
        (0..nc)
            .flat_map(|c| (0..N_GOODS).map(move |g| f(c, g)))
            .collect(),
        N_GOODS,
    );
    inp.countries.owners = Rows::from_flat(
        (0..nc)
            .flat_map(|h| (0..nc).map(move |o| if o == usize::from(h == 0) { 1.0 } else { 0.0 }))
            .collect(),
        nc,
    );
    inp.validate();
    inp
}

fn world(rates: Option<&[f64]>) -> EconWorld {
    let params = EconParams {
        transfers: Transfers::FixedShares,
        ..EconParams::default()
    };
    let w = EconWorld::new(Arc::new(inputs()), params, TradeParams::default());
    let Some(rates) = rates else { return w };
    // countries 0 and 1 share a currency; 2 and 3 float alone
    let mut w = w.with_currencies(vec![0, 0, 1, 2]);
    let c = w.currencies.as_mut().expect("on");
    for (e, r) in c.e.iter_mut().zip(rates) {
        *e = *r;
    }
    w
}

fn sim(w: EconWorld) -> Sim<EconWorld> {
    Sim::new(w, systems::schedule(), SEED)
}

/// With every rate at 1 and the switch on, the real allocation and every country account equal
/// the run without the closure (the E3 run); only the mints, which hold each currency's
/// issue, and the financiers differ.
#[test]
fn bop_at_unit_rates_matches_e3() {
    let (mut off, mut on) = (sim(world(None)), sim(world(Some(&[1.0; 3]))));
    let n_accounts = off.world().ledger.n_accounts();
    for tick in 0..24 {
        off.step().unwrap_or_else(|v| panic!("{v}"));
        on.step().unwrap_or_else(|v| panic!("{v}"));
        let (a, b) = (off.world(), on.world());
        assert_eq!(a.output, b.output, "tick {tick}");
        assert_eq!(a.sales, b.sales, "tick {tick}");
        assert_eq!(a.price_index, b.price_index, "tick {tick}");
        assert_eq!(a.wage, b.wage, "tick {tick}");
        assert_eq!(a.income, b.income, "tick {tick}");
        let mints = &b.currencies.as_ref().expect("on").mint;
        for acc in (0..n_accounts as u32).filter(|&a| !mints.contains(&AccountId(a))) {
            assert_eq!(
                a.ledger.money_wide(AccountId(acc)),
                b.ledger.money_wide(AccountId(acc)),
                "account {acc} tick {tick}"
            );
        }
    }
}

/// Every currency's balances sum to zero after every tick, at rates other than 1, and the
/// financiers do carry the border flows (the switch-on behaviour).
#[test]
fn bop_money_conserved_per_currency() {
    let mut s = sim(world(Some(&[1.0, 1.25, 0.8])));
    let numeraire = s.world().currencies.as_ref().expect("on").numeraire;
    let mut financier_moved = false;
    for tick in 0..24 {
        s.step().unwrap_or_else(|v| panic!("{v}"));
        let w = s.world();
        let c = w.currencies.as_ref().expect("on");
        assert_eq!(c.numeraire, numeraire);
        assert!(c.balances(&w.ledger).iter().all(|&b| b == 0), "tick {tick}");
        financier_moved |= c.financier.iter().any(|&f| w.ledger.money_wide(f) != 0);
    }
    assert!(financier_moved);
}

#[test]
fn bop_snapshot_replays() {
    let run = || {
        let mut s = sim(world(Some(&[1.0, 1.25, 0.8])));
        s.run_until(12).unwrap_or_else(|v| panic!("{v}"));
        s
    };
    let (a, b) = (run(), run());
    assert_eq!(a.state_hash(), b.state_hash());
}
