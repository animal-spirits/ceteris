//! Properties of the P2 tick pipeline on the small-world fixture (`specs/econ/SPEC.md`,
//! Invariants and Test plan).

use std::sync::Arc;

use ceteris_core::sim::{Sim, State};
use ceteris_econ::fixture::small_world;
use ceteris_econ::inputs::EconInputs;
use ceteris_econ::local::N_LOCAL;
use ceteris_econ::params::Technology;
use ceteris_econ::production::{rho, value_added};
use ceteris_econ::systems::{self, SEED};
use ceteris_econ::world::{
    MINOR_PER_UNIT, MINT, PURCHASE, TARIFF, TRADE, active_cells, static_digest, to_minor,
};
use ceteris_econ::{EconParams, EconWorld};
use ceteris_trade::goods::TRADED;
use ceteris_trade::source::{HOUSEHOLD, INTERMEDIATE};
use ceteris_trade::{N_GOODS, Rationing, Sourced, TradeParams, dispatch, source};
use proptest::prelude::*;

fn world(params: EconParams) -> EconWorld {
    EconWorld::new(Arc::new(small_world()), params, TradeParams::default())
}

fn sim_of(world: EconWorld) -> Sim<EconWorld> {
    Sim::new(world, systems::schedule(), SEED)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(6))]
    #[test]
    fn conservation_over_120_ticks(
        factors in proptest::collection::vec(0.5f64..2.0, 25 * N_GOODS),
        frictionless in any::<bool>(),
    ) {
        let params = if frictionless { EconParams::frictionless() } else { EconParams::default() };
        let mut w = world(params);
        for (p, f) in w.markets.price.iter_mut().zip(&factors) {
            *p *= f;
        }
        let mut sim = sim_of(w);
        let run = sim.run_until(120);
        prop_assert!(run.is_ok(), "{}", run.unwrap_err());
        let w = sim.world();
        prop_assert!(w.markets.price.iter().all(|p| p.is_finite() && *p > 0.0));
        // all money is back with the pops at the end of every tick
        let acc = &w.layout().accounts;
        for a in acc.enterprise.iter().chain(&acc.market).chain(&acc.rebate) {
            prop_assert_eq!(w.ledger.money(*a), 0);
        }
    }
}

#[test]
fn sim_runs_the_schedule_from_the_data_state() {
    let mut sim = EconWorld::sim(
        Arc::new(small_world()),
        EconParams::default(),
        TradeParams::default(),
    );
    sim.run_until(3).unwrap();
    assert_eq!(sim.tick(), 3);
    assert!(sim.world().output.iter().any(|&y| y > 0.0));
}

#[test]
fn employment_within_bounds() {
    for params in [EconParams::default(), EconParams::frictionless()] {
        let mut sim = sim_of(world(params));
        let mut moved = false;
        for _ in 0..120 {
            sim.step().unwrap();
            let w = sim.world();
            let (inp, lay) = (w.inputs(), w.layout());
            for k in 0..inp.n_pops() {
                let (e, lf) = (w.employed[k], inp.pops.labour_force[k]);
                assert!(e >= 0.0 && e <= lf * (1.0 + 1e-12), "pop {k}: {e} of {lf}");
            }
            for i in 0..inp.n_provinces() {
                let l: f64 = lay.enterprises(i).map(|e| w.employment[e]).sum();
                assert!(lay.enterprises(i).all(|e| w.employment[e] >= 0.0));
                assert!(l <= lay.labour_supply[i] * (1.0 + 1e-12), "province {i}");
                let l0: f64 = lay.enterprises(i).map(|e| inp.enterprises.l0[e]).sum();
                moved |= (l / l0 - 1.0).abs() > 1e-6;
            }
            assert!(w.wage.iter().all(|x| x.is_finite() && *x > 0.0));
        }
        assert!(moved, "employment never left the base point");
    }
}

/// Multiplies prices, the price index, the last world price level, wages and every pop's
/// money by `scale`; panics on ledger overflow. Expected inflation is a log difference and
/// stays.
fn scaled(mut w: EconWorld, scale: i64) -> EconWorld {
    let s = scale as f64;
    w.markets.price.iter_mut().for_each(|p| *p *= s);
    w.price_index.iter_mut().for_each(|p| *p *= s);
    w.price_level *= s;
    w.wage.iter_mut().for_each(|x| *x *= s);
    let acc = w.layout().accounts.clone();
    for &pop in &acc.pop {
        let extra = w.ledger.money(pop) * (scale - 1);
        w.ledger.transfer(MINT, acc.mint, pop, extra);
    }
    w
}

