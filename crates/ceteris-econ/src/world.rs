//! The P2 world: static inputs, the built kernel and both parameter sets outside snapshots,
//! and the dynamic state that snapshots carry (`specs/econ/SPEC.md`, State components;
//! `specs/trade/SPEC.md`, State components).

use std::ops::Range;
use std::sync::Arc;

use rayon::prelude::*;

use ceteris_core::invariant::Violation;
use ceteris_core::ledger::{
    AccountId, Entry, Flow, GOODS_TOLERANCE, GoodColumn, Ledger, PostScratch,
};
use ceteris_core::reduce::fixed_sum;
use ceteris_core::rows::Rows;
use ceteris_core::sim::{Sim, State};
use ceteris_core::snapshot::{Decode, DecodeError, Decoder, Encode, Encoder, Hash, state_hash};
use ceteris_core::timing;
use ceteris_trade::source::{INTERMEDIATE, arrivals, dispatch, source};
use ceteris_trade::{Goods, Kernel, Markets, Network, Pipeline, Rationing, Sourced, TradeParams};

use crate::calibration::Steady;
use crate::demand::{PiglWeights, PopAt, invert_pigl_pops, pigl, within_sigma};
use crate::flows::FlowWindow;
use crate::inputs::{EconInputs, network_digest};
use crate::params::{EconParams, Pigl, Technology};

/// Minor units per numeraire unit: 1 minor unit = 1e-4 numeraire.
pub const MINOR_PER_UNIT: f64 = 1e4;

/// Relative slack of the labour bounds (spec Invariants, Labour).
const LABOUR_TOLERANCE: f64 = 1e-12;
/// Relative tolerance of input use against coef_k Y (spec Invariants, Leontief).
const LEONTIEF_TOLERANCE: f64 = 1e-12;
/// What a user of a local good may miss of coef_k Y beyond [`LEONTIEF_TOLERANCE`], relative to
/// the larger of its coef_k Y and what was available (spec Invariants, Leontief): where the
/// chain's passes stop short, a good is short by at most [`crate::local::FIT`] of its flows,
/// U - R <= FIT (U + R), so R / U >= (1 - FIT) / (1 + FIT), and a user drawing pro rata misses
/// (n - b)(1 - R / U) <= 2 FIT / (1 + FIT) (n - b) < 2 FIT of its need n.
const CHAIN_STOP_TOLERANCE: f64 = 2.0 * crate::local::FIT;
/// What a user of a local good may miss beyond that, relative to the units of the good drawn at
/// its province market this tick, D (spec Invariants, Leontief): the users draw from the market
/// account one after the other, each draw rounding its balance by at most eps / 2 of what the
/// market held, M: with the split's sums up to 2 (n + 1) roundings for n goods, (n + 1) eps M
/// in all, which the user at which the market runs dry absorbs. The draws bind only where the market
/// ends empty, so M is D plus those roundings, M <= D / (1 - (n + 1) eps) < 2 D.
fn market_drain_tolerance(n_goods: usize) -> f64 {
    2.0 * (n_goods as f64 + 1.0) * f64::EPSILON
}
/// Relative tolerance of dispatched units against origin sales (trade SPEC Invariants,
/// Dispatch mass).
const DISPATCH_TOLERANCE: f64 = 1e-12;

/// How far an origin cell's dispatched units `sent` may lie from its sales `sold` (trade SPEC,
/// Invariants, Dispatch mass): [`DISPATCH_TOLERANCE`] of the larger, the `rounding` of products
/// below the normal range that the kernel carries for the cell, and 2^-1022 for results below
/// that range.
fn dispatch_tolerance(sent: f64, sold: f64, rounding: f64) -> f64 {
    DISPATCH_TOLERANCE * sent.abs().max(sold.abs()) + rounding + f64::MIN_POSITIVE
}

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
    to_numeraire_wide(i128::from(minor))
}

/// [`to_numeraire`] of a wide balance or of a sum of balances.
pub fn to_numeraire_wide(minor: i128) -> f64 {
    minor as f64 / MINOR_PER_UNIT
}

/// Ledger accounts, opened at load in field order and ID order within each field, so the
/// layout follows from the inputs alone. Pops, enterprises and transit accounts are narrow
/// (`i64`); markets, gateways, the pools and the mint, which aggregate them, are wide (`i128`,
/// ADR 0106).
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
    pub coef: Rows<f64>,
    /// Value-added coefficient a_VA per enterprise, `EntData::a_va`.
    pub a_va: Vec<f64>,
    /// Pops of each country in ID order, for the tariff rebate.
    pub country_pops: Vec<Vec<u32>>,
    /// Weights of the world price index, [`price_index_weights`].
    pub index_weights: Vec<f64>,
    /// PIGL weights per country: the calibrated ones (`Countries::pigl`), else inverted at
    /// load over its pops at prices 1 ([`household_preferences`]).
    pub pigl: Vec<PiglWeights>,
    /// Within-pop log income dispersion of every pop of a country, which its weights carry
    /// (ADR 0094).
    pub sigma_income: Vec<f64>,
    /// Retained loss each enterprise keeps before the write-off (ADR 0112), minor units: M
    /// months of its base sales VA0 / a_VA (the calibrated steady-state sales value).
    pub ent_loss_cap: Vec<i64>,
    /// Selection constant per traded good and province, `ProvinceData::gamma` good by good.
    pub gamma: Vec<Vec<f64>>,
    /// Price gain kappa1_cell of each market (ADR 0263), [`price_gains`].
    pub price_gain: Vec<f64>,
}

impl Layout {
    /// The layout of `inp` under `params` and `trade`, over the accounts opened at load.
    pub fn build(
        inp: &EconInputs,
        params: &EconParams,
        trade: &TradeParams,
        accounts: Accounts,
    ) -> Layout {
        let (np, n_pops, n_ent) = (inp.n_provinces(), inp.n_pops(), inp.n_enterprises());
        let mut country_pops = vec![Vec::new(); inp.n_countries()];
        for k in 0..n_pops {
            country_pops[inp.country_of(inp.pops.province[k] as usize)].push(k as u32);
        }
        let (pigl, sigma_income) = household_preferences(inp, params);
        let coef = match params.technology {
            Technology::PuttyClay => inp.enterprises.coef.clone(),
            Technology::FixedLeontief => inp.enterprises.column.clone(),
        };
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
        Layout {
            ent_loss_cap: (0..n_ent)
                .map(|e| cap(inp.enterprises.va0[e] / inp.enterprises.a_va[e]))
                .collect(),
            index_weights: price_index_weights(inp, params),
            pigl,
            sigma_income,
            accounts,
            pop_offsets: offsets(&inp.pops.province, np),
            ent_offsets: offsets(&inp.enterprises.province, np),
            labour_supply: inp.labour_supply(&params.edu_eff),
            ownership,
            country_pops,
            a_va: inp.enterprises.a_va.clone(),
            coef,
            gamma: gamma_columns(&inp.provinces.gamma, &inp.goods),
            price_gain: price_gains(inp, params, trade),
        }
    }

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
        price: &[f64],
        money: f64,
        out: &mut [f64],
    ) {
        out.fill(0.0);
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
    /// tick's output) and the quantity used at E2, `[enterprise * ng + k]`.
    pub input_before: Vec<f64>,
    pub input_used: Vec<f64>,
    /// Units of each local good an enterprise draws from this tick's output in the same-tick
    /// chain (`crate::local`), `[enterprise * ng + k]`; the rest of its use comes from
    /// its stock.
    pub from_output: Vec<f64>,
    /// Units of local output drawn by intermediate users at E2 per province market,
    /// `[province * ng + g]` (0 for traded goods): part of the market's demand and sales.
    pub drawn: Vec<f64>,
    /// Passes of the same-tick chain per province at E2 (`local::fills`).
    pub chain_iterations: Vec<u32>,
    /// Whether the same-tick chain of a province met a fall its stall rule could not resolve
    /// (`local::Fills::stalled`): a diagnostic.
    pub chain_stalled: Vec<bool>,
    /// Units dispatched at T2b by each origin market, home purchases included,
    /// `[province * ng + g]`: the filled orders, equal to the market's sales.
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
    /// Pipeline arrivals at T1, `[buyer * ng + g]` (pops, then enterprises).
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
    /// Per traded good, its ledger column while T2b moves its goods; per local good at T3.
    pub stock: Vec<GoodColumn>,
    /// Per local good, its T3 clearing ([`LocalClear`]), and every local purchase in posting
    /// order.
    pub local: Vec<LocalClear>,
    pub local_entries: Vec<Entry>,
    /// Expected months in transit of each traded good to each destination, the arrival
    /// slots weighted by the units arriving per unit of spending (home 0, domestic 1, a
    /// crossing 1 + lag); 0 for local goods. `[province * ng + g]`, refreshed with
    /// [`EconWorld::price_index`].
    pub lead: Vec<f64>,
    /// Country-pair flows of the last 12 ticks, for the run export.
    pub flows: FlowWindow,
    /// Per traded good, this tick's T2b: goods it created less goods it destroyed (selection
    /// at home purchases and in transit, the melt), the same due from `dispatch`
    /// (`Sourced::selected - Sourced::lost`), and its shipped plus delivered units, the scale of
    /// the selection invariant.
    pub dispatch_made: Vec<f64>,
    pub dispatch_due: Vec<f64>,
    pub dispatch_scale: Vec<f64>,
    /// Per enterprise, the traded inputs whose cap cut its labour target at E1, a bit per good
    /// (`systems::scarcest_inputs`), and the employment E1 would have allocated on targets
    /// without the input cap (its employment where no cap binds in its province); E3 orders
    /// those inputs on the output at that employment (ADR 0266).
    pub short: Vec<u64>,
    pub notional: Vec<f64>,
}

