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

use ceteris_core::ledger::{AccountId, Flow, Ledger};
use ceteris_core::reduce::fixed_sum;
use ceteris_core::rng::Seed;
use ceteris_core::schedule::{Cadence, Schedule, SystemDef, SystemId};
use ceteris_core::sim::State;
use ceteris_trade::goods::TRADED;
use ceteris_trade::kernel::Csr;
use ceteris_trade::source::{HOUSEHOLD, INTERMEDIATE};
use ceteris_trade::{N_GOODS, dispatch as dispatch_good, source as source_good};

use crate::labour::{allocate, clearing_wage, wage_step};
use crate::local::{self, LOCAL};
use crate::production::{labour_target, rho, value_added};
use crate::world::{
    DIVIDEND, EconWorld, PURCHASE, REBATE, SALE, TARIFF, TRADE, WAGE, to_minor, to_numeraire,
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

/// Entries of a CSR grouped by column, stable: `(offsets by column, (row, entry) pairs)`.
fn by_column<E>(
    csr: &Csr<E>,
    col: impl Fn(&E) -> usize,
    n_cols: usize,
) -> (Vec<usize>, Vec<(usize, usize)>) {
    let mut offsets = vec![0usize; n_cols + 1];
    for e in &csr.entries {
        offsets[col(e) + 1] += 1;
    }
    for c in 0..n_cols {
        offsets[c + 1] += offsets[c];
    }
    let mut next = offsets.clone();
    let mut pairs = vec![(0, 0); csr.entries.len()];
    for row in 0..csr.n_rows() {
        let lo = csr.offsets[row] as usize;
        for (j, e) in csr.row(row).iter().enumerate() {
            let c = col(e);
            pairs[next[c]] = (row, lo + j);
            next[c] += 1;
        }
    }
    (offsets, pairs)
}

/// T1: goods due this tick move from the transit accounts to their buyers: pops consume
/// them on arrival, enterprises add them to their input stocks.
pub fn deliver(w: &mut EconWorld) {
    let lay = &*w.layout;
    let acc = &lay.accounts;
    let n_buyers = w.pipeline.n_buyers();
    let n_pops = acc.pop.len();
    w.scratch.due.copy_from_slice(w.pipeline.pop_due());
    let account = |b: usize| {
        if b < n_pops {
            acc.pop[b]
        } else {
            acc.enterprise[b - n_pops]
        }
    };
    for (g, &transit) in acc.transit.iter().enumerate() {
        let due = |b: usize| w.scratch.due[b * N_GOODS + g];
        let Some(last) = largest((0..n_buyers).map(due)) else {
            continue;
        };
        let receive = |ledger: &mut Ledger, b: usize, x: f64| {
            ledger.ship(transit, account(b), g, x);
            if b < n_pops {
                ledger.destroy(account(b), g, x);
            }
        };
        for b in (0..n_buyers).filter(|&b| b != last && due(b) > 0.0) {
            let x = due(b).min(w.ledger.goods(transit, g));
            receive(&mut w.ledger, b, x);
        }
        // the largest delivery takes what the account holds beyond the goods still in flight
        let x = (w.ledger.goods(transit, g) - w.pipeline.in_flight(g)).max(0.0);
        receive(&mut w.ledger, last, x);
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
    let (inp, lay) = (
        std::sync::Arc::clone(&w.inputs),
        std::sync::Arc::clone(&w.layout),
    );
    let (inp, lay) = (&*inp, &*lay);
    let acc = &lay.accounts;
    let edu_eff = w.params.edu_eff;
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

    w.markets.supply.fill(0.0);
    w.scratch.drawn.fill(0.0);
    let mut parts = Vec::new();
    for i in 0..inp.n_provinces() {
        w.scratch.chain_iterations[i] = local::chain(w, i) as u32;
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
    w.markets.demand.copy_from_slice(&sc.drawn);
    let mut x = [0.0; N_GOODS];
    for i in 0..inp.n_provinces() {
        let c = inp.country_of(i);
        let row = i * N_GOODS..(i + 1) * N_GOODS;
        let prices: [f64; N_GOODS] = w.price_index[row.clone()]
            .try_into()
            .expect("one row of goods");
        for k in lay.pops(i) {
            let money = to_numeraire(w.ledger.money(acc.pop[k]));
            lay.pop_demand(inp, &w.params, k, &prices, money, &mut x);
            for g in 0..N_GOODS {
                let at = k * N_GOODS + g;
                sc.pop_demand[at] = x[g];
                sc.pop_spend[at] = if TRADED[g] && x[g] > 0.0 {
                    x[g] * prices[g]
                } else {
                    0.0
                };
                if !TRADED[g] {
                    w.markets.demand[i * N_GOODS + g] += x[g];
                }
            }
        }
        let s_l = inp.countries.s_l[c];
        for e in lay.enterprises(i) {
            let ent = &inp.enterprises;
            let va = value_added(ent.a[e], ent.va0[e], ent.l0[e], s_l, r, w.employment[e]);
            let capacity = va / lay.a_va[e];
            for k in 0..N_GOODS {
                let a = lay.coef[e][k];
                let at = e * N_GOODS + k;
                let order = if a > 0.0 {
                    let held = w.ledger.goods(acc.enterprise[e], k);
                    let on_order = w.pipeline.on_order(n_pops + e, k);
                    let target = 1.0 + COVER + sc.lead[i * N_GOODS + k];
                    (target * a * capacity - held - on_order).max(0.0)
                } else {
                    0.0
                };
                sc.ent_order[at] = order;
                sc.ent_spend[at] = 0.0;
                if !TRADED[k] {
                    w.markets.demand[i * N_GOODS + k] += order;
                } else if order > 0.0 && prices[k].is_finite() {
                    sc.ent_spend[at] = order * prices[k];
                }
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
    let (mut price, mut active) = (vec![0.0; np], vec![false; np]);
    let mut spend = vec![[0.0; 2]; np];
    for (g, sourced) in sc.sourced.iter_mut().enumerate() {
        for d in 0..np {
            let cell = d * N_GOODS + g;
            let mut s = [0.0; 2];
            for e in lay.enterprises(d) {
                s[INTERMEDIATE] += sc.ent_spend[e * N_GOODS + g];
            }
            for k in lay.pops(d) {
                s[HOUSEHOLD] += sc.pop_spend[k * N_GOODS + g];
            }
            sc.class_spend[cell] = s;
            spend[d] = s;
            price[d] = w.markets.price[cell];
            active[d] = w.markets.active[cell];
        }
        source_good(kernel, g, &price, &active, &spend, sourced);
        for o in 0..np {
            let cell = o * N_GOODS + g;
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

/// One buyer of a traded good at its destination: ledger account, pipeline index, class
/// and spending.
struct Buyer {
    account: AccountId,
    index: usize,
    class: usize,
    spend: f64,
}

/// Buyers of good `g` in province `d` with positive spending, enterprises then pops.
fn buyers_of(w: &EconWorld, d: usize, g: usize, out: &mut Vec<Buyer>) {
    let (lay, sc) = (&*w.layout, &w.scratch);
    let acc = &lay.accounts;
    let n_pops = acc.pop.len();
    out.clear();
    for e in lay.enterprises(d) {
        let spend = sc.ent_spend[e * N_GOODS + g];
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
        let spend = sc.pop_spend[k * N_GOODS + g];
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
    let (np, ng) = (kernel.n_provinces, kernel.n_gateways);
    let mut price = vec![0.0; np];
    let (mut fill, mut parts, mut buyers) = (vec![[0.0; 2]; np], Vec::new(), Vec::new());
    let mut sourced = std::mem::take(&mut w.scratch.sourced);
    for (g, r) in sourced.iter_mut().enumerate() {
        let transit = acc.transit[g];
        for o in 0..np {
            let cell = o * N_GOODS + g;
            price[o] = w.markets.price[cell];
            fill[o] = w.scratch.fill[cell];
        }
        dispatch_good(kernel, g, &price, &fill, r);
        w.scratch.flows.record(
            kernel,
            &inp.network.province_country,
            &inp.network.gateway_country,
            g,
            &price,
            r,
        );

        for d in 0..np {
            let (cell, market) = (d * N_GOODS + g, acc.market[d]);
            w.scratch.dispatched[cell] = r.shipped[d];
            // buyers at d: home purchases now, shipments into the pipeline, payments
            buyers_of(w, d, g, &mut buyers);
            for b in &buyers {
                let share = b.spend / w.scratch.class_spend[cell][b.class];
                let home = (r.arrive[0][d][b.class] * share).min(w.ledger.goods(market, g));
                if home > 0.0 {
                    w.ledger.ship(market, b.account, g, home);
                    if b.class == HOUSEHOLD {
                        w.ledger.destroy(b.account, g, home);
                    }
                }
                for slot in 1..r.arrive.len() {
                    let units = r.arrive[slot][d][b.class] * share;
                    if units > 0.0 {
                        w.pipeline.push(slot, b.index, g, units);
                    }
                }
                let value = to_minor(r.paid_by_class[d][b.class] * share);
                pay(&mut w.ledger, PURCHASE, b.account, market, value);
            }
            // what leaves the origin market beyond its home sales and its carried stock
            let x = (w.ledger.goods(market, g) - w.markets.inventory[cell]).max(0.0);
            if x > 0.0 {
                w.ledger.ship(market, transit, g, x);
            }
        }
        // the losses take what the account holds beyond the goods now in flight
        let lost = (w.ledger.goods(transit, g) - w.pipeline.in_flight(g)).max(0.0);
        w.ledger.destroy(transit, g, lost);

        let gk = kernel.good(g);
        for o in 0..np {
            let lo = gk.domestic.offsets[o] as usize;
            for (j, e) in gk.domestic.row(o).iter().enumerate() {
                let value = to_minor(r.domestic_value[lo + j]);
                pay(
                    &mut w.ledger,
                    TRADE,
                    acc.market[e.col as usize],
                    acc.market[o],
                    value,
                );
            }
        }
        // destination to its gateway b, net of tariff, rounded per leg
        let mut at_gateway = vec![0i64; ng];
        for d in 0..np {
            let lo = gk.access.offsets[d] as usize;
            for (j, e) in gk.access.row(d).iter().enumerate() {
                let b = e.col as usize;
                let value = to_minor(r.access_in_value[lo + j]);
                pay(&mut w.ledger, TRADE, acc.market[d], acc.gateway[b], value);
                at_gateway[b] += value;
            }
        }
        // b to the origin gateways a that fed it, then a to the origins: exact splits
        let mut from_gateway = vec![0i64; ng];
        let (off, pairs) = by_column(&gk.crossings, |e| e.col as usize, ng);
        for b in 0..ng {
            let legs = &pairs[off[b]..off[b + 1]];
            let weights = legs.iter().map(|&(_, j)| r.crossing_value[j]);
            split(at_gateway[b], weights, &mut parts);
            for (&(a, _), &part) in legs.iter().zip(&parts) {
                pay(&mut w.ledger, TRADE, acc.gateway[b], acc.gateway[a], part);
                from_gateway[a] += part;
            }
        }
        let (off, pairs) = by_column(&gk.access, |e| e.col as usize, ng);
        for a in 0..ng {
            let legs = &pairs[off[a]..off[a + 1]];
            let weights = legs.iter().map(|&(_, j)| r.access_out_value[j]);
            split(from_gateway[a], weights, &mut parts);
            for (&(o, _), &part) in legs.iter().zip(&parts) {
                pay(&mut w.ledger, TRADE, acc.gateway[a], acc.market[o], part);
            }
        }
        for d in 0..np {
            let rebate = acc.rebate[inp.country_of(d)];
            pay(
                &mut w.ledger,
                TARIFF,
                acc.market[d],
                rebate,
                to_minor(r.tariff[d]),
            );
        }
    }
    w.scratch.sourced = sourced;
}

/// T4: the stock-flow price rule with expected inflation on every active market (cells
/// without a market keep their price, which nothing reads), then the update of expected
/// inflation from the world price index (ADR 0019).
pub fn reprice(w: &mut EconWorld) {
    w.markets.reprice(&w.trade, w.expected_inflation);
    w.update_expectation();
}

/// E4 (national ownership, owner 2026-10-25): every enterprise and province market pays its
/// signed cash flow of the tick into its country's profit pool, and each country pays its pool
/// and its rebate account (tariffs) to its pops by population share N_k / N_c, so all of them
/// end the tick at zero. Each pool is one i64 sum, split once per tick by [`split`]: the
/// cumulative population shares of the country's pops in ID order rounded half to even, so the
/// parts add up to the pool exactly. Records each pop's income of the tick.
pub fn distribute(w: &mut EconWorld) {
    let (inp, lay) = (&*w.inputs, &*w.layout);
    let acc = &lay.accounts;
    for i in 0..inp.n_provinces() {
        let pool = acc.dividend[inp.country_of(i)];
        for e in lay.enterprises(i) {
            let cash = w.ledger.money(acc.enterprise[e]);
            w.profit[e] = to_numeraire(cash);
            pay(&mut w.ledger, DIVIDEND, acc.enterprise[e], pool, cash);
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
