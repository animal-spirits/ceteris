//! The P2 world: static inputs, the built kernel and both parameter sets outside snapshots,
//! and the dynamic state that snapshots carry (`specs/econ/SPEC.md`, State components;
//! `specs/trade/SPEC.md`, State components).

use std::ops::Range;
use std::sync::Arc;

use rayon::prelude::*;

use ceteris_core::invariant::Violation;
use ceteris_core::ledger::{AccountId, Entry, Flow, GOODS_TOLERANCE, Ledger, PostScratch};
use ceteris_core::reduce::fixed_sum;
use ceteris_core::sim::{Sim, State};
use ceteris_core::snapshot::{Decode, DecodeError, Decoder, Encode, Encoder, Hash, state_hash};
use ceteris_trade::goods::TRADED;
use ceteris_trade::source::{HOUSEHOLD, INTERMEDIATE, dispatch, source};
use ceteris_trade::{Kernel, Markets, N_GOODS, Network, Pipeline, Sourced, TradeParams};

use crate::calibration::Steady;
use crate::demand::{PiglWeights, invert_pigl, lognormal_sigma, pigl};
use crate::flows::FlowWindow;
use crate::inputs::EconInputs;
use crate::params::{EconParams, Pigl, Technology};

/// Minor units per numeraire unit: 1 minor unit = 1e-4 numeraire.
pub const MINOR_PER_UNIT: f64 = 1e4;

/// Relative slack of the labour bounds (spec Invariants, Labour).
const LABOUR_TOLERANCE: f64 = 1e-12;
/// Relative tolerance of input use against coef_k Y (spec Invariants, Leontief).
const LEONTIEF_TOLERANCE: f64 = 1e-12;
/// Relative tolerance of dispatched units against origin sales (trade SPEC Invariants,
/// Dispatch mass).
const DISPATCH_TOLERANCE: f64 = 1e-12;

pub const WAGE: Flow = Flow(1);
pub const SALE: Flow = Flow(2);
pub const PURCHASE: Flow = Flow(3);
pub const TRADE: Flow = Flow(4);
pub const TARIFF: Flow = Flow(5);
pub const DIVIDEND: Flow = Flow(6);
pub const REBATE: Flow = Flow(7);
pub const MINT: Flow = Flow(8);
pub const WRITE_OFF: Flow = Flow(9);

/// A numeraire amount as ledger minor units, rounded half to even.
pub fn to_minor(value: f64) -> i64 {
    let minor = (value * MINOR_PER_UNIT).round_ties_even();
    assert!(
        minor.abs() < i64::MAX as f64,
        "payment {value} does not fit the ledger"
    );
    minor as i64
}

pub fn to_numeraire(minor: i64) -> f64 {
    minor as f64 / MINOR_PER_UNIT
}

/// Ledger accounts, opened at load in field order and ID order within each field, so the
/// layout follows from the inputs alone.
#[derive(Clone, Debug, PartialEq)]
pub struct Accounts {
    /// Per pop: money balance (spent at E3, refilled by wages, dividends and rebates).
    pub pop: Vec<AccountId>,
    /// Per enterprise: input stocks (goods) and signed cash flow (money).
    pub enterprise: Vec<AccountId>,
    /// Per province: every good's market inventory and the market's cash flow.
    pub market: Vec<AccountId>,
    /// Per gateway: international payments pass through and net to zero every tick.
    pub gateway: Vec<AccountId>,
    /// Goods in flight, one account per traded good, indexed by good (traded goods are the
    /// first goods), mirroring the pipeline.
    pub transit: Vec<AccountId>,
    /// Per country: tariff revenue for pops by population.
    pub rebate: Vec<AccountId>,
    /// Per country: the profit pool of E4, the cash flow of its enterprises and province
    /// markets, paid to its pops by population (national ownership); empty between ticks.
    pub dividend: Vec<AccountId>,
    /// Issues the initial money; holds minus all money ever minted.
    pub mint: AccountId,
}

/// Static structure derived at load from the inputs and parameters: never in snapshots,
/// shared between a world and its restored copies.
#[derive(Clone, Debug)]
pub struct Layout {
    pub accounts: Accounts,
    /// CSR offsets of pops by province.
    pub pop_offsets: Vec<u32>,
    /// CSR offsets of enterprises by province.
    pub ent_offsets: Vec<u32>,
    /// Effective labour supply L^s per province.
    pub labour_supply: Vec<f64>,
    /// Pop's share N_k / N_c of its country's profit pool and rebate (national ownership,
    /// owner 2026-10-25): its population over its country's pops' population.
    pub ownership: Vec<f64>,
    /// Input coefficients each enterprise runs on (S09): its own putty-clay coefficients
    /// `EntData::coef`, or its 2000 column `EntData::column` under fixed Leontief.
    pub coef: Vec<[f64; N_GOODS]>,
    /// Value-added coefficient a_VA per enterprise, `EntData::a_va`.
    pub a_va: Vec<f64>,
    /// Pops of each country in ID order, for the tariff rebate.
    pub country_pops: Vec<Vec<u32>>,
    /// Weights of the world price index, [`price_index_weights`].
    pub index_weights: Vec<f64>,
    /// PIGL weights per country: the calibrated ones (`Countries::pigl`), else inverted at
    /// load from its final demand shares at prices 1 and its base spending per person
    /// ([`base_spend`]).
    pub pigl: Vec<PiglWeights>,
    /// Log income dispersion sigma of every pop of a country, from the country's Gini.
    pub sigma_income: Vec<f64>,
    /// Retained loss each enterprise keeps before the write-off (ADR 0112), minor units: M
    /// months of its base sales VA0 / a_VA (the calibrated steady-state sales value).
    pub ent_loss_cap: Vec<i64>,
}

impl Layout {
    pub fn pops(&self, i: usize) -> Range<usize> {
        self.pop_offsets[i] as usize..self.pop_offsets[i + 1] as usize
    }

    pub fn enterprises(&self, i: usize) -> Range<usize> {
        self.ent_offsets[i] as usize..self.ent_offsets[i + 1] as usize
    }

    /// Pop `k`'s demand at buyer prices `price` from its money balance `money`: PIGL at its
    /// mean spending per person, money / population, over its country's lognormal income
    /// (ADR 0021). Nothing without money.
    pub fn pop_demand(
        &self,
        inp: &EconInputs,
        params: &EconParams,
        k: usize,
        price: &[f64; N_GOODS],
        money: f64,
        out: &mut [f64; N_GOODS],
    ) {
        *out = [0.0; N_GOODS];
        if money <= 0.0 {
            return;
        }
        let c = inp.country_of(inp.pops.province[k] as usize);
        let people = inp.pops.population[k];
        pigl(
            &self.pigl[c],
            price,
            money / people,
            self.sigma_income[c],
            params,
            out,
        );
        for x in out.iter_mut() {
            *x *= people;
        }
    }
}

/// Per-tick evidence and buffers of the systems. Written within a tick before it is read,
/// so it is not part of snapshots; a restored world starts with it zeroed.
#[derive(Clone, Debug)]
pub struct Scratch {
    /// Output Y of each enterprise at E2.
    pub produced: Vec<f64>,
    /// Inputs available before use (stock, plus for a local good the units drawn from this
    /// tick's output) and the quantity used at E2, `[enterprise * N_GOODS + k]`.
    pub input_before: Vec<f64>,
    pub input_used: Vec<f64>,
    /// Units of each local good an enterprise draws from this tick's output in the same-tick
    /// chain (`crate::local`), `[enterprise * N_GOODS + k]`; the rest of its use comes from
    /// its stock.
    pub from_output: Vec<f64>,
    /// Units of local output drawn by intermediate users at E2 per province market,
    /// `[province * N_GOODS + g]` (0 for traded goods): part of the market's demand and sales.
    pub drawn: Vec<f64>,
    /// Iterations of the same-tick chain per province at E2.
    pub chain_iterations: Vec<u32>,
    /// Units dispatched at T2b by each origin market, home purchases included,
    /// `[province * N_GOODS + g]`: the filled orders, equal to the market's sales.
    pub dispatched: Vec<f64>,
    /// Notional demand per pop and input orders per enterprise in units, by good.
    pub pop_demand: Vec<f64>,
    pub ent_order: Vec<f64>,
    /// Spending on traded goods at the delivered price index, per pop and per enterprise,
    /// by good (0 for local goods).
    pub pop_spend: Vec<f64>,
    pub ent_spend: Vec<f64>,
    /// Spending by class `[intermediate, household]` per destination cell, the sums of
    /// `ent_spend` and `pop_spend` over the province in ID order, given to `source` at T2.
    pub class_spend: Vec<[f64; 2]>,
    /// Wages, dividends and rebates paid to each pop this tick, minor units.
    pub income_paid: Vec<i64>,
    /// Fill ratios `[intermediate, household]` per origin cell written by `Markets::clear`.
    pub fill: Vec<[f64; 2]>,
    /// Local output perished per cell at T3.
    pub spoiled: Vec<f64>,
    /// Pipeline arrivals at T1, `[buyer * N_GOODS + g]` (pops, then enterprises).
    pub due: Vec<f64>,
    /// Sourcing of each traded good: at unit spending from the price refresh of E3, then
    /// from T2 to its dispatch at T2b.
    pub sourced: Vec<Sourced>,
    /// Per traded good, its columns for `sourced`.
    pub columns: Vec<Columns>,
    /// Per traded good, the money legs of its dispatch at T2b.
    pub payments: Vec<Payments>,
    /// Working space of `Ledger::post` at T2b.
    pub post: PostScratch,
    /// Expected months in transit of each traded good to each destination, the arrival
    /// slots weighted by the units arriving per unit of spending (home 0, domestic 1, a
    /// crossing 1 + lag); 0 for local goods. `[province * N_GOODS + g]`, refreshed with
    /// [`EconWorld::price_index`].
    pub lead: Vec<f64>,
    /// Country-pair flows of the last 12 ticks, for the run export.
    pub flows: FlowWindow,
}

