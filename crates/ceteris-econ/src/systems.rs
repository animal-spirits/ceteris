//! The P2 tick pipeline (`specs/econ/SPEC.md`, System order; `specs/trade/SPEC.md`, System
//! order): T1 deliver, E1 labour, E2 produce, E3 demand, T2 source, T3 clear, T2b dispatch,
//! T4 reprice, E4 distribute, E5 assert.
//!
//! Per-entity computations that are heavy (labour targets, the clearing wage, production)
//! run with rayon into fixed slots; every ledger operation is applied afterwards, serially,
//! in province, enterprise or pop ID order, so results do not depend on the thread count.
//!
//! The ledger is the authority on quantities held. Where an account is emptied by computed
//! amounts (transit at T1, markets at T3 and T2b), the largest recipient takes the exact
//! remaining balance less what must stay, so floating-point residue never leaves a negative
//! holding.
//!
//! Buyers of traded goods are pops and enterprises, numbered in the pipeline as pops
//! `0..n_pops`, then enterprises. Within a destination province and buyer class, each buyer
//! receives and pays its share of the class's spending on the good: every buyer of a class
//! faces the same delivered prices, so its orders split over origins like the class total.

use rayon::prelude::*;

use ceteris_core::ledger::{AccountId, Entry, Flow, Ledger};
use ceteris_core::reduce::fixed_sum;
use ceteris_core::rng::Seed;
use ceteris_core::schedule::{Cadence, Schedule, SystemDef, SystemId};
use ceteris_core::sim::State;
use ceteris_trade::goods::TRADED;
use ceteris_trade::source::{HOUSEHOLD, INTERMEDIATE};
use ceteris_trade::{Kernel, N_GOODS, Sourced, dispatch as dispatch_good, source as source_good};

use crate::inputs::EconInputs;
use crate::labour::{allocate, clearing_wage, wage_step};
use crate::local::{self, LOCAL};
use crate::production::{labour_target, rho, value_added};
use crate::world::{
    Accounts, Buyer, DIVIDEND, EconWorld, Layout, Order, PURCHASE, Payments, REBATE, SALE, TARIFF,
    TRADE, WAGE, WRITE_OFF, to_minor, to_numeraire,
};

/// Months of use an enterprise keeps in stock beyond replacement and transit (Task 7b).
pub const COVER: f64 = 1.0;

/// No RNG draws in P2; streams stay reserved per system ID.
pub const SEED: Seed = Seed(42);

/// Relative gap between labour demand and supply above which the clearing wage is taken to
/// have no root (all value-added prices nonpositive, say).
const NO_ROOT: f64 = 1e-9;

pub fn schedule() -> Schedule<EconWorld> {
    let def = |id, name, run| SystemDef {
        id: SystemId(id),
        name,
        cadence: Cadence::Monthly,
        run,
    };
    Schedule::new(vec![
        def(201, "deliver", |w, _| deliver(w)),
        def(202, "labour", |w, _| labour(w)),
        def(203, "produce", |w, _| produce(w)),
        def(204, "demand", |w, _| demand(w)),
        def(205, "source", |w, _| source(w)),
        def(206, "clear", |w, _| clear(w)),
        def(207, "dispatch", |w, _| dispatch(w)),
        def(208, "reprice", |w, _| reprice(w)),
        def(209, "distribute", |w, _| distribute(w)),
        def(210, "assert", |w, ctx| assert_invariants(w, ctx.tick)),
    ])
}

fn pay(ledger: &mut Ledger, flow: Flow, from: AccountId, to: AccountId, amount: i64) {
    if amount != 0 {
        ledger.transfer(flow, from, to, amount);
    }
}

/// Splits `total` minor units in proportion to `weights`, rounding the cumulative shares
/// half to even, so the parts add up to `total` exactly. Without a positive weight the last
/// part takes everything.
fn split(total: i64, weights: impl ExactSizeIterator<Item = f64> + Clone, out: &mut Vec<i64>) {
    out.clear();
    let n = weights.len();
    let sum: f64 = weights.clone().sum();
    let (mut cum, mut paid) = (0.0, 0i64);
    for (j, w) in weights.enumerate() {
        cum += w;
        let upto = if j + 1 == n {
            total
        } else if sum > 0.0 {
            (total as f64 * (cum / sum)).round_ties_even() as i64
        } else {
            0
        };
        out.push(upto - paid);
        paid = upto;
    }
}

/// Index of the first largest positive value, if any.
fn largest(values: impl Iterator<Item = f64>) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (j, v) in values.enumerate() {
        if v > 0.0 && best.is_none_or(|(_, b)| v > b) {
            best = Some((j, v));
        }
    }
    best.map(|(j, _)| j)
}