/// Laspeyres index over active cells, weights Omega_g times province population.
fn price_index(w: &EconWorld) -> f64 {
    let inp = w.inputs();
    let (mut num, mut den) = (0.0, 0.0);
    for i in 0..inp.n_provinces() {
        let omega = &inp.countries.omega[inp.country_of(i)];
        for (g, &share) in omega.iter().enumerate() {
            let weight = share * inp.provinces.population[i];
            num += weight * w.markets.price[i * N_GOODS + g];
            den += weight;
        }
    }
    num / den
}

/// Equal within 1e-9 of `size[i]` (the value itself, or a larger magnitude it is computed
/// from) plus 1e-12 of the column's largest value.
fn assert_same(what: &str, a: &[f64], b: &[f64], size: &[f64]) {
    assert_eq!(a.len(), b.len());
    let scale = a.iter().chain(b).fold(0.0f64, |m, x| m.max(x.abs()));
    for (i, (x, y)) in a.iter().zip(b).enumerate() {
        let tol = 1e-9 * x.abs().max(y.abs()).max(size[i]) + 1e-12 * scale;
        assert!((x - y).abs() <= tol, "{what}[{i}]: {x} vs {y}");
    }
}

/// Real quantities, each with the magnitude its comparison is relative to.
fn real_state(w: &EconWorld) -> Vec<(&'static str, Vec<f64>, Vec<f64>)> {
    let acc = &w.layout().accounts;
    let m = &w.markets;
    let index = price_index(w);
    let stocks = acc
        .enterprise
        .iter()
        .flat_map(|&a| (0..N_GOODS).map(move |g| w.ledger.goods(a, g)))
        .collect();
    let in_flight = (0..N_GOODS)
        .filter(|&g| TRADED[g])
        .map(|g| w.pipeline.in_flight(g))
        .collect();
    let columns: Vec<(&'static str, Vec<f64>)> = vec![
        ("employment", w.employment.clone()),
        ("output", w.output.clone()),
        ("input stocks", stocks),
        ("supply", m.supply.clone()),
        ("sales", m.sales.clone()),
        ("in flight", in_flight),
        (
            "relative price",
            m.price.iter().map(|p| p / index).collect(),
        ),
        ("relative wage", w.wage.iter().map(|x| x / index).collect()),
    ];
    let mut out: Vec<_> = columns
        .into_iter()
        .map(|(what, v)| (what, v.clone(), v))
        .collect();
    // S' = S + (Q - D) loses relative precision when the stock is small against its flows
    let turnover = (0..m.inventory.len())
        .map(|c| m.inventory[c] + m.supply[c] + m.demand[c])
        .collect();
    out.push(("inventory", m.inventory.clone(), turnover));
    out
}

#[test]
fn homogeneity_of_degree_zero() {
    for params in [EconParams::default(), EconParams::frictionless()] {
        let mut base = sim_of(world(params.clone()));
        let mut big = sim_of(scaled(world(params), 10_000));
        // 120 ticks: the fixture's dynamics are stable (Task 7b) and the rounding gap between
        // the two runs is about 1e-11, except that under PIGL demand (MIGRATION Task 7) the
        // default run passes a transient between ticks 40 and 90 where it peaks at 7.5e-9
        // (the island's sales and input stocks), back to 2e-10 by tick 100
        base.run_until(120).unwrap();
        big.run_until(120).unwrap();
        // 1e-9, not bit for bit: payments round half to even to minor units and 1e4 is not
        // a power of two, so nominal roundings differ between the two runs
        for ((what, a, size), (_, b, _)) in real_state(base.world())
            .into_iter()
            .zip(real_state(big.world()))
        {
            assert_same(what, &a, &b, &size);
        }
    }
}

/// pi^e <- pi^e + g (ln(I_t / I_{t-1}) - pi^e) after T4, I the world price index at the
/// buyer prices of the tick (ADR 0019).
#[test]
fn expected_inflation_follows_the_world_index() {
    let mut sim = sim_of(world(EconParams::default()));
    let g = sim.world().trade().expectation_gain;
    assert!(g > 0.0 && g < 1.0);
    let mut level = sim.world().world_price_index();
    assert_eq!(sim.world().price_level, level);
    assert_eq!(sim.world().expected_inflation, 0.0);
    let mut pi = 0.0;
    for tick in 0..36 {
        sim.step().unwrap();
        let w = sim.world();
        let now = w.world_price_index();
        pi += g * (libm::log(now / level) - pi);
        assert_eq!(w.price_level, now, "tick {tick}");
        assert!((w.expected_inflation - pi).abs() <= 1e-15, "tick {tick}");
        level = now;
    }
    assert!(pi.abs() > 1e-6, "the data state never moved the index");
}