impl Scratch {
    fn new(nc: usize, np: usize, n_pops: usize, n_ent: usize) -> Self {
        let n_traded = TRADED.iter().filter(|&&t| t).count();
        Self {
            produced: vec![0.0; n_ent],
            input_before: vec![0.0; n_ent * N_GOODS],
            input_used: vec![0.0; n_ent * N_GOODS],
            from_output: vec![0.0; n_ent * N_GOODS],
            drawn: vec![0.0; np * N_GOODS],
            chain_iterations: vec![0; np],
            dispatched: vec![0.0; np * N_GOODS],
            pop_demand: vec![0.0; n_pops * N_GOODS],
            ent_order: vec![0.0; n_ent * N_GOODS],
            pop_spend: vec![0.0; n_pops * N_GOODS],
            ent_spend: vec![0.0; n_ent * N_GOODS],
            class_spend: vec![[0.0; 2]; np * N_GOODS],
            income_paid: vec![0; n_pops],
            fill: vec![[1.0; 2]; np * N_GOODS],
            spoiled: vec![0.0; np * N_GOODS],
            due: vec![0.0; (n_pops + n_ent) * N_GOODS],
            sourced: (0..n_traded).map(|_| Sourced::new()).collect(),
            columns: vec![Columns::default(); n_traded],
            payments: vec![Payments::default(); n_traded],
            post: PostScratch::default(),
            lead: vec![0.0; np * N_GOODS],
            flows: FlowWindow::new(nc, np),
        }
    }
}

/// One traded good's columns over provinces, gathered from the `[province * N_GOODS + g]`
/// arrays so that each good's sourcing and dispatch run on their own: posted prices, active
/// origins, spending by class, fill ratios, and the buyer price and lead of the price refresh.
#[derive(Clone, Debug, Default)]
pub struct Columns {
    pub price: Vec<f64>,
    pub active: Vec<bool>,
    pub spend: Vec<[f64; 2]>,
    pub fill: Vec<[f64; 2]>,
    pub index: Vec<f64>,
    pub lead: Vec<f64>,
}

impl Columns {
    /// Posted prices and active origins of good `g`.
    pub fn gather(&mut self, price: &[f64], active: &[bool], g: usize) {
        let cells = (0..price.len() / N_GOODS).map(|o| o * N_GOODS + g);
        self.price.clear();
        self.price.extend(cells.clone().map(|c| price[c]));
        self.active.clear();
        self.active.extend(cells.map(|c| active[c]));
    }
}

/// One traded good's dispatch to its buyers and its money legs at T2b, computed in the
/// good's task and applied afterwards (`systems::buyer_orders`, `systems::trade_legs`): each
/// buyer's order in province and buyer order, with the end of each province's orders, and the
/// ledger entries, the buyers' payments then the legs. `buyers`, `at_gateway`,
/// `from_gateway` and `parts` are working space.
#[derive(Clone, Debug, Default)]
pub struct Payments {
    pub orders: Vec<Order>,
    pub order_ends: Vec<u32>,
    pub entries: Vec<Entry>,
    pub buyers: Vec<Buyer>,
    pub at_gateway: Vec<i64>,
    pub from_gateway: Vec<i64>,
    pub parts: Vec<i64>,
}

/// One buyer of a traded good at its destination: ledger account, pipeline index, class
/// and spending.
#[derive(Clone, Debug)]
pub struct Buyer {
    pub account: AccountId,
    pub index: usize,
    pub class: usize,
    pub spend: f64,
}

/// One buyer's share of a good's dispatch: units it may take home now (at most what the
/// market holds), units due in each pipeline slot 1 to 4, and whether it consumes them.
#[derive(Clone, Debug)]
pub struct Order {
    pub account: AccountId,
    pub buyer: usize,
    pub household: bool,
    pub home: f64,
    pub slots: [f64; 4],
}

/// The P2 world. Static parts (inputs, kernel, parameters, layout) are read-only; the
/// public fields are the dynamic state that snapshots carry.
#[derive(Clone, Debug)]
pub struct EconWorld {
    pub(crate) inputs: Arc<EconInputs>,
    pub(crate) kernel: Arc<Kernel>,
    pub(crate) params: EconParams,
    pub(crate) trade: TradeParams,
    pub(crate) layout: Arc<Layout>,
    digest: Hash,
    pub ledger: Ledger,
    /// Prices (origin markets of traded goods, province markets of local goods), merchant
    /// inventories, last D, Q and sales.
    pub markets: Markets,
    /// Goods paid for and in flight per buyer: pops `0..n_pops`, then enterprises.
    pub pipeline: Pipeline,
    /// Buyer prices `[province * N_GOODS + g]`: the delivered unit price q_d = 1 / alpha_d of
    /// each traded good, the numeraire a buyer at d needs per arriving unit under the current
    /// sourcing shares (tariffs and iceberg losses included; infinite where no active origin
    /// is reachable), and the posted price of each local good (owner decision, 2026-10-24).
    /// Refreshed at E3 from the prices of the moment, read by E1 of the next tick.
    pub price_index: Vec<f64>,
    /// Wage w per province, numeraire per effective labour unit.
    pub wage: Vec<f64>,
    /// Ticks at which the wage step clip c_max (P03c) bound, per province, since the start of
    /// the run; a Walrasian wage (frictionless) has no step and never counts.
    pub wage_clip_ticks: Vec<u32>,
    /// Expected inflation pi^e of the price and wage rules (ADR 0019), log points per month:
    /// the adaptive expectation of the world price index, updated after T4.
    pub expected_inflation: f64,
    /// World price index I at the last update of `expected_inflation`.
    pub price_level: f64,
    /// Employment L per enterprise, effective labour units.
    pub employment: Vec<f64>,
    /// Employed persons per pop.
    pub employed: Vec<f64>,
    /// Income last tick per pop, numeraire.
    pub income: Vec<f64>,
    /// Output, sales and profit last tick per enterprise.
    pub output: Vec<f64>,
    pub sales: Vec<f64>,
    pub profit: Vec<f64>,
    /// Not state: see [`Scratch`].
    pub scratch: Scratch,
}

/// BLAKE3 over the inputs digest and both parameter sets except the write-off threshold P148,
/// which the snapshot header carries after the digest: everything else a snapshot leaves out.
pub fn static_digest(inputs: &EconInputs, params: &EconParams, trade: &TradeParams) -> Hash {
    // destructured so a new parameter cannot be left out of the digest
    let EconParams {
        sigma_demand,
        pigl,
        sigma_fl,
        kappa_w,
        delta_w,
        eta,
        edu_eff,
        frictionless,
        rationing,
        technology,
        // fires only on retained losses, which the steady states that calibration and the
        // reference solutions describe do not have; the snapshot header carries it
        write_off_months: _,
    } = params;
    let TradeParams {
        theta,
        coverage,
        kappa1,
        kappa2,
        c_max,
        expectation_gain,
        border_nu,
        time_cost,
        // reaches the kernel only through Network.route_taus, in the inputs digest
        theta_r: _,
        // groups goods for kernel sharing; the dynamics read theta alone
        theta_class: _,
    } = trade;
    let mut e = Encoder::default();
    inputs.digest().iter().for_each(|&b| e.u8(b));
    e.f64s(&[*sigma_demand, *sigma_fl, *kappa_w, *delta_w, *eta]);
    let Pigl {
        sigma_b,
        phi,
        gamma,
        theta: pigl_theta,
        eps,
    } = pigl;
    e.f64s(&[*sigma_b, *phi, *gamma, *eps]);
    e.f64s(pigl_theta);
    e.f64s(edu_eff);
    e.u8(u8::from(*frictionless));
    e.u8(*rationing as u8);
    // with one vintage both settings read the same coefficients and run identically
    if !inputs.one_vintage() {
        e.u8(*technology as u8);
    }
    e.f64s(theta);
    e.f64s(&[*kappa1, *kappa2, *c_max, *expectation_gain, *border_nu]);
    e.f64s(coverage);
    e.f64s(time_cost);
    state_hash(&e.into_bytes())
}

/// [`static_digest`] of `inputs` under the structural subset of the parameters, the rest at
/// their defaults: the parameters a steady state (calibrated or solved) depends on, `sigma_demand`,
/// the PIGL parameters, `sigma_fl`, `edu_eff`, `theta`, `coverage`, `border_nu` and `time_cost`.
/// The adjustment speeds and the frictionless and rationing switches are free.
pub fn structural_digest(inputs: &EconInputs, params: &EconParams, trade: &TradeParams) -> Hash {
    static_digest(
        inputs,
        &EconParams {
            sigma_demand: params.sigma_demand,
            pigl: params.pigl.clone(),
            sigma_fl: params.sigma_fl,
            edu_eff: params.edu_eff,
            ..EconParams::default()
        },
        &TradeParams {
            theta: trade.theta,
            coverage: trade.coverage,
            border_nu: trade.border_nu,
            time_cost: trade.time_cost,
            ..TradeParams::default()
        },
    )
}

