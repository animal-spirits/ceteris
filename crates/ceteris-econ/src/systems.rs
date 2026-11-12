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

use ceteris_core::entity::EntityId;
use ceteris_core::ledger::{AccountId, Entry, Flow, GoodColumn, Ledger};
use ceteris_core::reduce::fixed_sum;
use ceteris_core::rng::Seed;
use ceteris_core::schedule::{Cadence, Ctx, Schedule, SystemDef, SystemId};
use ceteris_core::sim::State;
use ceteris_core::timing::{self, NONE};
use ceteris_trade::pipeline::Push;
use ceteris_trade::source::{HOUSEHOLD, INTERMEDIATE};
use ceteris_trade::{Kernel, Sourced, dispatch as dispatch_good, source_weighted};

use crate::demand::PiglAt;
use crate::inputs::{EconInputs, TRANSFER_YEARS};
use crate::labour::{allocate, clearing_wage, wage_step};
use crate::local;
use crate::params::{EconParams, Transfers};
use crate::production::{
    labour_for_value_added, labour_target, labour_target_uncapped, marginal_value_added, rho,
    value_added,
};
use crate::world::{
    Accounts, Buyer, DIVIDEND, EconWorld, FOREIGN_DIVIDEND, GOV_INTEREST, GOV_TRANSFER, Layout,
    LocalClear, Order, PURCHASE, Payments, REBATE, SALE, SPEND_BLOCK, TARIFF, TAX, TRADE, TRANSFER,
    WAGE, WRITE_OFF, adjust_stock, to_minor, to_numeraire,
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
        def(201, "deliver", |w, ctx| {
            w.advance_kernel();
            drift(w, ctx.tick);
            deliver(w)
        }),
        def(202, "labour", |w, ctx| {
            labour(w);
            crate::channels::after_labour(w, ctx.tick)
        }),
        def(203, "produce", |w, _| produce(w)),
        def(204, "demand", |w, _| demand(w)),
        def(205, "source", |w, _| source(w)),
        def(206, "clear", |w, _| clear(w)),
        def(207, "dispatch", |w, _| {
            dispatch(w);
            pay_sales(w)
        }),
        def(208, "reprice", |w, ctx| {
            reprice(w);
            crate::channels::after_reprice(w, ctx.tick)
        }),
        def(209, "distribute", |w, ctx| distribute(w, ctx.tick)),
        def(220, "demography", |w, ctx| demography(w, ctx.tick)),
        def(225, "wage_grid", |w, _| wage_grid(w)),
        def(227, "migration", |w, ctx| migration(w, ctx.tick)),
        def(229, "settlement", |w, _| settlement(w)),
        def(230, "monetary", monetary),
        def(232, "fiscal", |w, _| fiscal(w)),
        def(233, "energy_policy", |w, _| energy_policy(w)),
        def(210, "assert", |w, ctx| assert_invariants(w, ctx.tick)),
    ])
}

/// Each country's multipliers of A and L0 at `tick` from the initial drift (ADR 0103, S23 and
/// P153): with fade(t) = 1 - 2^(-t / h), t = tick / 12 years, ln alpha = g_A h / ln 2 fade(t)
/// (TFP growing at g_A 2^(-t / h)) and ln phi = f fade(t) (the fixed factor F closing its log gap
/// f at half-life h). F enters VA as F / F0 = phi, and by homogeneity VA(A, F, L) =
/// VA(A phi, F0, L / phi): the multipliers are [alpha phi, phi]. Ones at tick 0, under S23 off
/// and for inputs without drifts.
pub fn drift_factors(inp: &EconInputs, params: &EconParams, tick: u32) -> Vec<[f64; 2]> {
    let c = &inp.countries;
    let n = inp.n_countries();
    if !params.drift || !inp.has_drift() {
        return vec![[1.0; 2]; n];
    }
    let h = params.drift_half_life;
    let fade = 1.0 - libm::exp2(-f64::from(tick) / 12.0 / h);
    (0..n)
        .map(|k| {
            let ln_phi = c.drift_f[k] * fade;
            let ln_alpha = c.drift_a[k] * h / std::f64::consts::LN_2 * fade;
            [libm::exp(ln_alpha + ln_phi), libm::exp(ln_phi)]
        })
        .collect()
}

/// E0: this tick's drift multipliers into `Scratch::drift`.
pub fn drift(w: &mut EconWorld, tick: u32) {
    w.scratch.drift = drift_factors(&w.inputs, &w.params, tick);
}