/// Expected inflation above c_max makes every price step and every nominal wage step bind;
/// a Walrasian wage has no step to clip.
#[test]
fn world_counts_clip_bindings() {
    for params in [EconParams::default(), EconParams::frictionless()] {
        let frictionless = params.frictionless;
        let mut w = world(params);
        w.expected_inflation = 10.0;
        let mut sim = sim_of(w);
        sim.step().unwrap();
        let w = sim.world();
        for (c, &active) in w.markets.active.iter().enumerate() {
            assert_eq!(w.markets.clip_ticks[c], u32::from(active), "cell {c}");
        }
        let want = u32::from(!frictionless);
        assert!(w.wage_clip_ticks.iter().all(|&n| n == want));
    }
    let mut sim = sim_of(world(EconParams::default()));
    sim.run_until(120).unwrap();
    let w = sim.world();
    let counts = w.markets.clip_ticks.iter().chain(&w.wage_clip_ticks);
    assert!(counts.clone().all(|&n| n <= 120));
    assert!(counts.clone().any(|&n| n > 0), "the data state never clips");
}

#[test]
#[should_panic(expected = "ledger overflow")]
fn nominal_scale_beyond_the_ledger_fails_loudly() {
    let mut sim = sim_of(scaled(world(EconParams::default()), 1_000_000));
    sim.run_until(1).unwrap();
}

/// Runs T1 to T3 of a tick: everything up to the dispatch of the filled orders.
fn up_to_dispatch(w: &mut EconWorld) {
    w.begin_tick();
    systems::deliver(w);
    systems::labour(w);
    systems::produce(w);
    systems::demand(w);
    systems::source(w);
    systems::clear(w);
}

/// Runs T2b to E4, then checks the invariants.
fn from_dispatch(w: &mut EconWorld, tick: u32) {
    systems::dispatch(w);
    systems::reprice(w);
    systems::distribute(w);
    w.end_tick(tick).unwrap();
}

/// A traded cell has a market only where an enterprise of the province produces the good:
/// nobody orders there, nothing is stocked or sold there, and its price never moves, while
/// the province's own buyers still buy the good from other origins.
#[test]
fn origins_without_capacity_have_no_market() {
    let mut w = world(EconParams::default());
    let inp = w.inputs().clone();
    let np = inp.n_provinces();
    let produces = |i: usize, g: usize| {
        (0..inp.n_enterprises()).any(|e| {
            inp.enterprises.province[e] as usize == i
                && usize::from(inp.enterprises.good[e]) == g
                && inp.enterprises.a[e] * inp.enterprises.va0[e] > 0.0
        })
    };
    let mut without: Vec<usize> = vec![];
    for i in 0..np {
        for g in (0..N_GOODS).filter(|&g| TRADED[g]) {
            let cell = i * N_GOODS + g;
            assert_eq!(w.markets.active[cell], produces(i, g), "cell {cell}");
            assert_eq!(active_cells(&inp)[cell], w.markets.active[cell]);
            if !produces(i, g) {
                without.push(cell);
            }
        }
    }
    let buyers_without_market = without
        .iter()
        .filter(|&&c| inp.countries.omega[inp.country_of(c / N_GOODS)][c % N_GOODS] > 0.0)
        .count();
    assert!(
        buyers_without_market > 0,
        "no province buys a good it does not produce"
    );

    let before = w.markets.price.clone();
    let acc = w.layout().accounts.clone();
    for tick in 0..6 {
        up_to_dispatch(&mut w);
        let mut bought = 0.0;
        for &cell in &without {
            let (i, g) = (cell / N_GOODS, cell % N_GOODS);
            let m = &w.markets;
            assert_eq!(
                (m.demand_int[cell], m.demand_hh[cell]),
                (0.0, 0.0),
                "cell {cell}"
            );
            assert_eq!(
                (m.sales[cell], m.inventory[cell]),
                (0.0, 0.0),
                "cell {cell}"
            );
            assert_eq!(w.ledger.goods(acc.market[i], g), 0.0);
            bought += w.scratch.class_spend[cell][HOUSEHOLD];
        }
        assert!(
            bought > 0.0,
            "tick {tick}: buyers without a local market spend nothing"
        );
        from_dispatch(&mut w, tick);
        for &cell in &without {
            assert_eq!(w.markets.price[cell].to_bits(), before[cell].to_bits());
        }
    }
}