/// Which (province, good) cells have a market, `[province * N_GOODS + g]`. A traded good's
/// market is the origin market of the province's output: active where an enterprise of the
/// province produces the good with positive capacity (A VA0 > 0). A local good's market is
/// the province market: active where anyone in the province buys it (final demand in the
/// province's country, or an enterprise of the province using it as an input under either S09
/// setting). Fixed by the inputs for the whole run.
pub fn active_cells(inp: &EconInputs) -> Vec<bool> {
    let mut active = vec![false; inp.n_provinces() * N_GOODS];
    for i in 0..inp.n_provinces() {
        let omega = &inp.countries.omega[inp.country_of(i)];
        for g in (0..N_GOODS).filter(|&g| !TRADED[g]) {
            active[i * N_GOODS + g] = omega[g] > 0.0;
        }
    }
    let ent = &inp.enterprises;
    for e in 0..inp.n_enterprises() {
        let i = ent.province[e] as usize;
        let sector = usize::from(ent.good[e]);
        if TRADED[sector] && ent.a[e] * ent.va0[e] > 0.0 {
            active[i * N_GOODS + sector] = true;
        }
        for k in (0..N_GOODS).filter(|&k| !TRADED[k]) {
            if inp.coef(e, k) > 0.0 || inp.a(inp.country_of(i), k, sector) > 0.0 {
                active[i * N_GOODS + k] = true;
            }
        }
    }
    active
}

/// Population of each province's pops and of each country's pops.
fn pop_people(inp: &EconInputs) -> (Vec<f64>, Vec<f64>) {
    let (mut province, mut country) = (vec![0.0; inp.n_provinces()], vec![0.0; inp.n_countries()]);
    for k in 0..inp.n_pops() {
        let i = inp.pops.province[k] as usize;
        province[i] += inp.pops.population[k];
        country[inp.country_of(i)] += inp.pops.population[k];
    }
    (province, country)
}

/// The profit pool of each country at the base point, sum (1 - s_L) VA0 over its enterprises.
fn base_profit_pool(inp: &EconInputs) -> Vec<f64> {
    let mut pool = vec![0.0; inp.n_countries()];
    for e in 0..inp.n_enterprises() {
        let c = inp.country_of(inp.enterprises.province[e] as usize);
        pool[c] += (1.0 - inp.countries.s_l[c]) * inp.enterprises.va0[e];
    }
    pool
}

/// Household income per province at the base point, rebates aside: wages w0 (1 - u*) L^s
/// plus the province's population share N_i / N_c of its country's profit pool sum (1 -
/// s_L) VA0 (national ownership). As `ceteris.reference.io.Inputs.base_income`.
pub fn base_income(inp: &EconInputs, params: &EconParams) -> Vec<f64> {
    let supply = inp.labour_supply(&params.edu_eff);
    let pool = base_profit_pool(inp);
    let (people, country_people) = pop_people(inp);
    (0..inp.n_provinces())
        .map(|i| {
            let c = inp.country_of(i);
            let u = inp.countries.u_star[c];
            let share = if country_people[c] > 0.0 {
                people[i] / country_people[c]
            } else {
                0.0
            };
            inp.provinces.w0[i] * (1.0 - u) * supply[i] + share * pool[c]
        })
        .collect()
}

/// Base spending per person of each country: its provinces' [`base_income`] over its pops'
/// population, 0 for a country without people. As `ceteris.reference.io.Inputs.base_spend`.
pub fn base_spend(inp: &EconInputs, params: &EconParams) -> Vec<f64> {
    let nc = inp.n_countries();
    let (mut spend, mut people) = (vec![0.0; nc], vec![0.0; nc]);
    for (i, income) in base_income(inp, params).into_iter().enumerate() {
        spend[inp.country_of(i)] += income;
    }
    for k in 0..inp.n_pops() {
        people[inp.country_of(inp.pops.province[k] as usize)] += inp.pops.population[k];
    }
    // a country without people (an uninhabited territory keeps a gateway) spends nothing
    spend
        .iter()
        .zip(&people)
        .map(|(s, &n)| if n > 0.0 { s / n } else { 0.0 })
        .collect()
}

/// Per country, the calibrated PIGL weights (`Countries::pigl`) or else the weights inverted
/// from its final demand shares at prices 1 and its [`base_spend`], and the log income
/// dispersion of its pops from its Gini (ADR 0021). Panics where a country's shares cannot be
/// inverted (`invert_pigl`).
pub fn household_preferences(
    inp: &EconInputs,
    params: &EconParams,
) -> (Vec<PiglWeights>, Vec<f64>) {
    let sigma: Vec<f64> = inp
        .countries
        .gini
        .iter()
        .map(|&g| lognormal_sigma(g))
        .collect();
    let weights = match &inp.countries.pigl {
        Some(w) => w.clone(),
        None => base_spend(inp, params)
            .iter()
            .enumerate()
            .map(|(c, &m)| invert_pigl(&inp.countries.omega[c], m, sigma[c], params))
            .collect(),
    };
    (weights, sigma)
}

/// Base household consumption quantities at prices 1, the weights of the world price index
/// over buyer prices: each country's final demand shares times the province's
/// [`base_income`], `[province * N_GOODS + g]`. The same weights as
/// `ceteris.reference.io.Inputs.index_weights`, so V1 normalizes run and reference alike.
pub fn price_index_weights(inp: &EconInputs, params: &EconParams) -> Vec<f64> {
    let base = base_income(inp, params);
    let mut out = vec![0.0; inp.n_provinces() * N_GOODS];
    for i in 0..inp.n_provinces() {
        let c = inp.country_of(i);
        let income = base[i];
        let omega = &inp.countries.omega[c];
        let total: f64 = omega.iter().sum();
        for g in 0..N_GOODS {
            out[i * N_GOODS + g] = omega[g] / total * income;
        }
    }
    out
}

fn open_n(ledger: &mut Ledger, n: usize) -> Vec<AccountId> {
    (0..n).map(|_| ledger.open()).collect()
}

fn adjust_stock(ledger: &mut Ledger, account: AccountId, good: usize, change: f64) {
    if change >= 0.0 {
        ledger.create(account, good, change);
    } else {
        ledger.destroy(account, good, -change);
    }
}

fn offsets(rows: &[u32], n: usize) -> Vec<u32> {
    let mut off = vec![0u32; n + 1];
    for &r in rows {
        off[r as usize + 1] += 1;
    }
    for i in 0..n {
        off[i + 1] += off[i];
    }
    off
}

/// Buyer prices of every cell, `[province * N_GOODS + g]`, with origins posting `price` on
/// `active` markets: a traded good's delivered unit price q_d = 1 / alpha_d (alpha_d the units
/// arriving per unit of intermediate spending, `source` and `dispatch` of one unit at every
/// destination; infinite where no active origin is reachable) and the expected months in
/// transit (`lead`, arrival slots weighted by units); a local good's posted price, lead 0.
/// `sourced` holds one sourcing per traded good, left at the unit spending: T2 sources the
/// same good at the same prices on it again, and `source` then reuses the origin access.
/// Traded goods run in parallel, each on its own `sourced` and `columns` entry.
fn delivered_prices(
    kernel: &Kernel,
    price: &[f64],
    active: &[bool],
    sourced: &mut [Sourced],
    columns: &mut [Columns],
    index: &mut [f64],
    lead: &mut [f64],
) {
    let np = price.len() / N_GOODS;
    // `sourced[g]` and `columns[g]` belong to good g, so the traded goods must come first
    let n_traded = sourced.len();
    assert!(
        columns.len() == n_traded
            && TRADED[..n_traded].iter().all(|&t| t)
            && !TRADED[n_traded..].contains(&true),
        "traded goods must be goods 0 to {n_traded}"
    );
    let (spend, fill) = (vec![[1.0, 0.0]; np], vec![[1.0; 2]; np]);
    sourced
        .par_iter_mut()
        .zip(columns.par_iter_mut())
        .enumerate()
        .for_each(|(g, (unit, col))| {
            col.gather(price, active, g);
            source(kernel, g, &col.price, &col.active, &spend, unit);
            dispatch(kernel, g, &col.price, &fill, unit);
            col.index.clear();
            col.lead.clear();
            for d in 0..np {
                let slots = unit.arrive.iter().map(|a| a[d][INTERMEDIATE]);
                let alpha: f64 = slots.clone().sum();
                let weighted: f64 = slots.enumerate().map(|(s, x)| s as f64 * x).sum();
                let (q, l) = if alpha > 0.0 {
                    (1.0 / alpha, weighted / alpha)
                } else {
                    (f64::INFINITY, 0.0)
                };
                col.index.push(q);
                col.lead.push(l);
            }
        });
    for d in 0..np {
        for g in 0..N_GOODS {
            let cell = d * N_GOODS + g;
            (index[cell], lead[cell]) = match columns.get(g) {
                Some(col) => (col.index[d], col.lead[d]),
                None => (price[cell], 0.0),
            };
        }
    }
}

