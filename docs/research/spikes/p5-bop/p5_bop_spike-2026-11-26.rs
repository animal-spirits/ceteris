use std::path::Path;
use std::sync::Arc;

use ceteris_core::sim::Sim;
use ceteris_econ::equilibrium::Equilibrium;
use ceteris_econ::fixture::small_world;
use ceteris_econ::systems::{SEED, schedule};
use ceteris_econ::{EconParams, EconWorld};
use ceteris_trade::{PriceAnchor, TradeParams};

const E3: PriceAnchor = PriceAnchor {
    kappa_r_traded: 0.25,
    kappa_r_local: 0.05,
    kappa_b: 0.3,
    kappa_m: 0.2,
    e3: true,
};

#[test]
#[ignore]
fn baseline() {
    let trade = TradeParams { anchor: Some(E3), ..TradeParams::default() };
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/p2-fixture/reference");
    let mut w = EconWorld::new(Arc::new(small_world()), EconParams::frictionless(), trade);
    w.settle(&Equilibrium::read(&dir).unwrap());
    let (i0, y0) = (w.world_price_index(), w.output.iter().sum::<f64>());
    let mut wedges = ceteris_trade::network::Wedges::none(w.inputs().n_countries());
    wedges.ntb = vec![10.0; wedges.tariff.len()];
    w.set_wedges(wedges);
    let mut sim = Sim::new(w, schedule(), SEED);
    let mut imax = 1.0f64;
    for t in 1..=1500 {
        sim.run_until(t).unwrap();
        let w = sim.world();
        let i = w.world_price_index() / i0;
        imax = imax.max(i);
        if t % 300 == 0 {
            let inp = w.inputs();
            let acc = w.layout().accounts.clone();
            let nc = inp.n_countries();
            let mut pop = vec![0.0; nc];
            let mut ent = vec![0.0; nc];
            for k in 0..inp.n_pops() {
                pop[inp.country_of(inp.pops.province[k] as usize)] += ceteris_econ::world::to_numeraire(w.ledger.money(acc.pop[k]));
            }
            for e in 0..inp.n_enterprises() {
                ent[inp.country_of(inp.enterprises.province[e] as usize)] += ceteris_econ::world::to_numeraire(w.ledger.money(acc.enterprise[e]));
            }
            eprintln!("  pop {pop:?} ent {ent:?}");
        }
        if t % 100 == 0 || t < 5 {
            eprintln!("t {t} index {i:.4} max {imax:.4} y {:.4}", w.output.iter().sum::<f64>() / y0);
        }
    }
}

fn country_money(w: &EconWorld) -> Vec<f64> {
    let inp = w.inputs();
    let acc = &w.layout().accounts;
    let mut m = vec![0.0; inp.n_countries()];
    for k in 0..inp.n_pops() {
        m[inp.country_of(inp.pops.province[k] as usize)] += ceteris_econ::world::to_numeraire(w.ledger.money(acc.pop[k]));
    }
    for e in 0..inp.n_enterprises() {
        m[inp.country_of(inp.enterprises.province[e] as usize)] += ceteris_econ::world::to_numeraire(w.ledger.money(acc.enterprise[e]));
    }
    for i in 0..inp.n_provinces() {
        m[inp.country_of(i)] += ceteris_econ::world::to_numeraire(w.ledger.money(acc.market[i]));
    }
    m
}

/// PROXY spike: q_c accumulates the country's money change (external balance); a deficit
/// country's imports carry an ad valorem wedge gamma * max(0, -q_c / M_c0).
#[test]
#[ignore]
fn proxy() {
    let gamma: f64 = std::env::var("G").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let trade = TradeParams { anchor: Some(E3), ..TradeParams::default() };
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/p2-fixture/reference");
    let mut w = EconWorld::new(Arc::new(small_world()), EconParams::frictionless(), trade);
    w.settle(&Equilibrium::read(&dir).unwrap());
    let (i0, y0) = (w.world_price_index(), w.output.iter().sum::<f64>());
    let m0 = country_money(&w);
    let nc = m0.len();
    let mbar = m0.iter().sum::<f64>() / nc as f64;
    let mut prev = m0.clone();
    let mut q = vec![0.0; nc];
    let mut wedges = ceteris_trade::network::Wedges::none(nc);
    wedges.ntb = vec![10.0; wedges.tariff.len()];
    w.set_wedges(wedges.clone());
    let mut sim = Sim::new(w, schedule(), SEED);
    let mut imax = 1.0f64;
    for t in 1..=1500 {
        sim.run_until(t).unwrap();
        let m = country_money(sim.world());
        for c in 0..nc { q[c] += m[c] - prev[c]; }
        prev = m;
        let mut wd = wedges.clone();
        for c in 0..nc {
            let tau = (gamma * (-q[c] / mbar)).max(0.0) as f32;
            for o in 0..nc { if o != c { for g in 0..2 { wd.tariff[(c * nc + o) * 2 + g] = tau; } } }
        }
        sim.world_mut().set_wedges(wd);
        let w = sim.world();
        let i = w.world_price_index() / i0;
        imax = imax.max(i);
        if t % 300 == 0 {
            eprintln!("t {t} index {i:.4} max {imax:.4} y {:.4} q/M0 {:?}", w.output.iter().sum::<f64>() / y0, q.iter().zip(&m0).map(|(a,b)| a/b.abs().max(1.0)).collect::<Vec<_>>());
        }
    }
    eprintln!("RESULT gamma {gamma} max index {imax:.4}");
}