/// T1: goods due this tick move from the transit accounts to their buyers: pops consume
/// them on arrival, enterprises add them to their input stocks.
pub fn deliver(w: &mut EconWorld) {
    let lay = &*w.layout;
    let acc = &lay.accounts;
    let n_buyers = w.pipeline.n_buyers();
    let n_pops = acc.pop.len();
    w.scratch.due.copy_from_slice(w.pipeline.pop_due());
    let in_flight = w.pipeline.in_flight_all();
    let due = &w.scratch.due;
    let account = |b: usize| {
        if b < n_pops {
            acc.pop[b]
        } else {
            acc.enterprise[b - n_pops]
        }
    };
    let n_traded = acc.transit.len();
    // the first largest delivery of each good, as `largest` finds it
    let mut last: [Option<(usize, f64)>; N_GOODS] = [None; N_GOODS];
    for (b, row) in due.chunks_exact(N_GOODS).enumerate() {
        for (best, &x) in last.iter_mut().zip(&row[..n_traded]) {
            if x > 0.0 && best.is_none_or(|(_, v)| x > v) {
                *best = Some((b, x));
            }
        }
    }
    let receive = |ledger: &mut Ledger, g: usize, b: usize, x: f64| {
        ledger.ship(acc.transit[g], account(b), g, x);
        if b < n_pops {
            ledger.destroy(account(b), g, x);
        }
    };
    // buyer by buyer; each good's account sees its deliveries in buyer order
    for (b, row) in due.chunks_exact(N_GOODS).enumerate().take(n_buyers) {
        for g in 0..n_traded {
            if row[g] > 0.0 && last[g].is_some_and(|(l, _)| l != b) {
                let x = row[g].min(w.ledger.goods(acc.transit[g], g));
                receive(&mut w.ledger, g, b, x);
            }
        }
    }
    // the largest delivery takes what the account holds beyond the goods still in flight
    for (g, best) in last.iter().enumerate().take(n_traded) {
        if let Some((b, _)) = *best {
            let x = (w.ledger.goods(acc.transit[g], g) - in_flight[g]).max(0.0);
            receive(&mut w.ledger, g, b, x);
        }
    }
}

/// Value-added price `(p_g - sum_k a_kg P_k) / a_VA` of a sector with input coefficients
/// `a[k]` at its own price `p_g` and input price indices `index`. Inputs with a_kg = 0 are
/// left out, so an unreachable good it does not use (infinite index) changes nothing; an
/// unreachable input gives minus infinity, and the enterprise hires no one.
pub fn value_added_price(p_g: f64, a: impl Fn(usize) -> f64, index: &[f64], a_va: f64) -> f64 {
    let inputs: f64 = (0..N_GOODS)
        .filter(|&k| a(k) > 0.0)
        .map(|k| a(k) * index[k])
        .sum();
    (p_g - inputs) / a_va
}

/// E1: labour targets L* at the current own price, last tick's input price indices and the
/// current wage, then the wage (wage rule on expected inflation and excess demand against the
/// natural rate u*, or when frictionless the wage at which labour demand is (1 - u*) L^s, ADR
/// 0089), employment toward L* and employed pops.
pub fn labour(w: &mut EconWorld) {
    let (inp, lay, params) = (&*w.inputs, &*w.layout, &w.params);
    let c_max = w.trade.c_max;
    let r = rho(params.sigma_fl);
    let (price, wage, prev) = (&w.markets.price, &w.wage, &w.employment);
    let (index, pi) = (&w.price_index, w.expected_inflation);
    let results: Vec<(f64, Vec<f64>, bool)> = (0..inp.n_provinces())
        .into_par_iter()
        .map(|i| {
            let c = inp.country_of(i);
            let (s_l, u_star) = (inp.countries.s_l[c], inp.countries.u_star[c]);
            let supply = lay.labour_supply[i];
            let row = i * N_GOODS..(i + 1) * N_GOODS;
            let (p, p_in) = (&price[row.clone()], &index[row]);
            let ents = lay.enterprises(i);
            let p_va: Vec<f64> = ents
                .clone()
                .map(|e| {
                    let g = usize::from(inp.enterprises.good[e]);
                    value_added_price(p[g], |k| lay.coef[e][k], p_in, lay.a_va[e])
                })
                .collect();
            let target = |e: usize, p_va: f64, w: f64| {
                let ent = &inp.enterprises;
                labour_target(p_va, w, ent.a[e], ent.va0[e], ent.l0[e], s_l, r, supply)
            };
            let targets = |w: f64| -> Vec<f64> {
                ents.clone()
                    .zip(&p_va)
                    .map(|(e, &pv)| target(e, pv, w))
                    .collect()
            };
            let w0 = wage[i];
            let (w1, t, clipped) = if params.frictionless {
                let demand = |w: f64| -> f64 {
                    ents.clone()
                        .zip(&p_va)
                        .map(|(e, &pv)| target(e, pv, w))
                        .sum()
                };
                let natural = (1.0 - u_star) * supply;
                let mut w1 = clearing_wage(w0, natural, demand);
                // no root: the search ran to an extreme; take at most a doubling or halving
                if (demand(w1) - natural).abs() > NO_ROOT * natural {
                    w1 = w1.clamp(0.5 * w0, 2.0 * w0);
                }
                (w1, targets(w1), false)
            } else {
                let t = targets(w0);
                let excess = (fixed_sum(&t) - supply) / supply;
                let clipped = (pi + params.kappa_w * (excess + u_star)).abs() > c_max;
                (wage_step(w0, excess, u_star, pi, params, c_max), t, clipped)
            };
            let mut l = vec![0.0; t.len()];
            allocate(&t, &prev[ents], params.eta, supply, &mut l);
            (w1, l, clipped)
        })
        .collect();
    for (i, (w1, l, clipped)) in results.into_iter().enumerate() {
        w.wage[i] = w1;
        w.wage_clip_ticks[i] += u32::from(clipped);
        w.employment[lay.enterprises(i)].copy_from_slice(&l);
        let employed = fixed_sum(&l) / lay.labour_supply[i];
        for k in lay.pops(i) {
            w.employed[k] = inp.pops.labour_force[k] * employed;
        }
    }
}