/// Base delivered prices per province: [`delivered_prices`] in the data state, every price 1,
/// the traded markets of [`active_cells`] (which do not depend on input coefficients), over
/// the kernel of `inp`'s network under `trade`. The buyer prices of tick 0.
pub fn base_delivered_prices(inp: &EconInputs, trade: &TradeParams) -> Vec<[f64; N_GOODS]> {
    let kernel = Kernel::build(&inp.network, &inp.weight, &inp.wedges, trade);
    let np = inp.n_provinces();
    let ent = &inp.enterprises;
    let mut active = vec![false; np * N_GOODS];
    for e in 0..inp.n_enterprises() {
        let g = usize::from(ent.good[e]);
        if TRADED[g] && ent.a[e] * ent.va0[e] > 0.0 {
            active[ent.province[e] as usize * N_GOODS + g] = true;
        }
    }
    let (mut index, mut lead) = (vec![0.0; np * N_GOODS], vec![0.0; np * N_GOODS]);
    let price = vec![1.0; np * N_GOODS];
    let n_traded = TRADED.iter().filter(|&&t| t).count();
    let mut sourced: Vec<Sourced> = (0..n_traded).map(|_| Sourced::new()).collect();
    let mut columns = vec![Columns::default(); n_traded];
    delivered_prices(
        &kernel,
        &price,
        &active,
        &mut sourced,
        &mut columns,
        &mut index,
        &mut lead,
    );
    index
        .chunks_exact(N_GOODS)
        .map(|row| row.try_into().expect("one row of goods"))
        .collect()
}

impl EconWorld {
    /// Builds the kernel, opens the accounts and seeds the initial state: the calibrated
    /// steady state of `EconInputs::steady` (ADR 0089, [`EconWorld::start_steady`]) when the
    /// inputs carry one, else the 2000 data state: prices 1, base wages w0 and employment L0
    /// from the inputs, unemployment at the natural rate u*, input stocks at one month of base
    /// use (base output Y0 = A VA0 / a_VA), origin inventories at coverage times initial demand
    /// (see [`EconWorld::stock_base_demand`]), nothing in flight, and one month of base
    /// dividends (the pop's population share of its country's profit pool, `(1 - s_L) VA0`
    /// per enterprise) minted to each pop: tick 0 wages are paid at E2, so tick 0 spending is
    /// one month of base income. Panics on invalid inputs or a base point other than base
    /// employment = (1 - u*) L^s under `params.edu_eff`, and on a calibrated start under structural
    /// parameters other than the defaults ([`structural_digest`]).
    pub fn new(inputs: Arc<EconInputs>, params: EconParams, trade: TradeParams) -> EconWorld {
        inputs.validate();
        let inp = &*inputs;
        let (np, nc) = (inp.n_provinces(), inp.n_countries());
        let (n_pops, n_ent) = (inp.n_pops(), inp.n_enterprises());
        let kernel = Arc::new(Kernel::build(
            &inp.network,
            &inp.weight,
            &inp.wedges,
            &trade,
        ));
        let digest = static_digest(inp, &params, &trade);

        let n_traded = TRADED.iter().filter(|&&t| t).count();
        assert!(
            TRADED[..n_traded].iter().all(|&t| t),
            "traded goods must come first"
        );
        let mut ledger = Ledger::new(N_GOODS);
        let accounts = Accounts {
            pop: open_n(&mut ledger, n_pops),
            enterprise: open_n(&mut ledger, n_ent),
            market: open_n(&mut ledger, np),
            gateway: open_n(&mut ledger, inp.network.n_gateways()),
            transit: open_n(&mut ledger, n_traded),
            rebate: open_n(&mut ledger, nc),
            dividend: open_n(&mut ledger, nc),
            mint: ledger.open(),
        };

        let edu_eff = |k: usize| params.edu_eff[usize::from(inp.pops.edu[k])];
        let pop_offsets = offsets(&inp.pops.province, np);
        let ent_offsets = offsets(&inp.enterprises.province, np);
        let labour_supply = inp.labour_supply(&params.edu_eff);
        let mut country_pops = vec![Vec::new(); nc];
        for k in 0..n_pops {
            country_pops[inp.country_of(inp.pops.province[k] as usize)].push(k as u32);
        }
        let employed: Vec<f64> = (0..n_pops)
            .map(|k| {
                let i = inp.pops.province[k] as usize;
                (1.0 - inp.countries.u_star[inp.country_of(i)]) * inp.pops.labour_force[k]
            })
            .collect();
        let (pigl, sigma_income) = household_preferences(inp, &params);
        let coef: Vec<[f64; N_GOODS]> = (0..n_ent)
            .map(|e| match params.technology {
                Technology::PuttyClay => inp.enterprises.coef[e],
                Technology::FixedLeontief => inp.base_column(e),
            })
            .collect();
        let (_, country_people) = pop_people(inp);
        let ownership = (0..n_pops)
            .map(|k| {
                let c = inp.country_of(inp.pops.province[k] as usize);
                inp.pops.population[k] / country_people[c]
            })
            .collect();
        // an infinite M turns the write-off off
        let cap = |v: f64| {
            let months = params.write_off_months;
            if months.is_finite() {
                to_minor(months * v)
            } else {
                i64::MAX
            }
        };
        let layout = Layout {
            ent_loss_cap: (0..n_ent)
                .map(|e| cap(inp.enterprises.va0[e] / inp.enterprises.a_va[e]))
                .collect(),
            index_weights: price_index_weights(inp, &params),
            pigl,
            sigma_income,
            accounts,
            pop_offsets,
            ent_offsets,
            labour_supply,
            ownership,
            country_pops,
            a_va: inp.enterprises.a_va.clone(),
            coef,
        };
        for i in 0..np {
            let l0: f64 = layout.enterprises(i).map(|e| inp.enterprises.l0[e]).sum();
            let u = inp.countries.u_star[inp.country_of(i)];
            let base = (1.0 - u) * layout.labour_supply[i];
            assert!(
                (l0 / base - 1.0).abs() <= LABOUR_TOLERANCE,
                "province {i}: base employment {l0} is not (1 - u*) L^s = {base}; \
                 fill the base point with these education efficiencies"
            );
            let effective: f64 = layout.pops(i).map(|k| edu_eff(k) * employed[k]).sum();
            assert!(effective > 0.0, "province {i} has no employed labour");
        }

        let mut markets = Markets::new(np);
        markets.active = active_cells(inp);

        let mut w = EconWorld {
            wage: inp.provinces.w0.clone(),
            wage_clip_ticks: vec![0; np],
            expected_inflation: 0.0,
            price_level: 0.0,
            employment: inp.enterprises.l0.clone(),
            employed,
            income: vec![0.0; n_pops],
            output: vec![0.0; n_ent],
            sales: vec![0.0; n_ent],
            profit: vec![0.0; n_ent],
            scratch: Scratch::new(nc, np, n_pops, n_ent),
            ledger,
            markets,
            pipeline: Pipeline::new(n_pops + n_ent),
            price_index: vec![0.0; np * N_GOODS],
            inputs,
            kernel,
            params,
            trade,
            layout: Arc::new(layout),
            digest,
        };
        match w.inputs.steady.clone() {
            Some(steady) => {
                assert_eq!(
                    structural_digest(&w.inputs, &w.params, &w.trade),
                    structural_digest(&w.inputs, &EconParams::default(), &TradeParams::default()),
                    "the calibrated steady state was built under the default structural \
                     parameters; start from the data state (`--data-state`) or recalibrate"
                );
                w.start_steady(&steady)
            }
            None => w.start_data_state(),
        }
        w
    }

    /// The 2000 data state of [`EconWorld::new`] on a world fresh from its accounts.
    fn start_data_state(&mut self) {
        let (inp, lay) = (Arc::clone(&self.inputs), Arc::clone(&self.layout));
        let (inp, lay) = (&*inp, &*lay);
        let accounts = &lay.accounts;
        let edu_eff = |k: usize| self.params.edu_eff[usize::from(inp.pops.edu[k])];
        // base income, minted: the national profit pool now, wages at E2
        let pool = base_profit_pool(inp);
        let mut base_income = vec![0.0; inp.n_pops()];
        for (k, income) in base_income.iter_mut().enumerate() {
            let i = inp.pops.province[k] as usize;
            let dividends = lay.ownership[k] * pool[inp.country_of(i)];
            self.ledger
                .transfer(MINT, accounts.mint, accounts.pop[k], to_minor(dividends));
            *income = inp.provinces.w0[i] * edu_eff(k) * self.employed[k] + dividends;
        }
        // input stocks at one month of base use (Y0 = A VA0 / a_VA)
        let n_ent = inp.n_enterprises();
        let mut base_use = vec![0.0; n_ent * N_GOODS];
        for e in 0..n_ent {
            let y0 = inp.enterprises.a[e] * inp.enterprises.va0[e] / lay.a_va[e];
            for k in 0..N_GOODS {
                let q = lay.coef[e][k] * y0;
                if q > 0.0 {
                    self.ledger.create(accounts.enterprise[e], k, q);
                    base_use[e * N_GOODS + k] = q;
                }
            }
        }
        self.refresh_price_index();
        self.price_level = self.world_price_index();
        self.stock_base_demand(&base_income, &base_use);
    }

