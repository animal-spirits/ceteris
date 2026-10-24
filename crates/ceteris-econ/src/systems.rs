//! The P2 tick pipeline (`specs/econ/SPEC.md`, System order; `specs/trade/SPEC.md`, System
//! order): T1 deliver, E1 labour, E2 produce, T2 route, E3 demand, T3 clear, T4 reprice,
//! E4 distribute, E5 assert.
//!
//! Per-entity computations that are heavy (labour targets, the clearing wage, production)
//! run with rayon into fixed slots; every ledger operation is applied afterwards, serially,
//! in province, enterprise or pop ID order, so results do not depend on the thread count.
//!
//! The ledger is the authority on quantities held. Where an account is emptied by computed
//! amounts (transit at T1, markets at T3), the largest recipient takes the exact remaining
//! balance less what must stay, so floating-point residue never leaves a negative holding.

use rayon::prelude::*;

use ceteris_core::ledger::{AccountId, Flow, Ledger};
use ceteris_core::reduce::fixed_sum;
use ceteris_core::rng::Seed;
use ceteris_core::schedule::{Cadence, Schedule, SystemDef, SystemId};
use ceteris_core::sim::State;
use ceteris_trade::goods::TRADED;
use ceteris_trade::kernel::Csr;
use ceteris_trade::{N_GOODS, route as route_good};

use crate::demand::ces;
use crate::labour::{allocate, clearing_wage, wage_step};
use crate::production::{labour_target, rho, value_added};
use crate::world::{
    DIVIDEND, EconWorld, PURCHASE, REBATE, SALE, TARIFF, TRADE, WAGE, to_minor, to_numeraire,
};

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
        def(204, "route", |w, _| route(w)),
        def(205, "demand", |w, _| demand(w)),
        def(206, "clear", |w, _| clear(w)),
        def(207, "reprice", |w, _| reprice(w)),
        def(208, "distribute", |w, _| distribute(w)),
        def(209, "assert", |w, ctx| assert_invariants(w, ctx.tick)),
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

/// T1: goods due this tick move from the transit accounts into the destination markets and
/// make up Q with the home flow routed at T2. Resets Q.
pub fn deliver(w: &mut EconWorld) {
    let lay = &*w.layout;
    let np = w.markets.n_provinces();
    w.markets.supply.fill(0.0);
    w.scratch.due.copy_from_slice(w.transit.pop_due());
    for (g, &transit) in lay.accounts.transit.iter().enumerate() {
        let due = |i: usize| w.scratch.due[i * N_GOODS + g];
        let Some(last) = largest((0..np).map(due)) else {
            continue;
        };
        for i in (0..np).filter(|&i| i != last && due(i) > 0.0) {
            let x = due(i).min(w.ledger.goods(transit, g));
            w.ledger.ship(transit, lay.accounts.market[i], g, x);
            w.markets.supply[i * N_GOODS + g] = x;
        }
        // the largest delivery takes what the account holds beyond the goods still in flight
        let left = w.transit.in_flight(g);
        let x = (w.ledger.goods(transit, g) - left).max(0.0);
        w.ledger.ship(transit, lay.accounts.market[last], g, x);
        w.markets.supply[last * N_GOODS + g] = x;
    }
}