/// E2: Leontief output from employment, traded input stocks and the same-tick chain of local
/// goods ([`local::chain`], per province). Output is sold to the province's market (the origin
/// market of a traded good) at its current price and becomes the market's supply Q; traded
/// inputs used are destroyed from stock; each local input is drawn from this tick's output at
/// the province market, paid at its price, and the rest of its use from the enterprise's
/// stock. Wages are paid to the pops of the province by effective employed labour.
pub fn produce(w: &mut EconWorld) {
    cap_output(w);
    let (inp, lay) = (
        std::sync::Arc::clone(&w.inputs),
        std::sync::Arc::clone(&w.layout),
    );
    let (inp, lay) = (&*inp, &*lay);
    let acc = &lay.accounts;
    let edu_eff = w.params.edu_eff;

    w.markets.supply.fill(0.0);
    w.scratch.drawn.fill(0.0);
    let mut parts = Vec::new();
    local::chain_all(w);
    for i in 0..inp.n_provinces() {
        let pops = lay.pops(i);
        let effective: Vec<f64> = pops
            .clone()
            .map(|k| edu_eff[usize::from(inp.pops.edu[k])] * w.employed[k])
            .collect();
        for e in lay.enterprises(i) {
            let (g, y) = (usize::from(inp.enterprises.good[e]), w.scratch.produced[e]);
            let (account, cell) = (acc.enterprise[e], i * N_GOODS + g);
            for k in (0..N_GOODS).filter(|&k| TRADED[k]) {
                let a = lay.coef[e][k];
                let before = w.ledger.goods(account, k);
                if a > 0.0 {
                    w.ledger.destroy(account, k, (a * y).min(before));
                }
                w.scratch.input_before[e * N_GOODS + k] = before;
                w.scratch.input_used[e * N_GOODS + k] = before - w.ledger.goods(account, k);
            }
            w.output[e] = y;
            if y > 0.0 {
                w.ledger.create(account, g, y);
                w.ledger.ship(account, acc.market[i], g, y);
            }
            let sale = w.markets.price[cell] * y;
            w.sales[e] = sale;
            pay(&mut w.ledger, SALE, acc.market[i], account, to_minor(sale));
            w.markets.supply[cell] += y;

            let bill = to_minor(w.wage[i] * w.employment[e]);
            split(bill, effective.iter().copied(), &mut parts);
            for (k, &part) in pops.clone().zip(&parts) {
                pay(&mut w.ledger, WAGE, account, acc.pop[k], part);
                w.scratch.income_paid[k] += part;
            }
        }
        // local inputs: this tick's output at the market first, the stock second
        for e in lay.enterprises(i) {
            let account = acc.enterprise[e];
            for k in LOCAL {
                let (at, cell) = (e * N_GOODS + k, i * N_GOODS + k);
                let need = lay.coef[e][k] * w.scratch.produced[e];
                let stock = w.ledger.goods(account, k);
                let take = w.scratch.from_output[at].min(w.ledger.goods(acc.market[i], k));
                if take > 0.0 {
                    w.ledger.ship(acc.market[i], account, k, take);
                    let value = to_minor(w.markets.price[cell] * take);
                    pay(&mut w.ledger, PURCHASE, account, acc.market[i], value);
                    w.scratch.drawn[cell] += take;
                }
                let used = take + (need - take).clamp(0.0, stock);
                if used > 0.0 {
                    w.ledger.destroy(account, k, used);
                }
                w.scratch.input_before[at] = stock + take;
                w.scratch.input_used[at] = used;
            }
        }
    }
}

/// E2's first step: each enterprise's output capped by capacity and its traded input stocks,
/// into `Scratch::produced`, the start of the same-tick chain.
pub fn cap_output(w: &mut EconWorld) {
    let (inp, lay) = (&*w.inputs, &*w.layout);
    let acc = &lay.accounts;
    let r = rho(w.params.sigma_fl);
    let (ledger, employment) = (&w.ledger, &w.employment);
    let capped: Vec<f64> = (0..inp.n_enterprises())
        .into_par_iter()
        .map(|e| {
            let ent = &inp.enterprises;
            let s_l = inp.countries.s_l[inp.country_of(ent.province[e] as usize)];
            let va = value_added(ent.a[e], ent.va0[e], ent.l0[e], s_l, r, employment[e]);
            let mut y = va / lay.a_va[e];
            for k in (0..N_GOODS).filter(|&k| TRADED[k]) {
                let a = lay.coef[e][k];
                if a > 0.0 {
                    y = y.min(ledger.goods(acc.enterprise[e], k) / a);
                }
            }
            y
        })
        .collect();
    w.scratch.produced.copy_from_slice(&capped);
}