    /// The calibrated steady state at the start of a tick (ADR 0089, stage 5), on a world
    /// fresh from its accounts: posted prices, the last D, Q and sales, merchant and input
    /// stocks, the goods in flight, each pop's money and last income, each enterprise's last
    /// output, sales and profit; wages w0, employment L0 and unemployment u* as in the data
    /// state; buyer prices refreshed at the posted prices, pi^e = 0.
    fn start_steady(&mut self, s: &Steady) {
        let (inp, lay) = (Arc::clone(&self.inputs), Arc::clone(&self.layout));
        let (inp, lay) = (&*inp, &*lay);
        let acc = &lay.accounts;
        let m = &mut self.markets;
        m.price.copy_from_slice(&s.price);
        m.supply.copy_from_slice(&s.supply);
        m.demand.copy_from_slice(&s.demand);
        m.demand_int.copy_from_slice(&s.demand_int);
        m.demand_hh.copy_from_slice(&s.demand_hh);
        m.sales.copy_from_slice(&s.sales);
        for (cell, &stock) in s.inventory.iter().enumerate() {
            let held = if m.active[cell] && TRADED[cell % N_GOODS] {
                stock
            } else {
                0.0
            };
            m.inventory[cell] = held;
            if held > 0.0 {
                self.ledger
                    .create(acc.market[cell / N_GOODS], cell % N_GOODS, held);
            }
        }
        for e in 0..inp.n_enterprises() {
            for (k, &stock) in s.stock[e].iter().enumerate() {
                if stock > 0.0 {
                    self.ledger.create(acc.enterprise[e], k, stock);
                }
            }
        }
        let mut in_flight = [0.0; N_GOODS];
        for &(buyer, g, ahead, units) in &s.pipeline {
            let g = usize::from(g);
            self.pipeline
                .push(usize::from(ahead), buyer as usize, g, units);
            in_flight[g] += units;
        }
        for (g, &units) in in_flight.iter().enumerate() {
            if units > 0.0 {
                self.ledger.create(acc.transit[g], g, units);
            }
        }
        for k in 0..inp.n_pops() {
            self.ledger
                .transfer(MINT, acc.mint, acc.pop[k], to_minor(s.money[k]));
        }
        self.income.copy_from_slice(&s.income);
        self.output.copy_from_slice(&s.output);
        self.sales.copy_from_slice(&s.ent_sales);
        self.profit.copy_from_slice(&s.profit);
        self.refresh_price_index();
        self.price_level = self.world_price_index();
    }

    /// Moves a world at the start of a tick off its state, for V1 from a perturbed steady
    /// state (ADR 0089, Tests): every active posted price, every wage, and every positive
    /// input stock and merchant stock times its own factor 1 + size (2u - 1), u uniform from
    /// one Philox stream of `seed`, drawn in that order (cells, provinces, enterprise by
    /// good, cells). Stocks are created or destroyed in the ledger; buyer prices and the price
    /// level follow the new prices, pi^e is kept. Returns the factors in drawing order.
    pub fn perturb(&mut self, seed: u64, size: f64) -> Vec<f64> {
        use ceteris_core::entity::{EntityId, Kind};
        use ceteris_core::rng::Seed;
        use ceteris_core::schedule::SystemId;
        let mut stream = Seed(seed).stream(0, SystemId(0), Kind(0), EntityId(0));
        let mut factors = Vec::new();
        let mut draw = || {
            let f = 1.0 + size * (2.0 * stream.uniform() - 1.0);
            factors.push(f);
            f
        };
        let lay = Arc::clone(&self.layout);
        let acc = &lay.accounts;
        for c in 0..self.markets.price.len() {
            if self.markets.active[c] {
                self.markets.price[c] *= draw();
            }
        }
        for w in &mut self.wage {
            *w *= draw();
        }
        for &account in &acc.enterprise {
            for k in 0..N_GOODS {
                let held = self.ledger.goods(account, k);
                if held > 0.0 {
                    adjust_stock(&mut self.ledger, account, k, held * (draw() - 1.0));
                }
            }
        }
        for c in 0..self.markets.inventory.len() {
            let held = self.markets.inventory[c];
            if held > 0.0 {
                let change = held * (draw() - 1.0);
                self.markets.inventory[c] += change;
                adjust_stock(
                    &mut self.ledger,
                    acc.market[c / N_GOODS],
                    c % N_GOODS,
                    change,
                );
            }
        }
        self.refresh_price_index();
        self.price_level = self.world_price_index();
        factors
    }

    /// The world price index I over buyer prices ([`EconWorld::price_index`]) with the base
    /// consumption weights of [`price_index_weights`]. Cells whose buyer price is infinite (no
    /// reachable origin) are left out.
    pub fn world_price_index(&self) -> f64 {
        let (mut num, mut den) = (0.0, 0.0);
        for (&a, &p) in self.layout.index_weights.iter().zip(&self.price_index) {
            if p.is_finite() {
                num += a * p;
                den += a;
            }
        }
        num / den
    }

    /// The adaptive expectation of ADR 0019, after T4: pi^e <- pi^e + g (ln(I_t / I_{t-1}) -
    /// pi^e), I_t the world price index at this tick's buyer prices (refreshed at E3, so the
    /// inflation observed is that of the last repricing).
    pub fn update_expectation(&mut self) {
        let level = self.world_price_index();
        let inflation = libm::log(level / self.price_level);
        self.expected_inflation +=
            self.trade.expectation_gain * (inflation - self.expected_inflation);
        self.price_level = level;
    }

    /// Origin inventories of the data state at coverage times initial demand: the orders
    /// that one month of base demand places at each origin market at the data state's
    /// prices, households spending their base income with PIGL at the price index and
    /// enterprises buying one month of base use at the index.
    fn stock_base_demand(&mut self, base_income: &[f64], base_use: &[f64]) {
        let (inp, lay) = (Arc::clone(&self.inputs), Arc::clone(&self.layout));
        let (inp, lay) = (&*inp, &*lay);
        let np = inp.n_provinces();
        let mut spend = vec![[0.0; 2]; np * N_GOODS];
        let mut x = [0.0; N_GOODS];
        for d in 0..np {
            let row: [f64; N_GOODS] = self.price_index[d * N_GOODS..(d + 1) * N_GOODS]
                .try_into()
                .expect("one row of goods");
            for k in lay.pops(d) {
                lay.pop_demand(inp, &self.params, k, &row, base_income[k], &mut x);
                for g in (0..N_GOODS).filter(|&g| TRADED[g] && x[g] > 0.0) {
                    spend[d * N_GOODS + g][HOUSEHOLD] += x[g] * row[g];
                }
            }
            for e in lay.enterprises(d) {
                for g in (0..N_GOODS).filter(|&g| TRADED[g] && row[g].is_finite()) {
                    spend[d * N_GOODS + g][INTERMEDIATE] += base_use[e * N_GOODS + g] * row[g];
                }
            }
        }
        let (mut price, mut active) = (vec![0.0; np], vec![false; np]);
        let mut sourced = Sourced::new();
        for g in (0..N_GOODS).filter(|&g| TRADED[g]) {
            for o in 0..np {
                price[o] = self.markets.price[o * N_GOODS + g];
                active[o] = self.markets.active[o * N_GOODS + g];
            }
            let s: Vec<[f64; 2]> = (0..np).map(|d| spend[d * N_GOODS + g]).collect();
            source(&self.kernel, g, &price, &active, &s, &mut sourced);
            for o in (0..np).filter(|&o| active[o]) {
                let stock = self.trade.coverage[g] * (sourced.orders[o][0] + sourced.orders[o][1]);
                if stock > 0.0 {
                    self.markets.inventory[o * N_GOODS + g] = stock;
                    self.ledger.create(lay.accounts.market[o], g, stock);
                }
            }
        }
    }

    /// Recomputes [`EconWorld::price_index`] and `Scratch::lead` at the current prices. For a
    /// traded good, every destination spends one unit, sourced over the floored origin
    /// weights (`ceteris_trade::source`) and dispatched in full: alpha_d is the units
    /// arriving at d, q_d = 1 / alpha_d, and the lead is the mean arrival slot of those units.
    pub fn refresh_price_index(&mut self) {
        let sc = &mut self.scratch;
        delivered_prices(
            &self.kernel,
            &self.markets.price,
            &self.markets.active,
            &mut sc.sourced,
            &mut sc.columns,
            &mut self.price_index,
            &mut sc.lead,
        );
    }

    /// Sets every crossing's transit lag to `lag` months (`Kernel::set_crossing_lags`), on
    /// a world with nothing in flight. Lags only time deliveries, so steady states stay;
    /// for nesting tests and stability studies. The lags are not in the static digest:
    /// snapshots restore only through this world, which shares its kernel.
    pub fn set_crossing_lags(&mut self, lag: u8) {
        let empty = (0..N_GOODS).all(|g| self.pipeline.in_flight(g) == 0.0);
        assert!(
            empty,
            "crossing lags change only on a world with nothing in flight"
        );
        Arc::make_mut(&mut self.kernel).set_crossing_lags(lag);
    }