impl Scratch {
    fn new(nc: usize, np: usize, n_pops: usize, n_ent: usize, goods: &Goods) -> Self {
        let (ng, n_traded) = (goods.n(), goods.traded);
        Self {
            produced: vec![0.0; n_ent],
            input_before: vec![0.0; n_ent * ng],
            input_used: vec![0.0; n_ent * ng],
            from_output: vec![0.0; n_ent * ng],
            drawn: vec![0.0; np * ng],
            chain_iterations: vec![0; np],
            chain_stalled: vec![false; np],
            dispatched: vec![0.0; np * ng],
            pop_demand: vec![0.0; n_pops * ng],
            ent_order: vec![0.0; n_ent * ng],
            pop_spend: vec![0.0; n_pops * ng],
            ent_spend: vec![0.0; n_ent * ng],
            class_spend: vec![[0.0; 2]; np * ng],
            income_paid: vec![0; n_pops],
            fill: vec![[1.0; 2]; np * ng],
            spoiled: vec![0.0; np * ng],
            due: vec![0.0; (n_pops + n_ent) * ng],
            sourced: (0..n_traded).map(|_| Sourced::new()).collect(),
            columns: vec![Columns::default(); n_traded],
            payments: vec![Payments::default(); n_traded],
            post: PostScratch::default(),
            stock: Vec::new(),
            local: Vec::new(),
            local_entries: Vec::new(),
            lead: vec![0.0; np * ng],
            flows: FlowWindow::new(nc, np, ng, n_traded),
            dispatch_made: vec![0.0; n_traded],
            dispatch_due: vec![0.0; n_traded],
            dispatch_scale: vec![0.0; n_traded],
            short: vec![0; n_ent],
            notional: vec![0.0; n_ent],
        }
    }
}

/// One traded good's columns over provinces, gathered from the `[province * ng + g]`
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
    /// Posted prices and active origins of good `g` of `n_goods`.
    pub fn gather(&mut self, price: &[f64], active: &[bool], g: usize, n_goods: usize) {
        let cells = (0..price.len() / n_goods).map(|o| o * n_goods + g);
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
    /// T2b's pipeline shipments of the good, (slot, buyer, units) in order, applied after
    /// the per-good tasks, and the goods its selection created.
    pub pushes: Vec<ceteris_trade::pipeline::Push>,
    pub made: f64,
}

/// One local good's T3 clearing: its buyers in the province at hand (account, allocation,
/// whether it consumes), its purchases, and where each province's end in them.
#[derive(Clone, Debug, Default)]
pub struct LocalClear {
    pub buyers: Vec<(AccountId, f64, bool)>,
    pub entries: Vec<Entry>,
    pub ends: Vec<u32>,
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
    /// Delivered units bought at home, and the origin units shipped for them (ADR 0200).
    pub home: f64,
    pub home_shipped: f64,
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
    /// [`EconInputs::base_digest`] and the [`network_digest`] of the inputs' network, which
    /// open the encoding.
    base_digest: Hash,
    network_digest: Hash,
    /// Every network this run has had, by digest, so a restore can return to one; shared
    /// with restored copies and not state.
    networks: Arc<Vec<(Hash, Arc<Network>)>>,
    pub ledger: Ledger,
    /// Prices (origin markets of traded goods, province markets of local goods), merchant
    /// inventories, last D, Q and sales.
    pub markets: Markets,
    /// Goods paid for and in flight per buyer: pops `0..n_pops`, then enterprises.
    pub pipeline: Pipeline,
    /// Buyer prices `[province * ng + g]`: q_d = Gamma_d P_d of each traded good (ADR
    /// 0200), the numeraire a buyer at d pays per arriving unit, tariffs included (infinite
    /// where no active origin is reachable), and the posted price of each local good.
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

/// BLAKE3 over the inputs digest and both parameter sets except the write-off threshold P148
/// and the storable price clip P151, which the snapshot header carries after the digest:
/// everything else a snapshot leaves out.
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
        // the inputs' goods (`EconWorld::new`), in their digest
        goods: _,
        theta,
        theta_scale,
        coverage,
        kappa1,
        gain_share,
        kappa2,
        c_max,
        // binds only off a steady state, so calibrations do not depend on it; the snapshot
        // header carries it
        c_max_storable: _,
        expectation_gain,
        border_nu,
        time_cost,
        // reaches the kernel only through Network.route_taus, in the inputs digest
        theta_r: _,
        // groups goods for kernel sharing; the dynamics read theta alone
        theta_class: _,
        // reach the kernel only through Network.bulk, in the inputs digest; Kernel::build
        // refuses values other than the ones the network was built with
        bulk_sea: _,
        bulk_port: _,
        storage_loss,
        storage_cost,
        speculation,
        convenience,
        storage_gain,
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
    e.f64(*theta_scale);
    e.f64s(&[*kappa1, *kappa2, *c_max, *expectation_gain]);
    // reaches the dynamics only through the calibration's market elasticities
    if inputs.market_elasticity.is_some() {
        e.f64(*gain_share);
    }
    e.f64s(border_nu);
    e.f64s(coverage);
    e.f64s(time_cost);
    e.f64s(storage_loss);
    e.f64s(storage_cost);
    e.f64s(speculation);
    e.f64s(convenience);
    e.f64(*storage_gain);
    state_hash(&e.into_bytes())
}

/// [`static_digest`] of `inputs` under the structural subset of the parameters, the rest at
/// their defaults: the parameters a steady state (calibrated or solved) depends on, `sigma_demand`,
/// the PIGL parameters, `sigma_fl`, `edu_eff`, `theta`, `theta_scale`, `coverage`, `border_nu`,
/// `time_cost` and `storage_loss` (output replaces spoilage at a steady state, ADR 0091), and the
/// other storage parameters `storage_cost`, `speculation`, `convenience` and `storage_gain`,
/// which leave the steady state but enter the calibration's block map and market elasticities
/// (ADRs 0091, 0267): a run under other storage parameters would keep stale price-gain limits.
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
            goods: trade.goods.clone(),
            theta: trade.theta.clone(),
            theta_scale: trade.theta_scale,
            coverage: trade.coverage.clone(),
            border_nu: trade.border_nu.clone(),
            time_cost: trade.time_cost.clone(),
            storage_loss: trade.storage_loss.clone(),
            storage_cost: trade.storage_cost.clone(),
            speculation: trade.speculation.clone(),
            convenience: trade.convenience.clone(),
            storage_gain: trade.storage_gain,
            ..TradeParams::default()
        },
    )
}

/// kappa1_cell of every cell (ADRs 0263, 0267; trade SPEC, Per-market gain and Block gain):
/// kappa1 capped at the share P03d of the cell's J2 limit, J from the calibration's market
/// elasticities at the run's employment speed eta (P41), and at the share P03d of its coupled
/// block's limit gain ([`MarketElasticity::price_gain`]); kappa1 in every cell of inputs without
/// a calibration.
pub fn price_gains(inp: &EconInputs, params: &EconParams, trade: &TradeParams) -> Vec<f64> {
    let ng = inp.n_goods();
    let n = inp.n_provinces() * ng;
    match &inp.market_elasticity {
        Some(m) => (0..n).map(|i| m.price_gain(i, params, trade)).collect(),
        None => vec![trade.kappa1; n],
    }
}