/// Enterprise `e`'s A and L0 under this tick's drift multipliers.
fn tech(inp: &EconInputs, drift: &[[f64; 2]], e: usize) -> (f64, f64) {
    let [a, l0] = drift[inp.country_of(inp.enterprises.province[e] as usize)];
    (inp.enterprises.a[e] * a, inp.enterprises.l0[e] * l0)
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
    w.start_refresh();
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
    // each traded good on its own ledger column, in parallel: its deliveries in buyer order,
    // then its largest, as one good after another
    {
        let _span = timing::span("deliver.take", NONE);
        w.ledger.take_columns(0..n_traded, &mut w.scratch.stock);
    }
    w.scratch.stock.par_iter_mut().for_each(|stock| {
        let g = stock.good();
        let transit = acc.transit[g];
        let run = &due[g * n_buyers..(g + 1) * n_buyers];
        // the first largest delivery, as `largest` finds it
        let mut last: Option<(usize, f64)> = None;
        for (b, &x) in run.iter().enumerate() {
            if x > 0.0 && last.is_none_or(|(_, v)| x > v) {
                last = Some((b, x));
            }
        }
        let receive = |stock: &mut GoodColumn, b: usize, x: f64| {
            stock.ship(transit, account(b), x);
            if b < n_pops {
                stock.destroy(account(b), x);
            }
        };
        for (b, &x) in run.iter().enumerate() {
            if x > 0.0 && last.is_some_and(|(l, _)| l != b) {
                let x = x.min(stock.goods(transit));
                receive(stock, b, x);
            }
        }
        // the largest delivery takes what the account holds beyond the goods still in flight
        if let Some((b, _)) = last {
            let x = (stock.goods(transit) - in_flight[g]).max(0.0);
            receive(stock, b, x);
        }
    });
    let _span = timing::span("deliver.put", NONE);
    w.ledger.put_columns(&w.scratch.stock);
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
    let drift = &w.scratch.drift;
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
                    let (a, l0) = tech(inp, drift, e);
                    labour_for_value_added(va, a, ent.va0[e], l0, inp.ent_s_l(e), r)
                })
                .collect();
            let targets = |w: f64| -> Vec<f64> {
                ents.clone()
                    .zip(p_va.iter().zip(&equipped))
                    .map(|(e, (&pv, &cap))| {
                        let (s_l, (a, l0)) = (inp.ent_s_l(e), tech(inp, drift, e));
                        labour_target(pv, w, a, ent.va0[e], l0, s_l, r, supply).min(cap)
                    })
                    .collect()
            };
            let w0 = wage[i];
            let (w1, t, clipped) = if params.frictionless {
                let natural = (1.0 - u_star) * supply;
                let free = |e: usize, pv: f64, w: f64| {
                    let (s_l, (a, l0)) = (inp.ent_s_l(e), tech(inp, drift, e));
                    labour_target_uncapped(pv, w, a, ent.va0[e], l0, s_l, r)
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
                            let (a, l0) = tech(inp, drift, e);
                            pv * marginal_value_added(a, ent.va0[e], l0, inp.ent_s_l(e), r, cap)
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
                    let (a, l0) = tech(inp, drift, e);
                    labour_target(pv, wt, a, ent.va0[e], l0, inp.ent_s_l(e), r, supply)
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
    {
        let _span = timing::span("e2.cap", NONE);
        cap_output(w);
    }
    let (inp, lay) = (
        std::sync::Arc::clone(&w.inputs),
        std::sync::Arc::clone(&w.layout),
    );
    let (inp, lay) = (&*inp, &*lay);
    let acc = &lay.accounts;
    let edu_eff = w.params.edu_eff;

    w.markets.supply.fill(0.0);
    w.scratch.drawn.fill(0.0);
    {
        let _span = timing::span("e2.local_chain", NONE);
        local::chain_all(w);
    }
    let _span = timing::span("e2.provinces", NONE);
    let np = inp.n_provinces();
    let ranges: Vec<[std::ops::Range<usize>; 3]> = (0..np)
        .map(|i| {
            [
                account_run(&acc.pop, lay.pops(i)),
                account_run(&acc.enterprise, lay.enterprises(i)),
                account_run(&acc.market, i..i + 1),
            ]
        })
        .collect();
    // every province's tables are contiguous runs, so each task owns its own slices
    let sc = &mut w.scratch;
    let from_output = &sc.from_output;
    let produced = &sc.produced;
    let setup = timing::span("e2.setup", NONE);
    let mut tasks: Vec<ProduceTask<'_>> = {
        let ents = (0..np).map(|i| lay.enterprises(i));
        let pops = (0..np).map(|i| lay.pops(i));
        let mut ledgers = w.ledger.province_slices(&ranges).into_iter();
        let mut before =
            split_runs(&mut sc.input_before, ents.clone().map(|r| r.len() * ng)).into_iter();
        let mut used =
            split_runs(&mut sc.input_used, ents.clone().map(|r| r.len() * ng)).into_iter();
        let mut output = split_runs(&mut w.output, ents.clone().map(|r| r.len())).into_iter();
        let mut sales = split_runs(&mut w.sales, ents.clone().map(|r| r.len())).into_iter();
        let mut paid = split_runs(&mut sc.income_paid, pops.clone().map(|r| r.len())).into_iter();
        let mut drawn = split_runs(&mut sc.drawn, (0..np).map(|_| ng)).into_iter();
        let mut supply = split_runs(&mut w.markets.supply, (0..np).map(|_| ng)).into_iter();
        (0..np)
            .map(|i| ProduceTask {
                i,
                pops: lay.pops(i),
                ents: lay.enterprises(i),
                ledger: ledgers.next().expect("one ledger per province"),
                input_before: before.next().expect("one slice per province"),
                input_used: used.next().expect("one slice per province"),
                output: output.next().expect("one slice per province"),
                sales: sales.next().expect("one slice per province"),
                income_paid: paid.next().expect("one slice per province"),
                drawn: drawn.next().expect("one slice per province"),
                supply: supply.next().expect("one slice per province"),
            })
            .collect()
    };
    let (price, wage, employment, employed) =
        (&w.markets.price, &w.wage, &w.employment, &w.employed);
    let on_sales = w.trade.anchor.is_some_and(|a| a.e3);
    drop(setup);
    let run = timing::span("e2.run", NONE);
    tasks.par_iter_mut().for_each(|t| {
        let i = t.i;
        let effective: Vec<f64> = t
            .pops
            .clone()
            .map(|k| edu_eff[usize::from(inp.pops.edu[k])] * employed[k])
            .collect();
        let mut parts = Vec::new();
        for e in t.ents.clone() {
            let (g, y) = (usize::from(inp.enterprises.good[e]), produced[e]);
            let (account, cell) = (acc.enterprise[e], i * ng + g);
            let (le, lk) = (e - t.ents.start, t.pops.start);
            for k in 0..nt {
                let a = lay.coef[e][k];
                let before = t.ledger.goods(account, k);
                if a > 0.0 {
                    t.ledger.destroy(account, k, (a * y).min(before));
                }
                t.input_before[le * ng + k] = before;
                t.input_used[le * ng + k] = before - t.ledger.goods(account, k);
            }
            t.output[le] = y;
            if y > 0.0 {
                t.ledger.create(account, g, y);
                t.ledger.ship(account, acc.market[i], g, y);
            }
            if !on_sales {
                let sale = price[cell] * y;
                t.sales[le] = sale;
                pay_in(&mut t.ledger, SALE, acc.market[i], account, to_minor(sale));
            }
            t.supply[g] += y;

            let bill = to_minor(wage[i] * employment[e]);
            split(bill, effective.iter().copied(), &mut parts);
            for (k, &part) in t.pops.clone().zip(parts.iter()) {
                pay_in(&mut t.ledger, WAGE, account, acc.pop[k], part);
                t.income_paid[k - lk] += part;
            }
        }
        // local inputs: this tick's output at the market first, the stock second
        for e in t.ents.clone() {
            let account = acc.enterprise[e];
            let le = e - t.ents.start;
            for k in nt..ng {
                let (at, cell) = (e * ng + k, i * ng + k);
                let need = lay.coef[e][k] * produced[e];
                let stock = t.ledger.goods(account, k);
                let take = from_output[at].min(t.ledger.goods(acc.market[i], k));
                if take > 0.0 {
                    t.ledger.ship(acc.market[i], account, k, take);
                    let value = to_minor(price[cell] * take);
                    pay_in(&mut t.ledger, PURCHASE, account, acc.market[i], value);
                    t.drawn[k] += take;
                }
                let used = take + (need - take).clamp(0.0, stock);
                if used > 0.0 {
                    t.ledger.destroy(account, k, used);
                }
                t.input_before[le * ng + k] = stock + take;
                t.input_used[le * ng + k] = used;
            }
        }
    });
    drop(run);
    let _put = timing::span("e2.put", NONE);
    let done = tasks.into_iter().map(|t| t.ledger.finish()).collect();
    w.ledger.put_province_slices(done);
    if on_sales {
        for (c, &y) in w.consigned.iter_mut().zip(&w.output) {
            *c += y;
        }
    }
}

/// E3 package (RFC 0342): after clearing, each market pays its producers p for the units it sold,
/// this tick's output first (pro rata), then the unpaid backlog (pro rata); unsold traded units
/// stay owed up to the market's stock, unsold local units perish.
pub fn pay_sales(w: &mut EconWorld) {
    if !w.trade.anchor.is_some_and(|a| a.e3) {
        return;
    }
    let (inp, lay) = (&*w.inputs, &*w.layout);
    let acc = &lay.accounts;
    let ng = inp.n_goods();
    let m = &w.markets;
    for i in 0..inp.n_provinces() {
        for g in 0..ng {
            let cell = i * ng + g;
            let ents: Vec<usize> = lay
                .enterprises(i)
                .filter(|&e| usize::from(inp.enterprises.good[e]) == g)
                .collect();
            if ents.is_empty() {
                continue;
            }
            let fresh: f64 = ents.iter().map(|&e| w.output[e]).sum();
            let back: f64 = ents.iter().map(|&e| w.consigned[e] - w.output[e]).sum();
            let sold = m.sales[cell];
            let f = sold.min(fresh);
            let b = (sold - f).min(back).max(0.0);
            for &e in &ents {
                let prior = w.consigned[e] - w.output[e];
                let units = if fresh > 0.0 {
                    w.output[e] / fresh * f
                } else {
                    0.0
                } + if back > 0.0 { prior / back * b } else { 0.0 };
                w.consigned[e] = (w.consigned[e] - units).max(0.0);
                let sale = m.price[cell] * units;
                w.sales[e] = sale;
                pay(
                    &mut w.ledger,
                    SALE,
                    acc.market[i],
                    acc.enterprise[e],
                    to_minor(sale),
                );
            }
            let owed: f64 = ents.iter().map(|&e| w.consigned[e]).sum();
            let cap = if m.is_traded(cell) {
                m.inventory[cell]
            } else {
                0.0
            };
            if owed > cap {
                let r = if owed > 0.0 { cap / owed } else { 0.0 };
                for &e in &ents {
                    w.consigned[e] *= r;
                }
            }
        }
    }
}

