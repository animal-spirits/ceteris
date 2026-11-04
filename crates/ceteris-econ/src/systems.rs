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
use ceteris_trade::source::{HOUSEHOLD, INTERMEDIATE};
use ceteris_trade::{Kernel, Sourced, dispatch as dispatch_good, source as source_good};

use crate::inputs::{EconInputs, TRANSFER_YEARS};
use crate::labour::{allocate, clearing_wage, wage_step};
use crate::local;
use crate::params::Transfers;
use crate::production::{
    labour_for_value_added, labour_target, labour_target_uncapped, marginal_value_added, rho,
    value_added,
};
use crate::world::{
    Accounts, Buyer, DIVIDEND, EconWorld, FOREIGN_DIVIDEND, Layout, Order, PURCHASE, Payments,
    REBATE, SALE, TARIFF, TRADE, TRANSFER, WAGE, WRITE_OFF, adjust_stock, to_minor, to_numeraire,
};

/// P149 months of use an enterprise keeps in stock beyond replacement and transit (owner
/// decision, 2026-10-24; a constant, outside the static digest: calibration stage 5 uses it).
pub const COVER: f64 = 1.0;

/// No RNG draws in P2; streams stay reserved per system ID.
pub const SEED: Seed = Seed(42);

/// Relative gap between labour demand and supply above which the clearing wage is taken to
/// have no root (all value-added prices nonpositive, say).
const NO_ROOT: f64 = 1e-9;