/// E3: refreshes the buyer prices at the current prices, then posts demand on top of the
/// local goods drawn at E2, which count as demand at their province markets. Households
/// spend their whole money balance with PIGL across categories and CES within them
/// (`Layout::pop_demand`, ADR 0021) at the delivered unit price q_d of traded goods and the
/// posted price of local goods. Enterprises order up to an inventory
/// position (stock plus goods on order) of `(1 + COVER + L) coef_k Y_cap` of each input k:
/// orders `(target - stock_k - on_order_k)`, at least zero, where Y_cap = VA(L) / a_VA is
/// capacity output at this tick's employment and L the expected months in transit of the
/// input to the province (0 for local goods), so where L = 0 this is the two-month rule of
/// Task 7b. Owner decisions, 2026-10-24: planning on realized output Y, which a short input
/// caps, let one shortage cut the next order and the next output; a fixed target that counts
/// goods on order cannot hold a month in transit. Traded orders are spending, units times
/// q_d, so the units ordered are the units that arrive; local orders are units at the
/// province market.
pub fn demand(w: &mut EconWorld) {
    w.refresh_price_index();
    let (inp, lay) = (&*w.inputs, &*w.layout);
    let acc = &lay.accounts;
    let n_pops = inp.n_pops();
    let r = rho(w.params.sigma_fl);
    let sc = &mut w.scratch;
    let (ledger, params, index) = (&w.ledger, &w.params, &w.price_index);
    let row = |i: usize| -> [f64; N_GOODS] {
        index[i * N_GOODS..(i + 1) * N_GOODS]
            .try_into()
            .expect("one row of goods")
    };
    sc.pop_demand
        .par_chunks_mut(N_GOODS)
        .zip(sc.pop_spend.par_chunks_mut(N_GOODS))
        .enumerate()
        .for_each(|(k, (x, spend))| {
            let prices = row(inp.pops.province[k] as usize);
            let money = to_numeraire(ledger.money(acc.pop[k]));
            let x: &mut [f64; N_GOODS] = x.try_into().expect("one row of goods");
            lay.pop_demand(inp, params, k, &prices, money, x);
            for g in 0..N_GOODS {
                spend[g] = if TRADED[g] && x[g] > 0.0 {
                    x[g] * prices[g]
                } else {
                    0.0
                };
            }
        });
    let (pipeline, lead) = (&w.pipeline, &sc.lead);
    sc.ent_order
        .par_chunks_mut(N_GOODS)
        .zip(sc.ent_spend.par_chunks_mut(N_GOODS))
        .enumerate()
        .for_each(|(e, (orders, spend))| {
            let ent = &inp.enterprises;
            let i = ent.province[e] as usize;
            let (prices, s_l) = (row(i), inp.countries.s_l[inp.country_of(i)]);
            let va = value_added(ent.a[e], ent.va0[e], ent.l0[e], s_l, r, w.employment[e]);
            let capacity = va / lay.a_va[e];
            for k in 0..N_GOODS {
                let a = lay.coef[e][k];
                let order = if a > 0.0 {
                    let held = ledger.goods(acc.enterprise[e], k);
                    let on_order = pipeline.on_order(n_pops + e, k);
                    let target = 1.0 + COVER + lead[i * N_GOODS + k];
                    (target * a * capacity - held - on_order).max(0.0)
                } else {
                    0.0
                };
                orders[k] = order;
                spend[k] = if TRADED[k] && order > 0.0 && prices[k].is_finite() {
                    order * prices[k]
                } else {
                    0.0
                };
            }
        });
    // local goods: demand at the province market, in pop then enterprise order
    w.markets.demand.copy_from_slice(&sc.drawn);
    for i in 0..inp.n_provinces() {
        for g in (0..N_GOODS).filter(|&g| !TRADED[g]) {
            let cell = i * N_GOODS + g;
            for k in lay.pops(i) {
                w.markets.demand[cell] += sc.pop_demand[k * N_GOODS + g];
            }
            for e in lay.enterprises(i) {
                w.markets.demand[cell] += sc.ent_order[e * N_GOODS + g];
            }
        }
    }
}

/// T2: spreads each destination's spending on every traded good over the active origins at
/// posted prices (`ceteris_trade::source`) and places the orders at the origin markets.
pub fn source(w: &mut EconWorld) {
    let (inp, lay, kernel) = (&*w.inputs, &*w.layout, &*w.kernel);
    let np = inp.n_provinces();
    let sc = &mut w.scratch;
    let (ent_spend, pop_spend) = (&sc.ent_spend, &sc.pop_spend);
    let markets = &w.markets;
    sc.sourced
        .par_iter_mut()
        .zip(sc.columns.par_iter_mut())
        .enumerate()
        .for_each(|(g, (sourced, col))| {
            col.spend.clear();
            col.spend.extend((0..np).map(|d| {
                let mut s = [0.0; 2];
                for e in lay.enterprises(d) {
                    s[INTERMEDIATE] += ent_spend[e * N_GOODS + g];
                }
                for k in lay.pops(d) {
                    s[HOUSEHOLD] += pop_spend[k * N_GOODS + g];
                }
                s
            }));
            col.gather(&markets.price, &markets.active, g);
            source_good(kernel, g, &col.price, &col.active, &col.spend, sourced);
        });
    for (g, (sourced, col)) in sc.sourced.iter().zip(&sc.columns).enumerate() {
        for o in 0..np {
            let cell = o * N_GOODS + g;
            sc.class_spend[cell] = col.spend[o];
            w.markets.demand_int[cell] = sourced.orders[o][INTERMEDIATE];
            w.markets.demand_hh[cell] = sourced.orders[o][HOUSEHOLD];
        }
    }
}

