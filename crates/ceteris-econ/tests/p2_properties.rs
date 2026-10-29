//! Properties of the P2 tick pipeline on the small-world fixture (`specs/econ/SPEC.md`,
//! Invariants and Test plan).

use std::sync::Arc;

use ceteris_core::ledger::AccountId;
use ceteris_core::sim::{Sim, State};
use ceteris_econ::fixture::{ISLAND_USER, UNREACHABLE, small_world, unreachable_island};
use ceteris_econ::inputs::EconInputs;
use ceteris_econ::local::{self, Province};
use ceteris_econ::params::Technology;
use ceteris_econ::production::{rho, value_added};
use ceteris_econ::systems::{self, SEED};
use ceteris_econ::world::{
    DIVIDEND, MINOR_PER_UNIT, MINT, PURCHASE, TARIFF, TRADE, WRITE_OFF, active_cells,
    static_digest, to_minor, to_numeraire,
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
        // every profit is back with the pops at the end of every tick; losses stay with
        // their enterprise (limited liability)
        let acc = &w.layout().accounts;
        for a in &acc.enterprise {
            prop_assert!(w.ledger.money(*a) <= 0);
        }
        for a in acc.market.iter().chain(&acc.rebate).chain(&acc.dividend) {
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
                // frictionless provinces stay at (1 - u*) L^s = sum L0, their enterprises move
                moved |= lay
                    .enterprises(i)
                    .any(|e| (w.employment[e] / inp.enterprises.l0[e] - 1.0).abs() > 1e-6);
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
        let mut extra = i128::from(w.ledger.money(pop)) * i128::from(scale - 1);
        while extra != 0 {
            let part = extra.clamp(i128::from(i64::MIN), i128::from(i64::MAX));
            w.ledger
                .transfer(MINT, acc.mint, pop, i64::try_from(part).unwrap());
            extra -= part;
        }
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

/// With the loss write-off off: its cap is M months of base sales in base numeraire (ADR 0112),
/// so scaling every nominal value moves losses past the cap in one run and not the other.
#[test]
fn homogeneity_of_degree_zero() {
    for params in [EconParams::default(), EconParams::frictionless()] {
        let params = EconParams {
            write_off_months: f64::INFINITY,
            ..params
        };
        let mut base = sim_of(world(params.clone()));
        let mut big = sim_of(scaled(world(params), 10_000));
        // 120 ticks: the fixture's dynamics are stable and the rounding gap between
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

/// Expected inflation far above c_max passes through the clip (ADR 0106): every active price
/// and every rule-set wage moves by pi^e within its clip, which bounds only the gap terms.
#[test]
fn world_counts_clip_bindings() {
    let pi = 10.0;
    let mut w = world(EconParams::default());
    w.expected_inflation = pi;
    let (price, wage) = (w.markets.price.clone(), w.wage.clone());
    let trade = w.trade().clone();
    let mut sim = sim_of(w);
    sim.step().unwrap();
    let w = sim.world();
    for (c, &active) in w.markets.active.iter().enumerate() {
        let step = libm::log(w.markets.price[c] / price[c]);
        if active {
            let c_max = trade.price_clip(c % N_GOODS);
            assert!(
                (step - pi).abs() <= c_max * (1.0 + 1e-12),
                "cell {c}: {step}"
            );
        } else {
            assert_eq!(step, 0.0, "cell {c}");
        }
    }
    for (i, (&now, &before)) in w.wage.iter().zip(&wage).enumerate() {
        let step = libm::log(now / before);
        assert!(
            (step - pi).abs() <= trade.c_max * (1.0 + 1e-12),
            "province {i}: {step}"
        );
    }
    let mut sim = sim_of(world(EconParams::default()));
    sim.run_until(120).unwrap();
    let w = sim.world();
    let counts = w.markets.clip_ticks.iter().chain(&w.wage_clip_ticks);
    assert!(counts.clone().all(|&n| n <= 120));
    assert!(counts.clone().any(|&n| n > 0), "the data state never clips");
}

/// The full pack's mint holds -1.15e16 minor units at scale 1, beyond i64 from a scale of about
/// 802 (ADR 0089, Evidence). The fixture, scaled until its mint holds what the pack's would at
/// scale 1,000, runs a year on the wide mint and pools (ADR 0106), and its real state matches the
/// unscaled run.
#[test]
fn mint_survives_scale_1000() {
    let params = EconParams {
        write_off_months: f64::INFINITY,
        ..EconParams::default()
    };
    let w = world(params.clone());
    let mint = w.layout().accounts.mint;
    let issued = -w.ledger.money_wide(mint);
    let scale = i64::try_from(1000 * 11_500_000_000_000_000 / issued).unwrap();
    let mut big = sim_of(scaled(world(params.clone()), scale));
    assert!(big.world().ledger.money_wide(mint) < i128::from(i64::MIN));
    let mut base = sim_of(world(params));
    base.run_until(12).unwrap();
    big.run_until(12).unwrap();
    assert!(big.world().ledger.money_wide(mint) < i128::from(i64::MIN));
    let (a, b) = (base.world(), big.world());
    assert_same("output", &a.output, &b.output, &a.output);
    assert_same("employment", &a.employment, &b.employment, &a.employment);
}

/// Pops and single payments stay i64: scaled until the richest pop's money sits just below
/// i64::MAX, the run (not the scaling) stops loudly. On the fixture the largest payment of the
/// tick exceeds the richest pop's money, so the payment's entry, not a balance, fails first.
#[test]
fn nominal_scale_beyond_the_ledger_fails_loudly() {
    let w = world(EconParams::default());
    let pops = w.layout().accounts.pop.clone();
    let richest = |w: &EconWorld| pops.iter().map(|&a| w.ledger.money(a)).max().unwrap();
    let largest = richest(&w);
    let w = scaled(w, i64::MAX / largest);
    assert!(richest(&w) > i64::MAX - largest);
    let mut sim = sim_of(w);
    let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sim.run_until(1)))
        .expect_err("the run overflows");
    let message = err
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| err.downcast_ref::<&str>().copied())
        .unwrap_or_default();
    assert!(message.contains("does not fit the ledger"), "{message}");
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
                for a in 0..kernel.n_gateways() {
                    let lo = gk.crossings.offsets[a] as usize;
                    for (j, e) in gk.crossings.row(a).iter().enumerate() {
                        tariff_due += (1.0 / e.inv_duty - 1.0) * sourced.crossing_value[lo + j];
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

/// National profit ownership (owner, 2026-10-25; SPEC Income) with limited liability (owner,
/// 2026-10-26): at E4 every enterprise's and province market's positive balance joins one pool
/// per country, paid to the country's pops by population share N_k / N_c, the cumulative shares
/// rounded half to even, with every province market's signed cash flow. Money is conserved
/// exactly: each country's pops receive, in minor units, exactly what joined its pool; each
/// pop's part is within one minor unit of N_k / N_c of the pool; enterprises keep a negative
/// balance, less any write-off (ADR 0112), and end at zero otherwise; markets and rebate
/// accounts end at zero.
#[test]
fn national_profit_pool_conserves_money_exactly() {
    for params in [EconParams::default(), EconParams::frictionless()] {
        let mut w = world(params);
        for tick in 0..6 {
            up_to_dispatch(&mut w);
            systems::dispatch(&mut w);
            systems::reprice(&mut w);
            let inp = w.inputs().clone();
            let lay = w.layout().clone();
            let acc = &lay.accounts;
            let nc = inp.n_countries();
            let mut pool = vec![0i64; nc];
            for i in 0..inp.n_provinces() {
                let c = inp.country_of(i);
                pool[c] += w.ledger.money(acc.market[i]);
                pool[c] += lay
                    .enterprises(i)
                    .map(|e| w.ledger.money(acc.enterprise[e]).max(0))
                    .sum::<i64>();
            }
            let held: Vec<i64> = acc.enterprise.iter().map(|&a| w.ledger.money(a)).collect();
            let start = w.ledger.journal().len();
            systems::distribute(&mut w);
            let pop_of = |a| acc.pop.iter().position(|&p| p == a);
            let mut dividend = vec![0i64; inp.n_pops()];
            for e in &w.ledger.journal()[start..] {
                if e.flow == DIVIDEND
                    && let Some(k) = pop_of(e.to)
                {
                    dividend[k] += e.amount;
                }
            }
            let mut people = vec![0.0; nc];
            for k in 0..inp.n_pops() {
                people[inp.country_of(inp.pops.province[k] as usize)] += inp.pops.population[k];
            }
            let mut paid = vec![0i64; nc];
            for (k, &part) in dividend.iter().enumerate() {
                let c = inp.country_of(inp.pops.province[k] as usize);
                paid[c] += part;
                let share = pool[c] as f64 * inp.pops.population[k] / people[c];
                assert!(
                    (part as f64 - share).abs() <= 1.0,
                    "tick {tick} pop {k}: dividend {part} of share {share}"
                );
            }
            assert_eq!(paid, pool, "tick {tick}: pools paid out exactly");
            for (a, &before) in acc.enterprise.iter().zip(&held) {
                let written = written(&w, start, *a);
                assert_eq!(w.ledger.money(*a), before.min(0) + written, "tick {tick}");
            }
            for a in acc.market.iter().chain(&acc.rebate).chain(&acc.dividend) {
                assert_eq!(w.ledger.money(*a), 0, "tick {tick}");
            }
            w.end_tick(tick).unwrap();
        }
    }
}

/// Limited liability of enterprises (owner, 2026-10-26; SPEC Income): an enterprise whose
/// balance is negative at E4 keeps it as retained losses, pays no dividend and takes none from
/// its country's pool; while enterprises earn, no pop is paid a negative dividend; later profit
/// first repays the loss and only the rest joins the pool. Money is conserved exactly.
#[test]
fn limited_liability_keeps_losses_and_repays_them_first() {
    let calibration = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/golden/p2-fixture/calibration");
    let mut inputs = small_world();
    inputs.calibrate(&calibration).unwrap();
    let mut w = EconWorld::new(
        Arc::new(inputs),
        EconParams::default(),
        TradeParams::default(),
    );
    let lay = w.layout().clone();
    let acc = &lay.accounts;
    // DIVIDEND entries from account `from`, or else to pops
    let dividends = |w: &EconWorld, start: usize, from: Option<usize>| -> Vec<i64> {
        w.ledger.journal()[start..]
            .iter()
            .filter(|x| x.flow == DIVIDEND)
            .filter(|x| from.map_or(acc.pop.contains(&x.to), |a| x.from.0 as usize == a))
            .map(|x| x.amount)
            .collect()
    };

    // tick 0: the most profitable enterprise ends with a loss of half its profit (the other
    // half taken by the mint)
    up_to_dispatch(&mut w);
    systems::dispatch(&mut w);
    systems::reprice(&mut w);
    let e = (0..acc.enterprise.len())
        .max_by_key(|&e| w.ledger.money(acc.enterprise[e]))
        .unwrap();
    let profit = w.ledger.money(acc.enterprise[e]);
    assert!(profit > 0, "the steady state is profitable");
    let loss = profit / 2;
    w.ledger
        .transfer(MINT, acc.enterprise[e], acc.mint, profit + loss);
    let start = w.ledger.journal().len();
    systems::distribute(&mut w);
    assert_eq!(w.ledger.money(acc.enterprise[e]), -loss, "the loss stays");
    let enterprise_account = acc.enterprise[e].0 as usize;
    assert!(dividends(&w, start, Some(enterprise_account)).is_empty());
    assert!(
        dividends(&w, start, None).iter().all(|&d| d >= 0),
        "a negative dividend"
    );
    w.end_tick(0).unwrap();

    // tick 1: the profit repays the loss before anything joins the pool
    up_to_dispatch(&mut w);
    systems::dispatch(&mut w);
    systems::reprice(&mut w);
    let balance = w.ledger.money(acc.enterprise[e]);
    let start = w.ledger.journal().len();
    systems::distribute(&mut w);
    let paid: i64 = dividends(&w, start, Some(enterprise_account)).iter().sum();
    assert_eq!(
        paid,
        balance.max(0),
        "pays out only what is left after the loss"
    );
    assert_eq!(w.ledger.money(acc.enterprise[e]), balance.min(0));
    assert!(
        dividends(&w, start, None).iter().all(|&d| d >= 0),
        "a negative dividend"
    );
    w.end_tick(1).unwrap();
}

/// ADR 0089: the frictionless wage clears labour at the natural rate, sum_g L*_ig(w) =
/// (1 - u*_i) L^s_i, so unemployment is u* in every province at every tick.
#[test]
fn frictionless_unemployment_is_the_natural_rate() {
    let mut sim = sim_of(world(EconParams::frictionless()));
    for tick in 0..24 {
        sim.step().unwrap();
        let w = sim.world();
        let (inp, lay) = (w.inputs(), w.layout());
        for i in 0..inp.n_provinces() {
            let l: f64 = lay.enterprises(i).map(|e| w.employment[e]).sum();
            let u = 1.0 - l / lay.labour_supply[i];
            let u_star = inp.countries.u_star[inp.country_of(i)];
            assert!(
                (u - u_star).abs() <= 1e-10,
                "tick {tick} province {i}: unemployment {u}, u* {u_star}"
            );
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
    let mut position = [0.0; N_GOODS];
    // the lead E3 will use: the prices do not move between here and E3
    w.refresh_price_index();
    for (e, months) in [(full, None), (half, Some(0.5))] {
        let i = inp.enterprises.province[e] as usize;
        for k in (0..N_GOODS).filter(|&k| TRADED[k] && lay.coef[e][k] > 0.0) {
            let target = 1.0 + systems::COVER + w.scratch.lead[i * N_GOODS + k];
            let units = months.unwrap_or(target) * lay.coef[e][k] * capacity(&w, e);
            if e == full {
                position[k] = units;
            }
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
    // nothing beyond the rounding of the three slots that hold the target position
    let i = inp.enterprises.province[full] as usize;
    for k in (0..N_GOODS).filter(|&k| TRADED[k] && lay.coef[full][k] > 0.0) {
        let q = w.price_index[i * N_GOODS + k];
        assert!(w.scratch.ent_order[full * N_GOODS + k] <= 1e-12 * position[k]);
        assert!(w.scratch.ent_spend[full * N_GOODS + k] <= 1e-12 * position[k] * q);
    }
}

/// Input orders plan on capacity output VA(L) / a_VA at this tick's employment, not on output
/// cut by a short input (owner decision, 2026-10-24), less the goods already on order.
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

/// Local-good chains: services feeding services, and construction and services feeding each
/// other. From the data state the chain binds in every mainland province at tick 1; over 120
/// ticks every province tick settles in at most 8 passes (6 as built; the old chain ran up to
/// its bound of 32 and then cleared the rest), and every local input used equals coef Y (the
/// Leontief invariant, exactly, at every tick).
#[test]
fn local_chain_terminates_on_cycles() {
    let coefficients = [
        (SERVICES, SERVICES, 0.3),
        (SERVICES, CONSTRUCTION, 0.2),
        (CONSTRUCTION, SERVICES, 0.2),
    ];
    let inputs = Arc::new(fixture_with(&coefficients));
    let mut sim = EconWorld::sim(inputs, EconParams::default(), TradeParams::default());
    let mut bound = 0;
    for tick in 0..120 {
        sim.step().unwrap_or_else(|v| panic!("{v}"));
        let w = sim.world();
        let passes = &w.scratch.chain_iterations;
        assert!(
            passes.iter().all(|&n| (1..=8).contains(&n)),
            "tick {tick}: {passes:?}"
        );
        if tick == 1 {
            assert!(passes[..24].iter().all(|&n| n > 1));
        }
        bound += passes.iter().filter(|&&n| n > 1).count();
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
    println!("chain bound in {bound} province ticks");
    assert!(bound > 0);
}

/// Outputs at fills `f`, as ADR 0093's Amendment defines them, written out here independently.
fn outputs_at(p: &Province, f: &[f64; 3]) -> Vec<f64> {
    (0..p.cap.len())
        .map(|u| {
            let least = (0..3)
                .filter(|&j| p.coef[u][j] > 0.0)
                .fold(1.0f64, |m, j| m.min(f[j]));
            p.cap[u] * least
        })
        .collect()
}

fn fits_at(p: &Province, j: usize, f: &[f64; 3]) -> bool {
    let y = outputs_at(p, f);
    let rest = p.producer[j].map_or(0.0, |q| (1.0 - p.own[j]) * y[q]);
    let uncovered: f64 = (0..y.len())
        .map(|u| (p.coef[u][j] * y[u] - p.stock[u][j]).max(0.0))
        .sum();
    uncovered <= rest
}

/// The fill iteration run until a pass changes nothing, each step by bisection: the reference
/// the exact solve must reproduce. None where that takes more than a million passes (a loop
/// gain near 1) or a fill falls towards 0 below 1e-200.
fn converged_fills(p: &Province) -> Option<[f64; 3]> {
    let mut f = [1.0; 3];
    for _ in 0..1_000_000 {
        let before = f;
        for j in 0..3 {
            if fits_at(p, j, &f) {
                continue;
            }
            let (mut lo, mut hi) = (0.0, f[j]);
            for _ in 0..200 {
                let mid = 0.5 * (lo + hi);
                if mid <= lo || mid >= hi {
                    break;
                }
                let mut g = f;
                g[j] = mid;
                if fits_at(p, j, &g) {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            // near the bottom of f64 the iteration underflows good by good, which is no limit
            if lo > 0.0 && lo < 1e-200 {
                return None;
            }
            f[j] = lo;
        }
        if f == before {
            return Some(f);
        }
    }
    None
}

/// Checks that every good fits at the exact fills and, where the fill iteration converges,
/// that they are its limit. Returns the passes of the exact solve and whether it compared.
fn fills_match(p: &Province, what: &str) -> (usize, bool) {
    let (exact, passes) = local::fills(p);
    for j in 0..3 {
        assert!(fits_at(p, j, &exact), "{what}: good {j} short at {exact:?}");
    }
    let Some(iterated) = converged_fills(p) else {
        return (passes, false);
    };
    for j in 0..3 {
        assert!(
            (exact[j] - iterated[j]).abs() <= 1e-12,
            "{what}: good {j}: exact {exact:?}, iterated {iterated:?}, {p:?}"
        );
    }
    // fills agree to 1e-12, so outputs to 1e-12 of capacity
    let (a, b) = (outputs_at(p, &exact), outputs_at(p, &iterated));
    for (u, (x, y)) in a.iter().zip(&b).enumerate() {
        assert!(
            (x - y).abs() <= 1e-12 * p.cap[u],
            "{what}: enterprise {u}: {x} vs {y}"
        );
    }
    (passes, true)
}

/// A small random province from `seed` (xorshift): two to six enterprises, each local good made
/// by at most one of them, coefficients up to 0.6, stocks often 0 and otherwise up to about
/// the need at capacity.
fn random_province(seed: u64) -> Province {
    let mut s = seed | 1;
    let mut next = move || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s >> 11) as f64 / (1u64 << 53) as f64
    };
    let n = 2 + (next() * 5.0) as usize;
    let mut producer = [None; 3];
    for j in 0..3 {
        let u = (next() * n as f64) as usize;
        if next() < 0.85 && !producer.contains(&Some(u)) {
            producer[j] = Some(u);
        }
    }
    let cap: Vec<f64> = (0..n).map(|_| 0.1 + 10.0 * next()).collect();
    let coef: Vec<[f64; 3]> = (0..n)
        .map(|u| {
            std::array::from_fn(|j| {
                if producer[j] == Some(u) || next() < 0.4 {
                    0.0
                } else {
                    0.6 * next()
                }
            })
        })
        .collect();
    let stock = (0..n)
        .map(|u| {
            std::array::from_fn(|j| {
                if next() < 0.5 {
                    0.0
                } else {
                    1.2 * next() * coef[u][j] * cap[u]
                }
            })
        })
        .collect();
    let own = std::array::from_fn(|_| 0.3 * next());
    Province {
        cap,
        coef,
        stock,
        producer,
        own,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]
    /// ADR 0093, Amendment: on random small provinces every good fits at the exact fills, and
    /// they are the converged iteration's (1e-12) wherever it converges (about 98% of draws;
    /// the rest fall towards 0 below 1e-200, where bisection underflows good by good).
    #[test]
    fn exact_fill_equals_converged_iteration_on_random_provinces(seed in any::<u64>()) {
        fills_match(&random_province(seed), &format!("seed {seed}"));
    }
}

/// ADR 0093, Bounded work (2026-10-29): provinces whose fills fall towards 0 settle in bounded
/// passes, at fills exactly 0 (never subnormal), every good fitting. Seed 13622680876955865696
/// falls at a rate near 1 on one pass map (it took 781 passes and seconds before the closed
/// form of a collapse); seed 709302389291714235 switches irregularly between two maps (15,678
/// passes before the pass bound).
#[test]
fn collapsing_provinces_settle_in_bounded_passes() {
    for seed in [13622680876955865696u64, 709302389291714235] {
        let p = random_province(seed);
        let (f, passes) = local::fills(&p);
        assert!(
            passes <= 4 * local::STALL_PASSES + 16,
            "seed {seed}: {passes} passes"
        );
        for j in 0..3 {
            assert!(fits_at(&p, j, &f), "seed {seed}: good {j} short at {f:?}");
            assert!(
                f[j] == 0.0 || f[j] >= f64::MIN_POSITIVE,
                "seed {seed}: {f:?}"
            );
        }
        assert_eq!(f, [0.0; 3], "seed {seed}");
    }
}

/// ADR 0093, Amendment: at every province tick of the fixture's default run (60 ticks) and of
/// the cycle fixture (120 ticks), every good fits at the exact fills and they equal the fill
/// iteration run to convergence (1e-12) wherever it converges (all but 2 of the 413 province
/// ticks where the chain binds, all in the cycle fixture).
#[test]
fn exact_fill_equals_converged_iteration() {
    let cycles = [
        (SERVICES, SERVICES, 0.3),
        (SERVICES, CONSTRUCTION, 0.2),
        (CONSTRUCTION, SERVICES, 0.2),
    ];
    let (mut bound, mut skipped) = (0, 0);
    for (inputs, ticks) in [(small_world(), 60), (fixture_with(&cycles), 120)] {
        let mut sim = EconWorld::sim(
            Arc::new(inputs),
            EconParams::default(),
            TradeParams::default(),
        );
        for tick in 0..ticks {
            let mut probe = sim.world().clone();
            probe.begin_tick();
            systems::deliver(&mut probe);
            systems::labour(&mut probe);
            systems::cap_output(&mut probe);
            let (inp, lay) = (probe.inputs().clone(), probe.layout().clone());
            for i in 0..inp.n_provinces() {
                let cap = &probe.scratch.produced[lay.enterprises(i)];
                let p = Province::new(&inp, &lay, &probe.ledger, i, cap);
                let what = format!("tick {tick} province {i}");
                let (passes, compared) = fills_match(&p, &what);
                skipped += usize::from(!compared);
                if passes > 1 {
                    bound += 1;
                }
            }
            sim.step().unwrap_or_else(|v| panic!("{v}"));
        }
    }
    println!("chain bound in {bound} province ticks, {skipped} without a converged iteration");
    assert!(skipped * 100 < bound);
    assert!(bound > 0);
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

/// ADR 0020: with one vintage (the 2000 ICIO cost shares at base prices) putty-clay coefficients run exactly as
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
    assert_eq!(fixed_world.layout().coef[e], inputs.base_column(e));
    assert_eq!(putty_world.layout().coef[e], inputs.enterprises.coef[e]);
}

/// Unreachable origins at world level (`fixture::unreachable_island`): machinery has no reachable
/// origin at the island. Its delivered price there is the only infinite one and the world
/// price index leaves it out; island pops buy none of it and spend the rest of their money;
/// the island's light manufacturing, its one user there, orders none, never restocks, and is
/// capped by the stock it started with until it produces nothing; no price, wage or flow
/// turns NaN, and the invariants hold every tick.
#[test]
fn unreachable_destination_buys_nothing_in_the_world() {
    for params in [EconParams::default(), EconParams::frictionless()] {
        let trade = TradeParams::default();
        let mut w = EconWorld::new(Arc::new(unreachable_island(&trade)), params, trade);
        let (inp, lay) = (w.inputs().clone(), w.layout().clone());
        let island = inp.n_provinces() - 1;
        let cell = island * N_GOODS + UNREACHABLE;
        let user = producer(&w, island, ISLAND_USER);
        let (account, buyer) = (lay.accounts.enterprise[user], inp.n_pops() + user);
        let coef = lay.coef[user][UNREACHABLE];
        let mut stock = w.ledger.goods(account, UNREACHABLE);
        assert!(coef > 0.0 && stock > 0.0);
        assert_eq!(w.price_index[cell], f64::INFINITY);
        let mut spending = 0;
        for tick in 0..240 {
            w.begin_tick();
            systems::deliver(&mut w);
            systems::labour(&mut w);
            systems::produce(&mut w);
            assert!(
                w.output[user] * coef <= stock * (1.0 + 1e-12),
                "tick {tick}"
            );
            let money: Vec<f64> = lay
                .pops(island)
                .map(|k| to_numeraire(w.ledger.money(lay.accounts.pop[k])))
                .collect();
            systems::demand(&mut w);
            for (c, p) in w.price_index.iter().enumerate() {
                assert!(p.is_finite() == (c != cell), "tick {tick} cell {c}: {p}");
            }
            assert!(w.world_price_index().is_finite());
            let prices = &w.price_index[island * N_GOODS..(island + 1) * N_GOODS];
            for (k, money) in lay.pops(island).zip(money) {
                let x = &w.scratch.pop_demand[k * N_GOODS..(k + 1) * N_GOODS];
                assert_eq!(x[UNREACHABLE], 0.0);
                let spent: f64 = (0..N_GOODS)
                    .filter(|&g| x[g] > 0.0)
                    .map(|g| x[g] * prices[g])
                    .sum();
                // a pop whose money a loss-making tick left negative buys nothing, as on the
                // plain fixture
                let want = money.max(0.0);
                assert!(
                    (spent - want).abs() <= 1e-9 * want,
                    "tick {tick} pop {k}: spent {spent} of {money}"
                );
                spending += usize::from(money > 0.0);
            }
            assert_eq!(w.scratch.ent_spend[user * N_GOODS + UNREACHABLE], 0.0);
            systems::source(&mut w);
            assert_eq!(w.scratch.class_spend[cell], [0.0; 2]);
            systems::clear(&mut w);
            from_dispatch(&mut w, tick);
            let held = w.ledger.goods(account, UNREACHABLE);
            assert!(held <= stock, "tick {tick}: restocked {stock} -> {held}");
            stock = held;
            for b in lay.pops(island).chain([buyer]) {
                assert_eq!(w.pipeline.on_order(b, UNREACHABLE), 0.0);
            }
            assert!(w.markets.price.iter().all(|p| p.is_finite() && *p > 0.0));
            assert!(w.wage.iter().all(|x| x.is_finite() && *x > 0.0));
            assert!(w.output.iter().chain(&w.employment).all(|x| x.is_finite()));
            assert!(w.expected_inflation.is_finite());
            let traded = (0..N_GOODS).filter(|&g| TRADED[g]);
            assert!(traded.map(|g| w.pipeline.in_flight(g)).all(f64::is_finite));
            for a in &lay.accounts.enterprise {
                assert!(w.ledger.money(*a) <= 0, "a profit was not paid out");
            }
            for a in &lay.accounts.market {
                assert_eq!(w.ledger.money(*a), 0);
            }
        }
        assert!(spending > 0, "no island pop had money to spend");
        assert_eq!(w.output[user], 0.0);
        let mut others = lay.enterprises(island).filter(|&e| e != user);
        assert!(others.all(|e| w.output[e] > 0.0));
    }
}

/// The calibrated fixture, run up to E4 of its first tick.
fn calibrated_before_distribute() -> EconWorld {
    let calibration = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/golden/p2-fixture/calibration");
    let mut inputs = small_world();
    inputs.calibrate(&calibration).unwrap();
    let mut w = EconWorld::new(
        Arc::new(inputs),
        EconParams::default(),
        TradeParams::default(),
    );
    up_to_dispatch(&mut w);
    systems::dispatch(&mut w);
    systems::reprice(&mut w);
    w
}

/// Sets `account`'s balance to `target` against the mint (no pop involved).
fn set_balance(w: &mut EconWorld, account: AccountId, target: i64) {
    let mint = w.layout().accounts.mint;
    let have = w.ledger.money(account);
    w.ledger.transfer(MINT, account, mint, have - target);
}

/// Sum of WRITE_OFF entries from `start` on into (positive) or out of (negative) `account`.
fn written(w: &EconWorld, start: usize, account: AccountId) -> i64 {
    w.ledger.journal()[start..]
        .iter()
        .filter(|x| x.flow == WRITE_OFF)
        .map(|x| i64::from(x.to == account) * x.amount - i64::from(x.from == account) * x.amount)
        .sum()
}

/// Loss write-off (ADR 0112): an enterprise's retained loss beyond M months of its base sales
/// is written off at E4 against its country's pops in proportion to their positive money,
/// exactly in minor units; no pop pays more than it holds.
#[test]
fn write_off_takes_the_excess_beyond_the_cap_from_the_pops() {
    let mut w = calibrated_before_distribute();
    let lay = w.layout().clone();
    let acc = &lay.accounts;
    let (account, cap) = (acc.enterprise[0], lay.ent_loss_cap[0]);
    let c = w
        .inputs()
        .country_of(w.inputs().enterprises.province[0] as usize);
    assert!(cap > 0);
    let excess = 12_345;
    set_balance(&mut w, account, -cap - excess);
    let start = w.ledger.journal().len();
    systems::distribute(&mut w);
    assert_eq!(w.ledger.money(account), -cap, "only the excess goes");
    assert_eq!(written(&w, start, account), excess);
    let levied: i64 = lay.country_pops[c]
        .iter()
        .map(|&k| -written(&w, start, acc.pop[k as usize]))
        .sum();
    assert_eq!(levied, excess, "the country's pops pay exactly the excess");
    assert!(acc.pop.iter().all(|&a| written(&w, start, a) <= 0));
    assert!(acc.pop.iter().all(|&a| w.ledger.money(a) >= 0));
    assert_eq!(w.ledger.money(acc.dividend[c]), 0);
    w.end_tick(0).unwrap();
}

/// The cap is M months of base sales, the calibrated steady-state sales value, and a loss up to
/// it stays where it is.
#[test]
fn write_off_spares_losses_within_the_cap() {
    let mut w = calibrated_before_distribute();
    let lay = w.layout().clone();
    let acc = &lay.accounts;
    let steady = w.inputs().steady.clone().unwrap();
    for (e, &cap) in lay.ent_loss_cap.iter().enumerate() {
        let want = to_minor(3.0 * steady.ent_sales[e]) as f64;
        assert!(
            (cap as f64 - want).abs() <= 1e-12 * want + 1.0,
            "enterprise {e}"
        );
    }
    let cap = lay.ent_loss_cap[0];
    set_balance(&mut w, acc.enterprise[0], -cap);
    let start = w.ledger.journal().len();
    systems::distribute(&mut w);
    assert!(
        w.ledger.journal()[start..]
            .iter()
            .all(|x| x.flow != WRITE_OFF)
    );
    assert_eq!(w.ledger.money(acc.enterprise[0]), -cap);
    w.end_tick(0).unwrap();
}

/// Where the country's pops hold less positive money than the excess, all of it is taken (no
/// pop below zero, pops already negative untouched), shared over the enterprises by their
/// excess, and the rest stays with them for the next tick.
#[test]
fn write_off_takes_what_the_pops_hold_and_carries_the_rest() {
    let mut w = calibrated_before_distribute();
    let lay = w.layout().clone();
    let acc = &lay.accounts;
    let inp = w.inputs().clone();
    let c = inp.country_of(inp.enterprises.province[0] as usize);
    let e2 = (1..inp.n_enterprises())
        .find(|&e| inp.country_of(inp.enterprises.province[e] as usize) == c)
        .unwrap();
    let (a1, cap1) = (acc.enterprise[0], lay.ent_loss_cap[0]);
    let (a2, cap2) = (acc.enterprise[e2], lay.ent_loss_cap[e2]);
    // nothing else reaches the pops this tick: every holder and rebate account at zero
    for &a in acc.enterprise.iter().chain(&acc.market).chain(&acc.rebate) {
        set_balance(&mut w, a, 0);
    }
    let pops: Vec<usize> = lay.country_pops[c].iter().map(|&k| k as usize).collect();
    for (n, &k) in pops.iter().enumerate() {
        set_balance(&mut w, acc.pop[k], if n == 0 { -500 } else { 7 });
    }
    let room = 7 * (pops.len() as i64 - 1);
    let (x1, x2) = (3 * room, room);
    set_balance(&mut w, a1, -cap1 - x1);
    set_balance(&mut w, a2, -cap2 - x2);
    let start = w.ledger.journal().len();
    systems::distribute(&mut w);
    assert_eq!(written(&w, start, a1) + written(&w, start, a2), room);
    assert!(
        (written(&w, start, a1) - 3 * room / 4).abs() <= 1,
        "shared by excess"
    );
    assert_eq!(
        w.ledger.money(acc.pop[pops[0]]),
        -500,
        "a negative pop pays nothing"
    );
    for &k in &pops[1..] {
        assert_eq!(
            w.ledger.money(acc.pop[k]),
            0,
            "pop {k} paid all it had, no more"
        );
    }
    assert!(
        w.ledger.money(a1) < -cap1 && w.ledger.money(a2) < -cap2,
        "the rest is carried"
    );
    w.end_tick(0).unwrap();

    // next tick the pops hold enough again, and the carried rest is written off down to the caps
    w.begin_tick();
    for &a in acc.market.iter().chain(&acc.rebate) {
        set_balance(&mut w, a, 0);
    }
    for &a in acc.enterprise.iter().filter(|&&a| a != a1 && a != a2) {
        set_balance(&mut w, a, 0);
    }
    for &k in &pops[1..] {
        set_balance(&mut w, acc.pop[k], 10 * room);
    }
    let start = w.ledger.journal().len();
    systems::distribute(&mut w);
    assert_eq!(
        written(&w, start, a1) + written(&w, start, a2),
        x1 + x2 - room
    );
    assert_eq!((w.ledger.money(a1), w.ledger.money(a2)), (-cap1, -cap2));
    assert!(acc.pop.iter().all(|&a| written(&w, start, a) <= 0));
    w.end_tick(1).unwrap();
}

/// A run with write-offs is identical at 1 and 4 threads.
#[test]
fn write_off_runs_are_thread_independent() {
    let run = |threads: usize| {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        pool.install(|| {
            let mut w = world(EconParams::frictionless());
            for (c, p) in w.markets.price.iter_mut().enumerate() {
                *p *= if c % 3 == 0 { 0.2 } else { 3.0 };
            }
            let mut sim = sim_of(w);
            let mut fired = 0;
            for _ in 0..120 {
                sim.step().unwrap();
                fired += sim
                    .world()
                    .ledger
                    .journal()
                    .iter()
                    .filter(|x| x.flow == WRITE_OFF)
                    .count();
            }
            (sim.snapshot_hashes(), fired)
        })
    };
    let ((one, fired), (four, _)) = (run(1), run(4));
    assert!(fired > 0, "no write-off fired");
    assert_eq!(one, four);
}