/// One province's slices of E2's tables and its accounts.
struct ProduceTask<'a> {
    i: usize,
    pops: std::ops::Range<usize>,
    ents: std::ops::Range<usize>,
    ledger: ceteris_core::ledger::ProvinceLedger<'a>,
    input_before: &'a mut [f64],
    input_used: &'a mut [f64],
    output: &'a mut [f64],
    sales: &'a mut [f64],
    income_paid: &'a mut [i64],
    drawn: &'a mut [f64],
    supply: &'a mut [f64],
}

fn pay_in(
    ledger: &mut ceteris_core::ledger::ProvinceLedger<'_>,
    flow: Flow,
    from: AccountId,
    to: AccountId,
    amount: i64,
) {
    if amount != 0 {
        ledger.transfer(flow, from, to, amount);
    }
}

/// The account ids of `accounts[range]` as one run of the ledger's account space.
fn account_run(accounts: &[AccountId], range: std::ops::Range<usize>) -> std::ops::Range<usize> {
    let ids = &accounts[range];
    let Some(first) = ids.first() else {
        return 0..0;
    };
    let start = first.0 as usize;
    assert!(
        ids.iter()
            .enumerate()
            .all(|(j, a)| a.0 as usize == start + j),
        "province accounts are not one run"
    );
    start..start + ids.len()
}