/// T3: clears every active market. Origin markets of traded goods clear their orders
/// against output and merchant stock (S' = S + Q - sales), a shortfall shared between buyer
/// classes by `EconParams::rationing`; the fills are dispatched at T2b. Province markets of
/// local goods hand out what the same-tick chain left of their output (E2): enterprise input
/// orders (refilling stocks) and households share it by `EconParams::rationing`, pro rata
/// within each class; unsold output perishes. Buyers of local goods pay the posted price;
/// households consume what they receive, enterprises stock it.
pub fn clear(w: &mut EconWorld) {
    let (inp, lay) = (&*w.inputs, &*w.layout);
    let acc = &lay.accounts;
    w.markets.clear(w.params.rationing, &mut w.scratch.fill);
    w.markets.clear_local(&mut w.scratch.spoiled);
    let mut buyers: Vec<(AccountId, f64, bool)> = Vec::new();
    for i in 0..inp.n_provinces() {
        let market = acc.market[i];
        for g in (0..N_GOODS).filter(|&g| !TRADED[g]) {
            let cell = i * N_GOODS + g;
            let m = &w.markets;
            if !m.active[cell] {
                continue;
            }
            let keep = w.scratch.spoiled[cell];
            let price = m.price[cell];
            let rest = (m.supply[cell] - w.scratch.drawn[cell]).max(0.0);
            buyers.clear();
            for k in lay.pops(i) {
                buyers.push((acc.pop[k], w.scratch.pop_demand[k * N_GOODS + g], true));
            }
            for e in lay.enterprises(i) {
                buyers.push((
                    acc.enterprise[e],
                    w.scratch.ent_order[e * N_GOODS + g],
                    false,
                ));
            }
            let total = |households: bool| -> f64 {
                buyers
                    .iter()
                    .filter(|b| b.2 == households)
                    .map(|b| b.1)
                    .sum()
            };
            let (orders, households) = (total(false), total(true));
            let [fill_orders, fill_households] = w.params.rationing.fill(rest, orders, households);
            for b in buyers.iter_mut() {
                b.1 *= if b.2 { fill_households } else { fill_orders };
            }
            if let Some(last) = largest(buyers.iter().map(|b| b.1)) {
                let n = buyers.len();
                buyers.swap(last, n - 1);
                for (j, &(buyer, allocation, consumes)) in buyers.iter().enumerate() {
                    let q = if j + 1 == n {
                        // the largest buyer takes the market's balance beyond what stays
                        (w.ledger.goods(market, g) - keep).max(0.0)
                    } else {
                        allocation.min(w.ledger.goods(market, g))
                    };
                    if q > 0.0 {
                        w.ledger.ship(market, buyer, g, q);
                        if consumes {
                            w.ledger.destroy(buyer, g, q);
                        }
                    }
                    pay(&mut w.ledger, PURCHASE, buyer, market, to_minor(price * q));
                }
            }
            let rest = w.ledger.goods(market, g);
            if rest > 0.0 {
                w.ledger.destroy(market, g, rest);
            }
        }
    }
}

/// Spending on traded goods per enterprise, per pop and per destination cell by class
/// (`Scratch::ent_spend`, `pop_spend`, `class_spend`).
#[derive(Clone, Copy)]
struct Spending<'a> {
    enterprise: &'a [f64],
    pop: &'a [f64],
    class: &'a [[f64; 2]],
}

/// Buyers of good `g` in province `d` with positive spending, enterprises then pops.
fn buyers_of(lay: &Layout, spend: Spending, d: usize, g: usize, out: &mut Vec<Buyer>) {
    let acc = &lay.accounts;
    let n_pops = acc.pop.len();
    out.clear();
    for e in lay.enterprises(d) {
        let spend = spend.enterprise[e * N_GOODS + g];
        if spend > 0.0 {
            out.push(Buyer {
                account: acc.enterprise[e],
                index: n_pops + e,
                class: INTERMEDIATE,
                spend,
            });
        }
    }
    for k in lay.pops(d) {
        let spend = spend.pop[k * N_GOODS + g];
        if spend > 0.0 {
            out.push(Buyer {
                account: acc.pop[k],
                index: k,
                class: HOUSEHOLD,
                spend,
            });
        }
    }
}