/// T2b in minor units: each buyer pays its destination market, per good, its share of its
/// class's filled spending, origin price times (1 + t) per shipped unit, rounded half to
/// even; markets pass the payments on to origins, gateways and rebate accounts, and gateway
/// accounts net to zero.
#[test]
fn buyers_pay_origin_price_at_dispatch_in_minor_units() {
    for params in [EconParams::default(), EconParams::frictionless()] {
        let mut w = world(params);
        for tick in 0..4 {
            up_to_dispatch(&mut w);
            let inp = w.inputs().clone();
            let (lay, kernel) = (w.layout().clone(), w.kernel().clone());
            let (np, n_pops) = (inp.n_provinces(), inp.n_pops());
            let start = w.ledger.journal().len();

            // the filled value of every buyer, from its own sourcing of the class spending
            let mut want = vec![0i64; n_pops + inp.n_enterprises()];
            let (mut home_value, mut tariff_due, mut sourced) = (0.0, 0.0, Sourced::new());
            for g in (0..N_GOODS).filter(|&g| TRADED[g]) {
                let cells = || (0..np).map(|d| d * N_GOODS + g);
                let price: Vec<f64> = cells().map(|c| w.markets.price[c]).collect();
                let active: Vec<bool> = cells().map(|c| w.markets.active[c]).collect();
                let spend: Vec<[f64; 2]> = cells().map(|c| w.scratch.class_spend[c]).collect();
                let fill: Vec<[f64; 2]> = cells().map(|c| w.scratch.fill[c]).collect();
                source(&kernel, g, &price, &active, &spend, &mut sourced);
                dispatch(&kernel, g, &price, &fill, &mut sourced);
                for d in 0..np {
                    for c in [INTERMEDIATE, HOUSEHOLD] {
                        assert!(sourced.paid_by_class[d][c] <= spend[d][c] * (1.0 + 1e-12));
                    }
                    home_value += price[d] * (sourced.arrive[0][d][0] + sourced.arrive[0][d][1]);
                    let row = |e: usize| w.scratch.ent_spend[e * N_GOODS + g];
                    let paid = |c: usize, x: f64| {
                        if x > 0.0 {
                            to_minor(sourced.paid_by_class[d][c] * (x / spend[d][c]))
                        } else {
                            0
                        }
                    };
                    for e in lay.enterprises(d) {
                        want[n_pops + e] += paid(INTERMEDIATE, row(e));
                    }
                    for k in lay.pops(d) {
                        want[k] += paid(HOUSEHOLD, w.scratch.pop_spend[k * N_GOODS + g]);
                    }
                }
                let gk = kernel.good(g);
                for a in 0..kernel.n_gateways {
                    let lo = gk.crossings.offsets[a] as usize;
                    for (j, e) in gk.crossings.row(a).iter().enumerate() {
                        tariff_due += f64::from(e.tariff) * sourced.crossing_value[lo + j];
                    }
                }
            }

            systems::dispatch(&mut w);
            let acc = &lay.accounts;
            let buyer = |a| acc.pop.iter().chain(&acc.enterprise).position(|&b| b == a);
            let market = |a| acc.market.iter().position(|&m| m == a);
            let mut got = vec![0i64; want.len()];
            let (mut passed, mut tariff, mut received) = (0i64, 0i64, 0i64);
            for e in &w.ledger.journal()[start..] {
                if e.flow == PURCHASE {
                    got[buyer(e.from).expect("buyers pay")] += e.amount;
                    assert!(market(e.to).is_some(), "buyers pay their market");
                } else if e.flow == TRADE {
                    if market(e.from).is_some() {
                        passed += e.amount;
                    }
                    if market(e.to).is_some() {
                        received += e.amount;
                    }
                } else if e.flow == TARIFF {
                    assert!(market(e.from).is_some(), "markets pay tariffs");
                    tariff += e.amount;
                }
            }
            assert_eq!(got, want, "tick {tick}: buyer payments");
            for &g in &acc.gateway {
                assert_eq!(w.ledger.money(g), 0, "tick {tick}: gateway nets to zero");
            }
            // markets pass on what their buyers paid, less home purchases; origins receive
            // it net of tariff; at most half a minor unit per payment and leg apart
            let total: i64 = got.iter().sum();
            let legs = (w.ledger.journal().len() - start) as f64;
            let near = |a: f64, b: f64| (a - b).abs() <= 0.5 * legs + 1e-9 * b.abs();
            let home = home_value * MINOR_PER_UNIT;
            assert!(
                near((passed + tariff) as f64, total as f64 - home),
                "tick {tick}"
            );
            assert!(near(received as f64, total as f64 - home - tariff as f64));
            assert!(
                near(tariff as f64, tariff_due * MINOR_PER_UNIT),
                "tick {tick}: tariff"
            );

            systems::reprice(&mut w);
            systems::distribute(&mut w);
            w.end_tick(tick).unwrap();
        }
    }
}