/// `slice` cut into consecutive pieces of the given lengths.
fn split_runs<T>(mut slice: &mut [T], lens: impl Iterator<Item = usize>) -> Vec<&mut [T]> {
    lens.map(|n| {
        let (head, rest) = std::mem::take(&mut slice).split_at_mut(n);
        slice = rest;
        head
    })
    .collect()
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
            let (s_l, (a, l0)) = (inp.ent_s_l(e), tech(inp, &w.scratch.drift, e));
            let va = value_added(a, ent.va0[e], l0, s_l, r, employment[e]);
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
    // households choose at the buyer price, market price plus the energy wedge (system 233)
    let buyer_index = w.energy.as_ref().map(|p| {
        let mut v = w.price_index.clone();
        for i in 0..inp.n_provinces() {
            let wedges = &p.countries[inp.country_of(i)];
            for (&g, &wedge) in p.goods.iter().zip(&wedges.wedge) {
                v[i * ng + g] += to_numeraire(wedge as i64);
            }
        }
        v
    });
    let sc = &mut w.scratch;
    let (ledger, params, index) = (&w.ledger, &w.params, &w.price_index);
    let row = |i: usize| &index[i * ng..(i + 1) * ng];
    let buyer = buyer_index.as_deref().unwrap_or(index);
    let buyer_row = |i: usize| &buyer[i * ng..(i + 1) * ng];
    // the price-only part of PIGL once per province, for all its pops
    let at: Vec<PiglAt> = (0..inp.n_provinces())
        .into_par_iter()
        .map(|i| lay.pigl_at(inp, params, i, buyer_row(i)))
        .collect();
    sc.pop_demand
        .par_chunks_mut(ng)
        .zip(sc.pop_spend.par_chunks_mut(ng))
        .enumerate()
        .for_each(|(k, (x, spend))| {
            let i = inp.pops.province[k] as usize;
            let prices = row(i);
            let money = to_numeraire(ledger.money(acc.pop[k]));
            lay.pop_demand_at(inp, params, k, &at[i], buyer_row(i), money, x);
            for g in 0..ng {
                spend[g] = if g < nt && x[g] > 0.0 {
                    x[g] * prices[g]
                } else {
                    0.0
                };
            }
        });
    let (pipeline, lead) = (&w.pipeline, &sc.lead);
    let (short, notional, drift) = (&sc.short, &sc.notional, &sc.drift);
    let credit = w.trade.anchor.is_some_and(|a| a.e3);
    sc.ent_order
        .par_chunks_mut(ng)
        .zip(sc.ent_spend.par_chunks_mut(ng))
        .enumerate()
        .for_each(|(e, (orders, spend))| {
            let ent = &inp.enterprises;
            let i = ent.province[e] as usize;
            let (prices, s_l, (a, l0)) = (row(i), inp.ent_s_l(e), tech(inp, drift, e));
            let output = |l: f64| value_added(a, ent.va0[e], l0, s_l, r, l) / lay.a_va[e];
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
            // E3 package: traded input spending within cash plus a credit line of one loss cap (P148)
            if credit {
                let cost: f64 = spend[..nt].iter().sum();
                let budget = (to_numeraire(ledger.money(acc.enterprise[e]))
                    + to_numeraire(lay.ent_loss_cap[e]))
                .max(0.0);
                if cost > budget {
                    let s = budget / cost;
                    orders[..nt].iter_mut().for_each(|o| *o *= s);
                    spend[..nt].iter_mut().for_each(|x| *x *= s);
                }
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
    source_inputs(w);
    source_kernel(w);
    source_commit(w);
}

/// The sourcing kernel of every traded good, one task per good.
pub(crate) fn source_kernel(w: &mut EconWorld) {
    let (lay, kernel) = (&*w.layout, &*w.kernel);
    let sc = &mut w.scratch;
    let weight_price = &w.weight_price;
    sc.sourced
        .par_iter_mut()
        .zip(&sc.columns)
        .enumerate()
        .for_each(|(g, (sourced, col))| {
            let wp = (!weight_price.is_empty()).then_some(&col.weight_price[..]);
            source_weighted(
                kernel,
                g,
                &col.price,
                wp,
                &col.active,
                &lay.gamma[g],
                &col.spend,
                sourced,
            );
        });
}

/// The per-good columns [`source`] reads: spending by class, posted prices, active origins and
/// the smoothed prices.
pub(crate) fn source_inputs(w: &mut EconWorld) {
    let ng = w.inputs.n_goods();
    let (inp, lay) = (&*w.inputs, &*w.layout);
    let np = inp.n_provinces();
    let sc = &mut w.scratch;
    let (ent_spend, pop_spend) = (&sc.ent_spend, &sc.pop_spend);
    let (markets, weight_price) = (&w.markets, &w.weight_price);
    sc.columns.par_iter_mut().enumerate().for_each(|(g, col)| {
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
        col.gather_weight(weight_price, g, ng);
    });
}

/// Writes the orders of the `sourced` columns into the markets' demand.
pub(crate) fn source_commit(w: &mut EconWorld) {
    let ng = w.inputs.n_goods();
    let np = w.inputs.n_provinces();
    let sc = &mut w.scratch;
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
    // the local goods in parallel, each on its own ledger column: a good's goods moves read and
    // write only that good, in province order; its purchases are collected per province and
    // posted in province then good order, as one good after another within each province paid
    let sc = &mut w.scratch;
    {
        let _span = timing::span("clear.take", NONE);
        w.ledger.take_columns(nt..ng, &mut sc.stock);
    }
    sc.local.resize_with(ng - nt, LocalClear::default);
    let (m, rationing) = (&w.markets, w.params.rationing);
    let (spoiled, drawn) = (&sc.spoiled, &sc.drawn);
    let (pop_demand, ent_order) = (&sc.pop_demand, &sc.ent_order);
    (&mut sc.stock, &mut sc.local)
        .into_par_iter()
        .for_each(|(stock, out)| {
            let g = stock.good();
            let LocalClear {
                buyers,
                entries,
                ends,
            } = out;
            entries.clear();
            ends.clear();
            for i in 0..inp.n_provinces() {
                let market = acc.market[i];
                let cell = i * ng + g;
                if m.active[cell] {
                    let keep = spoiled[cell];
                    let price = m.price[cell];
                    let rest = (m.supply[cell] - drawn[cell]).max(0.0);
                    buyers.clear();
                    for k in lay.pops(i) {
                        buyers.push((acc.pop[k], pop_demand[k * ng + g], true));
                    }
                    for e in lay.enterprises(i) {
                        buyers.push((acc.enterprise[e], ent_order[e * ng + g], false));
                    }
                    let total = |households: bool| -> f64 {
                        buyers
                            .iter()
                            .filter(|b| b.2 == households)
                            .map(|b| b.1)
                            .sum()
                    };
                    let (orders, households) = (total(false), total(true));
                    let [fill_orders, fill_households] = rationing.fill(rest, orders, households);
                    for b in buyers.iter_mut() {
                        b.1 *= if b.2 { fill_households } else { fill_orders };
                    }
                    if let Some(last) = largest(buyers.iter().map(|b| b.1)) {
                        let n = buyers.len();
                        buyers.swap(last, n - 1);
                        for (j, &(buyer, allocation, consumes)) in buyers.iter().enumerate() {
                            let q = if j + 1 == n {
                                // the largest buyer takes the market's balance beyond what stays
                                (stock.goods(market) - keep).max(0.0)
                            } else {
                                allocation.min(stock.goods(market))
                            };
                            if q > 0.0 {
                                stock.ship(market, buyer, q);
                                if consumes {
                                    stock.destroy(buyer, q);
                                }
                            }
                            let amount = to_minor(price * q);
                            if amount != 0 {
                                entries.push(Entry {
                                    flow: PURCHASE,
                                    from: buyer,
                                    to: market,
                                    amount,
                                });
                            }
                        }
                    }
                    let rest = stock.goods(market);
                    if rest > 0.0 {
                        stock.destroy(market, rest);
                    }
                }
                ends.push(entries.len() as u32);
            }
        });
    {
        let _span = timing::span("clear.put", NONE);
        w.ledger.put_columns(&sc.stock);
    }
    sc.local_entries.clear();
    for i in 0..inp.n_provinces() {
        for l in &sc.local {
            let lo = if i == 0 { 0 } else { l.ends[i - 1] as usize };
            sc.local_entries
                .extend_from_slice(&l.entries[lo..l.ends[i] as usize]);
        }
    }
    w.ledger.post(&sc.local_entries, &mut sc.post);
}

/// Spending on traded goods per enterprise, per pop and per destination cell by class
/// (`Scratch::ent_spend`, `pop_spend`, `class_spend`).
#[derive(Clone, Copy)]
struct Spending<'a> {
    /// Goods per row of `class` and per block of `enterprise` and `pop`.
    goods: usize,
    /// `Scratch::ent_spend_t` and `pop_spend_t`, blocked good-major.
    enterprise: &'a [f64],
    pop: &'a [f64],
    class: &'a [[f64; 2]],
}

/// Index of buyer `i`'s spending on good `g` in a blocked good-major array.
fn blocked(i: usize, g: usize, goods: usize) -> usize {
    (i / SPEND_BLOCK) * SPEND_BLOCK * goods + g * SPEND_BLOCK + i % SPEND_BLOCK
}

/// Regroups `rows` (`[buyer * goods + g]`) into `out` by blocks of `SPEND_BLOCK` buyers, good
/// major inside a block; each block is one task, reading its rows once.
fn transpose_spend(rows: &[f64], goods: usize, out: &mut [f64]) {
    out.par_chunks_mut(SPEND_BLOCK * goods)
        .zip(rows.par_chunks(SPEND_BLOCK * goods))
        .for_each(|(block, rows)| {
            for (i, row) in rows.chunks_exact(goods).enumerate() {
                for (g, &x) in row.iter().enumerate() {
                    block[g * SPEND_BLOCK + i] = x;
                }
            }
        });
}

/// Buyers of good `g` in province `d` with positive spending, enterprises then pops.
fn buyers_of(lay: &Layout, spend: Spending, d: usize, g: usize, out: &mut Vec<Buyer>) {
    let acc = &lay.accounts;
    let n_pops = acc.pop.len();
    out.clear();
    for e in lay.enterprises(d) {
        let spend = spend.enterprise[blocked(e, g, spend.goods)];
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
        let spend = spend.pop[blocked(k, g, spend.goods)];
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
fn buyer_orders(
    lay: &Layout,
    spend: Spending,
    g: usize,
    r: &Sourced,
    p: &mut Payments,
    detail: bool,
) {
    let acc = &lay.accounts;
    p.orders.clear();
    p.order_ends.clear();
    p.entries.clear();
    p.buyer_amt.clear();
    if !detail {
        p.buyer_amt.resize(acc.pop.len() + acc.enterprise.len(), 0);
    }
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
            if detail {
                if amount != 0 {
                    p.entries.push(Entry {
                        flow: PURCHASE,
                        from: b.account,
                        to: market,
                        amount,
                    });
                }
            } else {
                p.buyer_amt[b.index] = amount;
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
    {
        let _span = timing::span("t2b.gather_all", NONE);
        dispatch_gather(w);
    }
    {
        let _span = timing::span("t2b.kernel_all", NONE);
        dispatch_kernel(w);
    }
    dispatch_finish(w);
}

/// The per-good columns of the dispatch kernel: posted prices and the fill ratios.
pub(crate) fn dispatch_gather(w: &mut EconWorld) {
    let ng = w.inputs.n_goods();
    let np = w.kernel.n_provinces();
    let sc = &mut w.scratch;
    transpose_spend(&sc.pop_spend, ng, &mut sc.pop_spend_t);
    transpose_spend(&sc.ent_spend, ng, &mut sc.ent_spend_t);
    let (fill, markets) = (&sc.fill, &w.markets);
    sc.columns.par_iter_mut().enumerate().for_each(|(g, col)| {
        col.gather(&markets.price, &markets.active, g, ng);
        col.fill.clear();
        col.fill.extend((0..np).map(|o| fill[o * ng + g]));
    });
}

/// The dispatch kernel of every traded good, one task per good.
pub(crate) fn dispatch_kernel(w: &mut EconWorld) {
    let kernel = &*w.kernel;
    let sc = &mut w.scratch;
    sc.sourced
        .par_iter_mut()
        .zip(&sc.columns)
        .enumerate()
        .for_each(|(g, (r, col))| {
            let _span = timing::span("t2b.kernel", g as u32);
            dispatch_good(kernel, g, &col.price, &col.fill, r);
        });
}

pub(crate) fn dispatch_finish(w: &mut EconWorld) {
    let ng = w.inputs.n_goods();
    let inp = std::sync::Arc::clone(&w.inputs);
    let lay = std::sync::Arc::clone(&w.layout);
    let kernel = std::sync::Arc::clone(&w.kernel);
    let (inp, lay, kernel) = (&*inp, &*lay, &*kernel);
    let acc = &lay.accounts;
    let np = kernel.n_provinces();
    let detail = w.scratch.detail;
    let sc = &mut w.scratch;
    let (markets, net) = (&w.markets, &inp.network);
    let spending = Spending {
        goods: ng,
        enterprise: &sc.ent_spend_t,
        pop: &sc.pop_spend_t,
        class: &sc.class_spend,
    };
    // each good's goods moves run in its own task, on its own ledger column: they read and
    // write only that good, in the same order as one good after another
    let nt = sc.payments.len();
    {
        let _span = timing::span("t2b.take", NONE);
        w.ledger.take_columns(0..nt, &mut sc.stock);
    }
    let inventory = &markets.inventory;
    let goods_span = timing::span("t2b.goods", NONE);
    (
        &mut sc.sourced,
        &mut sc.columns,
        &mut sc.payments,
        sc.flows.current(),
        &mut sc.stock,
    )
        .into_par_iter()
        .enumerate()
        .for_each(|(g, (r, col, legs, flows, stock))| {
            let gi = g as u32;
            let _good = timing::span("t2b.good", gi);
            let r = &*r;
            let record = || {
                let _span = timing::span("t2b.record", gi);
                let (pc, gc) = (&net.province_country, &net.gateway_country);
                flows.record(kernel, pc, gc, g, &col.price, r);
            };
            if detail {
                // the buyers' entries come before the legs' in one list
                rayon::join(record, || {
                    let _span = timing::span("t2b.legs", gi);
                    buyer_orders(lay, spending, g, r, legs, detail);
                    trade_legs(inp, acc, kernel, g, r, legs, (detail, g == 0));
                });
            } else {
                // netted: the buyers' and the legs' amounts are separate vectors, so the legs
                // run on their own `Payments` holding those vectors meanwhile
                let mut apart = Payments {
                    edge_amt: std::mem::take(&mut legs.edge_amt),
                    topo: std::mem::take(&mut legs.topo),
                    at_gateway: std::mem::take(&mut legs.at_gateway),
                    from_gateway: std::mem::take(&mut legs.from_gateway),
                    parts: std::mem::take(&mut legs.parts),
                    ..Payments::default()
                };
                rayon::join(record, || {
                    rayon::join(
                        || {
                            let _span = timing::span("t2b.buyers", gi);
                            buyer_orders(lay, spending, g, r, legs, detail);
                        },
                        || {
                            let _span = timing::span("t2b.legs", gi);
                            trade_legs(inp, acc, kernel, g, r, &mut apart, (detail, g == 0));
                        },
                    )
                });
                legs.edge_amt = apart.edge_amt;
                legs.topo = apart.topo;
                legs.at_gateway = apart.at_gateway;
                legs.from_gateway = apart.from_gateway;
                legs.parts = apart.parts;
            }
            let _span = timing::span("t2b.moves", gi);
            goods_moves(acc, np, ng, inventory, legs, stock);
        });
    drop(goods_span);
    let moves_span = timing::span("t2b.apply", NONE);
    {
        let _span = timing::span("t2b.put", NONE);
        w.ledger.put_columns(&w.scratch.stock);
    }
    let sourced = std::mem::take(&mut w.scratch.sourced);
    let mut payments = std::mem::take(&mut w.scratch.payments);
    for (g, (r, legs)) in sourced.iter().zip(&payments).enumerate() {
        for d in 0..np {
            w.scratch.dispatched[d * ng + g] = r.shipped[d];
        }
        w.scratch.dispatch_made[g] = legs.made;
        w.scratch.dispatch_due[g] = r.selected - r.lost;
        w.scratch.dispatch_scale[g] = r.shipped.iter().sum::<f64>()
            + r.arrive.iter().flatten().map(|a| a[0] + a[1]).sum::<f64>();
    }
    let pushes: Vec<&[Push]> = payments.iter().map(|p| &p.pushes[..]).collect();
    w.pipeline.push_all(&pushes);
    drop(moves_span);
    // every good's buyers' payments, then its legs; nothing above reads money
    {
        let _span = timing::span("t2b.post", NONE);
        if detail {
            let mut blocks: Vec<&mut Vec<Entry>> =
                payments.iter_mut().map(|p| &mut p.entries).collect();
            w.ledger.post_blocks(&mut blocks, &mut w.scratch.post);
        } else {
            let sc = &mut w.scratch;
            net_legs(
                lay,
                &payments,
                &mut sc.buyer_tot,
                &mut sc.edge_tot,
                &mut sc.netted,
            );
            w.ledger.post_blocks(&mut [&mut sc.netted], &mut sc.post);
        }
    }
    let _transit = timing::span("t2b.transit", NONE);
    // each transit account is brought to the goods now in flight: the losses melt what it
    // holds beyond them, selection adds what they hold beyond it
    let in_flight = w.pipeline.in_flight_cached();
    for (g, &transit) in acc.transit.iter().enumerate() {
        let change = in_flight[g] - w.ledger.goods(transit, g);
        adjust_stock(&mut w.ledger, transit, g, change);
        w.scratch.dispatch_made[g] += change;
    }
    w.scratch.sourced = sourced;
    w.scratch.payments = payments;
}

/// Chunk of buyers or legs summed over goods in one task by [`net_legs`].
const NET_CHUNK: usize = 1 << 12;

/// `out[j] = sum over goods of column(good)[lo + j]` for the chunk starting at `lo`; the sum of
/// a leg's rounded amounts is asserted to fit `i64`.
fn sum_columns(out: &mut [i64], lo: usize, columns: &[&[i64]]) {
    out.fill(0);
    for col in columns {
        for (t, &x) in out.iter_mut().zip(&col[lo..]) {
            *t = t.checked_add(x).expect("netted leg overflows i64");
        }
    }
}

/// Netted T2b journal (RFC 0316): the goods' rounded payments summed per buyer and per network
/// leg, as one entry each with a non-zero total, purchases in buyer order (province by province,
/// enterprises then pops) and then trade and tariff legs in the network's leg order. Sums of
/// rounded amounts, so every balance equals the per-leg posting's.
fn net_legs(
    lay: &Layout,
    payments: &[Payments],
    buyer_tot: &mut Vec<i64>,
    edge_tot: &mut Vec<i64>,
    out: &mut Vec<Entry>,
) {
    out.clear();
    let Some(first) = payments.first() else {
        return;
    };
    let acc = &lay.accounts;
    let n_pops = acc.pop.len();
    let buyers: Vec<&[i64]> = payments.iter().map(|p| &p.buyer_amt[..]).collect();
    buyer_tot.resize(buyers[0].len(), 0);
    buyer_tot
        .par_chunks_mut(NET_CHUNK)
        .enumerate()
        .for_each(|(k, t)| sum_columns(t, k * NET_CHUNK, &buyers));
    for d in 0..acc.market.len() {
        let market = acc.market[d];
        let ents = lay.enterprises(d).map(|e| (acc.enterprise[e], n_pops + e));
        let pops = lay.pops(d).map(|k| (acc.pop[k], k));
        for (account, i) in ents.chain(pops) {
            if buyer_tot[i] != 0 {
                out.push(Entry {
                    flow: PURCHASE,
                    from: account,
                    to: market,
                    amount: buyer_tot[i],
                });
            }
        }
    }
    let edges: Vec<&[i64]> = payments.iter().map(|p| &p.edge_amt[..]).collect();
    assert!(
        edges.iter().all(|e| e.len() == first.topo.len()),
        "goods differ in network legs"
    );
    edge_tot.resize(first.topo.len(), 0);
    edge_tot
        .par_chunks_mut(NET_CHUNK)
        .enumerate()
        .for_each(|(k, t)| sum_columns(t, k * NET_CHUNK, &edges));
    for (&(flow, from, to), &amount) in first.topo.iter().zip(edge_tot.iter()) {
        if amount != 0 {
            out.push(Entry {
                flow,
                from,
                to,
                amount,
            });
        }
    }
}

/// T2b's goods moves of one traded good on its ledger column, buyer by buyer in destination
/// order: home purchases now (households consume what they hold), shipments queued for the
/// pipeline in `legs.pushes`, then what leaves each origin market beyond its home sales and
/// its carried stock into transit. Sets `legs.made`, the goods selection created.
fn goods_moves(
    acc: &Accounts,
    np: usize,
    ng: usize,
    inventory: &[f64],
    legs: &mut Payments,
    stock: &mut GoodColumn,
) {
    let g = stock.good();
    let transit = acc.transit[g];
    let mut made = 0.0;
    let mut first = 0;
    legs.pushes.clear();
    for d in 0..np {
        let market = acc.market[d];
        // buyers at d: home purchases now, shipments into the pipeline
        let end = legs.order_ends[d] as usize;
        for o in &legs.orders[first..end] {
            let shipped = o.home_shipped.min(stock.goods(market));
            if shipped > 0.0 {
                stock.ship(market, o.account, shipped);
                let home = if shipped == o.home_shipped {
                    o.home
                } else {
                    o.home * (shipped / o.home_shipped)
                };
                let change = home - shipped;
                if change >= 0.0 {
                    stock.create(o.account, change);
                } else {
                    stock.destroy(o.account, -change);
                }
                made += change;
                if o.household {
                    // pops hold nothing between purchases, so they consume what they hold:
                    // `home` itself can differ from it by the rounding of the adjustment
                    let held = stock.goods(o.account);
                    stock.destroy(o.account, held);
                }
            }
            for (slot, &units) in o.slots.iter().enumerate() {
                if units > 0.0 {
                    legs.pushes.push((slot as u8 + 1, o.buyer as u32, units));
                }
            }
        }
        first = end;
        // what leaves the origin market beyond its home sales and its carried stock
        let x = (stock.goods(market) - inventory[d * ng + g]).max(0.0);
        if x > 0.0 {
            stock.ship(market, transit, x);
        }
    }
    legs.made = made;
}

/// The money legs of good `g`'s dispatch `r` in minor units, rounded half to even per leg,
/// as ledger entries in payment order, after the buyers' ([`buyer_orders`]): destinations
/// pay origins and their gateways, each gateway splits what it takes in over the legs that
/// fed it ([`split`]), so it pays out exactly that, and destinations pay tariffs to their
/// country's rebate account. In detail mode zero amounts are left out, as `pay` leaves them;
/// otherwise every leg's amount goes to `edge_amt` in the order the network fixes, the same
/// for every good, and `topo` (when `topo` is set) names the legs.
fn trade_legs(
    inp: &EconInputs,
    acc: &Accounts,
    kernel: &Kernel,
    g: usize,
    r: &Sourced,
    p: &mut Payments,
    (detail, topo): (bool, bool),
) {
    let (np, ng) = (kernel.n_provinces(), kernel.n_gateways());
    let gk = kernel.good(g);
    let (entries, edge_amt, legs_of) = (&mut p.entries, &mut p.edge_amt, &mut p.topo);
    edge_amt.clear();
    legs_of.clear();
    let mut leg = |flow: Flow, from: AccountId, to: AccountId, amount: i64| {
        if detail {
            if amount != 0 {
                entries.push(Entry {
                    flow,
                    from,
                    to,
                    amount,
                });
            }
        } else {
            edge_amt.push(amount);
            if topo {
                legs_of.push((flow, from, to));
            }
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
        for (&(a, _), &part) in legs.iter().zip(p.parts.iter()) {
            leg(TRADE, acc.gateway[b], acc.gateway[a as usize], part);
            p.from_gateway[a as usize] += part;
        }
    }
    for a in 0..ng {
        let legs = kernel.access_by_gateway().column(a);
        let weights = legs.iter().map(|&(_, j)| r.access_out_value[j as usize]);
        split(p.from_gateway[a], weights, &mut p.parts);
        for (&(o, _), &part) in legs.iter().zip(p.parts.iter()) {
            leg(TRADE, acc.gateway[a], acc.market[o as usize], part);
        }
    }
    for d in 0..np {
        let rebate = acc.rebate[inp.country_of(d)];
        leg(TARIFF, acc.market[d], rebate, to_minor(r.tariff[d]));
    }
}

/// The anchor's index under the E3 package (trade SPEC, Money-anchored rule): the level of the
/// last repricing moved by the base-weight-by-last-price weighted mean log change over cells
/// bought at both repricings. Traded cells with a finite buyer price are bought, local cells
/// when they sold; a price nobody pays does not move the index.
fn chained_index(w: &mut EconWorld) -> f64 {
    let ng = w.inputs.n_goods();
    let traded = w.inputs.goods.traded;
    let bought: Vec<bool> = (0..w.price_index.len())
        .map(|c| w.price_index[c].is_finite() && (c % ng < traded || w.markets.sales[c] > 0.0))
        .collect();
    let level = match &w.anchor_chain {
        None => w.world_price_index(),
        Some(prev) => {
            let (mut num, mut den) = (0.0, 0.0);
            for (c, &now) in bought.iter().enumerate() {
                if now && prev.bought[c] {
                    let share = w.layout.index_weights[c] * prev.price[c];
                    num += share * libm::log(w.price_index[c] / prev.price[c]);
                    den += share;
                }
            }
            prev.level * libm::exp(if den > 0.0 { num / den } else { 0.0 })
        }
    };
    w.anchor_chain = Some(crate::world::AnchorChain {
        price: w.price_index.clone(),
        bought,
        level,
    });
    level
}

/// T4: the stock-flow price rule with expected inflation on every active market (cells
/// without a market keep their price, which nothing reads), then the update of expected
/// inflation from the world price index (ADR 0019).
pub fn reprice(w: &mut EconWorld) {
    if let Some(a) = w.trade.anchor {
        let index = if a.e3 {
            chained_index(w)
        } else {
            w.world_price_index()
        };
        let money = w.money_issued();
        let (i0, m0) = *w.anchor_base.get_or_insert((index, money));
        let nominal = a.kappa_m * (libm::log(index / i0) - libm::log(money / m0));
        w.markets.reprice_anchored(&w.trade, &a, nominal);
        w.update_expectation();
        return;
    }
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
    if let Some(government) = &mut w.fiscal {
        for (c, (&pool, &gov)) in acc.dividend.iter().zip(&acc.government).enumerate() {
            let base = if lay.country_pops[c].is_empty() {
                0
            } else {
                w.ledger.money_wide(pool).max(0)
            };
            let tax = government.corporate_tax(c, base);
            let tax = i64::try_from(tax).expect("ledger overflow: corporate tax beyond i64");
            pay(&mut w.ledger, TAX, pool, gov, tax);
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
            for (&k, &part) in pops.iter().zip(parts.iter()) {
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

/// Country `c`'s transfer ratio rho_c, minus its current account over its GDP, at tick `tick`
/// under `mode` (ADRs 0100, 0315, S22): the 2000 ratio (`FixedShares`), the ratio of the tick's
/// year 2000 + tick / 12 (`ObservedPath`), which after the last year fades as rho_c(2024)
/// 2^-((tick - 299) / 120), or 0 (`Off`, or inputs without ratios).
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

/// E4 transfers (ADR 0100 as amended by ADRs 0299, 0300, 0315 and 0318 to 0321): each country's D_c =
/// (rho_c - rho_bar) Y_c, Y_c its pops' income of the previous tick (`EconWorld::income`; the
/// tick's own where the world has none, at the first tick of a data-state start) and rho_bar =
/// sum_c rho_c Y_c / Y, Y = sum_c Y_c, so the transfers net to zero. With s_c = D_c / Y, each
/// paying country (s_c < 0) puts round_half_even(|s_c| Y) minor units into the world transfer
/// account, its pops in proportion to the income credited to them this tick; the account pays
/// their total out over the receiving countries in proportion to s_c, and each over its pops the
/// same way ([`split`]), so it ends at zero. Nothing without the account
/// (`world::transfers_on`).
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
    let paid: Vec<f64>;
    let income_k: &[f64] = if fixed_sum(&w.income) > 0.0 {
        &w.income
    } else {
        paid = w
            .scratch
            .income_paid
            .iter()
            .map(|&m| to_numeraire(m))
            .collect();
        &paid
    };
    let national: Vec<f64> = lay
        .country_pops
        .iter()
        .map(|pops| {
            fixed_sum(
                &pops
                    .iter()
                    .map(|&k| income_k[k as usize])
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    let world = fixed_sum(&national);
    let rho: Vec<f64> = (0..inp.n_countries())
        .map(|c| transfer_share(inp, mode, c, tick))
        .collect();
    let weighted: Vec<f64> = rho.iter().zip(&national).map(|(r, y)| r * y).collect();
    let rho_bar = if world > 0.0 {
        fixed_sum(&weighted) / world
    } else {
        0.0
    };
    let share: Vec<f64> = (0..inp.n_countries())
        .map(|c| {
            if world > 0.0 {
                (rho[c] - rho_bar) * national[c] / world
            } else {
                0.0
            }
        })
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
        for (&k, &part) in pops.iter().zip(parts.iter()) {
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
        for (&k, &part) in pops.iter().zip(parts.iter()) {
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
        for (&k, &part) in pops.iter().zip(parts.iter()) {
            pay(&mut w.ledger, WRITE_OFF, acc.pop[k as usize], pool, part);
        }
        weights.clear();
        weights.extend(holders.iter().map(|&(_, x)| x));
        largest_remainder(total, &weights, &mut parts);
        for (&(account, _), &part) in holders.iter().zip(parts.iter()) {
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

/// Systems 220 to 223 (`specs/pop/SPEC.md`): age, mortality, fertility, education on the cohort
/// tick. A no-op when population is off.
pub fn demography(w: &mut EconWorld, tick: u32) {
    let Some((d, cohorts)) = &mut w.pop else {
        return;
    };
    let before = cohorts.total();
    let extra = w.disease.as_ref().map(|(dis, cov)| dis.excess_q(cov));
    let flows = d.step_with(cohorts, tick, extra.as_deref());
    if let Some((dis, cov)) = &mut w.disease {
        dis.step(cov);
    }
    let expected = i128::from(before) + i128::from(flows.births) - i128::from(flows.deaths);
    w.pop_drift = i64::try_from(i128::from(cohorts.total()) - expected).unwrap_or(i64::MAX);
}

/// System 227 (`specs/pop/SPEC.md`): internal migration and capped international admissions on
/// the cohort cells. A no-op when population or migration is off. Moves persons between
/// provinces only, so any change of the total is a conservation breach.
pub fn migration(w: &mut EconWorld, tick: u32) {
    let (Some((_, cohorts)), Some(m)) = (&mut w.pop, &mut w.migration) else {
        return;
    };
    let country: Vec<usize> = (0..w.inputs.n_provinces())
        .map(|i| w.inputs.country_of(i))
        .collect();
    let before = cohorts.total();
    m.step(cohorts, &w.wage, &country, tick);
    let drift = i128::from(cohorts.total()) - i128::from(before);
    w.pop_drift = w
        .pop_drift
        .saturating_add(i64::try_from(drift).unwrap_or(i64::MAX));
}

/// System 225 (`specs/pop/SPEC.md`, P4 C1 and C2): monthly split of the province wages over
/// sectors and skills and sector switching on them. A no-op when the wage grid is off.
pub fn wage_grid(w: &mut EconWorld) {
    if let Some(g) = &mut w.wage_grid {
        g.step(&w.wage);
    }
}

/// System 229 (`specs/pop/SPEC.md`, P4 D3): monthly settlement class step after migration. Class
/// counts follow the cohort totals per province; classes only trade persons among themselves.
/// A no-op when settlement is off.
pub fn settlement(w: &mut EconWorld) {
    let (Some((_, cohorts)), Some(s)) = (&w.pop, &mut w.settlement) else {
        return;
    };
    let totals: Vec<u64> = (0..cohorts.n_provinces())
        .map(|p| {
            cohorts.counts()[p * crate::pop::CELLS..(p + 1) * crate::pop::CELLS]
                .iter()
                .sum()
        })
        .collect();
    s.sync_totals(&totals);
    let wage: Vec<f64> = w
        .wage
        .iter()
        .flat_map(|&x| [x; crate::settlement::N_CLASS])
        .collect();
    s.step(&wage);
}

/// System 230 (`specs/money/SPEC.md`, P5 B1): monthly policy rate of each country's bank from
/// its price level and employment. Reads the world, changes only the policy state. A no-op when
/// monetary policy is off.
pub fn monetary(w: &mut EconWorld, ctx: &Ctx) {
    let Some(policy) = &w.monetary else {
        return;
    };
    let nc = policy.banks.len();
    let ng = w.inputs.n_goods();
    let (mut num, mut den) = (vec![0.0; nc], vec![0.0; nc]);
    for (c, (&a, &p)) in w
        .layout
        .index_weights
        .iter()
        .zip(&w.price_index)
        .enumerate()
    {
        if p.is_finite() {
            let country = w.inputs.country_of(c / ng);
            num[country] += a * p;
            den[country] += a;
        }
    }
    let level: Vec<f64> = num.iter().zip(&den).map(|(n, d)| n / d).collect();
    let mut employment = vec![0.0; nc];
    for (e, &l) in w.employment.iter().enumerate() {
        employment[w
            .inputs
            .country_of(w.inputs.enterprises.province[e] as usize)] += l;
    }
    if let Some(policy) = &mut w.monetary {
        policy.step(&level, &employment);
        if policy.hazards.is_some() {
            let u: Vec<f64> = (0..nc)
                .map(|c| {
                    ctx.stream(crate::regime_hazard::KIND, EntityId(c as u32))
                        .uniform()
                })
                .collect();
            policy.switch_regimes(&u);
        }
    }
}

/// System 232 (`specs/money/SPEC.md`, P5 C1; ADRs 0359 to 0363): monthly collection and
/// spending through each country's government account. The income and payroll base is the pops'
/// wage income (their province's wage bill by effective labour, as the E-step splits it), the
/// VAT base their spending; corporate tax was collected from the profit pool at E4
/// ([`distribute`]). Each of these taxes is taken from the pops of its base by
/// [`largest_remainder`] over their minor-unit amounts; any other instrument from pops by
/// population (#546). Spending is paid to pops by population and interest by dividend share
/// ([`split`], as the E4 payouts). The deficit leaves the account negative. A country without
/// pops has no base and no budget. A no-op when the fiscal system is off.
pub fn fiscal(w: &mut EconWorld) {
    use crate::fiscal::{CORPORATE, N_REVENUE, PAYROLL, PERSONAL_INCOME, VAT};
    let Some(government) = &mut w.fiscal else {
        return;
    };
    let (inp, lay) = (&*w.inputs, &*w.layout);
    let (ng, n_pops) = (inp.n_goods(), inp.n_pops());
    let mut province_bill = vec![0.0; inp.n_provinces()];
    for (e, &p) in inp.enterprises.province.iter().enumerate() {
        province_bill[p as usize] += w.wage[p as usize] * w.employment[e];
    }
    let effective: Vec<f64> = (0..n_pops)
        .map(|k| w.params.edu_eff[usize::from(inp.pops.edu[k])] * w.employed[k])
        .collect();
    let mut province_effective = vec![0.0; inp.n_provinces()];
    for (k, &x) in effective.iter().enumerate() {
        province_effective[inp.pops.province[k] as usize] += x;
    }
    let wage_income: Vec<i64> = (0..n_pops)
        .map(|k| {
            let i = inp.pops.province[k] as usize;
            if province_effective[i] > 0.0 {
                to_minor(province_bill[i] * effective[k] / province_effective[i])
            } else {
                0
            }
        })
        .collect();
    let spending: Vec<i64> = w
        .scratch
        .pop_spend
        .chunks(ng)
        .map(|x| to_minor(x.iter().sum::<f64>()))
        .collect();
    let base = |amounts: &[i64], pops: &[u32]| -> i128 {
        pops.iter().map(|&k| i128::from(amounts[k as usize])).sum()
    };
    let bases: Vec<[Option<i128>; N_REVENUE]> = lay
        .country_pops
        .iter()
        .map(|pops| {
            if pops.is_empty() {
                return [Some(0); N_REVENUE];
            }
            let mut b = [None; N_REVENUE];
            b[PERSONAL_INCOME] = Some(base(&wage_income, pops));
            b[PAYROLL] = b[PERSONAL_INCOME];
            b[VAT] = Some(base(&spending, pops));
            b
        })
        .collect();
    let budgets = government.step(&bases);

    let acc = &lay.accounts;
    let (mut weights, mut parts) = (Vec::new(), Vec::new());
    for (c, (b, pops)) in budgets.iter().zip(&lay.country_pops).enumerate() {
        let gov = acc.government[c];
        for (tax, amounts) in [
            (b.revenue[PERSONAL_INCOME], &wage_income),
            (b.revenue[PAYROLL], &wage_income),
            (b.revenue[VAT], &spending),
        ] {
            weights.clear();
            weights.extend(pops.iter().map(|&k| amounts[k as usize]));
            largest_remainder(tax, &weights, &mut parts);
            for (&k, &part) in pops.iter().zip(&parts) {
                pay(&mut w.ledger, TAX, acc.pop[k as usize], gov, part);
            }
        }
        let other: i128 = (0..N_REVENUE)
            .filter(|&r| ![PERSONAL_INCOME, PAYROLL, VAT, CORPORATE].contains(&r))
            .map(|r| b.revenue[r])
            .sum();
        let population = pops.iter().map(|&k| inp.pops.population[k as usize]);
        let wealth = pops.iter().map(|&k| {
            let k = k as usize;
            if inp.pops.dividend.is_empty() {
                inp.pops.population[k]
            } else {
                inp.pops.dividend[k]
            }
        });
        split(other, population.clone(), &mut parts);
        for (&k, &part) in pops.iter().zip(&parts) {
            pay(&mut w.ledger, TAX, acc.pop[k as usize], gov, part);
        }
        split(b.spending, population, &mut parts);
        for (&k, &part) in pops.iter().zip(&parts) {
            pay(&mut w.ledger, GOV_TRANSFER, gov, acc.pop[k as usize], part);
        }
        split(b.interest, wealth, &mut parts);
        for (&k, &part) in pops.iter().zip(&parts) {
            pay(&mut w.ledger, GOV_INTEREST, gov, acc.pop[k as usize], part);
        }
    }
}

/// System 233 (`specs/money/SPEC.md`, P5 C3; ADR 0278): the energy wedges held at their t0 rates.
/// For each country and wedge good the quantity is the units its province markets sold this
/// tick (`Markets::sales`, origin basis, #532); an excise is paid to the country's government
/// account and a subsidy paid from it, under TAX and GOV_TRANSFER (ADR 0360), by the buyers of
/// the good in the country (households and enterprises) by their planned units
/// ([`split`]). Households chose at the buyer price in
/// [`demand`]. A no-op when the energy policy is off.
pub fn energy_policy(w: &mut EconWorld) {
    let Some(policy) = &w.energy else {
        return;
    };
    let (inp, lay) = (&*w.inputs, &*w.layout);
    let (ng, acc) = (inp.n_goods(), &lay.accounts);
    let mut parts = Vec::new();
    for (c, wedges) in policy.countries.iter().enumerate() {
        let gov = acc.government[c];
        for (&g, &wedge) in policy.goods.iter().zip(&wedges.wedge) {
            if wedge == 0 {
                continue;
            }
            let units: f64 = (0..inp.n_provinces())
                .filter(|&i| inp.country_of(i) == c)
                .map(|i| w.markets.sales[i * ng + g])
                .sum();
            let amount = (wedge.abs() as f64 * units).round() as i128;
            let buyers: Vec<(AccountId, f64)> = (0..inp.n_provinces())
                .filter(|&i| inp.country_of(i) == c)
                .flat_map(|i| {
                    let pops = lay
                        .pops(i)
                        .map(|k| (acc.pop[k], w.scratch.pop_demand[k * ng + g]));
                    let ents = lay
                        .enterprises(i)
                        .map(|e| (acc.enterprise[e], w.scratch.ent_order[e * ng + g]));
                    pops.chain(ents)
                })
                .map(|(a, x)| (a, x.max(0.0)))
                .collect();
            if buyers.iter().all(|b| b.1 == 0.0) {
                continue;
            }
            split(amount, buyers.iter().map(|b| b.1), &mut parts);
            for (&(account, _), &part) in buyers.iter().zip(&parts) {
                if wedge > 0 {
                    pay(&mut w.ledger, TAX, account, gov, part);
                } else {
                    pay(&mut w.ledger, GOV_TRANSFER, gov, account, part);
                }
            }
        }
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
#[allow(clippy::disallowed_methods)]
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

    /// ADR 0103: TFP grows at g_A 2^(-t / h) and F closes half its gap f by one half-life, so
    /// at t = h the log multipliers are g_A h / (2 ln 2) and f / 2, and at t = 2h three quarters
    /// of the totals g_A h / ln 2 and f.
    #[test]
    fn drift_fades_with_half_life() {
        use crate::fixture::small_world;

        let mut inp = small_world();
        let n = inp.n_countries();
        let g: Vec<f64> = (0..n).map(|k| 0.01 * (k as f64 + 1.0)).collect();
        let f: Vec<f64> = (0..n).map(|k| 0.2 - 0.15 * k as f64).collect();
        (inp.countries.drift_a, inp.countries.drift_f) = (g.clone(), f.clone());
        let params = EconParams::default();
        let h = params.drift_half_life;
        for (years, done) in [(h, 0.5), (2.0 * h, 0.75)] {
            let factors = drift_factors(&inp, &params, (12.0 * years) as u32);
            for k in 0..n {
                let [a, l0] = factors[k];
                let ln_phi = f[k] * done;
                let ln_alpha = g[k] * h / std::f64::consts::LN_2 * done;
                assert!((l0.ln() - ln_phi).abs() < 1e-14, "country {k}: {l0}");
                assert!(
                    (a.ln() - ln_alpha - ln_phi).abs() < 1e-14,
                    "country {k}: {a}"
                );
            }
        }
        // the rate halves every half-life: the log multiplier's monthly step at t = h is half
        // its step at t = 0
        let step = |t: u32| {
            let (now, next) = (
                drift_factors(&inp, &params, t),
                drift_factors(&inp, &params, t + 1),
            );
            next[0][0].ln() - next[0][1].ln() - (now[0][0].ln() - now[0][1].ln())
        };
        let ratio = step((12.0 * h) as u32) / step(0);
        assert!((ratio - 0.5).abs() < 1e-2, "{ratio}");
    }

    /// ADR 0103: the drift is zero at the data state's first tick and with S23 off.
    #[test]
    fn drift_factors_are_one_at_tick_0_and_when_off() {
        use crate::fixture::small_world;

        let mut inp = small_world();
        let n = inp.n_countries();
        (inp.countries.drift_a, inp.countries.drift_f) = (vec![0.03; n], vec![0.1; n]);
        let off = EconParams {
            drift: false,
            ..EconParams::default()
        };
        assert_eq!(
            drift_factors(&inp, &EconParams::default(), 0),
            vec![[1.0; 2]; n]
        );
        assert_eq!(drift_factors(&inp, &off, 120), vec![[1.0; 2]; n]);
        assert_ne!(
            drift_factors(&inp, &EconParams::default(), 1),
            vec![[1.0; 2]; n]
        );
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