/// Each buyer's share of good `g`'s dispatch `r` at its destination, in province and buyer
/// order: what it may take home now, what it gets in each pipeline slot, and its payment to
/// the destination market, a ledger entry (left out when zero, as `pay` leaves it).
fn buyer_orders(lay: &Layout, spend: Spending, g: usize, r: &Sourced, p: &mut Payments) {
    let acc = &lay.accounts;
    p.orders.clear();
    p.order_ends.clear();
    p.entries.clear();
    for d in 0..acc.market.len() {
        let (cell, market) = (d * N_GOODS + g, acc.market[d]);
        buyers_of(lay, spend, d, g, &mut p.buyers);
        for b in &p.buyers {
            let share = b.spend / spend.class[cell][b.class];
            let slots = std::array::from_fn(|s| r.arrive[s + 1][d][b.class] * share);
            p.orders.push(Order {
                account: b.account,
                buyer: b.index,
                household: b.class == HOUSEHOLD,
                home: r.arrive[0][d][b.class] * share,
                slots,
            });
            let amount = to_minor(r.paid_by_class[d][b.class] * share);
            if amount != 0 {
                p.entries.push(Entry {
                    flow: PURCHASE,
                    from: b.account,
                    to: market,
                    amount,
                });
            }
        }
        p.order_ends.push(p.orders.len() as u32);
    }
}

/// T2b: ships the filled orders of every traded good (`ceteris_trade::dispatch`). Home
/// purchases go straight from the market to their buyers (pops consume them); shipments
/// enter the transit account (iceberg and border losses destroyed) and each buyer's
/// pipeline, due at t + slot. Each buyer pays its filled spending, origin price times
/// (1 + t) per shipped unit, to its destination market, rounded half to even per buyer and
/// good; the market passes it on: domestic legs to the origin market, international legs
/// through the gateways of both countries (rounded per leg, gateway splits cumulatively, so
/// each gateway nets to zero), the tariff to its country's rebate account. Unfilled spending
/// stays with the buyer. Records the country-pair flows.
pub fn dispatch(w: &mut EconWorld) {
    let inp = std::sync::Arc::clone(&w.inputs);
    let lay = std::sync::Arc::clone(&w.layout);
    let kernel = std::sync::Arc::clone(&w.kernel);
    let (inp, lay, kernel) = (&*inp, &*lay, &*kernel);
    let acc = &lay.accounts;
    let np = kernel.n_provinces();
    let sc = &mut w.scratch;
    let (fill, markets, net) = (&sc.fill, &w.markets, &inp.network);
    let spending = Spending {
        enterprise: &sc.ent_spend,
        pop: &sc.pop_spend,
        class: &sc.class_spend,
    };
    (
        &mut sc.sourced,
        &mut sc.columns,
        &mut sc.payments,
        sc.flows.current(),
    )
        .into_par_iter()
        .enumerate()
        .for_each(|(g, (r, col, legs, flows))| {
            col.gather(&markets.price, &markets.active, g);
            col.fill.clear();
            col.fill.extend((0..np).map(|o| fill[o * N_GOODS + g]));
            dispatch_good(kernel, g, &col.price, &col.fill, r);
            let r = &*r;
            rayon::join(
                || {
                    let (pc, gc) = (&net.province_country, &net.gateway_country);
                    flows.record(kernel, pc, gc, g, &col.price, r);
                },
                || {
                    buyer_orders(lay, spending, g, r, legs);
                    trade_legs(inp, acc, kernel, g, r, legs);
                },
            );
        });
    let sourced = std::mem::take(&mut w.scratch.sourced);
    let payments = std::mem::take(&mut w.scratch.payments);
    for (g, (r, legs)) in sourced.iter().zip(&payments).enumerate() {
        let transit = acc.transit[g];
        let mut first = 0;
        for d in 0..np {
            let (cell, market) = (d * N_GOODS + g, acc.market[d]);
            w.scratch.dispatched[cell] = r.shipped[d];
            // buyers at d: home purchases now, shipments into the pipeline
            let end = legs.order_ends[d] as usize;
            for o in &legs.orders[first..end] {
                let home = o.home.min(w.ledger.goods(market, g));
                if home > 0.0 {
                    w.ledger.ship(market, o.account, g, home);
                    if o.household {
                        w.ledger.destroy(o.account, g, home);
                    }
                }
                for (slot, &units) in o.slots.iter().enumerate() {
                    if units > 0.0 {
                        w.pipeline.push(slot + 1, o.buyer, g, units);
                    }
                }
            }
            first = end;
            // what leaves the origin market beyond its home sales and its carried stock
            let x = (w.ledger.goods(market, g) - w.markets.inventory[cell]).max(0.0);
            if x > 0.0 {
                w.ledger.ship(market, transit, g, x);
            }
        }

        // the buyers' payments, then the legs
        w.ledger.post(&legs.entries, &mut w.scratch.post);
    }
    // the losses take what each transit account holds beyond the goods now in flight
    let in_flight = w.pipeline.in_flight_all();
    for (g, &transit) in acc.transit.iter().enumerate() {
        let lost = (w.ledger.goods(transit, g) - in_flight[g]).max(0.0);
        w.ledger.destroy(transit, g, lost);
    }
    w.scratch.sourced = sourced;
    w.scratch.payments = payments;
}