/// Which (province, good) cells have a market, `[province * ng + g]`. A traded good's
/// market is the origin market of the province's output: active where an enterprise of the
/// province produces the good with positive capacity (A VA0 > 0). A local good's market is
/// the province market: active where anyone in the province buys it (final demand in the
/// province's country, or an enterprise of the province using it as an input under either S09
/// setting). Fixed by the inputs for the whole run.
pub fn active_cells(inp: &EconInputs) -> Vec<bool> {
    let (ng, nt) = (inp.n_goods(), inp.goods.traded);
    let mut active = vec![false; inp.n_provinces() * ng];
    for i in 0..inp.n_provinces() {
        let omega = &inp.countries.omega[inp.country_of(i)];
        for g in nt..ng {
            active[i * ng + g] = omega[g] > 0.0;
        }
    }
    let ent = &inp.enterprises;
    for e in 0..inp.n_enterprises() {
        let i = ent.province[e] as usize;
        let sector = usize::from(ent.good[e]);
        if sector < nt && ent.a[e] * ent.va0[e] > 0.0 {
            active[i * ng + sector] = true;
        }
        for k in nt..ng {
            if inp.coef(e, k) > 0.0 || inp.a(inp.country_of(i), k, sector) > 0.0 {
                active[i * ng + k] = true;
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
/// s_L) VA0 (national ownership). As `ceteris_reference::io::Inputs::base_income`.
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
/// population, 0 for a country without people. As `ceteris_reference::io::Inputs::base_spend`.
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

/// Base income per person of each pop, rebates aside (ADR 0094): w0 (1 - u*) times its
/// effective labour, plus its population share of its country's base profit pool, over its
/// population; 0 without people. As `ceteris_reference::io::Inputs::pop_base_income`.
pub fn pop_base_income(inp: &EconInputs, params: &EconParams) -> Vec<f64> {
    let pool = base_profit_pool(inp);
    let (_, country_people) = pop_people(inp);
    (0..inp.n_pops())
        .map(|k| {
            let i = inp.pops.province[k] as usize;
            let c = inp.country_of(i);
            let n = inp.pops.population[k];
            if n > 0.0 {
                let effective =
                    params.edu_eff[usize::from(inp.pops.edu[k])] * inp.pops.labour_force[k];
                let wage = inp.provinces.w0[i] * (1.0 - inp.countries.u_star[c]) * effective;
                (wage + n / country_people[c] * pool[c]) / n
            } else {
                0.0
            }
        })
        .collect()
}

/// Per country, the calibrated PIGL weights (`Countries::pigl`) or else the weights inverted
/// jointly over its pops at prices 1, each at its [`pop_base_income`] and weighted by its base
/// spending (ADR 0094), and the within-pop log income dispersion every pop of the country uses,
/// which the weights carry. Panics where a country's shares cannot be inverted
/// (`invert_pigl_pops`).
pub fn household_preferences(
    inp: &EconInputs,
    params: &EconParams,
) -> (Vec<PiglWeights>, Vec<f64>) {
    let ng = inp.n_goods();
    let weights = match &inp.countries.pigl {
        Some(w) => w.clone(),
        None => {
            let income = pop_base_income(inp, params);
            let mut pops = vec![Vec::new(); inp.n_countries()];
            for k in 0..inp.n_pops() {
                pops[inp.country_of(inp.pops.province[k] as usize)].push(k);
            }
            pops.iter()
                .enumerate()
                .map(|(c, members)| {
                    let people: Vec<(f64, f64)> = members
                        .iter()
                        .map(|&k| (inp.pops.population[k], income[k]))
                        .collect();
                    let sigma = within_sigma(inp.countries.gini[c], &people);
                    let at: Vec<PopAt> = people
                        .iter()
                        .map(|&(n, m)| PopAt {
                            price: vec![1.0; ng],
                            income: m,
                            spend: n * m,
                        })
                        .collect();
                    invert_pigl_pops(
                        &inp.countries.omega[c],
                        &inp.goods.category,
                        &at,
                        sigma,
                        params,
                    )
                })
                .collect()
        }
    };
    let sigma = weights.iter().map(|w| w.sigma).collect();
    (weights, sigma)
}

/// Base household consumption quantities at prices 1, the weights of the world price index
/// over buyer prices: each country's final demand shares times the province's
/// [`base_income`], `[province * ng + g]`. The same weights as
/// `ceteris_reference::io::Inputs::index_weights`, so V1 normalizes run and reference alike.
pub fn price_index_weights(inp: &EconInputs, params: &EconParams) -> Vec<f64> {
    let ng = inp.n_goods();
    let base = base_income(inp, params);
    let mut out = vec![0.0; inp.n_provinces() * ng];
    for i in 0..inp.n_provinces() {
        let c = inp.country_of(i);
        let income = base[i];
        let omega = &inp.countries.omega[c];
        let total: f64 = omega.iter().sum();
        for g in 0..ng {
            out[i * ng + g] = omega[g] / total * income;
        }
    }
    out
}

fn open_n(ledger: &mut Ledger, n: usize) -> Vec<AccountId> {
    (0..n).map(|_| ledger.open()).collect()
}

fn open_wide_n(ledger: &mut Ledger, n: usize) -> Vec<AccountId> {
    (0..n).map(|_| ledger.open_wide()).collect()
}

/// Creates `change` of `good` in `account`, or destroys `-change` of it.
pub(crate) fn adjust_stock(ledger: &mut Ledger, account: AccountId, good: usize, change: f64) {
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

/// Selection constants of the traded goods, one column over provinces per good.
fn gamma_columns(gamma: &Rows<f64>, goods: &Goods) -> Vec<Vec<f64>> {
    goods
        .traded_goods()
        .map(|g| gamma.iter().map(|row| row[g]).collect())
        .collect()
}

/// Buyer prices of every cell, `[province * ng + g]`, with origins posting `price` on
/// `active` markets: a traded good's buyer price q_d = Gamma_d P_d (ADR 0200; `gamma` one
/// column per traded good; infinite where no active origin is reachable) and the expected
/// months in transit (`lead`, the arrival slots of `source` and `dispatch` of one unit at every
/// destination, weighted by the units arriving); a local good's posted price, lead 0.
/// `sourced` holds one sourcing per traded good, left at the unit spending: T2 sources the
/// same good at the same prices on it again, and `source` then reuses the origin access.
/// Traded goods run in parallel, each on its own `sourced` and `columns` entry. Without
/// `full`, the unit dispatch computes only the arrivals ([`arrivals`]), all that the index and
/// the lead read; with it, every output of `dispatch`.
#[allow(clippy::too_many_arguments)]
fn delivered_prices(
    full: bool,
    kernel: &Kernel,
    price: &[f64],
    active: &[bool],
    gamma: &[Vec<f64>],
    sourced: &mut [Sourced],
    columns: &mut [Columns],
    index: &mut [f64],
    lead: &mut [f64],
) {
    let ng = kernel.n_goods();
    let np = price.len() / ng;
    // `sourced[g]` and `columns[g]` belong to good g, so the traded goods must come first
    let n_traded = sourced.len();
    assert!(
        columns.len() == n_traded
            && (0..ng).all(|g| kernel.try_good(g).is_some() == (g < n_traded)),
        "traded goods must be goods 0 to {n_traded}"
    );
    let (spend, fill) = (vec![[1.0, 0.0]; np], vec![[1.0; 2]; np]);
    sourced
        .par_iter_mut()
        .zip(columns.par_iter_mut())
        .enumerate()
        .for_each(|(g, (unit, col))| {
            col.gather(price, active, g, ng);
            source(kernel, g, &col.price, &col.active, &gamma[g], &spend, unit);
            if full {
                dispatch(kernel, g, &col.price, &fill, unit);
            } else {
                arrivals(kernel, g, unit);
            }
            col.index.clone_from(&unit.buyer_price);
            col.lead.clear();
            for d in 0..np {
                let slots = unit.arrive.iter().map(|a| a[d][INTERMEDIATE]);
                let units: f64 = slots.clone().sum();
                let weighted: f64 = slots.enumerate().map(|(s, x)| s as f64 * x).sum();
                col.lead
                    .push(if units > 0.0 { weighted / units } else { 0.0 });
            }
        });
    for d in 0..np {
        for g in 0..ng {
            let cell = d * ng + g;
            (index[cell], lead[cell]) = match columns.get(g) {
                Some(col) => (col.index[d], col.lead[d]),
                None => (price[cell], 0.0),
            };
        }
    }
}

/// Base delivered prices and selection constants per province, in the data state (every price
/// 1, the traded markets of [`active_cells`], which do not depend on input coefficients) over
/// the kernel of `inp`'s network under `trade`. The base price of a traded good is the
/// delivered unit price of the physical arrivals, 1 / alpha_d with alpha_d the units that
/// survive the losses per unit of spending (infinite where no active origin is reachable), and
/// Gamma = (1 / alpha_d) / P_d (ADR 0200; 1 where unreachable), so that the buyer price of tick
/// 0, Gamma P_d, is the base price. Local goods: 1 and 1.
pub fn base_delivered_prices(inp: &EconInputs, trade: &TradeParams) -> (Rows<f64>, Rows<f64>) {
    let (ng, nt) = (inp.n_goods(), inp.goods.traded);
    let kernel = Kernel::build(&inp.network, &inp.weight, &inp.wedges, trade);
    let np = inp.n_provinces();
    let ent = &inp.enterprises;
    let mut active = vec![false; np * ng];
    for e in 0..inp.n_enterprises() {
        let g = usize::from(ent.good[e]);
        if g < nt && ent.a[e] * ent.va0[e] > 0.0 {
            active[ent.province[e] as usize * ng + g] = true;
        }
    }
    let (mut index, mut lead) = (vec![0.0; np * ng], vec![0.0; np * ng]);
    let price = vec![1.0; np * ng];
    let n_traded = inp.goods.traded;
    let mut sourced: Vec<Sourced> = (0..n_traded).map(|_| Sourced::new()).collect();
    let mut columns = vec![Columns::default(); n_traded];
    let ones = vec![vec![1.0; np]; n_traded];
    delivered_prices(
        true,
        &kernel,
        &price,
        &active,
        &ones,
        &mut sourced,
        &mut columns,
        &mut index,
        &mut lead,
    );
    let (mut base, mut gamma) = (Rows::filled(np, ng, 1.0), Rows::filled(np, ng, 1.0));
    for (g, unit) in sourced.iter().enumerate() {
        for d in 0..np {
            let alpha: f64 = unit.survived.iter().map(|s| s[d]).sum();
            let ces = unit.price_index[d];
            if alpha > 0.0 {
                base[d][g] = 1.0 / alpha;
                gamma[d][g] = base[d][g] / ces;
            } else {
                base[d][g] = f64::INFINITY;
            }
        }
    }
    (base, gamma)
}

/// Relative rounding of market holdings that the transit accounts carry (`EconWorld::end_tick`).
const TRANSIT_ROUNDING: f64 = 1e-12;

impl EconWorld {
    /// Builds the kernel, opens the accounts and seeds the initial state: the calibrated
    /// steady state of `EconInputs::steady` (ADR 0089, [`EconWorld::start_steady`]) when the
    /// inputs carry one, else the 2000 data state: prices 1, base wages w0 and employment L0
    /// from the inputs, unemployment at the natural rate u*, input stocks at one month of base
    /// use (base output Y0 = A VA0 / a_VA), origin inventories at coverage times own base
    /// output (see [`EconWorld::stock_base_output`]), nothing in flight, and one month of base
    /// dividends (the pop's population share of its country's profit pool, `(1 - s_L) VA0`
    /// per enterprise) minted to each pop: tick 0 wages are paid at E2, so tick 0 spending is
    /// one month of base income. Panics on invalid inputs or a base point other than base
    /// employment = (1 - u*) L^s under `params.edu_eff`, on a storage loss without speculation
    /// ([`TradeParams::check_storage`]), and on a calibrated start under structural
    /// parameters other than the defaults with the calibrated border friction
    /// ([`structural_digest`], [`EconInputs::trade_params`]).
    pub fn new(inputs: Arc<EconInputs>, params: EconParams, trade: TradeParams) -> EconWorld {
        inputs.validate();
        trade.check_storage().unwrap_or_else(|e| panic!("{e}"));
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
        let (base_digest, network) = (inp.base_digest(), network_digest(&inp.network));
        let networks = Arc::new(vec![(network, Arc::new(inp.network.clone()))]);

        assert_eq!(
            trade.goods, inp.goods,
            "the trade parameters are for other goods than the inputs"
        );
        let (ng, n_traded) = (inp.n_goods(), inp.goods.traded);
        let mut ledger = Ledger::new(ng);
        let accounts = Accounts {
            pop: open_n(&mut ledger, n_pops),
            enterprise: open_n(&mut ledger, n_ent),
            market: open_wide_n(&mut ledger, np),
            gateway: open_wide_n(&mut ledger, inp.network.n_gateways()),
            transit: open_n(&mut ledger, n_traded),
            rebate: open_wide_n(&mut ledger, nc),
            dividend: open_wide_n(&mut ledger, nc),
            mint: ledger.open_wide(),
        };

        let edu_eff = |k: usize| params.edu_eff[usize::from(inp.pops.edu[k])];
        let layout = Layout::build(inp, &params, &trade, accounts);
        let employed: Vec<f64> = (0..n_pops)
            .map(|k| {
                let i = inp.pops.province[k] as usize;
                (1.0 - inp.countries.u_star[inp.country_of(i)]) * inp.pops.labour_force[k]
            })
            .collect();
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

        let mut markets = Markets::new(np, &inp.goods);
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
            scratch: Scratch::new(nc, np, n_pops, n_ent, &inp.goods),
            ledger,
            markets,
            pipeline: Pipeline::new(n_pops + n_ent, ng),
            price_index: vec![0.0; np * ng],
            inputs,
            kernel,
            params,
            trade,
            layout: Arc::new(layout),
            digest,
            base_digest,
            network_digest: network,
            networks,
        };
        match w.inputs.steady.clone() {
            Some(steady) => {
                assert_eq!(
                    structural_digest(&w.inputs, &w.params, &w.trade),
                    structural_digest(&w.inputs, &EconParams::default(), &w.inputs.trade_params()),
                    "the calibrated steady state was built under the default structural \
                     parameters and the calibrated border friction (`EconInputs::trade_params`); \
                     start from the data state (`--data-state`) or recalibrate"
                );
                w.start_steady(&steady)
            }
            None => {
                if w.inputs.market_elasticity.is_some() {
                    assert_eq!(
                        structural_digest(&w.inputs, &w.params, &w.trade),
                        structural_digest(
                            &w.inputs,
                            &EconParams::default(),
                            &w.inputs.trade_params()
                        ),
                        "the market elasticities were computed under the default structural \
                         parameters and the calibrated border friction \
                         (`EconInputs::trade_params`), so their price gains (ADR 0263) would be \
                         stale; recalibrate, or start inputs without a calibration"
                    );
                }
                w.start_data_state()
            }
        }
        w
    }

    /// The 2000 data state of [`EconWorld::new`] on a world fresh from its accounts.
    fn start_data_state(&mut self) {
        let ng = self.inputs.n_goods();
        let (inp, lay) = (Arc::clone(&self.inputs), Arc::clone(&self.layout));
        let (inp, lay) = (&*inp, &*lay);
        let accounts = &lay.accounts;
        // one month of base dividends, minted: the national profit pool now, wages at E2
        let pool = base_profit_pool(inp);
        for k in 0..inp.n_pops() {
            let i = inp.pops.province[k] as usize;
            let dividends = lay.ownership[k] * pool[inp.country_of(i)];
            self.ledger
                .transfer(MINT, accounts.mint, accounts.pop[k], to_minor(dividends));
        }
        // input stocks at one month of base use (Y0 = A VA0 / a_VA)
        for e in 0..inp.n_enterprises() {
            let y0 = inp.enterprises.a[e] * inp.enterprises.va0[e] / lay.a_va[e];
            for k in 0..ng {
                let q = lay.coef[e][k] * y0;
                if q > 0.0 {
                    self.ledger.create(accounts.enterprise[e], k, q);
                }
            }
        }
        self.refresh_price_index();
        self.price_level = self.world_price_index();
        self.stock_base_output();
    }

    /// The calibrated steady state at the start of a tick (ADR 0089, stage 5), on a world
    /// fresh from its accounts: posted prices, the last D, Q and sales, merchant and input
    /// stocks, the goods in flight, each pop's money and last income, each enterprise's last
    /// output, sales and profit; wages w0, employment L0 and unemployment u* as in the data
    /// state; buyer prices refreshed at the posted prices, pi^e = 0.
    fn start_steady(&mut self, s: &Steady) {
        let (ng, nt) = (self.inputs.n_goods(), self.inputs.goods.traded);
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
            let held = if m.active[cell] && cell % ng < nt {
                stock
            } else {
                0.0
            };
            m.inventory[cell] = held;
            if held > 0.0 {
                self.ledger.create(acc.market[cell / ng], cell % ng, held);
            }
        }
        for e in 0..inp.n_enterprises() {
            for (k, &stock) in s.stock[e].iter().enumerate() {
                if stock > 0.0 {
                    self.ledger.create(acc.enterprise[e], k, stock);
                }
            }
        }
        let mut in_flight = vec![0.0; inp.n_goods()];
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
        let ng = self.inputs.n_goods();
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
            for k in 0..ng {
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
                adjust_stock(&mut self.ledger, acc.market[c / ng], c % ng, change);
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

    /// Origin inventories of the data state at coverage times the origin's own base output
    /// of the good, s_g Y0 with Y0 = A VA0 / a_VA, at every active traded cell; local goods
    /// hold none.
    fn stock_base_output(&mut self) {
        let (inp, lay) = (Arc::clone(&self.inputs), Arc::clone(&self.layout));
        let (inp, lay) = (&*inp, &*lay);
        let ng = inp.n_goods();
        for e in 0..inp.n_enterprises() {
            let (o, g) = (
                inp.enterprises.province[e] as usize,
                usize::from(inp.enterprises.good[e]),
            );
            let y0 = inp.enterprises.a[e] * inp.enterprises.va0[e] / lay.a_va[e];
            let stock = self.trade.coverage[g] * y0;
            if inp.goods.is_traded(g) && self.markets.active[o * ng + g] && stock > 0.0 {
                self.markets.inventory[o * ng + g] += stock;
                self.ledger.create(lay.accounts.market[o], g, stock);
            }
        }
    }

    /// Recomputes [`EconWorld::price_index`] and `Scratch::lead` at the current prices. For a
    /// traded good, every destination spends one unit, sourced over the origin weights
    /// (`ceteris_trade::source`) and dispatched in full: q_d = Gamma_d P_d (ADR 0200), and the
    /// lead is the mean arrival slot of the units arriving.
    pub fn refresh_price_index(&mut self) {
        let _span = timing::span("e3.price_refresh", timing::NONE);
        let sc = &mut self.scratch;
        delivered_prices(
            false,
            &self.kernel,
            &self.markets.price,
            &self.markets.active,
            &self.layout.gamma,
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
        let empty = (0..self.inputs.n_goods()).all(|g| self.pipeline.in_flight(g) == 0.0);
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
    /// whose accounts are open. The price gains stay those of the calibration (ADR 0263, As
    /// built), as the coefficients and Gamma do: J needs the calibrated spending, which a run
    /// does not have. Snapshots name the network by its digest; the run keeps every network
    /// it has had, so a restore returns to the one its snapshot names. Crossing lags set by
    /// [`EconWorld::set_crossing_lags`] do not carry over to the new kernel.
    pub fn set_network(&mut self, network: Network) {
        let old = &self.inputs.network;
        assert!(
            network.province_country == old.province_country
                && network.gateway_country == old.gateway_country,
            "a new network must keep the provinces and gateways"
        );
        let digest = network_digest(&network);
        if !self.networks.iter().any(|(d, _)| *d == digest) {
            Arc::make_mut(&mut self.networks).push((digest, Arc::new(network.clone())));
        }
        let mut inputs = (*self.inputs).clone();
        inputs.network = network;
        self.configure(
            Arc::new(inputs),
            digest,
            self.params.clone(),
            self.trade.clone(),
        );
    }

    /// Replaces the economic parameters between ticks, an ex-post intervention: the layout
    /// is rebuilt under them and the dynamic state stands. A structural change (see
    /// [`structural_digest`]) moves a calibrated world off its steady state, the PIGL
    /// parameters too, since the calibrated weights were inverted at the defaults.
    pub fn set_params(&mut self, params: EconParams) {
        self.configure(
            Arc::clone(&self.inputs),
            self.network_digest,
            params,
            self.trade.clone(),
        );
    }

    /// Replaces the trade parameters between ticks, an ex-post intervention as
    /// [`EconWorld::set_params`]; the kernel is rebuilt under them. Panics on parameters for
    /// other goods or an invalid storage set ([`TradeParams::check_storage`]).
    pub fn set_trade_params(&mut self, trade: TradeParams) {
        assert_eq!(
            trade.goods, self.inputs.goods,
            "the trade parameters are for other goods than the inputs"
        );
        trade.check_storage().unwrap_or_else(|e| panic!("{e}"));
        self.configure(
            Arc::clone(&self.inputs),
            self.network_digest,
            self.params.clone(),
            trade,
        );
    }

    /// Moves the world's static parts to `inputs` (its network named by `network`) under
    /// `params` and `trade`, rebuilding the kernel and the layout where they change.
    fn configure(
        &mut self,
        inputs: Arc<EconInputs>,
        network: Hash,
        params: EconParams,
        trade: TradeParams,
    ) {
        if network != self.network_digest || trade != self.trade {
            self.kernel = Arc::new(Kernel::build(
                &inputs.network,
                &inputs.weight,
                &inputs.wedges,
                &trade,
            ));
        }
        if params != self.params || trade != self.trade {
            self.layout = Arc::new(Layout::build(
                &inputs,
                &params,
                &trade,
                self.layout.accounts.clone(),
            ));
        }
        self.digest = static_digest(&inputs, &params, &trade);
        self.network_digest = network;
        self.inputs = inputs;
        self.params = params;
        self.trade = trade;
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

    /// [`static_digest`] of this world.
    pub fn digest(&self) -> Hash {
        self.digest
    }
}

fn encode_hash(e: &mut Encoder, h: &Hash) {
    h.iter().for_each(|&b| e.u8(b));
}

fn decode_hash(d: &mut Decoder) -> Result<Hash, DecodeError> {
    let mut h = [0u8; 32];
    for b in &mut h {
        *b = d.u8()?;
    }
    Ok(h)
}

/// `params` as snapshots carry them; S09 only where the inputs have more than one vintage, as
/// in [`static_digest`], since with one both settings run identically.
fn encode_params(e: &mut Encoder, params: &EconParams, one_vintage: bool) {
    // destructured so a new parameter cannot be left out of snapshots
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
        write_off_months,
    } = params;
    let Pigl {
        sigma_b,
        phi,
        gamma,
        theta,
        eps,
    } = pigl;
    e.f64s(&[*sigma_demand, *sigma_fl, *kappa_w, *delta_w, *eta]);
    e.f64s(&[*sigma_b, *phi, *gamma, *eps]);
    e.f64s(theta);
    e.f64s(edu_eff);
    e.bools(&[*frictionless]);
    e.u8(*rationing as u8);
    if !one_vintage {
        e.u8(*technology as u8);
    }
    e.f64(*write_off_months);
}

fn decode_array<const N: usize>(d: &mut Decoder, what: &str) -> Result<[f64; N], DecodeError> {
    let v = d.f64s()?;
    let n = v.len();
    v.try_into()
        .map_err(|_| DecodeError(format!("{what}: {n} entries, want {N}")))
}

/// Decodes [`encode_params`]; with one vintage S09 is `technology`, the restoring world's.
fn decode_params(
    d: &mut Decoder,
    one_vintage: bool,
    technology: Technology,
) -> Result<EconParams, DecodeError> {
    let [sigma_demand, sigma_fl, kappa_w, delta_w, eta] = decode_array(d, "econ parameters")?;
    let [sigma_b, phi, gamma, eps] = decode_array(d, "PIGL parameters")?;
    let theta = decode_array(d, "PIGL theta")?;
    let edu_eff = decode_array(d, "education efficiencies")?;
    let [frictionless] = decode_array_bool(d)?;
    let rationing = match d.u8()? {
        0 => Rationing::Priority,
        1 => Rationing::Proportional,
        2 => Rationing::Mixed,
        b => return Err(DecodeError(format!("rationing {b}"))),
    };
    let technology = if one_vintage {
        technology
    } else {
        match d.u8()? {
            0 => Technology::PuttyClay,
            1 => Technology::FixedLeontief,
            b => return Err(DecodeError(format!("technology {b}"))),
        }
    };
    Ok(EconParams {
        sigma_demand,
        pigl: Pigl {
            sigma_b,
            phi,
            gamma,
            theta,
            eps,
        },
        sigma_fl,
        kappa_w,
        delta_w,
        eta,
        edu_eff,
        frictionless,
        rationing,
        technology,
        write_off_months: d.f64()?,
    })
}

fn decode_array_bool(d: &mut Decoder) -> Result<[bool; 1], DecodeError> {
    let v = d.bools()?;
    let n = v.len();
    v.try_into()
        .map_err(|_| DecodeError(format!("frictionless switch: {n} entries")))
}

fn encode_trade(e: &mut Encoder, trade: &TradeParams) {
    // destructured so a new parameter cannot be left out of snapshots
    let TradeParams {
        // the inputs' goods
        goods: _,
        theta,
        theta_class,
        theta_scale,
        coverage,
        kappa1,
        gain_share,
        kappa2,
        c_max,
        c_max_storable,
        expectation_gain,
        border_nu,
        time_cost,
        theta_r,
        bulk_sea,
        bulk_port,
        storage_loss,
        storage_cost,
        speculation,
        convenience,
        storage_gain,
    } = trade;
    e.f64s(theta);
    e.count(theta_class.len());
    theta_class.iter().for_each(|&c| e.u8(c));
    e.f64s(&[
        *theta_scale,
        *kappa1,
        *gain_share,
        *kappa2,
        *c_max,
        *c_max_storable,
        *expectation_gain,
        *theta_r,
        *storage_gain,
    ]);
    for column in [
        coverage,
        border_nu,
        time_cost,
        storage_loss,
        storage_cost,
        speculation,
        convenience,
    ] {
        e.f64s(column);
    }
    e.f64s(bulk_sea);
    e.f64s(bulk_port);
}

fn decode_trade(d: &mut Decoder, goods: &Goods) -> Result<TradeParams, DecodeError> {
    let theta = d.f64s()?;
    let theta_class = (0..d.count()?).map(|_| d.u8()).collect::<Result<_, _>>()?;
    let [
        theta_scale,
        kappa1,
        gain_share,
        kappa2,
        c_max,
        c_max_storable,
        expectation_gain,
        theta_r,
        storage_gain,
    ] = decode_array(d, "trade parameters")?;
    let mut columns = Vec::with_capacity(7);
    for _ in 0..7 {
        columns.push(d.f64s()?);
    }
    let [
        coverage,
        border_nu,
        time_cost,
        storage_loss,
        storage_cost,
        speculation,
        convenience,
    ]: [Vec<f64>; 7] = columns.try_into().expect("seven columns");
    let trade = TradeParams {
        goods: goods.clone(),
        theta,
        theta_class,
        theta_scale,
        coverage,
        kappa1,
        gain_share,
        kappa2,
        c_max,
        c_max_storable,
        expectation_gain,
        border_nu,
        time_cost,
        theta_r,
        bulk_sea: decode_array(d, "bulk sea rates")?,
        bulk_port: decode_array(d, "bulk port rates")?,
        storage_loss,
        storage_cost,
        speculation,
        convenience,
        storage_gain,
    };
    trade.check_goods().map_err(DecodeError)?;
    Ok(trade)
}

impl Encode for EconWorld {
    /// The inputs' digest without the network, the network's digest and both parameter sets,
    /// then the dynamic state.
    fn encode(&self, e: &mut Encoder) {
        encode_hash(e, &self.base_digest);
        encode_hash(e, &self.network_digest);
        encode_params(e, &self.params, self.inputs.one_vintage());
        encode_trade(e, &self.trade);
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
        self.ledger.track(&[WAGE, DIVIDEND, REBATE]);
        self.ledger.begin_tick();
        self.scratch.income_paid.fill(0);
        self.scratch.flows.advance();
    }

    /// Invariants that hold on any world, in order: ledger (money, goods sign, goods
    /// identity), population, labour bounds, price bounds (every price and wage), inventory
    /// (S >= 0, and held by the market account to 1e-9 of the turnover S + Q + D), transit
    /// (pipeline and transit accounts agree to 1e-9, plus 1e-12 of the good's market turnover),
    /// selection (what T2b created less what it destroyed is its selection term less its melt,
    /// 1e-9 of its shipped and delivered units plus the transit rounding; ADR 0200), gateway
    /// clearing (money exact, goods 1e-9), and over this tick's evidence in
    /// [`Scratch`]: Leontief (inputs used = coef_k Y, 1e-12 relative to the larger of
    /// coef_k Y and what was available: the stock, and this tick's draws of a local good; a
    /// local good also [`CHAIN_STOP_TOLERANCE`] of the same and [`MARKET_DRAIN_TOLERANCE`] of
    /// its province's draws),
    /// dispatch mass (units dispatched = origin sales, 1e-12 of the larger plus the rounding below
    /// the normal range the kernel carries, [`dispatch_tolerance`])
    /// and income (wages, dividends and rebates credited to each pop in the journal equal
    /// its recorded income, exact in minor units).
    fn end_tick(&self, tick: u32) -> Result<(), Violation> {
        let (ng, nt) = (self.inputs.n_goods(), self.inputs.goods.traded);
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
            for g in 0..ng {
                let cell = i * ng + g;
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
            // the account is filled with differences of market holdings, so it carries their
            // rounding, a few ulps of each market's turnover: shipments far below a market's
            // stock can vanish from it
            let turnover: f64 = (0..lay.accounts.market.len())
                .map(|i| i * ng + g)
                .map(|c| {
                    self.markets.inventory[c] + self.markets.supply[c] + self.markets.demand[c]
                })
                .sum();
            let rounding = TRANSIT_ROUNDING * turnover;
            if (held - flight).abs() > GOODS_TOLERANCE * flight.max(held).max(1.0) + rounding {
                return fail(
                    "transit",
                    format!("good {g}: pipeline {flight}, account {held}"),
                );
            }
            // ADR 0200: what T2b created less what it destroyed is its selection less its melt
            let sc = &self.scratch;
            let (made, due) = (sc.dispatch_made[g], sc.dispatch_due[g]);
            let gap = (made - due).abs();
            if gap.is_nan() || gap > GOODS_TOLERANCE * sc.dispatch_scale[g].max(1.0) + rounding {
                return fail(
                    "selection",
                    format!("good {g}: dispatch made {made}, selection less melt {due}"),
                );
            }
        }
        if let Some(a) = lay
            .accounts
            .gateway
            .iter()
            .find(|&&a| self.ledger.money_wide(a) != 0)
        {
            let detail = format!("gateway account {a:?} holds {}", self.ledger.money_wide(*a));
            return fail("gateway clearing", detail);
        }
        for &a in &lay.accounts.gateway {
            if let Some(g) = (0..ng).find(|&g| self.ledger.goods(a, g).abs() > GOODS_TOLERANCE) {
                let detail = format!(
                    "gateway account {a:?} holds {} of good {g}",
                    self.ledger.goods(a, g)
                );
                return fail("gateway clearing", detail);
            }
        }

        let sc = &self.scratch;
        for e in 0..inp.n_enterprises() {
            let province = inp.enterprises.province[e] as usize;
            for k in 0..ng {
                let traded = k < nt;
                let want = lay.coef[e][k] * sc.produced[e];
                let (used, before) = (sc.input_used[e * ng + k], sc.input_before[e * ng + k]);
                let gap = (used - want).abs();
                let tolerance = if traded {
                    LEONTIEF_TOLERANCE * want.max(before)
                } else {
                    (LEONTIEF_TOLERANCE + CHAIN_STOP_TOLERANCE) * want.max(before)
                        + market_drain_tolerance(ng) * sc.drawn[province * ng + k]
                };
                if gap.is_nan() || gap > tolerance {
                    let detail = format!("enterprise {e} used {used} of good {k}, a Y = {want}");
                    return fail("leontief", detail);
                }
            }
        }

        let traded: Vec<usize> = (0..nt).collect();
        let m = &self.markets;
        for (cell, (&sent, &sold)) in sc.dispatched.iter().zip(&m.sales).enumerate() {
            let (o, g) = (cell / ng, cell % ng);
            let Ok(t) = traded.binary_search(&g) else {
                continue;
            };
            // products below the normal range round absolutely: what the kernel carries for
            // them, and a subnormal fill's own rounding of avail / D
            let r = &sc.sourced[t];
            let f = sc.fill[cell];
            let below = |x: f64| {
                if x < f64::MIN_POSITIVE {
                    f64::from_bits(1)
                } else {
                    0.0
                }
            };
            // nothing was dispatched before a world's first T2b, so nothing was rounded
            let rounding = r.shipped_rounding.get(o).copied().unwrap_or(0.0)
                + f[0] * r.orders_rounding[o][0]
                + f[1] * r.orders_rounding[o][1]
                + below(f[0]) * m.demand_int[cell]
                + below(f[1]) * m.demand_hh[cell];
            let gap = (sent - sold).abs();
            if gap.is_nan() || gap > dispatch_tolerance(sent, sold, rounding) {
                let detail = format!("cell {cell}: dispatched {sent} of sales {sold}");
                return fail("dispatch mass", detail);
            }
        }

        // pops hold the first account IDs; the ledger tallies the income flows per account
        // as it posts them, the sum the journal gives
        let credited = |k: usize| self.ledger.tally(AccountId(k as u32));
        if let Some(k) = (0..inp.n_pops()).find(|&k| credited(k) != sc.income_paid[k]) {
            let detail = format!(
                "pop {k}: journal credits {}, income paid {}",
                credited(k),
                sc.income_paid[k]
            );
            return fail("income", detail);
        }
        Ok(())
    }

    /// Restores a snapshot of a world built from the same inputs. The snapshot opens with the
    /// inputs' digest without the network, which must be this live world's (else a
    /// `DecodeError`), the digest of its network, which must be one this run has had, and
    /// its parameters. The restored world shares this world's inputs, kernel and layout where
    /// the network and parameters are this world's, and rebuilds them where not; only the
    /// dynamic state is decoded.
    fn restore(&self, d: &mut Decoder) -> Result<Self, DecodeError> {
        let ng = self.inputs.n_goods();
        if decode_hash(d)? != self.base_digest {
            return Err(DecodeError("snapshot belongs to other inputs".into()));
        }
        let network = decode_hash(d)?;
        let params = decode_params(d, self.inputs.one_vintage(), self.params.technology)?;
        let trade = decode_trade(d, &self.inputs.goods)?;
        let ledger = Ledger::decode(d)?;
        let markets = Markets::decode(d, &self.inputs.goods)?;
        let pipeline = Pipeline::decode(d, ng)?;
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
        expect_len("price index", price_index.len(), np * ng)?;
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

        let mut restored = EconWorld {
            inputs: Arc::clone(&self.inputs),
            kernel: Arc::clone(&self.kernel),
            params: self.params.clone(),
            trade: self.trade.clone(),
            layout: Arc::clone(&self.layout),
            digest: self.digest,
            base_digest: self.base_digest,
            network_digest: self.network_digest,
            networks: Arc::clone(&self.networks),
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
            scratch: Scratch::new(nc, np, n_pops, n_ent, &inp.goods),
        };
        if network != self.network_digest || params != self.params || trade != self.trade {
            let inputs = if network == self.network_digest {
                Arc::clone(&self.inputs)
            } else {
                let (_, net) = self
                    .networks
                    .iter()
                    .find(|(d, _)| *d == network)
                    .ok_or_else(|| {
                        DecodeError("snapshot names a network this run has not had".into())
                    })?;
                let mut inputs = (*self.inputs).clone();
                inputs.network = (**net).clone();
                Arc::new(inputs)
            };
            restored.configure(inputs, network, params, trade);
        }
        Ok(restored)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::small_world;
    use crate::inputs::MarketElasticity;
    use ceteris_core::snapshot::{canonical, decode_canonical_with};
    #[allow(unused_imports)]
    use ceteris_trade::goods::p2::{N as N_GOODS, TRADED as N_TRADED};

    /// Trade SPEC, Invariants, Dispatch mass: the x10 stop (block pack, tick 55, cell 33982)
    /// differs by 1.3e-9 of its sales, far beyond 1e-12 and the 2^-1022 floor, and passes with
    /// the rounding the kernel carries for it (here twice the gap); without it, it still fails,
    /// as does a real mismatch of 1e-6 at normal flows.
    #[test]
    fn dispatch_tolerance_covers_the_carried_rounding() {
        let (sent, sold): (f64, f64) = (1.4717616895918719e-297, 1.4717616915516175e-297);
        let gap = (sent - sold).abs();
        assert!(gap > DISPATCH_TOLERANCE * sold + f64::MIN_POSITIVE);
        assert!(gap <= dispatch_tolerance(sent, sold, 2.0 * gap));
        assert!(gap > dispatch_tolerance(sent, sold, 0.0));
        let (sent, sold): (f64, f64) = (1.0, 1.000001);
        assert!((sent - sold).abs() > dispatch_tolerance(sent, sold, 0.0));
        assert!((sent - sold).abs() > dispatch_tolerance(sent, sold, 1e-300));
    }

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
        // Gamma P_d reproduces 1 / alpha_d to two roundings, within about f64::EPSILON
        // relative; twice that allowed (ADR 0200, As built)
        let base = inp.provinces.base_price.as_flat();
        for (cell, (&q, &b)) in w.price_index.iter().zip(base).enumerate() {
            assert!(
                q == b || (q - b).abs() <= 2.0 * f64::EPSILON * b,
                "cell {cell}: {q} vs {b}"
            );
        }
        assert!(
            inp.provinces
                .base_price
                .iter()
                .any(|q| q.iter().any(|&x| x > 1.0))
        );
        for e in 0..inp.n_enterprises() {
            let i = inp.enterprises.province[e] as usize;
            let (a, coef) = (inp.country_column(e), &inp.enterprises.coef[e]);
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
        let (omega, io) = (c.omega[0].to_vec(), c.io[0].to_vec());
        c.omega.push(&omega);
        c.io.push(&io);
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
            .map(|a| w.ledger.money_wide(AccountId(a)))
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
        assert_eq!(acc.transit.len(), N_TRADED);
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
        // origin inventories at coverage times own base output; local goods and cells
        // without a market hold none
        for i in 0..25 {
            for g in 0..N_GOODS {
                let traded = g < N_TRADED;
                let cell = i * N_GOODS + g;
                let held = w.ledger.goods(w.layout.accounts.market[i], g);
                assert_eq!(held, w.markets.inventory[cell]);
                assert_eq!(held > 0.0, traded && w.markets.active[cell], "cell {cell}");
            }
        }
        for e in 0..inp.n_enterprises() {
            let (i, g) = (
                inp.enterprises.province[e] as usize,
                usize::from(inp.enterprises.good[e]),
            );
            if g < N_TRADED {
                let y0 = inp.enterprises.a[e] * inp.enterprises.va0[e] / w.layout.a_va[e];
                let held = w.markets.inventory[i * N_GOODS + g];
                let want = w.trade.coverage[g] * y0;
                assert!(
                    (held - want).abs() <= 1e-12 * want,
                    "{i} {g}: {held} {want}"
                );
            }
        }
        // nothing in flight; the price index starts at the data state's prices
        assert!((0..N_GOODS).all(|g| w.pipeline.in_flight(g) == 0.0));
        for i in 0..25 {
            for g in 0..N_GOODS {
                let traded = g < N_TRADED;
                let p = w.price_index[i * N_GOODS + g];
                if traded {
                    // Gamma P_d at prices 1 is 1 / alpha_d there (ADR 0200): a mean of delivered
                    // unit costs, each at least the price 1
                    assert!(p.is_finite() && p >= 1.0, "{i} {g}: {p}");
                } else {
                    assert_eq!(p, 1.0);
                }
            }
        }
    }

    /// Econ SPEC, Initial state (merchant stocks of the data state): a one-province economy
    /// of Pitcairn's scale, the fixture's island at 1e-4 of its population and value added,
    /// gets no windfall at tick 1. Its pops may hold at most their money before, one month of
    /// its base value added, and the value of its whole stock and a month's output at the
    /// step-clipped price. When merchants stocked coverage times the world's orders at prices 1
    /// they held about 5,850 times their money.
    #[test]
    fn a_tiny_economy_has_no_tick_one_windfall() {
        const ISLAND: u32 = 24;
        const SCALE: f64 = 1e-4;
        let mut inp = small_world();
        for k in 0..inp.n_pops() {
            if inp.pops.province[k] == ISLAND {
                inp.pops.population[k] *= SCALE;
                inp.pops.labour_force[k] *= SCALE;
            }
        }
        inp.provinces.population[ISLAND as usize] *= SCALE;
        for e in 0..inp.n_enterprises() {
            if inp.enterprises.province[e] == ISLAND {
                inp.enterprises.va0[e] *= SCALE;
                inp.enterprises.va_target[e] *= SCALE;
            }
        }
        inp.set_base_point(&EconParams::default().edu_eff);
        let inp = Arc::new(inp);
        let trade = TradeParams::default();
        let c = inp.country_of(ISLAND as usize);
        for params in [EconParams::default(), EconParams::frictionless()] {
            let mut sim = EconWorld::sim(Arc::clone(&inp), params, trade.clone());
            let money = |w: &EconWorld| -> f64 {
                let lay = &w.layout;
                lay.country_pops[c]
                    .iter()
                    .map(|&k| to_numeraire(w.ledger.money(lay.accounts.pop[k as usize])))
                    .sum()
            };
            let before = money(sim.world());
            let lay = Arc::clone(&sim.world().layout);
            let (mut va, mut traded) = (0.0, 0.0);
            for e in lay.enterprises(ISLAND as usize) {
                va += inp.enterprises.va0[e];
                if usize::from(inp.enterprises.good[e]) < N_TRADED {
                    traded += inp.enterprises.a[e] * inp.enterprises.va0[e] / lay.a_va[e];
                }
            }
            let cover = trade.coverage.iter().copied().fold(0.0, f64::max);
            let clip = (0..N_GOODS)
                .map(|g| trade.price_clip(g))
                .fold(0.0, f64::max);
            let bound = before + va + (1.0 + cover) * libm::exp(clip) * traded;
            sim.step().unwrap();
            let after = money(sim.world());
            assert!(
                after <= bound,
                "island pops hold {after:.4e} after tick 1 against {before:.4e} before, \
                 bound {bound:.4e}"
            );
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
            for g in 0..N_TRADED {
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

    /// Parameters are snapshot state: a world under other parameters restores a snapshot
    /// under the snapshot's, its kernel and layout rebuilt.
    #[test]
    fn restore_takes_the_snapshots_parameters() {
        let w = world();
        let bytes = canonical(0, &w);
        let others = [
            (
                EconParams {
                    write_off_months: 6.0,
                    ..EconParams::default()
                },
                TradeParams::default(),
            ),
            (
                EconParams::default(),
                TradeParams {
                    c_max_storable: 0.2,
                    ..TradeParams::default()
                },
            ),
            (
                EconParams::default(),
                TradeParams {
                    theta: vec![6.0; N_GOODS],
                    ..TradeParams::default()
                },
            ),
            (EconParams::frictionless(), TradeParams::default()),
        ];
        for (k, (params, trade)) in others.into_iter().enumerate() {
            let other = EconWorld::new(Arc::new(small_world()), params, trade);
            let (_, back) = decode_canonical_with(&bytes, |d| other.restore(d)).unwrap();
            assert_eq!(back.params, w.params, "world {k}");
            assert_eq!(back.trade, w.trade, "world {k}");
            assert_eq!(back.digest(), w.digest(), "world {k}");
            assert_eq!(back.layout.price_gain, w.layout.price_gain, "world {k}");
            assert_eq!(back.layout.ent_loss_cap, w.layout.ent_loss_cap, "world {k}");
            assert_eq!(canonical(0, &back), bytes, "world {k}");
        }
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
        let err = decode_canonical_with(&bytes, |d| other.restore(d)).unwrap_err();
        assert!(err.0.contains("other inputs"), "{err}");
    }

    /// A network the restoring run has never had cannot be named back.
    #[test]
    fn restore_rejects_a_network_the_run_has_not_had() {
        let w = world();
        let bytes = canonical(0, &w);
        let mut inputs = small_world();
        inputs.network.crossings[0].hours += 1.0;
        let other = EconWorld::new(
            Arc::new(inputs),
            EconParams::default(),
            TradeParams::default(),
        );
        assert_eq!(other.base_digest, w.base_digest);
        let err = decode_canonical_with(&bytes, |d| other.restore(d)).unwrap_err();
        assert!(err.0.contains("network"), "{err}");
    }

    /// ADR 0263: J = E + eta eps / (2 - eta) at the run's eta caps the gain of the markets
    /// above J_ref = 2 m / kappa1 (20 at the defaults); inputs without a calibration run kappa1
    /// everywhere, and P03d reaches the static digest only with a calibration.
    #[test]
    fn gain_per_cell_follows_eta() {
        let trade = TradeParams::default();
        let mut inputs = small_world();
        let n = inputs.n_provinces() * N_GOODS;
        for params in [EconParams::default(), EconParams::frictionless()] {
            assert!(
                price_gains(&inputs, &params, &trade)
                    .iter()
                    .all(|&k| k == trade.kappa1)
            );
        }
        let other_share = TradeParams {
            gain_share: 0.25,
            ..trade.clone()
        };
        let digest =
            |inp: &EconInputs, t: &TradeParams| static_digest(inp, &EconParams::default(), t);
        assert_eq!(digest(&inputs, &trade), digest(&inputs, &other_share));

        // cell 0: E 11.57, eps 30.4 (J 16.92 default, 41.97 frictionless); cell 1: E 25, eps 0
        let (mut demand, mut supply) = (vec![1.0; n], vec![0.0; n]);
        (demand[0], supply[0], demand[1]) = (11.57, 30.4, 25.0);
        inputs.market_elasticity = Some(MarketElasticity {
            demand,
            supply,
            limit: [vec![f64::INFINITY; n], vec![f64::INFINITY; n]],
        });
        assert_ne!(digest(&inputs, &trade), digest(&inputs, &other_share));
        let default = price_gains(&inputs, &EconParams::default(), &trade);
        let frictionless = price_gains(&inputs, &EconParams::frictionless(), &trade);
        assert_eq!(default[0], trade.kappa1);
        assert!((frictionless[0] - 1.0 / (11.57 + 30.4)).abs() < 1e-16);
        for gains in [&default, &frictionless] {
            assert!((gains[1] - 1.0 / 25.0).abs() < 1e-16);
            assert!(gains[2..].iter().all(|&k| k == trade.kappa1));
        }
        let mut w = EconWorld::new(Arc::new(inputs), EconParams::frictionless(), trade.clone());
        assert_eq!(w.layout().price_gain, frictionless);
        let mut net = w.inputs().network.clone();
        for l in &mut net.crossings {
            l.cost_per_t *= 4.0;
        }
        w.set_network(net);
        assert_eq!(w.layout().price_gain, frictionless);
    }

    /// ADR 0267: each cell's gain is the per-market cap at the run's settings, capped at the
    /// share P03d of its block's limit gain under the run's wage regime: at the defaults the
    /// limit's share, and continuous in kappa1, P03d and eta around them, so a kappa1 just off
    /// the default keeps the block cap.
    #[test]
    fn block_limit_caps_the_gain_continuously() {
        let trade = TradeParams::default();
        let mut inputs = small_world();
        let n = inputs.n_provinces() * N_GOODS;
        // cell 0: J 16.92 default (uncapped), 41.97 frictionless (capped at 1 / 41.97)
        let (mut demand, mut supply) = (vec![1.0; n], vec![0.0; n]);
        (demand[0], supply[0]) = (11.57, 30.4);
        let (mut default, mut frictionless) = (vec![f64::INFINITY; n], vec![f64::INFINITY; n]);
        // limits of blocks scaled by 0.5 (default) and 0.5, 0.25 (frictionless)
        default[0] = 0.5 * trade.kappa1 / trade.gain_share;
        frictionless[0] = 0.5 * trade.price_gain(11.57 + 30.4) / trade.gain_share;
        frictionless[1] = 0.25 * trade.kappa1 / trade.gain_share;
        inputs.market_elasticity = Some(MarketElasticity {
            demand,
            supply,
            limit: [default, frictionless],
        });
        let gain = |params: &EconParams, trade: &TradeParams| price_gains(&inputs, params, trade);
        let at_default = gain(&EconParams::default(), &trade);
        assert_eq!(at_default[0], 0.5 * trade.kappa1);
        assert!(at_default[1..].iter().all(|&k| k == trade.kappa1));
        let at_frictionless = gain(&EconParams::frictionless(), &trade);
        assert!((at_frictionless[0] - 0.5 / (11.57 + 30.4)).abs() < 1e-16);
        assert_eq!(at_frictionless[1], 0.25 * trade.kappa1);
        // just off the default kappa1 the gains move by the relative change of kappa1 at most
        let near = TradeParams {
            kappa1: 0.0500000001,
            ..trade.clone()
        };
        let moved = near.kappa1 / trade.kappa1 - 1.0;
        for params in [EconParams::default(), EconParams::frictionless()] {
            let (a, b) = (gain(&params, &trade), gain(&params, &near));
            for (x, y) in a.iter().zip(&b) {
                assert!((x - y).abs() <= 1.01 * moved * x, "{x} vs {y}");
            }
        }
        // a larger kappa1 raises the unblocked cells only; a larger P03d raises the cap with it
        let fast = TradeParams {
            kappa1: 0.1,
            ..trade.clone()
        };
        let g = gain(&EconParams::default(), &fast);
        assert_eq!((g[0], g[1]), (0.5 * trade.kappa1, fast.kappa1));
        let wide = TradeParams {
            gain_share: 0.75,
            ..trade.clone()
        };
        let g = gain(&EconParams::default(), &wide);
        assert!((g[0] - 0.75 * trade.kappa1).abs() < 1e-16);
        // at another eta the per-market cap moves with eta and the block cap stays
        let other_eta = EconParams {
            eta: 0.5,
            ..EconParams::default()
        };
        let g = gain(&other_eta, &trade);
        let per_market = trade.price_gain(11.57 + 0.5 * 30.4 / 1.5);
        assert_eq!(g[0], per_market.min(0.5 * trade.kappa1));
        let digest = |inp: &EconInputs| inp.digest();
        let mut unscaled = inputs.clone();
        unscaled.market_elasticity.as_mut().unwrap().limit =
            [vec![f64::INFINITY; n], vec![f64::INFINITY; n]];
        assert_ne!(digest(&inputs), digest(&unscaled));
    }

    /// A storage loss without speculation is an untargeted stock that spoils (ADR 0091; trade
    /// SPEC, Competitive storage): refused.
    #[test]
    #[should_panic(expected = "no speculation intensity")]
    fn storage_loss_without_speculation_is_refused() {
        let mut trade = TradeParams::default();
        trade.storage_loss[0] = 0.003;
        EconWorld::new(Arc::new(small_world()), EconParams::default(), trade);
    }

    /// A data-state start keeps the calibration's price gains, whose block limits were
    /// computed at its storage parameters (ADR 0267): a speculation intensity of its own is
    /// refused.
    #[test]
    #[should_panic(expected = "market elasticities were computed under the default structural")]
    fn data_state_start_refuses_elasticities_of_other_storage_parameters() {
        let mut inputs = small_world();
        let n = inputs.n_provinces() * N_GOODS;
        inputs.market_elasticity = Some(MarketElasticity {
            demand: vec![1.0; n],
            supply: vec![0.0; n],
            limit: [vec![f64::INFINITY; n], vec![f64::INFINITY; n]],
        });
        let mut trade = TradeParams::default();
        trade.speculation[5] = 5.714;
        EconWorld::new(Arc::new(inputs), EconParams::default(), trade);
    }

    #[test]
    #[should_panic(expected = "market elasticities were computed under the default structural")]
    fn data_state_start_refuses_elasticities_of_other_structural_parameters() {
        let mut inputs = small_world();
        let n = inputs.n_provinces() * N_GOODS;
        inputs.market_elasticity = Some(MarketElasticity {
            demand: vec![1.0; n],
            supply: vec![0.0; n],
            limit: [vec![f64::INFINITY; n], vec![f64::INFINITY; n]],
        });
        let params = EconParams {
            sigma_demand: EconParams::default().sigma_demand + 0.5,
            ..EconParams::default()
        };
        EconWorld::new(Arc::new(inputs), params, TradeParams::default());
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
        let loss = w.kernel().good(6).crossings.row(6)[0].inv_loss;
        let base = Kernel::build(&net, &w.inputs().weight, &w.inputs().wedges, w.trade());
        assert_eq!(loss, base.good(6).crossings.row(6)[0].inv_loss);
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