    /// Replaces the trade network (system T0 of the trade SPEC: the kernel follows the graph
    /// version), for example after a chokepoint closes: the inputs `Arc` gets a copy of the
    /// inputs with `network`, the kernel is rebuilt from it and the static digest follows.
    /// Called between ticks; the buyer prices of the last E3 stand until the next E3 sources
    /// over the new kernel, and goods already in flight arrive as scheduled, so the same
    /// network leaves the run unchanged. The network must keep the provinces and gateways,
    /// whose accounts are open. In P2 the caller triggers it; shocks arrive in P3.
    pub fn set_network(&mut self, network: Network) {
        let old = &self.inputs.network;
        assert!(
            network.province_country == old.province_country
                && network.gateway_country == old.gateway_country,
            "a new network must keep the provinces and gateways"
        );
        let mut inputs = (*self.inputs).clone();
        inputs.network = network;
        self.kernel = Arc::new(Kernel::build(
            &inputs.network,
            &inputs.weight,
            &inputs.wedges,
            &self.trade,
        ));
        self.digest = static_digest(&inputs, &self.params, &self.trade);
        self.inputs = Arc::new(inputs);
    }

    /// A run of this world from the data state on the P2 schedule.
    pub fn sim(inputs: Arc<EconInputs>, params: EconParams, trade: TradeParams) -> Sim<EconWorld> {
        Sim::new(
            EconWorld::new(inputs, params, trade),
            crate::systems::schedule(),
            crate::systems::SEED,
        )
    }

    pub fn inputs(&self) -> &EconInputs {
        &self.inputs
    }

    pub fn kernel(&self) -> &Kernel {
        &self.kernel
    }

    pub fn params(&self) -> &EconParams {
        &self.params
    }

    pub fn trade(&self) -> &TradeParams {
        &self.trade
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// [`static_digest`] of this world, the first 32 bytes of its encoding.
    pub fn digest(&self) -> Hash {
        self.digest
    }
}

impl Encode for EconWorld {
    /// The static digest, the write-off threshold P148, then the dynamic state.
    fn encode(&self, e: &mut Encoder) {
        self.digest.iter().for_each(|&b| e.u8(b));
        e.f64s(&[self.params.write_off_months]);
        self.ledger.encode(e);
        self.markets.encode(e);
        self.pipeline.encode(e);
        for column in [
            &self.wage,
            &self.employment,
            &self.employed,
            &self.income,
            &self.output,
            &self.sales,
            &self.profit,
            &self.price_index,
        ] {
            e.f64s(column);
        }
        e.f64s(&[self.expected_inflation, self.price_level]);
        e.u32s(&self.wage_clip_ticks);
    }
}

fn expect_len(what: &str, got: usize, want: usize) -> Result<(), DecodeError> {
    if got == want {
        Ok(())
    } else {
        Err(DecodeError(format!(
            "{what}: {got} entries, world has {want}"
        )))
    }
}

impl State for EconWorld {
    fn begin_tick(&mut self) {
        self.ledger.begin_tick();
        self.scratch.income_paid.fill(0);
        self.scratch.flows.advance();
    }

    /// Invariants that hold on any world, in order: ledger (money, goods sign, goods
    /// identity), population, labour bounds, price bounds (every price and wage), inventory
    /// (S >= 0, and held by the market account to 1e-9 of the turnover S + Q + D), transit
    /// (pipeline and transit accounts agree), gateway clearing (money exact, goods 1e-9), and over this tick's evidence in
    /// [`Scratch`]: Leontief (inputs used = coef_k Y, 1e-12 relative to the larger of
    /// coef_k Y and what was available: the stock, and this tick's draws of a local good),
    /// dispatch mass (units dispatched = origin sales, 1e-12)
    /// and income (wages, dividends and rebates credited to each pop in the journal equal
    /// its recorded income, exact in minor units).
    fn end_tick(&self, tick: u32) -> Result<(), Violation> {
        self.ledger.check(tick)?;
        let (inp, lay) = (&*self.inputs, &*self.layout);
        let fail = |invariant, detail| Err(Violation::new(tick, invariant, detail));

        // pops and enterprises are fixed tables; their populations live in the immutable
        // inputs, so only the dynamic columns can drift
        let pops = [self.employed.len(), self.income.len()];
        let ents = [
            self.employment.len(),
            self.output.len(),
            self.sales.len(),
            self.profit.len(),
        ];
        if pops.iter().any(|&n| n != inp.n_pops()) || ents.iter().any(|&n| n != inp.n_enterprises())
        {
            return fail(
                "population",
                format!("pop columns {pops:?}, enterprise columns {ents:?}"),
            );
        }

        for k in 0..inp.n_pops() {
            let (e, lf) = (self.employed[k], inp.pops.labour_force[k]);
            if !(e >= 0.0 && e <= lf * (1.0 + LABOUR_TOLERANCE)) {
                return fail(
                    "labour",
                    format!("pop {k} employs {e} of labour force {lf}"),
                );
            }
        }
        for i in 0..inp.n_provinces() {
            let l = &self.employment[lay.enterprises(i)];
            if let Some(x) = l.iter().find(|x| !(x.is_finite() && **x >= 0.0)) {
                return fail("labour", format!("province {i}: enterprise employment {x}"));
            }
            let (total, supply) = (fixed_sum(l), lay.labour_supply[i]);
            if total > supply * (1.0 + LABOUR_TOLERANCE) {
                return fail(
                    "labour",
                    format!("province {i}: employment {total} of supply {supply}"),
                );
            }
        }

        let bad = |x: &f64| !(x.is_finite() && *x > 0.0);
        if let Some(cell) = self.markets.price.iter().position(bad) {
            let detail = format!("cell {cell} price {}", self.markets.price[cell]);
            return fail("price bounds", detail);
        }
        if let Some(i) = self.wage.iter().position(bad) {
            return fail(
                "price bounds",
                format!("province {i} wage {}", self.wage[i]),
            );
        }

        for (i, &account) in lay.accounts.market.iter().enumerate() {
            for g in 0..N_GOODS {
                let cell = i * N_GOODS + g;
                let s = self.markets.inventory[cell];
                let held = self.ledger.goods(account, g);
                // both sides are sums of the order of the turnover, so their rounding is too
                let turnover = s + self.markets.supply[cell] + self.markets.demand[cell];
                if s.is_nan()
                    || s < 0.0
                    || (held - s).abs() > GOODS_TOLERANCE * turnover.max(held).max(1.0)
                {
                    let detail = format!("province {i} good {g}: stock {s}, market holds {held}");
                    return fail("inventory", detail);
                }
            }
        }
        let in_flight = self.pipeline.in_flight_all();
        for (g, &account) in lay.accounts.transit.iter().enumerate() {
            let (flight, held) = (in_flight[g], self.ledger.goods(account, g));
            if (held - flight).abs() > GOODS_TOLERANCE * flight.max(held).max(1.0) {
                return fail(
                    "transit",
                    format!("good {g}: pipeline {flight}, account {held}"),
                );
            }
        }
        if let Some(a) = lay
            .accounts
            .gateway
            .iter()
            .find(|&&a| self.ledger.money(a) != 0)
        {
            let detail = format!("gateway account {a:?} holds {}", self.ledger.money(*a));
            return fail("gateway clearing", detail);
        }
        for &a in &lay.accounts.gateway {
            if let Some(g) = (0..N_GOODS).find(|&g| self.ledger.goods(a, g).abs() > GOODS_TOLERANCE)
            {
                let detail = format!(
                    "gateway account {a:?} holds {} of good {g}",
                    self.ledger.goods(a, g)
                );
                return fail("gateway clearing", detail);
            }
        }

        let sc = &self.scratch;
        for e in 0..inp.n_enterprises() {
            for k in 0..N_GOODS {
                let want = lay.coef[e][k] * sc.produced[e];
                let (used, before) = (
                    sc.input_used[e * N_GOODS + k],
                    sc.input_before[e * N_GOODS + k],
                );
                let gap = (used - want).abs();
                if gap.is_nan() || gap > LEONTIEF_TOLERANCE * want.max(before) {
                    let detail = format!("enterprise {e} used {used} of good {k}, a Y = {want}");
                    return fail("leontief", detail);
                }
            }
        }

        for (cell, (&sent, &sold)) in sc.dispatched.iter().zip(&self.markets.sales).enumerate() {
            if !TRADED[cell % N_GOODS] {
                continue;
            }
            // the absolute floor only matters for subnormal sales, which have no relative precision
            let gap = (sent - sold).abs();
            if gap.is_nan() || gap > DISPATCH_TOLERANCE * sold + f64::MIN_POSITIVE {
                let detail = format!("cell {cell}: dispatched {sent} of sales {sold}");
                return fail("dispatch mass", detail);
            }
        }

        // pops hold the first account IDs
        let pop_of = |a: AccountId| (a.0 as usize) < inp.n_pops();
        // integer sums, so the chunks may add up in any order
        let credited = self
            .ledger
            .journal()
            .par_chunks(1 << 16)
            .fold(
                || vec![0i64; inp.n_pops()],
                |mut credited, entries| {
                    for entry in entries {
                        if [WAGE, DIVIDEND, REBATE].contains(&entry.flow) {
                            if pop_of(entry.to) {
                                credited[entry.to.0 as usize] += entry.amount;
                            }
                            if pop_of(entry.from) {
                                credited[entry.from.0 as usize] -= entry.amount;
                            }
                        }
                    }
                    credited
                },
            )
            .reduce(
                || vec![0i64; inp.n_pops()],
                |mut a, b| {
                    a.iter_mut().zip(&b).for_each(|(x, y)| *x += y);
                    a
                },
            );
        if let Some(k) = (0..inp.n_pops()).find(|&k| credited[k] != sc.income_paid[k]) {
            let detail = format!(
                "pop {k}: journal credits {}, income paid {}",
                credited[k], sc.income_paid[k]
            );
            return fail("income", detail);
        }
        Ok(())
    }