/// The frictionless wage (ADR 0089, 0093): the root of `demand(w) = natural`, `demand` the sum
/// of the uncapped labour targets, so it follows the closed form where one target alone
/// reaches the supply. With no root the search ran to an extreme, and the wage moves at most a
/// doubling or halving from `w0`.
fn frictionless_wage(w0: f64, natural: f64, demand: impl Fn(f64) -> f64) -> f64 {
    let w1 = clearing_wage(w0, natural, &demand);
    if (demand(w1) - natural).abs() > NO_ROOT * natural {
        w1.clamp(0.5 * w0, 2.0 * w0)
    } else {
        w1
    }
}

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
        def(209, "distribute", |w, ctx| distribute(w, ctx.tick)),
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
/// part takes everything. The total may be a wide (`i128`) balance; each part is one payment
/// and must fit `i64`.
fn split(
    total: impl Into<i128>,
    weights: impl ExactSizeIterator<Item = f64> + Clone,
    out: &mut Vec<i64>,
) {
    let total: i128 = total.into();
    out.clear();
    let n = weights.len();
    let sum: f64 = weights.clone().sum();
    let (mut cum, mut paid) = (0.0, 0i128);
    for (j, w) in weights.enumerate() {
        cum += w;
        let upto = if j + 1 == n {
            total
        } else if sum > 0.0 {
            (total as f64 * (cum / sum)).round_ties_even() as i128
        } else {
            0
        };
        out.push(i64::try_from(upto - paid).expect("ledger overflow: a payment beyond i64"));
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
    let ng = in_flight.len();
    let mut last: Vec<Option<(usize, f64)>> = vec![None; ng];
    for (b, row) in due.chunks_exact(ng).enumerate() {
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
    for (b, row) in due.chunks_exact(ng).enumerate().take(n_buyers) {
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
    let inputs: f64 = (0..index.len())
        .filter(|&k| a(k) > 0.0)
        .map(|k| a(k) * index[k])
        .sum();
    (p_g - inputs) / a_va
}

/// The output enterprise `e`'s traded inputs allow at E1 (ADR 0266): the least, over the
/// traded inputs k it uses, its own good included, of its stock after T1 over coef_k;
/// infinite where it uses none. Local inputs set no cap: the same-tick chain bounds their use
/// within the tick, and a bound from last tick's chain either rehires a cut enterprise every
/// other tick or holds a province whose local goods all fell to zero there for good.
pub fn input_output(w: &EconWorld, e: usize) -> f64 {
    let nt = w.inputs.goods.traded;
    let (lay, account) = (&*w.layout, w.layout.accounts.enterprise[e]);
    let mut y = f64::INFINITY;
    for k in 0..nt {
        let a = lay.coef[e][k];
        if a > 0.0 {
            y = y.min(w.ledger.goods(account, k) / a);
        }
    }
    y
}

/// The traded inputs that set enterprise `e`'s [`input_output`], a bit per good: those whose
/// stock over coef_k is within [`SCARCEST`] (relative) of the least.
pub fn scarcest_inputs(w: &EconWorld, e: usize) -> u64 {
    let (ng, nt) = (w.inputs.n_goods(), w.inputs.goods.traded);
    let (lay, account) = (&*w.layout, w.layout.accounts.enterprise[e]);
    let y = input_output(w, e);
    (0..ng)
        .filter(|&k| k < nt && lay.coef[e][k] > 0.0)
        .filter(|&k| w.ledger.goods(account, k) / lay.coef[e][k] <= y * (1.0 + SCARCEST))
        .fold(0, |bits, k| bits | 1 << k)
}

/// Relative rounding within which a traded input ties for the scarcest (ADR 0266): the data
/// state holds one month of every input, so their ratios agree only to the last bits.
pub const SCARCEST: f64 = 1e-9;

/// Whether the input cap cut a labour target (ADR 0266, `Scratch::short`): the target without
/// the cap exceeds the cap by more than [`SCARCEST`], relative. Frictionless, the rationed wage
/// is the marginal value product of the marginal enterprise at its cap, so its target equals
/// its cap up to the rounding of two closed forms; within the margin the target counts as
/// capped by nothing, and its inputs are ordered on output that differs from notional output
/// by at most the margin.
pub fn cap_cuts(target: f64, cap: f64) -> bool {
    target > cap * (1.0 + SCARCEST)
}

/// E1: labour targets L* at the current own price, last tick's input price indices and the
/// current wage, each capped at the labour that produces the output its inputs allow
/// ([`input_output`], ADR 0266), then the wage (wage rule on expected inflation and excess
/// demand against the natural rate u*, or when frictionless the wage at which the capped
/// labour demand is (1 - u*) L^s, ADR 0089), employment toward L* and employed pops. For E3 it
/// keeps, per enterprise, the traded inputs whose cap cut its target (`Scratch::short`) and
/// the employment it would have without the caps (`Scratch::notional`).
pub fn labour(w: &mut EconWorld) {
    let ng = w.inputs.n_goods();
    let world = &*w;
    let (inp, lay, params) = (&*w.inputs, &*w.layout, &w.params);
    let c_max = w.trade.c_max;
    let r = rho(params.sigma_fl);
    let (price, wage, prev) = (&w.markets.price, &w.wage, &w.employment);
    let (index, pi) = (&w.price_index, w.expected_inflation);
    type PerProvince = (f64, Vec<f64>, bool, Vec<f64>, Vec<u64>);
    let results: Vec<PerProvince> = (0..inp.n_provinces())
        .into_par_iter()
        .map(|i| {
            let c = inp.country_of(i);
            let u_star = inp.countries.u_star[c];
            let supply = lay.labour_supply[i];
            let row = i * ng..(i + 1) * ng;
            let (p, p_in) = (&price[row.clone()], &index[row]);
            let ents = lay.enterprises(i);
            let p_va: Vec<f64> = ents
                .clone()
                .map(|e| {
                    let g = usize::from(inp.enterprises.good[e]);
                    value_added_price(p[g], |k| lay.coef[e][k], p_in, lay.a_va[e])
                })
                .collect();
            let ent = &inp.enterprises;
            let equipped: Vec<f64> = ents
                .clone()
                .map(|e| {
                    let va = lay.a_va[e] * input_output(world, e);
                    labour_for_value_added(va, ent.a[e], ent.va0[e], ent.l0[e], inp.ent_s_l(e), r)
                })
                .collect();
            let targets = |w: f64| -> Vec<f64> {
                ents.clone()
                    .zip(p_va.iter().zip(&equipped))
                    .map(|(e, (&pv, &cap))| {
                        let s_l = inp.ent_s_l(e);
                        labour_target(pv, w, ent.a[e], ent.va0[e], ent.l0[e], s_l, r, supply)
                            .min(cap)
                    })
                    .collect()
            };
            let w0 = wage[i];
            let (w1, t, clipped) = if params.frictionless {
                let natural = (1.0 - u_star) * supply;
                let free = |e: usize, pv: f64, w: f64| {
                    let s_l = inp.ent_s_l(e);
                    labour_target_uncapped(pv, w, ent.a[e], ent.va0[e], ent.l0[e], s_l, r)
                };
                let hiring = || {
                    ents.clone()
                        .zip(p_va.iter().zip(&equipped))
                        .filter(|&(_, (&pv, &cap))| pv > 0.0 && cap > 0.0)
                };
                let demand = |w: f64| -> f64 {
                    ents.clone().zip(&p_va).map(|(e, &pv)| free(e, pv, w)).sum()
                };
                let w1 = frictionless_wage(w0, natural, demand);
                // Where no input cap binds at the wage that clears without them, it clears with
                // them: the search on the capped demand would find it too, but by another path
                // and so to other bits. Else the wage clears on the capped targets.
                let unbound = ents
                    .clone()
                    .zip(p_va.iter().zip(&equipped))
                    .all(|(e, (&pv, &cap))| pv <= 0.0 || free(e, pv, w1) < cap);
                let w1 = if unbound {
                    w1
                } else {
                    let demand = |w: f64| -> f64 {
                        ents.clone()
                            .zip(p_va.iter().zip(&equipped))
                            .map(|(e, (&pv, &cap))| free(e, pv, w).min(cap))
                            .sum()
                    };
                    let w1 = frictionless_wage(w0, natural, demand);
                    // Rationed: every enterprise that would hire is at its input cap, so demand
                    // is flat around w1 and every wage up to the lowest marginal value product
                    // at the caps clears; the last wage is kept where it does, else the
                    // highest that does is taken (ADR 0266).
                    let interior = hiring()
                        .any(|(e, (&pv, &cap))| cap == f64::INFINITY || free(e, pv, w1) < cap);
                    let top = hiring()
                        .map(|(e, (&pv, &cap))| {
                            pv * marginal_value_added(
                                ent.a[e],
                                ent.va0[e],
                                ent.l0[e],
                                inp.ent_s_l(e),
                                r,
                                cap,
                            )
                        })
                        .reduce(f64::min);
                    match top {
                        Some(top) if !interior => top.min(w0),
                        _ => w1,
                    }
                };
                (w1, targets(w1), false)
            } else {
                let t = targets(w0);
                let excess = (fixed_sum(&t) - supply) / supply;
                let clipped = (params.kappa_w * (excess + u_star)).abs() > c_max;
                (wage_step(w0, excess, u_star, pi, params, c_max), t, clipped)
            };
            let mut l = vec![0.0; t.len()];
            allocate(&t, &prev[ents.clone()], params.eta, supply, &mut l);
            // the wage the targets were set at
            let wt = if params.frictionless { w1 } else { w0 };
            let free: Vec<f64> = ents
                .clone()
                .zip(&p_va)
                .map(|(e, &pv)| {
                    labour_target(
                        pv,
                        wt,
                        ent.a[e],
                        ent.va0[e],
                        ent.l0[e],
                        inp.ent_s_l(e),
                        r,
                        supply,
                    )
                })
                .collect();
            let short: Vec<u64> = ents
                .clone()
                .zip(free.iter().zip(&equipped))
                .map(|(e, (&f, &cap))| {
                    if cap_cuts(f, cap) {
                        scarcest_inputs(world, e)
                    } else {
                        0
                    }
                })
                .collect();
            let notional = if short.iter().any(|&s| s != 0) {
                let mut n = vec![0.0; t.len()];
                allocate(&free, &prev[ents], params.eta, supply, &mut n);
                n
            } else {
                l.clone()
            };
            (w1, l, clipped, notional, short)
        })
        .collect();
    for (i, (w1, l, clipped, notional, short)) in results.into_iter().enumerate() {
        w.scratch.notional[lay.enterprises(i)].copy_from_slice(&notional);
        w.scratch.short[lay.enterprises(i)].copy_from_slice(&short);
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
    let (ng, nt) = (w.inputs.n_goods(), w.inputs.goods.traded);
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
            let (account, cell) = (acc.enterprise[e], i * ng + g);
            for k in 0..nt {
                let a = lay.coef[e][k];
                let before = w.ledger.goods(account, k);
                if a > 0.0 {
                    w.ledger.destroy(account, k, (a * y).min(before));
                }
                w.scratch.input_before[e * ng + k] = before;
                w.scratch.input_used[e * ng + k] = before - w.ledger.goods(account, k);
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
            for k in nt..ng {
                let (at, cell) = (e * ng + k, i * ng + k);
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
    let nt = w.inputs.goods.traded;
    let (inp, lay) = (&*w.inputs, &*w.layout);
    let acc = &lay.accounts;
    let r = rho(w.params.sigma_fl);
    let (ledger, employment) = (&w.ledger, &w.employment);
    let capped: Vec<f64> = (0..inp.n_enterprises())
        .into_par_iter()
        .map(|e| {
            let ent = &inp.enterprises;
            let s_l = inp.ent_s_l(e);
            let va = value_added(ent.a[e], ent.va0[e], ent.l0[e], s_l, r, employment[e]);
            let mut y = va / lay.a_va[e];
            for k in 0..nt {
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
/// (`Layout::pop_demand`, ADR 0021) at the buyer price q_d of traded goods (ADR 0200) and the
/// posted price of local goods. Enterprises order up to an inventory
/// position (stock plus goods on order) of `(1 + COVER + L) coef_k Y_plan,k` of each input k:
/// orders `(target - stock_k - on_order_k)`, at least zero, where L is the expected months in
/// transit of the input to the province (0 for local goods), so where L = 0 this is a fixed
/// two-month rule. Y_plan,k is VA(L_notional) / a_VA, the output E1's employment without the
/// input cap would give, for a traded input whose cap cut the enterprise's labour at E1
/// (`Scratch::short`), and VA(L) / a_VA at this tick's employment for every other input
/// (effective demand, Benassy 1975; owner decision 2026-10-30T22:43Z, ADR 0266). Owner
/// decisions, 2026-10-24: planning on realized output Y, which a short input caps, let one
/// shortage cut the next order and the next output; a fixed target that counts goods on order
/// cannot hold a month in transit. Traded orders are spending, units times q_d, so the units
/// ordered are the units that arrive; local orders are units at the province market.
pub fn demand(w: &mut EconWorld) {
    let (ng, nt) = (w.inputs.n_goods(), w.inputs.goods.traded);
    w.refresh_price_index();
    let (inp, lay) = (&*w.inputs, &*w.layout);
    let acc = &lay.accounts;
    let n_pops = inp.n_pops();
    let r = rho(w.params.sigma_fl);
    let sc = &mut w.scratch;
    let (ledger, params, index) = (&w.ledger, &w.params, &w.price_index);
    let row = |i: usize| &index[i * ng..(i + 1) * ng];
    sc.pop_demand
        .par_chunks_mut(ng)
        .zip(sc.pop_spend.par_chunks_mut(ng))
        .enumerate()
        .for_each(|(k, (x, spend))| {
            let prices = row(inp.pops.province[k] as usize);
            let money = to_numeraire(ledger.money(acc.pop[k]));
            lay.pop_demand(inp, params, k, prices, money, x);
            for g in 0..ng {
                spend[g] = if g < nt && x[g] > 0.0 {
                    x[g] * prices[g]
                } else {
                    0.0
                };
            }
        });
    let (pipeline, lead) = (&w.pipeline, &sc.lead);
    let (short, notional) = (&sc.short, &sc.notional);
    sc.ent_order
        .par_chunks_mut(ng)
        .zip(sc.ent_spend.par_chunks_mut(ng))
        .enumerate()
        .for_each(|(e, (orders, spend))| {
            let ent = &inp.enterprises;
            let i = ent.province[e] as usize;
            let (prices, s_l) = (row(i), inp.ent_s_l(e));
            let output =
                |l: f64| value_added(ent.a[e], ent.va0[e], ent.l0[e], s_l, r, l) / lay.a_va[e];
            let capacity = output(w.employment[e]);
            let free = if short[e] != 0 {
                output(notional[e])
            } else {
                capacity
            };
            for k in 0..ng {
                let a = lay.coef[e][k];
                let order = if a > 0.0 {
                    let held = ledger.goods(acc.enterprise[e], k);
                    let on_order = pipeline.on_order(n_pops + e, k);
                    let target = 1.0 + COVER + lead[i * ng + k];
                    let plan = if short[e] & 1 << k != 0 {
                        free
                    } else {
                        capacity
                    };
                    (target * a * plan - held - on_order).max(0.0)
                } else {
                    0.0
                };
                orders[k] = order;
                spend[k] = if k < nt && order > 0.0 && prices[k].is_finite() {
                    order * prices[k]
                } else {
                    0.0
                };
            }
        });
    // local goods: demand at the province market, in pop then enterprise order
    w.markets.demand.copy_from_slice(&sc.drawn);
    for i in 0..inp.n_provinces() {
        for g in nt..ng {
            let cell = i * ng + g;
            for k in lay.pops(i) {
                w.markets.demand[cell] += sc.pop_demand[k * ng + g];
            }
            for e in lay.enterprises(i) {
                w.markets.demand[cell] += sc.ent_order[e * ng + g];
            }
        }
    }
}

/// T2: spreads each destination's spending on every traded good over the active origins at
/// posted prices (`ceteris_trade::source`) and places the orders at the origin markets.
pub fn source(w: &mut EconWorld) {
    let ng = w.inputs.n_goods();
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
                    s[INTERMEDIATE] += ent_spend[e * ng + g];
                }
                for k in lay.pops(d) {
                    s[HOUSEHOLD] += pop_spend[k * ng + g];
                }
                s
            }));
            col.gather(&markets.price, &markets.active, g, ng);
            source_good(
                kernel,
                g,
                &col.price,
                &col.active,
                &lay.gamma[g],
                &col.spend,
                sourced,
            );
        });
    for (g, (sourced, col)) in sc.sourced.iter().zip(&sc.columns).enumerate() {
        for o in 0..np {
            let cell = o * ng + g;
            sc.class_spend[cell] = col.spend[o];
            w.markets.demand_int[cell] = sourced.orders[o][INTERMEDIATE];
            w.markets.demand_hh[cell] = sourced.orders[o][HOUSEHOLD];
        }
    }
}

/// T3: clears every active market. The origin markets of the storable goods first lose their
/// stock's spoilage, destroyed in the ledger, and place the merchants' storage orders
/// (`Markets::store`, ADR 0091). Origin markets of traded goods clear their orders
/// against output and merchant stock (S' = S + Q - sales), a shortfall shared between buyer
/// classes by `EconParams::rationing`; the fills are dispatched at T2b. Province markets of
/// local goods hand out what the same-tick chain left of their output (E2): enterprise input
/// orders (refilling stocks) and households share it by `EconParams::rationing`, pro rata
/// within each class; unsold output perishes. Buyers of local goods pay the posted price;
/// households consume what they receive, enterprises stock it.
pub fn clear(w: &mut EconWorld) {
    let (ng, nt) = (w.inputs.n_goods(), w.inputs.goods.traded);
    let (inp, lay) = (&*w.inputs, &*w.layout);
    let acc = &lay.accounts;
    w.markets.store(&w.trade, &mut w.scratch.spoiled);
    for (cell, &loss) in w.scratch.spoiled.iter().enumerate() {
        if loss > 0.0 {
            w.ledger.destroy(acc.market[cell / ng], cell % ng, loss);
        }
    }
    w.markets.clear(w.params.rationing, &mut w.scratch.fill);
    w.markets.clear_local(&mut w.scratch.spoiled);
    let mut buyers: Vec<(AccountId, f64, bool)> = Vec::new();
    for i in 0..inp.n_provinces() {
        let market = acc.market[i];
        for g in nt..ng {
            let cell = i * ng + g;
            let m = &w.markets;
            if !m.active[cell] {
                continue;
            }
            let keep = w.scratch.spoiled[cell];
            let price = m.price[cell];
            let rest = (m.supply[cell] - w.scratch.drawn[cell]).max(0.0);
            buyers.clear();
            for k in lay.pops(i) {
                buyers.push((acc.pop[k], w.scratch.pop_demand[k * ng + g], true));
            }
            for e in lay.enterprises(i) {
                buyers.push((acc.enterprise[e], w.scratch.ent_order[e * ng + g], false));
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
    /// Goods per row of `enterprise`, `pop` and `class`.
    goods: usize,
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
        let spend = spend.enterprise[e * spend.goods + g];
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
        let spend = spend.pop[k * spend.goods + g];
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
        let (cell, market) = (d * spend.goods + g, acc.market[d]);
        buyers_of(lay, spend, d, g, &mut p.buyers);
        for b in &p.buyers {
            let share = b.spend / spend.class[cell][b.class];
            let slots = std::array::from_fn(|s| r.arrive[s + 1][d][b.class] * share);
            p.orders.push(Order {
                account: b.account,
                buyer: b.index,
                household: b.class == HOUSEHOLD,
                home: r.arrive[0][d][b.class] * share,
                home_shipped: r.home[d][b.class] * share,
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
/// purchases go straight from the market to their buyers (pops consume them), the shipped
/// units turned into delivered units by selection (ADR 0200: created or destroyed at the
/// buyer); shipments enter the transit account and each buyer's pipeline, due at t + slot, in
/// delivered units, so the transit account is brought to the pipeline: what the losses melt is
/// destroyed and what selection adds is created. Both are tallied per good for the selection
/// invariant (`Scratch::dispatch_made`). Each buyer pays its filled spending, origin price times
/// (1 + t) per shipped unit, to its destination market, rounded half to even per buyer and
/// good; the market passes it on: domestic legs to the origin market, international legs
/// through the gateways of both countries (rounded per leg, gateway splits cumulatively, so
/// each gateway nets to zero), the tariff to its country's rebate account. Unfilled spending
/// stays with the buyer. Records the country-pair flows.
pub fn dispatch(w: &mut EconWorld) {
    let ng = w.inputs.n_goods();
    let inp = std::sync::Arc::clone(&w.inputs);
    let lay = std::sync::Arc::clone(&w.layout);
    let kernel = std::sync::Arc::clone(&w.kernel);
    let (inp, lay, kernel) = (&*inp, &*lay, &*kernel);
    let acc = &lay.accounts;
    let np = kernel.n_provinces();
    let sc = &mut w.scratch;
    let (fill, markets, net) = (&sc.fill, &w.markets, &inp.network);
    let spending = Spending {
        goods: ng,
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
            col.gather(&markets.price, &markets.active, g, ng);
            col.fill.clear();
            col.fill.extend((0..np).map(|o| fill[o * ng + g]));
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
        let mut made = 0.0;
        let mut first = 0;
        for d in 0..np {
            let (cell, market) = (d * ng + g, acc.market[d]);
            w.scratch.dispatched[cell] = r.shipped[d];
            // buyers at d: home purchases now, shipments into the pipeline
            let end = legs.order_ends[d] as usize;
            for o in &legs.orders[first..end] {
                let shipped = o.home_shipped.min(w.ledger.goods(market, g));
                if shipped > 0.0 {
                    w.ledger.ship(market, o.account, g, shipped);
                    let home = if shipped == o.home_shipped {
                        o.home
                    } else {
                        o.home * (shipped / o.home_shipped)
                    };
                    adjust_stock(&mut w.ledger, o.account, g, home - shipped);
                    made += home - shipped;
                    if o.household {
                        // pops hold nothing between purchases, so they consume what they hold:
                        // `home` itself can differ from it by the rounding of the adjustment
                        let held = w.ledger.goods(o.account, g);
                        w.ledger.destroy(o.account, g, held);
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
        w.scratch.dispatch_made[g] = made;
        w.scratch.dispatch_due[g] = r.selected - r.lost;
        w.scratch.dispatch_scale[g] = r.shipped.iter().sum::<f64>()
            + r.arrive.iter().flatten().map(|a| a[0] + a[1]).sum::<f64>();
    }
    // each transit account is brought to the goods now in flight: the losses melt what it
    // holds beyond them, selection adds what they hold beyond it
    let in_flight = w.pipeline.in_flight_all();
    for (g, &transit) in acc.transit.iter().enumerate() {
        let change = in_flight[g] - w.ledger.goods(transit, g);
        adjust_stock(&mut w.ledger, transit, g, change);
        w.scratch.dispatch_made[g] += change;
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
    w.markets
        .reprice(&w.trade, &w.layout.price_gain, w.expected_inflation);
    w.update_expectation();
}

/// E4 (national ownership, owner 2026-10-25; limited liability of enterprises, owner
/// 2026-10-26; loss write-off, ADR 0112): every enterprise pays its balance into its country's
/// profit pool when the balance is positive and otherwise keeps it, so a loss stays with the
/// enterprise as retained losses, which later cash flow repays before anything is paid out;
/// every province market pays its signed cash flow (ADR 0112, Markets). Each country pays its
/// pool and its rebate account (tariffs) to its pops by population share N_k / N_c, so both
/// end the tick at zero. Each pool is one i128 sum, split once per tick by [`split`]: the
/// cumulative population shares of the country's pops in ID order rounded half to even, so the
/// parts add up to the pool exactly. With foreign owners (ADR 0310) the foreign-owned share f
/// of each positive enterprise payout goes to the world dividend account instead ([`split`]
/// of the payout by 1 - f and f), which pays each host's total over its owners' pools by its
/// parent vector before the pools pay their pops. Then the transfers of tick `tick`
/// ([`transfer`]), then [`write_off`]. Records each pop's income of the tick, and as each
/// enterprise's profit its balance before the payout (this tick's cash flow less any loss it
/// carries).
pub fn distribute(w: &mut EconWorld, tick: u32) {
    let (inp, lay) = (&*w.inputs, &*w.layout);
    let acc = &lay.accounts;
    let mut parts = Vec::new();
    let mut abroad: Vec<i128> = vec![
        0;
        if acc.foreign.is_some() {
            inp.n_countries()
        } else {
            0
        }
    ];
    for i in 0..inp.n_provinces() {
        let c = inp.country_of(i);
        let pool = acc.dividend[c];
        for e in lay.enterprises(i) {
            let cash = w.ledger.money(acc.enterprise[e]);
            w.profit[e] = to_numeraire(cash);
            let paid = cash.max(0);
            let f = inp.ent_foreign(e);
            match acc.foreign {
                Some(account) if paid > 0 && f > 0.0 => {
                    split(paid, [1.0 - f, f].into_iter(), &mut parts);
                    pay(&mut w.ledger, DIVIDEND, acc.enterprise[e], pool, parts[0]);
                    pay(
                        &mut w.ledger,
                        FOREIGN_DIVIDEND,
                        acc.enterprise[e],
                        account,
                        parts[1],
                    );
                    abroad[c] += i128::from(parts[1]);
                }
                _ => pay(&mut w.ledger, DIVIDEND, acc.enterprise[e], pool, paid),
            }
        }
        let cash = i64::try_from(w.ledger.money_wide(acc.market[i]))
            .expect("ledger overflow: a market's cash beyond one payment");
        pay(&mut w.ledger, DIVIDEND, acc.market[i], pool, cash);
    }
    if let Some(account) = acc.foreign {
        for (h, &total) in abroad.iter().enumerate().filter(|(_, x)| **x > 0) {
            let owners = &inp.countries.owners[h];
            split(total, owners.iter().copied(), &mut parts);
            for (o, &part) in parts.iter().enumerate() {
                pay(
                    &mut w.ledger,
                    FOREIGN_DIVIDEND,
                    account,
                    acc.dividend[o],
                    part,
                );
            }
        }
    }
    for (c, pops) in lay.country_pops.iter().enumerate() {
        for (flow, payer) in [(DIVIDEND, acc.dividend[c]), (REBATE, acc.rebate[c])] {
            // the pool by dividend share (by wealth, ADR 0104), the rebate by population
            let weights = pops.iter().map(|&k| {
                let k = k as usize;
                if flow == DIVIDEND && !inp.pops.dividend.is_empty() {
                    inp.pops.dividend[k]
                } else {
                    inp.pops.population[k]
                }
            });
            split(w.ledger.money_wide(payer), weights, &mut parts);
            for (&k, &part) in pops.iter().zip(&parts) {
                pay(&mut w.ledger, flow, payer, acc.pop[k as usize], part);
                w.scratch.income_paid[k as usize] += part;
            }
        }
    }
    transfer(w, tick);
    for (income, &paid) in w.income.iter_mut().zip(&w.scratch.income_paid) {
        *income = to_numeraire(paid);
    }
    write_off(w);
}

/// Months of the half-life with which the observed transfer path fades after 2024 (ADR 0100:
/// ten years; not tuned).
pub const TRANSFER_HALF_LIFE: f64 = 120.0;

/// Country `c`'s transfer share s_c at tick `tick` under `mode` (ADR 0100, S22): the 2000 share
/// (`FixedShares`), the share of the tick's year 2000 + tick / 12 (`ObservedPath`), which after
/// the last year fades as s_c(2024) 2^-((tick - 299) / 120), or 0 (`Off`, or inputs without
/// shares).
pub fn transfer_share(inp: &EconInputs, mode: Transfers, c: usize, tick: u32) -> f64 {
    let countries = &inp.countries;
    match mode {
        Transfers::Off => 0.0,
        Transfers::FixedShares => countries.transfer.get(c).copied().unwrap_or(0.0),
        Transfers::ObservedPath => {
            if countries.transfer_path.is_empty() {
                return 0.0;
            }
            let path = &countries.transfer_path[c];
            let year = tick as usize / 12;
            if year < TRANSFER_YEARS {
                path[year]
            } else {
                let last = (TRANSFER_YEARS * 12 - 1) as f64;
                path[TRANSFER_YEARS - 1]
                    * libm::exp2(-(f64::from(tick) - last) / TRANSFER_HALF_LIFE)
            }
        }
    }
}

/// E4 transfers (ADR 0100 as amended by ADRs 0295 to 0300): each country's D_c = s_c Y, Y the
/// world income of the previous tick (sum of `EconWorld::income`; the tick's own where there is
/// none, at the first tick of a data-state start). Each paying country (s_c < 0) puts
/// round_half_even(|s_c| Y) minor units into the world transfer account, its pops in proportion
/// to the income credited to them this tick; the account pays their total out over the
/// receiving countries in proportion to s_c, and each over its pops the same way ([`split`]),
/// so it ends at zero. Nothing without the account (`world::transfers_on`).
pub fn transfer(w: &mut EconWorld, tick: u32) {
    let Some(account) = w.layout.accounts.transfer else {
        return;
    };
    if w.params.transfers == Transfers::Off {
        return;
    }
    let (inp, lay) = (&*w.inputs, &*w.layout);
    let acc = &lay.accounts;
    let mode = w.params.transfers;
    let mut world = fixed_sum(&w.income);
    if world <= 0.0 {
        let paid: Vec<f64> = w
            .scratch
            .income_paid
            .iter()
            .map(|&m| to_numeraire(m))
            .collect();
        world = fixed_sum(&paid);
    }
    let share: Vec<f64> = (0..inp.n_countries())
        .map(|c| transfer_share(inp, mode, c, tick))
        .collect();
    let income = |k: u32| w.scratch.income_paid[k as usize].max(0) as f64;
    let mut parts = Vec::new();
    let mut paid: i128 = 0;
    for (c, pops) in lay.country_pops.iter().enumerate() {
        if share[c] >= 0.0 || pops.is_empty() {
            continue;
        }
        let amount = to_minor(-share[c] * world);
        split(amount, pops.iter().map(|&k| income(k)), &mut parts);
        for (&k, &part) in pops.iter().zip(&parts) {
            pay(&mut w.ledger, TRANSFER, acc.pop[k as usize], account, part);
        }
        paid += i128::from(amount);
    }
    let receivers: Vec<usize> = (0..inp.n_countries())
        .filter(|&c| share[c] > 0.0 && !lay.country_pops[c].is_empty())
        .collect();
    let mut amounts = Vec::new();
    split(paid, receivers.iter().map(|&c| share[c]), &mut amounts);
    for (&c, &amount) in receivers.iter().zip(&amounts) {
        let pops = &lay.country_pops[c];
        split(amount, pops.iter().map(|&k| income(k)), &mut parts);
        for (&k, &part) in pops.iter().zip(&parts) {
            pay(&mut w.ledger, TRANSFER, account, acc.pop[k as usize], part);
        }
    }
}

/// The loss write-off of E4 (ADR 0112): per country, the excess of every enterprise's retained
/// loss over its cap (`Layout::ent_loss_cap`) is written off against the country's pops in
/// proportion to their positive money, never taking a pop below zero. Where the pops hold less than the excess, what they hold is
/// written off, shared over the holders by their excess, and the rest stays with them. Both
/// sides are exact [`largest_remainder`] splits of one total per country, moved through the
/// country's profit pool account, which ends at zero. The total is an aggregate, kept `i128`
/// (ADR 0106); each part is bounded by its own weight, so it fits `i64`.
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
        let total = held.min(excess);
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
fn largest_remainder(total: i128, weights: &[i64], out: &mut Vec<i64>) {
    let sum: i128 = weights.iter().map(|&x| i128::from(x)).sum();
    assert!(
        (0..=sum).contains(&total),
        "largest remainder: total within the weights"
    );
    let (total_u, sum_u) = (total.unsigned_abs(), sum.unsigned_abs());
    out.clear();
    let mut remainders: Vec<(u128, usize)> = Vec::with_capacity(weights.len());
    for (j, &x) in weights.iter().enumerate() {
        let (q, r) = mul_div(
            u64::try_from(x).expect("nonnegative weight"),
            total_u,
            sum_u,
        );
        out.push(i64::try_from(q).expect("part within its weight"));
        remainders.push((r, j));
    }
    let left = total - out.iter().map(|&p| i128::from(p)).sum::<i128>();
    remainders.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    for &(_, j) in remainders
        .iter()
        .take(usize::try_from(left).expect("nonnegative"))
    {
        out[j] += 1;
    }
}

/// x t / s as (floor, remainder), exactly, for t <= s: binary long division over the bits of x,
/// the remainder kept below s, since x t can exceed `u128`.
fn mul_div(x: u64, t: u128, s: u128) -> (u64, u128) {
    if t == 0 {
        return (0, 0);
    }
    let (mut q, mut r) = (0u64, 0u128);
    for bit in (0..64).rev() {
        // r < s throughout, so each step subtracts s at most once and never overflows
        q <<= 1;
        if r >= s - r {
            r -= s - r;
            q |= 1;
        } else {
            r += r;
        }
        if (x >> bit) & 1 == 1 {
            if r >= s - t {
                r -= s - t;
                q += 1;
            } else {
                r += t;
            }
        }
    }
    (q, r)
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
    use super::cap_cuts;

    /// A target at its cap, one unit in the last place either side, decides as at the cap: the
    /// cap did not cut it. Beyond the margin it did; an infinite cap never cuts.
    #[test]
    fn a_target_at_its_cap_decides_the_same_under_a_one_ulp_perturbation() {
        for cap in [1.0f64, 0.37, 2.5e3, 7.1e-4] {
            for target in [cap, cap.next_up(), cap.next_down()] {
                assert!(!cap_cuts(target, cap), "target {target} at cap {cap}");
            }
            assert!(!cap_cuts(cap * (1.0 + 0.5e-9), cap));
            assert!(cap_cuts(cap * (1.0 + 2e-9), cap));
            assert!(!cap_cuts(cap, f64::INFINITY));
        }
    }

    use super::*;
    #[allow(unused_imports)]
    use ceteris_trade::goods::p2::{N as N_GOODS, TRADED as N_TRADED};

    #[test]
    fn largest_remainder_is_exact_and_bounded_by_the_weights() {
        let mut out = Vec::new();
        largest_remainder(7, &[0, 3, 0, 5], &mut out);
        assert_eq!(out.iter().sum::<i64>(), 7);
        assert_eq!((out[0], out[2]), (0, 0), "zero weights get nothing");
        assert!(out[1] <= 3 && out[3] <= 5);
        // 5 over equal weights: 1 each and the two leftover units to the first two (ties)
        largest_remainder(5, &[4, 4, 4], &mut out);
        assert_eq!(out, [2, 2, 1]);
        // the whole weight: every part is its weight
        largest_remainder(9, &[2, 7], &mut out);
        assert_eq!(out, [2, 7]);
        let weights = [1, 2, 3, 1_000_000_007, i64::MAX / 4];
        for total in [0, 1, 17, 999, 1_000_000_000] {
            largest_remainder(total, &weights, &mut out);
            assert_eq!(out.iter().sum::<i64>(), total as i64);
            assert!(out.iter().zip(&weights).all(|(p, w)| 0 <= *p && p <= w));
        }
    }

    /// A total beyond i64 over weights near i64::MAX (ADR 0106), where total x weight exceeds
    /// u128: exact, and each part within its weight.
    #[test]
    fn largest_remainder_takes_a_wide_total() {
        let mut out = Vec::new();
        let weights = [i64::MAX; 64].map(|w| w - 3);
        let mut weights = weights.to_vec();
        weights.extend([1, 0, i64::MAX / 3, 12_345]);
        let sum: i128 = weights.iter().map(|&x| i128::from(x)).sum();
        for total in [sum, sum - 1, sum / 2 + 7, i128::from(i64::MAX) + 1, 5] {
            largest_remainder(total, &weights, &mut out);
            assert_eq!(out.iter().map(|&p| i128::from(p)).sum::<i128>(), total);
            assert!(out.iter().zip(&weights).all(|(p, w)| 0 <= *p && p <= w));
        }
        largest_remainder(sum, &weights, &mut out);
        assert_eq!(out, weights, "the whole weight: every part is its weight");
    }

    #[test]
    fn mul_div_is_exact() {
        for (x, t, s) in [
            (7u64, 5u128, 9u128),
            (u64::MAX, u128::MAX / 3, u128::MAX / 2),
            (u64::MAX, u128::MAX, u128::MAX),
            (12_345, 0, 1),
        ] {
            let (q, r) = mul_div(x, t, s);
            // x t = q s + r, checked in 256 bits as (high, low) halves of 128
            let wide = |a: u128, b: u128| {
                let (al, ah, bl, bh) = (a as u64 as u128, a >> 64, b as u64 as u128, b >> 64);
                let (ll, lh, hl, hh) = (al * bl, al * bh, ah * bl, ah * bh);
                let mid = (ll >> 64) + (lh as u64 as u128) + (hl as u64 as u128);
                let lo = (ll as u64 as u128) | (mid << 64);
                let hi = hh + (lh >> 64) + (hl >> 64) + (mid >> 64);
                (hi, lo)
            };
            let (h1, l1) = wide(u128::from(x), t);
            let (h2, l2) = wide(u128::from(q), s);
            let (l3, carry) = l2.overflowing_add(r);
            assert_eq!((h1, l1), (h2 + u128::from(carry), l3), "{x} {t} {s}");
            assert!(r < s);
        }
    }

    /// Cobb-Douglas targets L* = L0 (w L0 / (p A VA0 s_L))^(1 / (s_L - 1)): with u* = 0 and a
    /// second enterprise hiring no one, the clearing wage is where the first alone hires L^s.
    /// Capped at L^s, its demand equals the supply on every lower wage, and the search stopped
    /// at the first one it tried.
    #[test]
    fn frictionless_wage_matches_closed_form_when_one_target_reaches_supply() {
        let (a, va0, l0, s_l, supply) = (1.3, 100.0, 50.0, 0.6, 80.0);
        let p_va = [1.7, -0.3];
        let closed = p_va[0] * a * va0 * s_l / l0 * libm::pow(supply / l0, s_l - 1.0);
        let uncapped = |w: f64| -> f64 {
            p_va.iter()
                .map(|&p| labour_target_uncapped(p, w, a, va0, l0, s_l, 0.0))
                .sum()
        };
        let capped = |w: f64| -> f64 {
            p_va.iter()
                .map(|&p| labour_target(p, w, a, va0, l0, s_l, 0.0, supply))
                .sum()
        };
        for w0 in [
            0.1 * closed,
            0.9 * closed,
            closed,
            1.5 * closed,
            10.0 * closed,
        ] {
            let w = frictionless_wage(w0, supply, uncapped);
            assert!(
                (w / closed - 1.0).abs() < 1e-12,
                "from {w0}: {w} vs {closed}"
            );
        }
        assert_eq!(clearing_wage(0.1 * closed, supply, capped), 0.1 * closed);
    }

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

    /// ADR 0266: an enterprise with no stock of the traded inputs it uses, its own good
    /// included, hires no one; local inputs set no cap; with inputs to spare every target, and so the wage and employment,
    /// are the same whatever the stocks.
    #[test]
    fn labour_is_capped_at_what_inputs_allow() {
        use crate::fixture::small_world;
        use crate::params::EconParams;
        use ceteris_trade::TradeParams;
        use std::sync::Arc;

        let inputs = Arc::new(small_world());
        for params in [EconParams::frictionless(), EconParams::default()] {
            let fresh = EconWorld::new(Arc::clone(&inputs), params.clone(), TradeParams::default());
            let lay = Arc::clone(&fresh.layout);
            let e = (0..fresh.inputs.n_enterprises())
                .find(|&e| (0..N_GOODS).any(|k| (k < N_TRADED) && lay.coef[e][k] > 0.0))
                .expect("an enterprise using a traded input");
            let with_stocks = |factor: f64| {
                let mut w = fresh.clone();
                for (u, &account) in lay.accounts.enterprise.iter().enumerate() {
                    for k in (0..N_GOODS).filter(|&k| lay.coef[u][k] > 0.0) {
                        let held = w.ledger.goods(account, k);
                        w.ledger.create(account, k, held * (factor - 1.0));
                    }
                }
                w
            };
            let ample = |factor: f64| {
                let mut w = with_stocks(factor);
                labour(&mut w);
                (w.wage, w.employment)
            };
            let (wage, employment) = ample(100.0);
            assert!(employment[e] > 0.0);
            let (wage_more, employment_more) = ample(1e4);
            assert_eq!(wage, wage_more);
            assert_eq!(employment, employment_more);

            let mut w = with_stocks(100.0);
            let account = lay.accounts.enterprise[e];
            let own = usize::from(w.inputs.enterprises.good[e]);
            for k in (0..N_GOODS).filter(|&k| (k < N_TRADED) && lay.coef[e][k] > 0.0) {
                let held = w.ledger.goods(account, k);
                w.ledger.destroy(account, k, held);
            }
            assert_eq!(input_output(&w, e), 0.0);
            let prev = w.employment[e];
            labour(&mut w);
            assert_eq!(w.employment[e], prev + params.eta * (0.0 - prev));

            // local inputs: none in stock and none in the province's last output caps nothing
            let mut w = with_stocks(100.0);
            let i = w.inputs.enterprises.province[e] as usize;
            let k = (N_TRADED..N_GOODS)
                .find(|&k| k != own && lay.coef[e][k] > 0.0)
                .expect("a local input");
            let held = w.ledger.goods(account, k);
            w.ledger.destroy(account, k, held);
            w.markets.supply[i * N_GOODS + k] = 0.0;
            let traded = (0..N_GOODS)
                .filter(|&g| (g < N_TRADED) && lay.coef[e][g] > 0.0)
                .map(|g| w.ledger.goods(account, g) / lay.coef[e][g])
                .fold(f64::INFINITY, f64::min);
            assert_eq!(input_output(&w, e), traded);
            labour(&mut w);
            assert_eq!((w.wage, w.employment), (wage, employment));
        }
    }

    /// ADR 0266: the fixture's data state holds one month of base inputs, so every enterprise
    /// is at its cap L0, and frictionless labour demand is flat at (1 - u*) L^s below the
    /// lowest marginal value product at the caps, w_top, which there is w0. The wage stays where
    /// it clears (not at the bottom of the flat stretch, near 1e-58) and comes down to w_top
    /// from above it.
    #[test]
    fn rationed_frictionless_wage_is_the_last_one_that_clears() {
        use crate::fixture::small_world;
        use crate::params::EconParams;
        use ceteris_trade::TradeParams;
        use std::sync::Arc;

        let fresh = EconWorld::new(
            Arc::new(small_world()),
            EconParams::frictionless(),
            TradeParams::default(),
        );
        let (w0, l0) = (fresh.wage.clone(), fresh.employment.clone());
        let close = |a: &[f64], b: &[f64]| a.iter().zip(b).all(|(x, y)| (x / y - 1.0).abs() < 1e-9);
        let mut w = fresh.clone();
        labour(&mut w);
        assert!(close(&w.wage, &w0), "{:?} vs {w0:?}", w.wage);
        assert!(close(&w.employment, &l0));
        let mut w = fresh.clone();
        w.wage.iter_mut().for_each(|x| *x *= 2.0);
        labour(&mut w);
        assert!(close(&w.wage, &w0), "{:?} vs {w0:?}", w.wage);
        assert!(close(&w.employment, &l0));
    }

    /// ADR 0266, owner decision 2026-10-30T22:43Z (Benassy 1975): an enterprise whose labour is
    /// cut by a short traded input k orders k on its notional output, VA(L_notional) / a_VA, and
    /// every other input on its constrained output, VA(L) / a_VA at this tick's employment.
    /// L_notional is the employment E1 gives the same world with k to spare.
    #[test]
    fn short_input_is_ordered_on_notional_output_and_the_rest_on_constrained_output() {
        use crate::fixture::small_world;
        use crate::params::EconParams;
        use ceteris_trade::TradeParams;
        use std::sync::Arc;

        let mut w = EconWorld::new(
            Arc::new(small_world()),
            EconParams::default(),
            TradeParams::default(),
        );
        let lay = Arc::clone(&w.layout);
        let coef = lay.coef.clone();
        let used = |e: usize| {
            let row = coef[e].to_vec();
            (0..N_GOODS).filter(move |&k| row[k] > 0.0)
        };
        let e = (0..w.inputs.n_enterprises())
            .find(|&e| used(e).filter(|&k| k < N_TRADED).count() >= 2)
            .expect("an enterprise using two traded inputs");
        let k = used(e).find(|&k| k < N_TRADED).expect("a traded input");
        // a month and a half of every input, and half a month of k
        for (u, &account) in lay.accounts.enterprise.iter().enumerate() {
            for g in used(u) {
                let held = w.ledger.goods(account, g);
                w.ledger.create(account, g, 0.5 * held);
            }
        }
        let mut spare = w.clone();
        labour(&mut spare);
        assert!(spare.scratch.short.iter().all(|&s| s == 0));
        let account = lay.accounts.enterprise[e];
        let held = w.ledger.goods(account, k);
        w.ledger.destroy(account, k, held * 2.0 / 3.0);
        labour(&mut w);
        assert_eq!(w.scratch.short[e], 1 << k);
        assert_eq!(w.scratch.notional[e], spare.employment[e]);
        assert!(w.employment[e] < 0.99 * spare.employment[e]);

        produce(&mut w);
        demand(&mut w);
        let ent = &w.inputs.enterprises;
        let i = ent.province[e] as usize;
        let s_l = w.inputs.ent_s_l(e);
        let r = rho(w.params.sigma_fl);
        let output = |l: f64| value_added(ent.a[e], ent.va0[e], ent.l0[e], s_l, r, l) / lay.a_va[e];
        let (notional, constrained) = (output(spare.employment[e]), output(w.employment[e]));
        let mut others = 0;
        for g in used(e) {
            let plan = if g == k { notional } else { constrained };
            let target = 1.0 + COVER + w.scratch.lead[i * N_GOODS + g];
            let held = w.ledger.goods(account, g);
            let on_order = w.pipeline.on_order(w.inputs.n_pops() + e, g);
            let want = (target * lay.coef[e][g] * plan - held - on_order).max(0.0);
            let got = w.scratch.ent_order[e * N_GOODS + g];
            assert!(
                (got - want).abs() <= 1e-12 * want.max(1.0),
                "good {g}: order {got}, want {want}"
            );
            others += usize::from(g != k && want > 0.0);
        }
        assert!(
            others > 0,
            "no other input ordered: the test checks nothing there"
        );
    }

    #[test]
    fn largest_takes_the_first_maximum() {
        assert_eq!(largest([0.0, 2.0, 1.0, 2.0].into_iter()), Some(1));
        assert_eq!(largest([0.0, -1.0].into_iter()), None);
    }
}