/// Capacity output VA(L) / a_VA of enterprise `e` at its current employment.
fn capacity(w: &EconWorld, e: usize) -> f64 {
    let (inp, lay) = (w.inputs(), w.layout());
    let ent = &inp.enterprises;
    let c = inp.country_of(ent.province[e] as usize);
    let va = value_added(
        ent.a[e],
        ent.va0[e],
        ent.l0[e],
        inp.countries.s_l[c],
        rho(w.params().sigma_fl),
        w.employment[e],
    );
    va / lay.a_va[e]
}

/// The order rule of E3 for every enterprise and input: up to an inventory position of
/// (1 + COVER + L) coef_k Y_cap, L the expected months in transit to the province,
/// (target - stock - on order) at least zero; returns the orders that bind at zero only
/// because of goods on order.
fn assert_order_rule(w: &EconWorld) -> usize {
    let (inp, lay) = (w.inputs(), w.layout());
    let mut held_back = 0;
    for e in 0..inp.n_enterprises() {
        let i = inp.enterprises.province[e] as usize;
        for (k, &traded) in TRADED.iter().enumerate() {
            let a = lay.coef[e][k];
            let stock = w.ledger.goods(lay.accounts.enterprise[e], k);
            let on_order = w.pipeline.on_order(inp.n_pops() + e, k);
            let lead = w.scratch.lead[i * N_GOODS + k];
            assert!(traded || lead == 0.0);
            let gap = (1.0 + systems::COVER + lead) * a * capacity(w, e) - stock;
            let want = if a > 0.0 {
                (gap - on_order).max(0.0)
            } else {
                0.0
            };
            let got = w.scratch.ent_order[e * N_GOODS + k];
            assert!(
                (got - want).abs() <= 1e-12 * want.max(1.0),
                "enterprise {e} good {k}: order {got}, want {want}"
            );
            held_back += usize::from(a > 0.0 && want == 0.0 && gap > 0.0);
        }
    }
    held_back
}

/// An enterprise whose pipeline already holds its whole target position of an input orders
/// none of it; with half a month on order it orders the rest of the gap.
#[test]
fn input_orders_count_goods_on_order() {
    let mut w = world(EconParams::default());
    w.begin_tick();
    systems::deliver(&mut w);
    systems::labour(&mut w);
    systems::produce(&mut w);
    let (inp, lay) = (w.inputs().clone(), w.layout().clone());
    let acc = &lay.accounts;
    let (full, half) = (0, 1);
    let mut checked = 0;
    // the lead E3 will use: the prices do not move between here and E3
    w.refresh_price_index();
    for (e, months) in [(full, None), (half, Some(0.5))] {
        let i = inp.enterprises.province[e] as usize;
        for k in (0..N_GOODS).filter(|&k| TRADED[k] && lay.coef[e][k] > 0.0) {
            let target = 1.0 + systems::COVER + w.scratch.lead[i * N_GOODS + k];
            let units = months.unwrap_or(target) * lay.coef[e][k] * capacity(&w, e);
            // in flight over three slots; the goods sit in the transit account
            for slot in 2..=4 {
                w.pipeline.push(slot, inp.n_pops() + e, k, units / 3.0);
            }
            w.ledger.create(acc.transit[k], k, units);
            checked += 1;
        }
    }
    assert!(checked >= 2, "enterprises 0 and 1 use no traded input");
    systems::demand(&mut w);
    assert!(
        assert_order_rule(&w) > 0,
        "no order held back by goods on order"
    );
    for k in (0..N_GOODS).filter(|&k| TRADED[k] && lay.coef[full][k] > 0.0) {
        assert_eq!(w.scratch.ent_order[full * N_GOODS + k], 0.0);
        assert_eq!(w.scratch.ent_spend[full * N_GOODS + k], 0.0);
    }
}

/// Input orders plan on capacity output VA(L) / a_VA at this tick's employment, not on output
/// cut by a short input (owner decision, Task 7b), less the goods already on order.
#[test]
fn input_orders_plan_on_capacity_output() {
    let mut w = world(EconParams::default());
    // starve enterprise 0 of every input, so its output is cut far below capacity
    let account = w.layout().accounts.enterprise[0];
    for k in 0..N_GOODS {
        let held = w.ledger.goods(account, k);
        if held > 0.0 {
            w.ledger.destroy(account, k, 0.5 * held);
        }
    }
    for tick in 0..3 {
        w.begin_tick();
        systems::deliver(&mut w);
        systems::labour(&mut w);
        systems::produce(&mut w);
        systems::demand(&mut w);
        let binding = (0..w.inputs().n_enterprises())
            .filter(|&e| w.output[e] < 0.9 * capacity(&w, e))
            .count();
        assert!(tick > 0 || binding > 0, "no enterprise was input-bound");
        assert_order_rule(&w);
        systems::source(&mut w);
        systems::clear(&mut w);
        from_dispatch(&mut w, tick);
    }
}