    /// Restores a snapshot of a world built from the same inputs and parameters. The
    /// snapshot opens with the static digest (inputs and both parameter sets, none of which
    /// snapshots carry); a digest other than this live world's is a `DecodeError`. On a
    /// match the restored world shares this world's inputs `Arc`, kernel, parameters and
    /// layout, and only the dynamic state is decoded.
    fn restore(&self, d: &mut Decoder) -> Result<Self, DecodeError> {
        let mut digest = [0u8; 32];
        for b in &mut digest {
            *b = d.u8()?;
        }
        if digest != self.digest {
            return Err(DecodeError(
                "snapshot belongs to other inputs or parameters".into(),
            ));
        }
        let months = d.f64s()?;
        if months != [self.params.write_off_months] {
            return Err(DecodeError(format!(
                "snapshot taken with write-off threshold (P148) {months:?}"
            )));
        }
        let ledger = Ledger::decode(d)?;
        let markets = Markets::decode(d)?;
        let pipeline = Pipeline::decode(d)?;
        let mut columns = Vec::with_capacity(8);
        for _ in 0..8 {
            columns.push(d.f64s()?);
        }
        let [
            wage,
            employment,
            employed,
            income,
            output,
            sales,
            profit,
            price_index,
        ]: [Vec<f64>; 8] = columns.try_into().expect("eight columns");
        let expectation = d.f64s()?;
        let wage_clip_ticks = d.u32s()?;

        let inp = &*self.inputs;
        let (nc, np) = (inp.n_countries(), inp.n_provinces());
        let (n_pops, n_ent) = (inp.n_pops(), inp.n_enterprises());
        expect_len(
            "ledger accounts",
            ledger.n_accounts(),
            self.ledger.n_accounts(),
        )?;
        expect_len("market provinces", markets.n_provinces(), np)?;
        expect_len("pipeline buyers", pipeline.n_buyers(), n_pops + n_ent)?;
        expect_len("wages", wage.len(), np)?;
        expect_len("wage clip ticks", wage_clip_ticks.len(), np)?;
        expect_len("expectation", expectation.len(), 2)?;
        expect_len("price index", price_index.len(), np * N_GOODS)?;
        for (what, column) in [("employed", &employed), ("income", &income)] {
            expect_len(what, column.len(), n_pops)?;
        }
        for (what, column) in [
            ("employment", &employment),
            ("output", &output),
            ("sales", &sales),
            ("profit", &profit),
        ] {
            expect_len(what, column.len(), n_ent)?;
        }

        Ok(EconWorld {
            inputs: Arc::clone(&self.inputs),
            kernel: Arc::clone(&self.kernel),
            params: self.params.clone(),
            trade: self.trade.clone(),
            layout: Arc::clone(&self.layout),
            digest,
            ledger,
            markets,
            pipeline,
            price_index,
            wage,
            wage_clip_ticks,
            expected_inflation: expectation[0],
            price_level: expectation[1],
            employment,
            employed,
            income,
            output,
            sales,
            profit,
            scratch: Scratch::new(nc, np, n_pops, n_ent),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::small_world;
    use ceteris_core::snapshot::{canonical, decode_canonical_with};
    use ceteris_trade::N_GOODS;
    use ceteris_trade::goods::TRADED;

    /// Owner decision, 2026-10-25: ICIO coefficients are cost shares at base delivered prices.
    /// The base prices are the world's buyer prices at tick 0, and at them every input's cost
    /// share is the ICIO share, so a_VA = 1 - sum_k a_kg.
    #[test]
    fn coefficients_are_cost_shares_at_base_prices() {
        let inp = small_world();
        let w = EconWorld::new(
            Arc::new(inp.clone()),
            EconParams::default(),
            TradeParams::default(),
        );
        assert_eq!(w.price_index, inp.provinces.base_price.as_flattened());
        assert!(
            inp.provinces
                .base_price
                .iter()
                .any(|q| q.iter().any(|&x| x > 1.0))
        );
        for e in 0..inp.n_enterprises() {
            let i = inp.enterprises.province[e] as usize;
            let (a, coef) = (inp.country_column(e), inp.enterprises.coef[e]);
            for k in 0..N_GOODS {
                let share = coef[k] * inp.provinces.base_price[i][k];
                assert!(
                    (share - a[k]).abs() <= 1e-15 * a[k],
                    "enterprise {e} input {k}"
                );
            }
            let c = inp.country_of(i);
            let g = usize::from(inp.enterprises.good[e]);
            assert!((w.layout().a_va[e] - inp.a_va(c, g)).abs() <= 1e-15);
        }
    }

    #[test]
    fn country_without_people_spends_nothing() {
        let mut inp = small_world();
        let params = EconParams::default();
        let base = base_spend(&inp, &params);
        let c = &mut inp.countries;
        c.s_l.push(c.s_l[0]);
        c.omega.push(c.omega[0]);
        c.io.push(c.io[0]);
        c.unemployment.push(c.unemployment[0]);
        c.gini.push(c.gini[0]);
        let with_empty = base_spend(&inp, &params);
        assert_eq!(with_empty[..base.len()], base[..]);
        assert_eq!(with_empty[base.len()], 0.0);
    }

    fn world() -> EconWorld {
        EconWorld::new(
            Arc::new(small_world()),
            EconParams::default(),
            TradeParams::default(),
        )
    }

    fn money_sum(w: &EconWorld) -> i128 {
        let n = w.layout.accounts.mint.0 as usize + 1;
        (0..n as u32)
            .map(|a| i128::from(w.ledger.money(AccountId(a))))
            .sum()
    }

    #[test]
    fn fixture_world_opens_balanced_ledger() {
        let mut w = world();
        assert_eq!(money_sum(&w), 0);
        w.ledger.check(0).unwrap();
        let n = w.layout.accounts.mint.0 + 1;
        for a in 0..n {
            for g in 0..N_GOODS {
                assert!(w.ledger.goods(AccountId(a), g) >= 0.0);
            }
        }
        w.end_tick(0).unwrap();
        w.begin_tick();
        w.end_tick(0).unwrap();
    }

    #[test]
    fn world_accounts_follow_the_layout() {
        let w = world();
        let acc = &w.layout.accounts;
        let inp = small_world();
        assert_eq!(acc.pop.len(), inp.n_pops());
        assert_eq!(acc.enterprise.len(), inp.n_enterprises());
        assert_eq!(acc.market.len(), 25);
        assert_eq!(acc.gateway.len(), 7);
        assert_eq!(acc.transit.len(), TRADED.iter().filter(|&&t| t).count());
        assert_eq!(acc.rebate.len(), 4);
        assert_eq!(acc.dividend.len(), 4);
        let mut all: Vec<u32> = acc
            .pop
            .iter()
            .chain(&acc.enterprise)
            .chain(&acc.market)
            .chain(&acc.gateway)
            .chain(&acc.transit)
            .chain(&acc.rebate)
            .chain(&acc.dividend)
            .map(|a| a.0)
            .collect();
        all.push(acc.mint.0);
        assert_eq!(all, (0..all.len() as u32).collect::<Vec<_>>());
    }

    #[test]
    fn world_starts_in_the_data_state() {
        let w = world();
        let inp = small_world();
        assert!(w.markets.price.iter().all(|&p| p == 1.0));
        assert_eq!(w.wage, inp.provinces.w0);
        assert_eq!(w.employment, inp.enterprises.l0);
        for k in 0..inp.n_pops() {
            let u = inp.countries.unemployment[inp.country_of(inp.pops.province[k] as usize)];
            assert_eq!(w.employed[k], (1.0 - u) * inp.pops.labour_force[k]);
        }
        // input stocks: one month of base use, coef_k A VA0 / a_VA
        for e in 0..inp.n_enterprises() {
            let y0 = inp.enterprises.a[e] * inp.enterprises.va0[e] / w.layout.a_va[e];
            for k in 0..N_GOODS {
                let held = w.ledger.goods(w.layout.accounts.enterprise[e], k);
                assert!((held - inp.coef(e, k) * y0).abs() <= 1e-12 * y0);
            }
        }
        // origin inventories at coverage times initial demand; local goods and cells
        // without a market hold none
        for i in 0..25 {
            for (g, &traded) in TRADED.iter().enumerate() {
                let cell = i * N_GOODS + g;
                let held = w.ledger.goods(w.layout.accounts.market[i], g);
                assert_eq!(held, w.markets.inventory[cell]);
                assert_eq!(held > 0.0, traded && w.markets.active[cell], "cell {cell}");
            }
        }
        // nothing in flight; the price index starts at the data state's prices
        assert!((0..N_GOODS).all(|g| w.pipeline.in_flight(g) == 0.0));
        for i in 0..25 {
            for (g, &traded) in TRADED.iter().enumerate() {
                let p = w.price_index[i * N_GOODS + g];
                if traded {
                    // q_d = 1 / alpha_d at prices 1: a mean of delivered unit costs, each at
                    // least the price 1
                    assert!(p.is_finite() && p >= 1.0, "{i} {g}: {p}");
                } else {
                    assert_eq!(p, 1.0);
                }
            }
        }
    }

    /// National ownership: each pop gets its population share of its country's profit pool.
    #[test]
    fn world_mints_one_month_of_base_dividends() {
        let w = world();
        let inp = small_world();
        let lay = &w.layout;
        for (c, pops) in lay.country_pops.iter().enumerate() {
            let s_l = inp.countries.s_l[c];
            let pool: f64 = (0..inp.n_enterprises())
                .filter(|&e| inp.country_of(inp.enterprises.province[e] as usize) == c)
                .map(|e| (1.0 - s_l) * inp.enterprises.va0[e])
                .sum();
            let people: f64 = pops.iter().map(|&k| inp.pops.population[k as usize]).sum();
            for &k in pops {
                let k = k as usize;
                let minted = to_numeraire(w.ledger.money(lay.accounts.pop[k]));
                // the profit pool only: tick 0 wages are paid at E2
                let share = inp.pops.population[k] / people;
                assert!((minted / (share * pool) - 1.0).abs() < 1e-9, "pop {k}");
                assert!((lay.ownership[k] / share - 1.0).abs() < 1e-15);
            }
        }
        assert_eq!(
            w.ledger.money(w.layout.accounts.mint),
            -w.layout
                .accounts
                .pop
                .iter()
                .map(|&a| w.ledger.money(a))
                .sum::<i64>()
        );
    }

    #[test]
    fn world_opens_origin_markets_where_enterprises_produce() {
        let w = world();
        let inp = small_world();
        let mut without_producer = 0;
        for i in 0..25 {
            let goods: Vec<usize> = w
                .layout
                .enterprises(i)
                .map(|e| usize::from(inp.enterprises.good[e]))
                .collect();
            for g in (0..N_GOODS).filter(|&g| TRADED[g]) {
                let active = w.markets.active[i * N_GOODS + g];
                assert_eq!(active, goods.contains(&g), "province {i} good {g}");
                without_producer += usize::from(!active);
            }
            for g in [7, 8, 9] {
                assert!(w.markets.active[i * N_GOODS + g]);
            }
        }
        assert!(without_producer > 0);
    }

    #[test]
    fn world_rounds_payments_half_to_even() {
        let half = 2.5 / MINOR_PER_UNIT;
        assert_eq!(half * MINOR_PER_UNIT, 2.5);
        assert_eq!(to_minor(half), 2);
        assert_eq!(to_minor(3.5 / MINOR_PER_UNIT), 4);
        assert_eq!(to_minor(-2.5 / MINOR_PER_UNIT), -2);
        assert_eq!(to_minor(1.2345), 12345);
        assert_eq!(to_numeraire(12345), 1.2345);
    }

    fn round_trip(w: &EconWorld) -> (Vec<u8>, EconWorld) {
        let bytes = canonical(12, w);
        let (tick, back) = decode_canonical_with(&bytes, |d| w.restore(d)).unwrap();
        assert_eq!(tick, 12);
        (bytes, back)
    }

    #[test]
    fn encode_restore_round_trip() {
        let mut w = world();
        // move every kind of dynamic state off the data state
        w.wage[3] = 1.7;
        w.employment[5] *= 0.9;
        w.employed[2] *= 0.95;
        w.income[1] = 12.5;
        w.output[4] = 3.0;
        w.sales[4] = 2.0;
        w.profit[4] = -1.0;
        w.markets.price[17] = 0.8;
        w.markets.demand[17] = 4.0;
        w.pipeline.push(3, 7, 2, 0.5);
        w.price_index[4] = 0.75;
        w.expected_inflation = 0.004;
        w.price_level = 1.3;
        w.wage_clip_ticks[2] = 5;
        w.markets.clip_ticks[17] = 3;
        let (bytes, back) = round_trip(&w);
        assert_eq!(canonical(12, &back), bytes);
        assert!(Arc::ptr_eq(&back.inputs, &w.inputs));
        assert!(Arc::ptr_eq(&back.kernel, &w.kernel));
        assert_eq!(back.wage, w.wage);
        assert_eq!(back.pipeline, w.pipeline);
        assert_eq!(back.price_index, w.price_index);
        assert_eq!(back.expected_inflation, w.expected_inflation);
        assert_eq!(back.price_level, w.price_level);
        assert_eq!(back.wage_clip_ticks, w.wage_clip_ticks);
        assert_eq!(back.markets.clip_ticks, w.markets.clip_ticks);
    }

    #[test]
    fn restore_rejects_another_write_off_threshold() {
        let w = world();
        let bytes = canonical(0, &w);
        let other = EconWorld::new(
            Arc::new(small_world()),
            EconParams {
                write_off_months: 6.0,
                ..EconParams::default()
            },
            TradeParams::default(),
        );
        assert_eq!(other.digest(), w.digest());
        assert!(decode_canonical_with(&bytes, |d| other.restore(d)).is_err());
    }

    #[test]
    fn restore_rejects_other_inputs() {
        let w = world();
        let bytes = canonical(0, &w);

        let mut inputs = small_world();
        inputs.weight[0] *= 1.01;
        let other = EconWorld::new(
            Arc::new(inputs),
            EconParams::default(),
            TradeParams::default(),
        );
        assert!(decode_canonical_with(&bytes, |d| other.restore(d)).is_err());

        let other = EconWorld::new(
            Arc::new(small_world()),
            EconParams::default(),
            TradeParams {
                theta: [6.0; N_GOODS],
                ..TradeParams::default()
            },
        );
        assert!(decode_canonical_with(&bytes, |d| other.restore(d)).is_err());

        let other = EconWorld::new(
            Arc::new(small_world()),
            EconParams::frictionless(),
            TradeParams::default(),
        );
        assert!(decode_canonical_with(&bytes, |d| other.restore(d)).is_err());
    }

    #[test]
    fn digest_changes_with_inputs() {
        let base = small_world();
        let d = base.digest();
        let mut changed = small_world();
        changed.pops.labour_force[3] += 1.0;
        assert_ne!(changed.digest(), d);
        let mut changed = small_world();
        changed.network.crossings[0].hours += 1.0;
        assert_ne!(changed.digest(), d);
        let mut changed = small_world();
        changed.wedges.tariff[2] = 0.5;
        assert_ne!(changed.digest(), d);

        let (p, t) = (EconParams::default(), TradeParams::default());
        let s = static_digest(&base, &p, &t);
        assert_ne!(static_digest(&changed, &p, &t), s);
        let p2 = EconParams {
            eta: 0.31,
            ..EconParams::default()
        };
        assert_ne!(static_digest(&base, &p2, &t), s);
        let mut t2 = TradeParams::default();
        t2.coverage[2] = 2.5;
        assert_ne!(static_digest(&base, &p, &t2), s);
    }

    #[test]
    fn set_network_rebuilds_the_kernel_and_the_digest() {
        let mut w = world();
        let (digest, island) = (w.digest(), 24 * N_GOODS + 6);
        let before = w.price_index[island];
        let mut net = w.inputs().network.clone();
        w.set_network(net.clone());
        assert_eq!(w.digest(), digest);
        for l in &mut net.crossings {
            l.cost_per_t *= 4.0;
        }
        w.set_network(net.clone());
        assert_ne!(w.digest(), digest);
        assert_eq!(w.inputs().network.crossings, net.crossings);
        let tau = w.kernel().good(6).crossings.row(6)[0].tau;
        let base = Kernel::build(&net, &w.inputs().weight, &w.inputs().wedges, w.trade());
        assert_eq!(tau, base.good(6).crossings.row(6)[0].tau);
        // the buyer prices of the last E3 stand; at the next refresh the island's machinery,
        // all imported, costs more
        assert_eq!(w.price_index[island], before);
        w.refresh_price_index();
        assert!(w.price_index[island] > before);
        let snapshot = canonical(0, &w);
        let back = decode_canonical_with(&snapshot, |d| w.restore(d))
            .unwrap()
            .1;
        assert_eq!(back.digest(), w.digest());
    }

    #[test]
    #[should_panic(expected = "keep the provinces and gateways")]
    fn set_network_keeps_provinces_and_gateways() {
        let mut w = world();
        let mut net = w.inputs().network.clone();
        net.gateway_country.push(0);
        w.set_network(net);
    }

    #[test]
    fn world_end_tick_catches_broken_state() {
        let fresh = world();
        let mut w = fresh.clone();
        w.markets.price[11] = f64::NAN;
        assert_eq!(w.end_tick(0).unwrap_err().invariant, "price bounds");
        let mut w = fresh.clone();
        w.wage[0] = 0.0;
        assert_eq!(w.end_tick(0).unwrap_err().invariant, "price bounds");
        let mut w = fresh.clone();
        w.employed[0] = 1e12;
        assert_eq!(w.end_tick(0).unwrap_err().invariant, "labour");
        let mut w = fresh.clone();
        w.employment[0] = 1e12;
        assert_eq!(w.end_tick(0).unwrap_err().invariant, "labour");
        let mut w = fresh.clone();
        w.markets.inventory[0] += 1.0;
        assert_eq!(w.end_tick(0).unwrap_err().invariant, "inventory");
        let mut w = fresh.clone();
        w.pipeline.push(1, 0, 0, 1.0);
        assert_eq!(w.end_tick(0).unwrap_err().invariant, "transit");
        let mut w = fresh;
        let (g, m) = (w.layout.accounts.gateway[0], w.layout.accounts.mint);
        w.ledger.transfer(TRADE, m, g, 1);
        assert_eq!(w.end_tick(0).unwrap_err().invariant, "gateway clearing");
    }
}