/// The money legs of good `g`'s dispatch `r` in minor units, rounded half to even per leg,
/// as ledger entries in payment order, after the buyers' ([`buyer_orders`]): destinations
/// pay origins and their gateways, each gateway splits what it takes in over the legs that
/// fed it ([`split`]), so it pays out exactly that, and destinations pay tariffs to their
/// country's rebate account. Zero amounts are left out, as `pay` leaves them.
fn trade_legs(
    inp: &EconInputs,
    acc: &Accounts,
    kernel: &Kernel,
    g: usize,
    r: &Sourced,
    p: &mut Payments,
) {
    let (np, ng) = (kernel.n_provinces(), kernel.n_gateways());
    let gk = kernel.good(g);
    let entries = &mut p.entries;
    let mut leg = |flow: Flow, from: AccountId, to: AccountId, amount: i64| {
        if amount != 0 {
            entries.push(Entry {
                flow,
                from,
                to,
                amount,
            });
        }
    };
    for o in 0..np {
        let lo = gk.domestic.offsets[o] as usize;
        for (j, e) in gk.domestic.row(o).iter().enumerate() {
            let value = to_minor(r.domestic_value[lo + j]);
            leg(TRADE, acc.market[e.col as usize], acc.market[o], value);
        }
    }
    p.at_gateway.clear();
    p.at_gateway.resize(ng, 0);
    for d in 0..np {
        let lo = gk.access.offsets[d] as usize;
        for (j, e) in gk.access.row(d).iter().enumerate() {
            let (b, value) = (e.col as usize, to_minor(r.access_in_value[lo + j]));
            leg(TRADE, acc.market[d], acc.gateway[b], value);
            p.at_gateway[b] += value;
        }
    }
    p.from_gateway.clear();
    p.from_gateway.resize(ng, 0);
    for b in 0..ng {
        let legs = kernel.crossings_by_target().column(b);
        let weights = legs.iter().map(|&(_, j)| r.crossing_value[j as usize]);
        split(p.at_gateway[b], weights, &mut p.parts);
        for (&(a, _), &part) in legs.iter().zip(&p.parts) {
            leg(TRADE, acc.gateway[b], acc.gateway[a as usize], part);
            p.from_gateway[a as usize] += part;
        }
    }
    for a in 0..ng {
        let legs = kernel.access_by_gateway().column(a);
        let weights = legs.iter().map(|&(_, j)| r.access_out_value[j as usize]);
        split(p.from_gateway[a], weights, &mut p.parts);
        for (&(o, _), &part) in legs.iter().zip(&p.parts) {
            leg(TRADE, acc.gateway[a], acc.market[o as usize], part);
        }
    }
    for d in 0..np {
        let rebate = acc.rebate[inp.country_of(d)];
        leg(TARIFF, acc.market[d], rebate, to_minor(r.tariff[d]));
    }
}

/// T4: the stock-flow price rule with expected inflation on every active market (cells
/// without a market keep their price, which nothing reads), then the update of expected
/// inflation from the world price index (ADR 0019).
pub fn reprice(w: &mut EconWorld) {
    w.markets.reprice(&w.trade, w.expected_inflation);
    w.update_expectation();
}

/// E4 (national ownership, owner 2026-10-25; limited liability of enterprises, owner
/// 2026-10-26; loss write-off, ADR 0098): every enterprise pays its balance into its country's
/// profit pool when the balance is positive and otherwise keeps it, so a loss stays with the
/// enterprise as retained losses, which later cash flow repays before anything is paid out;
/// every province market pays its signed cash flow (ADR 0098, Markets). Each country pays its pool and its rebate account (tariffs) to its pops by population
/// share N_k / N_c, so both end the tick at zero. Each pool is one i64 sum, split once per tick
/// by [`split`]: the cumulative population shares of the country's pops in ID order rounded
/// half to even, so the parts add up to the pool exactly. Then [`write_off`]. Records each
/// pop's income of the tick, and as each enterprise's profit its balance before the payout
/// (this tick's cash flow less any loss it carries).
pub fn distribute(w: &mut EconWorld) {
    let (inp, lay) = (&*w.inputs, &*w.layout);
    let acc = &lay.accounts;
    for i in 0..inp.n_provinces() {
        let pool = acc.dividend[inp.country_of(i)];
        for e in lay.enterprises(i) {
            let cash = w.ledger.money(acc.enterprise[e]);
            w.profit[e] = to_numeraire(cash);
            pay(
                &mut w.ledger,
                DIVIDEND,
                acc.enterprise[e],
                pool,
                cash.max(0),
            );
        }
        let cash = w.ledger.money(acc.market[i]);
        pay(&mut w.ledger, DIVIDEND, acc.market[i], pool, cash);
    }
    let mut parts = Vec::new();
    for (c, pops) in lay.country_pops.iter().enumerate() {
        for (flow, payer) in [(DIVIDEND, acc.dividend[c]), (REBATE, acc.rebate[c])] {
            let weights = pops.iter().map(|&k| inp.pops.population[k as usize]);
            split(w.ledger.money(payer), weights, &mut parts);
            for (&k, &part) in pops.iter().zip(&parts) {
                pay(&mut w.ledger, flow, payer, acc.pop[k as usize], part);
                w.scratch.income_paid[k as usize] += part;
            }
        }
    }
    for (income, &paid) in w.income.iter_mut().zip(&w.scratch.income_paid) {
        *income = to_numeraire(paid);
    }
    write_off(w);
}