/// E1: labour targets L* at current prices and wages, then the wage (wage rule on excess
/// demand, or the clearing wage when frictionless), employment toward L* and employed pops.
pub fn labour(w: &mut EconWorld) {
    let (inp, lay, params) = (&*w.inputs, &*w.layout, &w.params);
    let c_max = w.trade.c_max;
    let r = rho(params.sigma_fl);
    let (price, wage, prev) = (&w.markets.price, &w.wage, &w.employment);
    let results: Vec<(f64, Vec<f64>)> = (0..inp.n_provinces())
        .into_par_iter()
        .map(|i| {
            let c = inp.country_of(i);
            let s_l = inp.countries.s_l[c];
            let supply = lay.labour_supply[i];
            let p = &price[i * N_GOODS..(i + 1) * N_GOODS];
            let ents = lay.enterprises(i);
            let p_va: Vec<f64> = ents
                .clone()
                .map(|e| {
                    let g = usize::from(inp.enterprises.good[e]);
                    let inputs: f64 = (0..N_GOODS).map(|k| inp.a(c, k, g) * p[k]).sum();
                    (p[g] - inputs) / lay.a_va[e]
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
            let (w1, t) = if params.frictionless {
                let demand = |w: f64| -> f64 {
                    ents.clone()
                        .zip(&p_va)
                        .map(|(e, &pv)| target(e, pv, w))
                        .sum()
                };
                let mut w1 = clearing_wage(w0, supply, demand);
                // no root: the search ran to an extreme; take at most a doubling or halving
                if (demand(w1) - supply).abs() > NO_ROOT * supply {
                    w1 = w1.clamp(0.5 * w0, 2.0 * w0);
                }
                (w1, targets(w1))
            } else {
                let t = targets(w0);
                let excess = (fixed_sum(&t) - supply) / supply;
                (wage_step(w0, excess, params, c_max), t)
            };
            let mut l = vec![0.0; t.len()];
            allocate(&t, &prev[ents], params.eta, supply, &mut l);
            (w1, l)
        })
        .collect();
    for (i, (w1, l)) in results.into_iter().enumerate() {
        w.wage[i] = w1;
        w.employment[lay.enterprises(i)].copy_from_slice(&l);
        let employed = fixed_sum(&l) / lay.labour_supply[i];
        for k in lay.pops(i) {
            w.employed[k] = inp.pops.labour_force[k] * employed;
        }
    }
}

/// E2: Leontief output from employment and input stocks; inputs used are destroyed, output
/// is sold to the province market at its current price, and wages are paid to the pops of
/// the province by effective employed labour.
pub fn produce(w: &mut EconWorld) {
    let (inp, lay) = (&*w.inputs, &*w.layout);
    let acc = &lay.accounts;
    let edu_eff = w.params.edu_eff;
    let r = rho(w.params.sigma_fl);
    let (ledger, employment) = (&w.ledger, &w.employment);
    let output: Vec<f64> = (0..inp.n_enterprises())
        .into_par_iter()
        .map(|e| {
            let ent = &inp.enterprises;
            let i = ent.province[e] as usize;
            let (c, g) = (inp.country_of(i), usize::from(ent.good[e]));
            let s_l = inp.countries.s_l[c];
            let va = value_added(ent.a[e], ent.va0[e], ent.l0[e], s_l, r, employment[e]);
            let mut y = va / lay.a_va[e];
            for k in 0..N_GOODS {
                let a = inp.a(c, k, g);
                if a > 0.0 {
                    y = y.min(ledger.goods(acc.enterprise[e], k) / a);
                }
            }
            y
        })
        .collect();

    w.scratch.route_supply.fill(0.0);
    let mut parts = Vec::new();
    for i in 0..inp.n_provinces() {
        let c = inp.country_of(i);
        let pops = lay.pops(i);
        let effective: Vec<f64> = pops
            .clone()
            .map(|k| edu_eff[usize::from(inp.pops.edu[k])] * w.employed[k])
            .collect();
        for e in lay.enterprises(i) {
            let (g, y) = (usize::from(inp.enterprises.good[e]), output[e]);
            let (account, cell) = (acc.enterprise[e], i * N_GOODS + g);
            for k in 0..N_GOODS {
                let a = inp.a(c, k, g);
                let before = w.ledger.goods(account, k);
                if a > 0.0 {
                    w.ledger.destroy(account, k, (a * y).min(before));
                }
                w.scratch.input_before[e * N_GOODS + k] = before;
                w.scratch.input_used[e * N_GOODS + k] = before - w.ledger.goods(account, k);
            }
            w.scratch.produced[e] = y;
            w.output[e] = y;
            if y > 0.0 {
                w.ledger.create(account, g, y);
                w.ledger.ship(account, acc.market[i], g, y);
            }
            let sale = w.markets.price[cell] * y;
            w.sales[e] = sale;
            pay(&mut w.ledger, SALE, acc.market[i], account, to_minor(sale));
            if TRADED[g] {
                w.scratch.route_supply[cell] = y;
            } else {
                w.markets.supply[cell] += y;
            }

            let bill = to_minor(w.wage[i] * w.employment[e]);
            split(bill, effective.iter().copied(), &mut parts);
            for (k, &part) in pops.clone().zip(&parts) {
                pay(&mut w.ledger, WAGE, account, acc.pop[k], part);
                w.scratch.income_paid[k] += part;
            }
        }
    }
}

/// T2: routes this tick's traded output (merchant logit), keeps the home flow in Q, moves
/// shipments into transit (losses destroyed) and the wheel, and pays for them at delivered
/// prices: each destination pays its price per arriving unit, the tariff share t / (1 + t)
/// to its country's rebate account and the rest to the origin (domestic directly,
/// international through the gateways of both countries). Sets inactive cells' price to this
/// tick's netback.
pub fn route(w: &mut EconWorld) {
    let (inp, lay, kernel) = (&*w.inputs, &*w.layout, &*w.kernel);
    let acc = &lay.accounts;
    let (np, ng) = (kernel.n_provinces, kernel.n_gateways);
    let (mut supply, mut price, mut active) = (vec![0.0; np], vec![0.0; np], vec![false; np]);
    let mut parts = Vec::new();
    for (g, &transit) in acc.transit.iter().enumerate() {
        for o in 0..np {
            let cell = o * N_GOODS + g;
            supply[o] = w.scratch.route_supply[cell];
            price[o] = w.markets.price[cell];
            active[o] = w.markets.active[cell];
        }
        route_good(kernel, g, &supply, &price, &active, &mut w.scratch.routed);
        let r = &w.scratch.routed;
        w.scratch.flows.record(
            kernel,
            &inp.network.province_country,
            &inp.network.gateway_country,
            g,
            &price,
            r,
        );

        for (o, &is_active) in active.iter().enumerate() {
            let (cell, market) = (o * N_GOODS + g, acc.market[o]);
            w.scratch.route_mass[cell] = r.home[o] + r.shipped[o];
            let held = w.ledger.goods(market, g);
            let x = if is_active {
                w.markets.supply[cell] += r.home[o];
                r.shipped[o].min(held)
            } else {
                // an inactive cell has no market: anything kept home idles as inventory
                w.markets.inventory[cell] += r.home[o];
                (held - w.markets.inventory[cell]).max(0.0)
            };
            if x > 0.0 {
                w.ledger.ship(market, transit, g, x);
            }
        }
        let lost = r.lost.min(w.ledger.goods(transit, g));
        w.ledger.destroy(transit, g, lost);
        for (lag, arrive) in r.arrive.iter().enumerate() {
            if arrive.iter().any(|&a| a > 0.0) {
                w.transit.push(lag, g, arrive);
            }
        }

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

        for o in (0..np).filter(|&o| !active[o]) {
            let netback = r.netback[o];
            assert!(
                netback.is_finite() && netback > 0.0,
                "province {o} good {g}: netback {netback}"
            );
            w.markets.price[o * N_GOODS + g] = netback;
        }
    }
}

/// E3: households post CES demand for their whole money balance at local prices;
/// enterprises order a_kg Y + (a_kg Y - stock after use), at least zero.
pub fn demand(w: &mut EconWorld) {
    let (inp, lay) = (&*w.inputs, &*w.layout);
    let acc = &lay.accounts;
    let sigma = w.params.sigma_demand;
    w.markets.demand.fill(0.0);
    let mut x = [0.0; N_GOODS];
    for i in 0..inp.n_provinces() {
        let c = inp.country_of(i);
        let row = i * N_GOODS..(i + 1) * N_GOODS;
        let prices: [f64; N_GOODS] = w.markets.price[row.clone()]
            .try_into()
            .expect("one row of goods");
        for k in lay.pops(i) {
            let money = to_numeraire(w.ledger.money(acc.pop[k]));
            ces(&inp.countries.omega[c], &prices, money, sigma, &mut x);
            w.scratch.pop_demand[k * N_GOODS..(k + 1) * N_GOODS].copy_from_slice(&x);
            for (d, &q) in w.markets.demand[row.clone()].iter_mut().zip(&x) {
                *d += q;
            }
        }
        for e in lay.enterprises(i) {
            let (g, y) = (usize::from(inp.enterprises.good[e]), w.output[e]);
            for k in 0..N_GOODS {
                let a = inp.a(c, k, g);
                let order = if a > 0.0 {
                    (2.0 * a * y - w.ledger.goods(acc.enterprise[e], k)).max(0.0)
                } else {
                    0.0
                };
                w.scratch.ent_order[e * N_GOODS + k] = order;
                w.markets.demand[i * N_GOODS + k] += order;
            }
        }
    }
}

/// T3: clears every active market (traded: stock-flow, S' = S + Q - sales; local: unsold
/// output perishes) and hands out the sales: enterprise input orders first, pro rata among
/// enterprises, then households, pro rata among pops, on what remains (DESIGN 8.2,
/// intermediate users by contract before the rest; a deviation from the econ SPEC's pro rata
/// across all buyers, ruled for P2 because pro rata lets households starve the IO network).
/// Buyers pay the posted price; households consume what they receive, enterprises stock it.
pub fn clear(w: &mut EconWorld) {
    let (inp, lay) = (&*w.inputs, &*w.layout);
    let acc = &lay.accounts;
    w.markets.clear(&mut w.scratch.fill);
    w.markets.clear_local(&mut w.scratch.spoiled);
    let mut buyers: Vec<(AccountId, f64, bool)> = Vec::new();
    for i in 0..inp.n_provinces() {
        let market = acc.market[i];
        for (g, &traded) in TRADED.iter().enumerate() {
            let cell = i * N_GOODS + g;
            let m = &w.markets;
            if !m.active[cell] {
                continue;
            }
            let keep = if traded {
                m.inventory[cell]
            } else {
                w.scratch.spoiled[cell]
            };
            let (sold, price) = (m.sales[cell], m.price[cell]);
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
            let fill_orders = if orders > 0.0 {
                (sold / orders).min(1.0)
            } else {
                1.0
            };
            let rest = (sold - orders).max(0.0);
            let fill_households = if households > 0.0 {
                (rest / households).min(1.0)
            } else {
                1.0
            };
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
            if !traded {
                let rest = w.ledger.goods(market, g);
                if rest > 0.0 {
                    w.ledger.destroy(market, g, rest);
                }
            }
        }
    }
}

/// T4: the stock-flow price rule on every active market; inactive cells keep their netback.
pub fn reprice(w: &mut EconWorld) {
    w.markets.reprice(&w.trade);
}

/// E4: every enterprise and market pays its signed cash flow of the tick to its province's
/// pops by ownership share, every rebate account pays its tariffs to its country's pops by
/// population, so all three end the tick at zero. Records each pop's income of the tick.
pub fn distribute(w: &mut EconWorld) {
    let (inp, lay) = (&*w.inputs, &*w.layout);
    let acc = &lay.accounts;
    let mut parts = Vec::new();
    for i in 0..inp.n_provinces() {
        let pops = lay.pops(i);
        let shares = &lay.ownership[pops.clone()];
        let payers = lay.enterprises(i).map(|e| (Some(e), acc.enterprise[e]));
        for (e, payer) in payers.chain([(None, acc.market[i])]) {
            let cash = w.ledger.money(payer);
            if let Some(e) = e {
                w.profit[e] = to_numeraire(cash);
            }
            split(cash, shares.iter().copied(), &mut parts);
            for (k, &part) in pops.clone().zip(&parts) {
                pay(&mut w.ledger, DIVIDEND, payer, acc.pop[k], part);
                w.scratch.income_paid[k] += part;
            }
        }
    }
    for (c, pops) in lay.country_pops.iter().enumerate() {
        let rebate = acc.rebate[c];
        let weights = pops.iter().map(|&k| inp.pops.population[k as usize]);
        split(w.ledger.money(rebate), weights, &mut parts);
        for (&k, &part) in pops.iter().zip(&parts) {
            pay(&mut w.ledger, REBATE, rebate, acc.pop[k as usize], part);
            w.scratch.income_paid[k as usize] += part;
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
    fn largest_takes_the_first_maximum() {
        assert_eq!(largest([0.0, 2.0, 1.0, 2.0].into_iter()), Some(1));
        assert_eq!(largest([0.0, -1.0].into_iter()), None);
    }
}