/// Buyers price a traded good at the delivered unit price q_d = 1 / alpha_d: spending q_d at
/// any destination, sourced at the current prices and fully filled, delivers one unit, and
/// the lead is the mean arrival slot of those units. Local goods keep their posted price.
#[test]
fn buyers_price_traded_goods_at_the_delivered_unit_price() {
    let mut sim = sim_of(world(EconParams::default()));
    sim.run_until(5).unwrap();
    let mut w = sim.world().clone();
    w.refresh_price_index();
    let (np, kernel) = (w.inputs().n_provinces(), w.kernel().clone());
    let mut sourced = Sourced::new();
    let mut leads = 0;
    for (g, &traded) in TRADED.iter().enumerate() {
        if !traded {
            for d in 0..np {
                assert_eq!(
                    w.price_index[d * N_GOODS + g],
                    w.markets.price[d * N_GOODS + g]
                );
            }
            continue;
        }
        let cells = || (0..np).map(|o| o * N_GOODS + g);
        let price: Vec<f64> = cells().map(|c| w.markets.price[c]).collect();
        let active: Vec<bool> = cells().map(|c| w.markets.active[c]).collect();
        for d in 0..np {
            let q = w.price_index[d * N_GOODS + g];
            assert!(q.is_finite() && q > 0.0);
            let mut spend = vec![[0.0; 2]; np];
            spend[d] = [0.0, q];
            source(&kernel, g, &price, &active, &spend, &mut sourced);
            dispatch(&kernel, g, &price, &vec![[1.0; 2]; np], &mut sourced);
            let units: Vec<f64> = sourced.arrive.iter().map(|a| a[d][HOUSEHOLD]).collect();
            let total: f64 = units.iter().sum();
            assert!(
                (total - 1.0).abs() < 1e-12,
                "cell {}: {total}",
                d * N_GOODS + g
            );
            let lead: f64 = units.iter().enumerate().map(|(s, x)| s as f64 * x).sum();
            assert!((lead - w.scratch.lead[d * N_GOODS + g]).abs() < 1e-12);
            // q_d is at least the cheapest delivered cost, so above the CES index P_d
            assert!(q >= sourced.price_index[d] * (1.0 - 1e-12));
            leads += usize::from(lead > 1.0);
        }
    }
    assert!(
        leads > 0,
        "no destination waits more than a month on average"
    );
}

const COAL: usize = 2;
const CONSTRUCTION: usize = 8;
const SERVICES: usize = 9;
const LOCAL: [usize; 3] = [7, 8, 9];

/// The fixture with input coefficients `(k, g, a_kg)` set in every country, one vintage.
fn fixture_with(coefficients: &[(usize, usize, f64)]) -> EconInputs {
    let mut inputs = small_world();
    for io in &mut inputs.countries.io {
        for &(k, g, a) in coefficients {
            io[k * N_GOODS + g] = a;
        }
    }
    inputs.set_coefficients();
    inputs.validate();
    inputs
}

/// The enterprise of province `i` producing `g`.
fn producer(w: &EconWorld, i: usize, g: usize) -> usize {
    let inp = w.inputs();
    w.layout()
        .enterprises(i)
        .find(|&e| usize::from(inp.enterprises.good[e]) == g)
        .expect("a producer")
}

/// One tick through the systems, with `hook` run right after E1 labour.
fn step_with(w: &mut EconWorld, tick: u32, hook: impl Fn(&mut EconWorld)) {
    w.begin_tick();
    systems::deliver(w);
    systems::labour(w);
    hook(w);
    systems::produce(w);
    systems::demand(w);
    systems::source(w);
    systems::clear(w);
    from_dispatch(w, tick);
}

/// Destroys the local input stocks (the chain's buffers) of every enterprise of province `i`.
fn drain_local_buffers(w: &mut EconWorld, i: usize) {
    let lay = w.layout().clone();
    for e in lay.enterprises(i) {
        for k in LOCAL {
            let held = w.ledger.goods(lay.accounts.enterprise[e], k);
            if held > 0.0 {
                w.ledger.destroy(lay.accounts.enterprise[e], k, held);
            }
        }
    }
}