/// The loss write-off of E4 (ADR 0098): per country, the excess of every enterprise's retained
/// loss over its cap (`Layout::ent_loss_cap`) is written off against the country's pops in proportion to their positive money, never
/// taking a pop below zero. Where the pops hold less than the excess, what they hold is
/// written off, shared over the holders by their excess, and the rest stays with them. Both
/// sides are exact [`largest_remainder`] splits of one i64 per country, moved through the
/// country's profit pool account, which ends at zero.
pub fn write_off(w: &mut EconWorld) {
    let (inp, lay) = (&*w.inputs, &*w.layout);
    let acc = &lay.accounts;
    let mut holders: Vec<Vec<(AccountId, i64)>> = vec![Vec::new(); inp.n_countries()];
    for i in 0..inp.n_provinces() {
        let c = inp.country_of(i);
        for e in lay.enterprises(i) {
            let excess = (-w.ledger.money(acc.enterprise[e])).saturating_sub(lay.ent_loss_cap[e]);
            if excess > 0 {
                holders[c].push((acc.enterprise[e], excess));
            }
        }
    }
    let (mut weights, mut parts) = (Vec::new(), Vec::new());
    for (c, holders) in holders.iter().enumerate().filter(|(_, h)| !h.is_empty()) {
        let pool = acc.dividend[c];
        let pops = &lay.country_pops[c];
        weights.clear();
        weights.extend(
            pops.iter()
                .map(|&k| w.ledger.money(acc.pop[k as usize]).max(0)),
        );
        let held: i128 = weights.iter().map(|&m| i128::from(m)).sum();
        let excess: i128 = holders.iter().map(|&(_, x)| i128::from(x)).sum();
        let total = i64::try_from(held.min(excess)).expect("write-off within i64");
        if total == 0 {
            continue;
        }
        largest_remainder(total, &weights, &mut parts);
        for (&k, &part) in pops.iter().zip(&parts) {
            pay(&mut w.ledger, WRITE_OFF, acc.pop[k as usize], pool, part);
        }
        weights.clear();
        weights.extend(holders.iter().map(|&(_, x)| x));
        largest_remainder(total, &weights, &mut parts);
        for (&(account, _), &part) in holders.iter().zip(&parts) {
            pay(&mut w.ledger, WRITE_OFF, pool, account, part);
        }
    }
}

/// Splits `total` minor units over nonnegative `weights` (summing to at least `total`) by the
/// largest remainder: each part is the floor of its exact share, and the units left over go
/// one each to the largest remainders, ties to the first. The parts add up to `total`, and
/// none exceeds the ceiling of its share, so none exceeds its weight.
fn largest_remainder(total: i64, weights: &[i64], out: &mut Vec<i64>) {
    let sum: i128 = weights.iter().map(|&x| i128::from(x)).sum();
    let t = i128::from(total);
    out.clear();
    let mut remainders: Vec<(i128, usize)> = Vec::with_capacity(weights.len());
    for (j, &x) in weights.iter().enumerate() {
        let product = t * i128::from(x);
        out.push(i64::try_from(product / sum).expect("part within i64"));
        remainders.push((product % sum, j));
    }
    let left = total - out.iter().sum::<i64>();
    remainders.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    for &(_, j) in remainders
        .iter()
        .take(usize::try_from(left).expect("nonnegative"))
    {
        out[j] += 1;
    }
}

/// E5: the invariants are checked by `State::end_tick` after the schedule; debug builds also
/// check them here, so a violation stops the run inside the tick that caused it.
pub fn assert_invariants(w: &mut EconWorld, tick: u32) {
    if cfg!(debug_assertions)
        && let Err(v) = w.end_tick(tick)
    {
        panic!("{v}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_adds_up_exactly() {
        let mut out = Vec::new();
        split(10, [1.0, 1.0, 1.0].into_iter(), &mut out);
        assert_eq!(out.iter().sum::<i64>(), 10);
        assert!(out.iter().all(|&p| (3..=4).contains(&p)));
        split(-7, [0.2, 0.3, 0.5].into_iter(), &mut out);
        assert_eq!(out.iter().sum::<i64>(), -7);
        split(5, [0.0, 0.0].into_iter(), &mut out);
        assert_eq!(out, [0, 5]);
        split(0, [0.4, 0.6].into_iter(), &mut out);
        assert_eq!(out, [0, 0]);
    }

    #[test]
    fn unreachable_inputs_hire_no_one_and_unused_ones_change_nothing() {
        let mut index = [1.0; N_GOODS];
        index[3] = f64::INFINITY;
        let a = |k: usize| [0.1, 0.0, 0.0, 0.2, 0.0, 0.0, 0.0, 0.0, 0.0, 0.1][k];
        let used = value_added_price(2.0, a, &index, 0.6);
        assert_eq!(used, f64::NEG_INFINITY);
        let target = labour_target(used, 1.0, 1.0, 10.0, 5.0, 0.6, rho(0.8), 100.0);
        assert_eq!(target, 0.0);
        let b = |k: usize| if k == 3 { 0.0 } else { a(k) };
        assert!((value_added_price(2.0, b, &index, 0.6) - 1.8 / 0.6).abs() < 1e-15);
    }

    #[test]
    fn largest_takes_the_first_maximum() {
        assert_eq!(largest([0.0, 2.0, 1.0, 2.0].into_iter()), Some(1));
        assert_eq!(largest([0.0, -1.0].into_iter()), None);
    }
}