/// Services feeding every sector at 0.3, so that intermediate users take most of services
/// output, and the first tick from the data state in two runs, both with the local buffers
/// of province 0 drained: control, and cut (half of its services labour). The cut is at tick
/// 0, not 10: at tick 10 the data state's traded shortage caps services output by its
/// traded input stocks, so its labour does not bind.
fn services_shortage() -> (EconWorld, EconWorld) {
    let coefficients: Vec<_> = (0..N_GOODS).map(|g| (SERVICES, g, 0.3)).collect();
    let w = EconWorld::new(
        Arc::new(fixture_with(&coefficients)),
        EconParams::default(),
        TradeParams::default(),
    );
    let services = producer(&w, 0, SERVICES);
    let mut control = w.clone();
    step_with(&mut control, 0, |w| drain_local_buffers(w, 0));
    let mut cut = w;
    step_with(&mut cut, 0, |w| {
        drain_local_buffers(w, 0);
        w.employment[services] *= 0.5;
    });
    (control, cut)
}

/// ADR 0020: a shortage of a local good reaches its users' output within the tick.
#[test]
fn local_shortage_reaches_downstream_same_tick() {
    let (control, cut) = services_shortage();
    let (services, construction) = (
        producer(&control, 0, SERVICES),
        producer(&control, 0, CONSTRUCTION),
    );
    assert!(cut.output[services] < 0.7 * control.output[services]);
    // without the cut, services covers its users from this tick's output
    assert_eq!(control.scratch.chain_iterations[0], 1);
    assert!(cut.scratch.chain_iterations[0] > 1);
    let (was, now) = (control.output[construction], cut.output[construction]);
    assert!(now < 0.95 * was, "construction {now} against {was}");
}

/// Intermediate users of a local good are served before households: in the shortage they
/// draw all of services output and households get none; without it households buy the rest.
/// At origin markets the default Priority fills intermediate orders before households.
#[test]
fn local_chain_respects_intermediate_priority() {
    let (control, cut) = services_shortage();
    let cell = SERVICES;
    let households = |w: &EconWorld| -> f64 {
        w.layout()
            .pops(0)
            .map(|k| w.scratch.pop_demand[k * N_GOODS + SERVICES])
            .sum()
    };
    for w in [&control, &cut] {
        assert!(households(w) > 0.0);
        assert!(w.scratch.drawn[cell] <= w.markets.supply[cell] * (1.0 + 1e-12));
    }
    let to_households = |w: &EconWorld| w.markets.sales[cell] - w.scratch.drawn[cell];
    assert!(to_households(&control) > 0.0);
    let supply = cut.markets.supply[cell];
    assert!((cut.scratch.drawn[cell] - supply).abs() <= 1e-12 * supply);
    assert!(to_households(&cut) <= 1e-12 * supply);

    let mut sim = sim_of(world(EconParams::default()));
    let mut short = 0;
    for _ in 0..36 {
        sim.step().unwrap();
        let w = sim.world();
        for (c, f) in w.scratch.fill.iter().enumerate() {
            if TRADED[c % N_GOODS] && w.markets.active[c] && f[0] < 1.0 {
                let none = if w.markets.demand_hh[c] > 0.0 {
                    0.0
                } else {
                    1.0
                };
                assert_eq!(f[1], none, "cell {c}: {f:?}");
                short += 1;
            }
        }
    }
    assert!(short > 0, "no intermediate shortage at an origin");
}

/// A traded input reaches its users after transit: cutting coal output leaves downstream
/// output unchanged in the tick of the cut, and lowers it once input stocks run out.
#[test]
fn traded_shortage_still_lags() {
    let mut sim = sim_of(world(EconParams::default()));
    sim.run_until(10).unwrap();
    let (mut control, mut cut) = (sim.world().clone(), sim.world().clone());
    let inp = control.inputs().clone();
    let coal: Vec<usize> = (0..inp.n_enterprises())
        .filter(|&e| usize::from(inp.enterprises.good[e]) == COAL)
        .collect();
    let users: Vec<usize> = (0..inp.n_enterprises())
        .filter(|&e| !coal.contains(&e) && control.layout().coef[e][COAL] > 0.0)
        .collect();
    let mut lower = None;
    for tick in 10..48 {
        step_with(&mut control, tick, |_| {});
        step_with(&mut cut, tick, |w| {
            coal.iter().for_each(|&e| w.employment[e] = 0.0)
        });
        if tick == 10 {
            assert!(coal.iter().all(|&e| cut.output[e] == 0.0));
            for &e in &users {
                assert_eq!(cut.output[e].to_bits(), control.output[e].to_bits());
            }
        } else if lower.is_none()
            && users
                .iter()
                .any(|&e| cut.output[e] < 0.99 * control.output[e])
        {
            lower = Some(tick);
        }
    }
    assert!(lower.is_some(), "downstream output never fell");
}

/// Review Focus 5: services feeding services, and construction and services feeding each
/// other. From the data state the chain binds in every mainland province at tick 1 and
/// converges there within its bound; over 120 ticks every local input used equals coef Y
/// (the Leontief invariant, exactly, at every tick), including the province ticks where a
/// cycle between two local goods is still converging at the bound and the chain clears the
/// rest by cutting the users that produce no local good.
#[test]
fn local_chain_terminates_on_cycles() {
    let coefficients = [
        (SERVICES, SERVICES, 0.3),
        (SERVICES, CONSTRUCTION, 0.2),
        (CONSTRUCTION, SERVICES, 0.2),
    ];
    let inputs = Arc::new(fixture_with(&coefficients));
    let mut sim = EconWorld::sim(inputs, EconParams::default(), TradeParams::default());
    let (mut bound, mut at_bound) = (0, 0);
    for tick in 0..120 {
        sim.step().unwrap_or_else(|v| panic!("{v}"));
        let w = sim.world();
        let iterations = &w.scratch.chain_iterations;
        assert!(
            iterations
                .iter()
                .all(|&n| (1..=N_LOCAL as u32).contains(&n))
        );
        if tick == 1 {
            assert!(
                iterations[..24]
                    .iter()
                    .all(|&n| n > 1 && n < N_LOCAL as u32)
            );
        }
        bound += iterations.iter().filter(|&&n| n > 1).count();
        at_bound += iterations.iter().filter(|&&n| n == N_LOCAL as u32).count();
        for e in 0..w.inputs().n_enterprises() {
            for k in LOCAL {
                let want = w.layout().coef[e][k] * w.scratch.produced[e];
                let used = w.scratch.input_used[e * N_GOODS + k];
                assert!(
                    (used - want).abs() <= 1e-12 * want,
                    "tick {tick} enterprise {e} good {k}"
                );
            }
        }
    }
    println!("chain bound in {bound} province ticks, {at_bound} of them at the bound");
    assert!(bound > 0 && at_bound < bound);
}

/// S11 Proportional: one fill ratio for both classes at every short origin market.
#[test]
fn proportional_rationing_switch() {
    let params = EconParams {
        rationing: Rationing::Proportional,
        ..EconParams::default()
    };
    let mut sim = sim_of(world(params));
    let mut short = 0;
    for _ in 0..36 {
        sim.step().unwrap();
        let w = sim.world();
        for (c, f) in w.scratch.fill.iter().enumerate() {
            if TRADED[c % N_GOODS] && w.markets.active[c] && f != &[1.0, 1.0] {
                assert_eq!(f[0], f[1], "cell {c}");
                short += 1;
            }
        }
    }
    assert!(short > 0, "no shortage at an origin");
}

/// ADR 0020: with one vintage (the 2000 ICIO column) putty-clay coefficients run exactly as
/// fixed Leontief (S09), down to the state hash; a second vintage would tell them apart.
#[test]
fn putty_clay_one_vintage_equals_fixed_leontief() {
    let with = |technology| EconParams {
        technology,
        ..EconParams::default()
    };
    let (putty, fixed) = (with(Technology::PuttyClay), with(Technology::FixedLeontief));
    let mut a = sim_of(world(putty.clone()));
    let mut b = sim_of(world(fixed.clone()));
    assert_eq!(a.world().digest(), b.world().digest());
    for tick in 0..120 {
        a.step().unwrap();
        b.step().unwrap();
        assert_eq!(a.state_hash(), b.state_hash(), "tick {tick}");
    }

    let mut inputs = small_world();
    inputs.enterprises.coef[0][SERVICES] *= 1.5;
    let t = TradeParams::default();
    assert_ne!(
        static_digest(&inputs, &putty, &t),
        static_digest(&inputs, &fixed, &t)
    );
    let (inputs, e) = (Arc::new(inputs), 0);
    let fixed_world = EconWorld::new(Arc::clone(&inputs), fixed, t.clone());
    let putty_world = EconWorld::new(Arc::clone(&inputs), putty, t);
    let c = inputs.country_of(inputs.enterprises.province[e] as usize);
    let g = usize::from(inputs.enterprises.good[e]);
    for k in 0..N_GOODS {
        assert_eq!(fixed_world.layout().coef[e][k], inputs.a(c, k, g));
        assert_eq!(putty_world.layout().coef[e][k], inputs.coef(e, k));
    }
}
