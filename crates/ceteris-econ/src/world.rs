//! The P2 world: static inputs, the built kernel and both parameter sets outside snapshots,
//! and the dynamic state that snapshots carry (`specs/econ/SPEC.md`, State components;
//! `specs/trade/SPEC.md`, State components).

use std::ops::Range;
use std::sync::{Arc, Mutex};

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
use ceteris_trade::source::{INTERMEDIATE, arrivals, dispatch, source_weighted};
use ceteris_trade::{
    Goods, Kernel, Markets, Network, Pipeline, PriceAnchor, Rationing, Sourced, TradeParams,
};

use crate::calibration::Steady;
use crate::demand::{
    PiglAt, PiglWeights, PopAt, fixed_shares, invert_pigl_pops, pigl, within_sigma,
};
use crate::disease::{Coverage, Disease};
use crate::flows::FlowWindow;
use crate::inputs::{EconInputs, network_digest};
use crate::migration::Migration;
use crate::params::{EconParams, Pigl, Technology, Transfers};
use crate::pop::{Cohorts, Demography};
use crate::settlement::Settlement;
use crate::wagegrid::WageGrid;

/// Minor units per numeraire unit: 1 minor unit = 1e-4 numeraire.
/// Buyers per block of the good-major spending arrays (`Scratch::pop_spend_t`).
pub(crate) const SPEND_BLOCK: usize = 512;

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
pub const TRANSFER: Flow = Flow(10);
pub const FOREIGN_DIVIDEND: Flow = Flow(11);
/// Fiscal Flows (ADR 0360): taxes to, transfers and interest from the government account.
pub const TAX: Flow = Flow(12);
pub const GOV_TRANSFER: Flow = Flow(13);
pub const GOV_INTEREST: Flow = Flow(14);

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
    /// The world transfer account of ADR 0100: takes the paying countries' transfers and pays
    /// the receiving countries', ending every tick at zero. Opened only where S22 is on and
    /// some transfer share is nonzero, after the mint, so a world without transfers keeps its
    /// ledger.
    pub transfer: Option<AccountId>,
    /// The world dividend account of ADR 0310: takes the foreign-owned part of each payout and
    /// pays it to the owners' profit pools, ending every tick at zero. Opened only where some
    /// foreign-owned share is nonzero, after the transfer account, so a world without foreign
    /// owners keeps its ledger.
    pub foreign: Option<AccountId>,
    /// Per country: the government account of the fiscal system (ADR 0359), opened in every
    /// world after the foreign account; zero and untouched with the fiscal system off, negative
    /// by the debt when it borrows (ADR 0363).
    pub government: Vec<AccountId>,
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
    /// Pop's share N_k / N_c of its country's rebate (national ownership, owner 2026-10-25):
    /// its population over its country's pops' population.
    pub ownership: Vec<f64>,
    /// Pop's share of its country's profit pool: `ownership`, or by wealth (ADR 0104,
    /// [`EconInputs::pop_dividend_shares`]).
    pub dividend: Vec<f64>,
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
    /// load over its pops at the data state's prices ([`household_preferences`]).
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
            dividend: inp.pop_dividend_shares(),
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
        let w = &self.pigl[c];
        // households spend their share by PIGL, government and investment theirs at fixed
        // shares (ADR 0099); without the split the household share is exactly 1
        let household = if w.gov.is_empty() {
            money
        } else {
            w.household_share() * money
        };
        pigl(
            w,
            price,
            household / people,
            self.sigma_income[c],
            params,
            out,
        );
        for x in out.iter_mut() {
            *x *= people;
        }
        if !w.gov.is_empty() {
            fixed_shares(&w.gov, price, money, out);
            fixed_shares(&w.inv, price, money, out);
        }
    }

    /// The price-only part of PIGL demand for the pops of province `i` at `price`
    /// ([`Layout::pop_demand_at`]).
    pub fn pigl_at(
        &self,
        inp: &EconInputs,
        params: &EconParams,
        i: usize,
        price: &[f64],
    ) -> PiglAt {
        PiglAt::new(&self.pigl[inp.country_of(i)], price, params)
    }

    /// [`Layout::pop_demand`] from its province's [`Layout::pigl_at`] at the same prices.
    #[allow(clippy::too_many_arguments)]
    pub fn pop_demand_at(
        &self,
        inp: &EconInputs,
        params: &EconParams,
        k: usize,
        at: &PiglAt,
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
        let w = &self.pigl[c];
        let household = if w.gov.is_empty() {
            money
        } else {
            w.household_share() * money
        };
        at.demand(
            w,
            price,
            household / people,
            self.sigma_income[c],
            params,
            out,
        );
        for x in out.iter_mut() {
            *x *= people;
        }
        if !w.gov.is_empty() {
            fixed_shares(&w.gov, price, money, out);
            fixed_shares(&w.inv, price, money, out);
        }
    }
}

/// Per-tick evidence and buffers of the systems. Written within a tick before it is read,
/// so it is not part of snapshots; a restored world starts with it zeroed.
#[derive(Clone, Debug)]
pub struct Scratch {
    /// Channel recording and shutdown ([`crate::channels`]); configuration, not state.
    pub channels: crate::channels::Channels,
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
    /// `pop_spend` and `ent_spend` regrouped by blocks of `SPEND_BLOCK` buyers, good-major
    /// inside a block (`[block][good][buyer % SPEND_BLOCK]`), so one good's buyers read
    /// sequentially in T2b; filled by `dispatch_gather`.
    pub pop_spend_t: Vec<f64>,
    pub ent_spend_t: Vec<f64>,
    /// Spending by class `[intermediate, household]` per destination cell, the sums of
    /// `ent_spend` and `pop_spend` over the province in ID order, given to `source` at T2.
    pub class_spend: Vec<[f64; 2]>,
    /// Wages, dividends and rebates paid to each pop this tick, minor units.
    pub income_paid: Vec<i64>,
    /// Fill ratios `[intermediate, household]` per origin cell written by `Markets::clear`.
    pub fill: Vec<[f64; 2]>,
    /// Local output perished per cell at T3.
    pub spoiled: Vec<f64>,
    /// Pipeline arrivals at T1, `[g * n_buyers + buyer]` (pops, then enterprises).
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
    /// Whether T2b journals every leg (detail) or one netted entry per buyer and network leg
    /// (RFC 0316). Balances and hashes are the same.
    pub detail: bool,
    /// T2b's netted entries, and the sums over goods they come from.
    pub netted: Vec<Entry>,
    pub buyer_tot: Vec<i64>,
    pub edge_tot: Vec<i64>,
    /// The price refresh of E3 when [`EconWorld::start_refresh`] began it early.
    pub refresh: Refresh,
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
    /// Per country, this tick's multipliers of A and of L0 from the initial drift
    /// (`systems::drift`, ADR 0103), set at E0; 1 until then.
    pub drift: Vec<[f64; 2]>,
}

impl Scratch {
    fn new(nc: usize, np: usize, n_pops: usize, n_ent: usize, goods: &Goods) -> Self {
        let (ng, n_traded) = (goods.n(), goods.traded);
        Self {
            channels: Default::default(),
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
            pop_spend_t: vec![0.0; n_pops.div_ceil(SPEND_BLOCK) * SPEND_BLOCK * ng],
            ent_spend_t: vec![0.0; n_ent.div_ceil(SPEND_BLOCK) * SPEND_BLOCK * ng],
            class_spend: vec![[0.0; 2]; np * ng],
            income_paid: vec![0; n_pops],
            fill: vec![[1.0; 2]; np * ng],
            spoiled: vec![0.0; np * ng],
            due: vec![0.0; (n_pops + n_ent) * ng],
            sourced: (0..n_traded).map(|_| Sourced::new()).collect(),
            columns: vec![Columns::default(); n_traded],
            payments: vec![Payments::default(); n_traded],
            post: PostScratch::default(),
            detail: false,
            netted: Vec::new(),
            buyer_tot: Vec::new(),
            edge_tot: Vec::new(),
            stock: Vec::new(),
            refresh: Refresh::default(),
            local: Vec::new(),
            local_entries: Vec::new(),
            lead: vec![0.0; np * ng],
            flows: FlowWindow::new(nc, np, ng, n_traded),
            dispatch_made: vec![0.0; n_traded],
            dispatch_due: vec![0.0; n_traded],
            dispatch_scale: vec![0.0; n_traded],
            short: vec![0; n_ent],
            notional: vec![0.0; n_ent],
            drift: vec![[1.0; 2]; nc],
        }
    }
}

/// One traded good's columns over provinces, gathered from the `[province * ng + g]`
/// arrays so that each good's sourcing and dispatch run on their own: posted prices, active
/// origins, spending by class, fill ratios, and the buyer price and lead of the price refresh.
#[derive(Clone, Debug, Default)]
pub struct Columns {
    pub price: Vec<f64>,
    /// The prices of the origin shares under sticky sourcing (S12), empty when instant.
    pub weight_price: Vec<f64>,
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

    /// The smoothed prices of good `g`, or none for an empty `weight`.
    pub fn gather_weight(&mut self, weight: &[f64], g: usize, n_goods: usize) {
        self.weight_price.clear();
        self.weight_price
            .extend(weight.iter().skip(g).step_by(n_goods));
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
    /// Netted mode: the good's rounded payment per buyer index (pops, then enterprises) and per
    /// network leg in the fixed leg order, zero where none; `topo` names the legs of the first
    /// good ([`Payments::entries`] holds the same amounts one by one in detail mode).
    pub buyer_amt: Vec<i64>,
    pub edge_amt: Vec<i64>,
    pub topo: Vec<(Flow, AccountId, AccountId)>,
    pub buyers: Vec<Buyer>,
    pub at_gateway: Vec<i64>,
    pub from_gateway: Vec<i64>,
    pub parts: Vec<i64>,
    /// T2b's pipeline shipments of the good, (slot, buyer, units) in order, applied after
    /// the per-good tasks, and the goods its selection created.
    pub pushes: Vec<ceteris_trade::pipeline::Push>,
    pub made: f64,
}

/// What a price refresh reads and writes: the traded goods' sourcing and columns, and the
/// index and lead it fills.
#[derive(Clone, Debug)]
struct RefreshParts {
    sourced: Vec<Sourced>,
    columns: Vec<Columns>,
    index: Vec<f64>,
    lead: Vec<f64>,
}

/// A price refresh running on the thread pool (`EconWorld::start_refresh`), its parts handed
/// back when it finishes; empty when none runs. A clone waits for it and copies its result.
#[derive(Default)]
pub struct Refresh(Option<Arc<Mutex<Option<RefreshParts>>>>);

impl Refresh {
    /// Waits for the refresh, helping the pool meanwhile, and takes its parts.
    fn wait(slot: &Mutex<Option<RefreshParts>>) -> RefreshParts {
        loop {
            if let Some(parts) = slot.lock().expect("refresh lock").take() {
                return parts;
            }
            // a worker thread runs pending jobs here, the refresh among them
            if rayon::yield_now().is_none() {
                std::thread::yield_now();
            }
        }
    }
}

impl Clone for Refresh {
    fn clone(&self) -> Self {
        match &self.0 {
            None => Self(None),
            Some(slot) => {
                let parts = Self::wait(slot);
                *slot.lock().expect("refresh lock") = Some(parts.clone());
                Self(Some(Arc::new(Mutex::new(Some(parts)))))
            }
        }
    }
}

impl std::fmt::Debug for Refresh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.is_some() {
            "Refresh(running)"
        } else {
            "Refresh"
        })
    }
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
    /// The kernel sourcing and dispatch run on: the target kernel (same `Arc`) under instant
    /// adjustment, else its crossings lagged behind the target (S12, ADR 0027). Its crossing k
    /// are dynamic state when lagged.
    pub(crate) kernel: Arc<Kernel>,
    /// The kernel of the network, tariffs and parameters now in force.
    pub(crate) target: Arc<Kernel>,
    /// Smoothed origin prices of the short-run adjustment (S12, ADR 0027), state: per cell
    /// `[province * ng + g]` of the traded goods the log `-theta ln p~` with `p~^-theta` the
    /// blend `(1 - rho) p~^-theta + rho p^-theta` of the posted prices. Empty under instant
    /// adjustment, and until the first tick.
    pub(crate) smooth: Vec<f64>,
    /// `p~` of [`EconWorld::smooth`] as the origin shares read it, with the posted price
    /// where the two agree; empty when `smooth` is. Derived, not state.
    pub(crate) weight_price: Vec<f64>,
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
    /// I0 and M0 of the money anchor (ADR 0338): the world price index and the money issued by
    /// the mint at the first repricing; `None` until then and with the anchor off.
    pub anchor_base: Option<(f64, f64)>,
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
    /// E3 package (ADR 0343): units an enterprise has consigned to its market and not been paid
    /// for; empty-valued (all 0) unless `PriceAnchor::e3`.
    pub consigned: Vec<f64>,
    /// E3 package: the chained anchor index (buyer prices, cells bought, level) of the last
    /// repricing; `None` until the first.
    pub anchor_chain: Option<AnchorChain>,
    /// Population on the monthly clock (P4, system 220): the mechanisms and the cohort cells.
    /// `None` is the status quo; the snapshot then has no population section.
    pub pop: Option<(Demography, Cohorts)>,
    /// Internal migration and admissions (P4 D1/D2, system 227); needs `pop`. `None` is off and
    /// adds nothing to the snapshot.
    pub migration: Option<Migration>,
    /// Settlement classes per province (P4 D3, system 229); needs `migration`. `None` is off and
    /// adds nothing to the snapshot.
    pub settlement: Option<Settlement>,
    /// Wages per (province, sector, skill) beside `wage` (P4 C1/C2, system 225). `None` is off
    /// and adds nothing to the snapshot; its section is tagged and last.
    pub wage_grid: Option<WageGrid>,
    /// Central banks of system 230; `None` is off.
    pub monetary: Option<crate::monetary::Policy>,
    /// Government accounts of system 232; `None` is off. Its section is tagged and after the
    /// monetary one.
    pub fiscal: Option<crate::fiscal::Government>,
    /// Energy wedges of system 233; `None` is off. Its section is tagged and after the fiscal one.
    pub energy: Option<crate::energy_policy::EnergyPolicy>,
    /// Endemic disease and its coverage levers per province (P4 B2); needs `pop`. Prevalence adds
    /// excess deaths in the demography step. `None` is off and adds nothing to the snapshot.
    pub disease: Option<(Disease, Coverage)>,
    /// Persons the last demography step created or lost beyond births less deaths; zero
    /// unless conservation broke. Not state.
    pub(crate) pop_drift: i64,
    /// Not state: see [`Scratch`].
    pub scratch: Scratch,
}

/// State of the chained anchor index of the E3 package (trade SPEC, Money-anchored rule).
#[derive(Clone, Debug, PartialEq)]
pub struct AnchorChain {
    pub price: Vec<f64>,
    pub bought: Vec<bool>,
    pub level: f64,
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
        transfers,
        drift,
        drift_half_life,
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
        rho,
        storage_loss,
        storage_cost,
        speculation,
        convenience,
        storage_gain,
        anchor,
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
    // without transfer shares all three settings run identically
    if inputs.has_transfers() {
        e.u8(*transfers as u8);
    }
    // without drifts both settings and every half-life run identically
    if inputs.has_drift() {
        e.u8(u8::from(*drift));
        if *drift {
            e.f64(*drift_half_life);
        }
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
    // off, the status quo's digest
    if let Some(a) = anchor {
        let gains = [a.kappa_r_traded, a.kappa_r_local, a.kappa_b, a.kappa_m];
        // a fifth entry marks the E3 package, so worlds without it keep their bytes
        if a.e3 {
            e.f64s(&[gains[0], gains[1], gains[2], gains[3], 1.0]);
        } else {
            e.f64s(&gains);
        }
    }
    // instant adjustment is the same dynamics at any class rate
    if trade.partial_adjustment() {
        e.f64s(rho);
    }
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
            transfers: params.transfers.calibrated(),
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

/// The profit pool of each country at the base point: sum (1 - f) (1 - s_L) VA0 over its
/// enterprises, plus its owner share of every host's foreign-owned sum f (1 - s_L) VA0 (ADR 0310).
fn base_profit_pool(inp: &EconInputs) -> Vec<f64> {
    let nc = inp.n_countries();
    let (mut pool, mut abroad) = (vec![0.0; nc], vec![0.0; nc]);
    for e in 0..inp.n_enterprises() {
        let c = inp.country_of(inp.enterprises.province[e] as usize);
        let profit = (1.0 - inp.ent_s_l(e)) * inp.enterprises.va0[e];
        let f = inp.ent_foreign(e);
        pool[c] += (1.0 - f) * profit;
        abroad[c] += f * profit;
    }
    inp.pay_abroad(&abroad, &mut pool);
    pool
}

/// Household income per province at the base point, rebates aside: wages w0 (1 - u*) L^s
/// plus its pops' dividend shares of their country's profit pool sum (1 - s_L) VA0 (national
/// ownership; by population, or by wealth, ADR 0104). As
/// `ceteris_reference::io::Inputs::base_income`.
pub fn base_income(inp: &EconInputs, params: &EconParams) -> Vec<f64> {
    let supply = inp.labour_supply(&params.edu_eff);
    let pool = base_profit_pool(inp);
    // by population as N_i / N_c, the order of the hashes before ADR 0104
    let share = if inp.pops.dividend.is_empty() {
        let (people, country_people) = pop_people(inp);
        (0..inp.n_provinces())
            .map(|i| {
                let n = country_people[inp.country_of(i)];
                if n > 0.0 { people[i] / n } else { 0.0 }
            })
            .collect()
    } else {
        let mut share = vec![0.0; inp.n_provinces()];
        for (k, &d) in inp.pops.dividend.iter().enumerate() {
            share[inp.pops.province[k] as usize] += d;
        }
        share
    };
    (0..inp.n_provinces())
        .map(|i| {
            let c = inp.country_of(i);
            let u = inp.countries.u_star[c];
            inp.provinces.w0[i] * (1.0 - u) * supply[i] + share[i] * pool[c]
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
/// effective labour, plus its dividend share of its country's base profit pool, over its
/// population; 0 without people. As `ceteris_reference::io::Inputs::pop_base_income`.
pub fn pop_base_income(inp: &EconInputs, params: &EconParams) -> Vec<f64> {
    let pool = base_profit_pool(inp);
    let dividend = inp.pop_dividend_shares();
    (0..inp.n_pops())
        .map(|k| {
            let i = inp.pops.province[k] as usize;
            let c = inp.country_of(i);
            let n = inp.pops.population[k];
            if n > 0.0 {
                let effective =
                    params.edu_eff[usize::from(inp.pops.edu[k])] * inp.pops.labour_force[k];
                let wage = inp.provinces.w0[i] * (1.0 - inp.countries.u_star[c]) * effective;
                (wage + dividend[k] * pool[c]) / n
            } else {
                0.0
            }
        })
        .collect()
}

/// Per country, the calibrated PIGL weights (`Countries::pigl`) or else the weights inverted
/// jointly over its pops at the data state's prices (1 for traded goods, p_c for local goods,
/// ADR 0098), each at its [`pop_base_income`] and weighted by its base
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
                    let split = Split::of(inp, c);
                    let at: Vec<PopAt> = members
                        .iter()
                        .zip(&people)
                        .map(|(&k, &(n, m))| PopAt {
                            price: (0..ng)
                                .map(|g| inp.data_state_price(inp.pops.province[k] as usize, g))
                                .collect(),
                            income: split.household * m,
                            spend: n * split.household * m,
                        })
                        .collect();
                    let mut w =
                        invert_pigl_pops(&split.omega, &inp.goods.category, &at, sigma, params);
                    (w.gov, w.inv) = (split.gov, split.inv);
                    w
                })
                .collect()
        }
    };
    let sigma = weights.iter().map(|w| w.sigma).collect();
    (weights, sigma)
}

/// A country's final demand split over its demanders at the data (ADR 0099): households' part
/// of the final demand shares, the spending shares of government and investment (their parts
/// over all final demand, empty without the split) and the households' share of spending.
struct Split {
    omega: Vec<f64>,
    gov: Vec<f64>,
    inv: Vec<f64>,
    household: f64,
}

impl Split {
    fn of(inp: &EconInputs, c: usize) -> Split {
        let (omega, cs) = (&inp.countries.omega[c], &inp.countries);
        if cs.gov.is_empty() {
            return Split {
                omega: omega.to_vec(),
                gov: Vec::new(),
                inv: Vec::new(),
                household: 1.0,
            };
        }
        let total: f64 = omega.iter().sum();
        let (gov, inv) = (&cs.gov[c], &cs.inv[c]);
        let hh: Vec<f64> = (0..omega.len())
            .map(|g| (omega[g] - gov[g] - inv[g]).max(0.0))
            .collect();
        let gov: Vec<f64> = gov.iter().map(|x| x / total).collect();
        let inv: Vec<f64> = inv.iter().map(|x| x / total).collect();
        Split {
            // as `PiglWeights::household_share`, so the run spends what was inverted
            household: 1.0 - gov.iter().sum::<f64>() - inv.iter().sum::<f64>(),
            omega: hh,
            gov,
            inv,
        }
    }
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

/// Whether a world of `inp` under `params` pays transfers (ADR 0100): S22 on and some share
/// nonzero.
pub fn transfers_on(inp: &EconInputs, params: &EconParams) -> bool {
    params.transfers != Transfers::Off && inp.has_transfers()
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
    weight_price: &[f64],
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
            col.gather_weight(weight_price, g, ng);
            let wp = (!weight_price.is_empty()).then_some(&col.weight_price[..]);
            source_weighted(
                kernel,
                g,
                &col.price,
                wp,
                &col.active,
                &gamma[g],
                &spend,
                unit,
            );
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
/// 0, Gamma P_d, is the base price. Local goods: the country's price level p_c (ADR 0098) and 1.
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
        &[],
        &active,
        &ones,
        &mut sourced,
        &mut columns,
        &mut index,
        &mut lead,
    );
    let (mut base, mut gamma) = (Rows::filled(np, ng, 1.0), Rows::filled(np, ng, 1.0));
    for d in 0..np {
        for g in nt..ng {
            base[d][g] = inp.data_state_price(d, g);
        }
    }
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
    /// inputs carry one, else the 2000 data state: prices 1 (p_c for local goods, ADR 0098),
    /// base wages w0 and employment L0 from the inputs, unemployment at the natural rate u*, input stocks at one month of base
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
        let kernel = Arc::new(Kernel::build(
            &inputs.network,
            &inputs.weight,
            &inputs.wedges,
            &trade,
        ));
        Self::with_kernel(inputs, params, trade, kernel)
    }

    /// Switches population on: `cohorts` in province order, evolving under `demography` at
    /// system 220.
    pub fn with_population(mut self, demography: Demography, cohorts: Cohorts) -> EconWorld {
        assert_eq!(
            cohorts.n_provinces(),
            self.inputs.n_provinces(),
            "one cohort block per province"
        );
        self.pop = Some((demography, cohorts));
        self
    }

    /// Switches migration (system 227) on; requires population.
    pub fn with_migration(mut self, migration: Migration) -> EconWorld {
        let np = self.inputs.n_provinces();
        assert!(self.pop.is_some(), "migration needs population");
        assert_eq!(migration.v.len(), np, "one value per province");
        assert_eq!(migration.accounts.len(), self.inputs.n_countries());
        self.migration = Some(migration);
        self
    }

    /// Switches endemic disease on, with `coverage` per province held until edited; requires
    /// population, whose snapshot section it follows.
    pub fn with_disease(mut self, disease: Disease, coverage: Coverage) -> EconWorld {
        let n = self.inputs.n_provinces();
        assert!(self.pop.is_some(), "disease needs population");
        assert_eq!(disease.n_provinces(), n);
        assert_eq!(coverage.len(), n);
        self.disease = Some((disease, coverage));
        self
    }

    /// Switches settlement classes (system 229) on; requires population and migration, whose
    /// snapshot section it follows.
    pub fn with_settlement(mut self, settlement: Settlement) -> EconWorld {
        assert!(self.migration.is_some(), "settlement needs migration");
        assert_eq!(settlement.n_provinces(), self.inputs.n_provinces());
        self.settlement = Some(settlement);
        self
    }

    /// Switches the wage grid (system 225) on; it needs no other section.
    pub fn with_wage_grid(mut self, grid: WageGrid) -> EconWorld {
        assert_eq!(grid.n_provinces(), self.inputs.n_provinces());
        self.wage_grid = Some(grid);
        self
    }

    /// Switches monetary policy (system 230) on, one bank per country.
    pub fn with_monetary(mut self, policy: crate::monetary::Policy) -> EconWorld {
        assert_eq!(policy.banks.len(), self.inputs.n_countries());
        self.monetary = Some(policy);
        self
    }

    /// Switches the energy wedges (system 233) on, one set per country.
    pub fn with_energy_policy(mut self, policy: crate::energy_policy::EnergyPolicy) -> EconWorld {
        assert_eq!(policy.countries.len(), self.inputs.n_countries());
        assert!(policy.goods.iter().all(|&g| g < self.inputs.n_goods()));
        self.energy = Some(policy);
        self
    }

    /// Switches the government accounts (system 232) on, one per country.
    pub fn with_fiscal(mut self, government: crate::fiscal::Government) -> EconWorld {
        assert_eq!(government.fiscal.len(), self.inputs.n_countries());
        let unit = |x: f64| (0.0..=1.0).contains(&x);
        for f in &government.fiscal {
            assert!(unit(f.compliance), "fiscal compliance outside [0, 1]");
            for x in &f.instruments {
                assert!(
                    unit(x.rate) && unit(x.informality) && unit(x.capacity),
                    "fiscal rate, informality or capacity outside [0, 1]"
                );
            }
        }
        self.fiscal = Some(government);
        self
    }

    /// [`EconWorld::new`] on a kernel built from these inputs and trade parameters, which
    /// worlds of equal trade parameters share (ADR 0024).
    pub fn with_kernel(
        inputs: Arc<EconInputs>,
        params: EconParams,
        trade: TradeParams,
        kernel: Arc<Kernel>,
    ) -> EconWorld {
        inputs.validate();
        trade.check_storage().unwrap_or_else(|e| panic!("{e}"));
        let inp = &*inputs;
        let (np, nc) = (inp.n_provinces(), inp.n_countries());
        let (n_pops, n_ent) = (inp.n_pops(), inp.n_enterprises());
        let target = Arc::clone(&kernel);
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
            transfer: transfers_on(inp, &params).then(|| ledger.open_wide()),
            foreign: inp.has_foreign().then(|| ledger.open_wide()),
            government: open_wide_n(&mut ledger, nc),
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
            anchor_base: None,
            employment: inp.enterprises.l0.clone(),
            employed,
            income: vec![0.0; n_pops],
            output: vec![0.0; n_ent],
            sales: vec![0.0; n_ent],
            profit: vec![0.0; n_ent],
            consigned: vec![0.0; n_ent],
            anchor_chain: None,
            pop: None,
            migration: None,
            settlement: None,
            wage_grid: None,
            monetary: None,
            fiscal: None,
            energy: None,
            disease: None,
            pop_drift: 0,
            scratch: Scratch::new(nc, np, n_pops, n_ent, &inp.goods),
            ledger,
            markets,
            pipeline: Pipeline::new(n_pops + n_ent, ng),
            price_index: vec![0.0; np * ng],
            inputs,
            kernel,
            target,
            smooth: Vec::new(),
            weight_price: Vec::new(),
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
            let dividends = lay.dividend[k] * pool[inp.country_of(i)];
            self.ledger
                .transfer(MINT, accounts.mint, accounts.pop[k], to_minor(dividends));
        }
        // local goods at the country's price level p_c (ADR 0098), traded goods at 1
        for i in 0..inp.n_provinces() {
            for g in inp.goods.traded..ng {
                self.markets.price[i * ng + g] = inp.data_state_price(i, g);
            }
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

    /// Money issued by the mint, numeraire units (the M of the money anchor, ADR 0338).
    pub fn money_issued(&self) -> f64 {
        -to_numeraire_wide(self.ledger.money_wide(self.layout.accounts.mint))
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
        if let Some(slot) = sc.refresh.0.take() {
            let parts = Refresh::wait(&slot);
            (sc.sourced, sc.columns) = (parts.sourced, parts.columns);
            (self.price_index, sc.lead) = (parts.index, parts.lead);
            return;
        }
        delivered_prices(
            false,
            &self.kernel,
            &self.markets.price,
            &self.weight_price,
            &self.markets.active,
            &self.layout.gamma,
            &mut sc.sourced,
            &mut sc.columns,
            &mut self.price_index,
            &mut sc.lead,
        );
    }

    /// Begins E3's price refresh on the thread pool, from the posted prices and active markets,
    /// which nothing changes between T1 and E3 (T4 reprices), so it overlaps T1 to E2;
    /// [`EconWorld::refresh_price_index`] then takes its result, the same as computing it
    /// there. It writes a new index and lead: E1 still reads last tick's index.
    pub fn start_refresh(&mut self) {
        let sc = &mut self.scratch;
        // one begun and never taken (systems run singly) is dropped: prices may have moved
        if let Some(stale) = sc.refresh.0.take() {
            let parts = Refresh::wait(&stale);
            (sc.sourced, sc.columns) = (parts.sourced, parts.columns);
        }
        let slot = Arc::new(Mutex::new(None));
        sc.refresh = Refresh(Some(Arc::clone(&slot)));
        let mut parts = RefreshParts {
            sourced: std::mem::take(&mut sc.sourced),
            columns: std::mem::take(&mut sc.columns),
            index: self.price_index.clone(),
            lead: sc.lead.clone(),
        };
        let (kernel, layout) = (Arc::clone(&self.kernel), Arc::clone(&self.layout));
        let (price, active) = (self.markets.price.clone(), self.markets.active.clone());
        let weight_price = self.weight_price.clone();
        rayon::spawn(move || {
            let _span = timing::span("e3.refresh_early", timing::NONE);
            delivered_prices(
                false,
                &kernel,
                &price,
                &weight_price,
                &active,
                &layout.gamma,
                &mut parts.sourced,
                &mut parts.columns,
                &mut parts.index,
                &mut parts.lead,
            );
            *slot.lock().expect("refresh lock") = Some(parts);
        });
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
        Arc::make_mut(&mut self.target).set_crossing_lags(lag);
        if self.trade.partial_adjustment() {
            Arc::make_mut(&mut self.kernel).set_crossing_lags(lag);
        } else {
            self.kernel = Arc::clone(&self.target);
        }
    }

    /// Blends the posted prices into the smoothed origin prices (P53) and refreshes the prices
    /// the origin shares read. A class at rho 1, an inactive market and a price within 1e-12
    /// of its smoothed value read the posted price.
    fn advance_smoothing(&mut self) {
        let (ng, nt) = (self.inputs.n_goods(), self.inputs.goods.traded);
        let cells = self.markets.price.len();
        let fresh = self.smooth.len() != cells;
        if fresh {
            self.smooth = vec![0.0; cells];
        }
        for cell in 0..cells {
            let g = cell % ng;
            let p = self.markets.price[cell];
            if g >= nt || p <= 0.0 || !self.markets.active[cell] {
                continue;
            }
            let (theta, rho) = (self.kernel.theta(g), self.trade.rho_of(g));
            let now = -theta * libm::log(p);
            let held = &mut self.smooth[cell];
            *held = if fresh || rho >= 1.0 {
                now
            } else {
                let (a, b) = (libm::log(1.0 - rho) + *held, libm::log(rho) + now);
                let top = a.max(b);
                top + libm::log(libm::exp(a - top) + libm::exp(b - top))
            };
            if (*held - now).abs() < 1e-12 {
                *held = now;
            }
        }
        self.refresh_weight_prices();
    }

    /// `p~` of every cell from [`EconWorld::smooth`]; the posted price where it equals the
    /// current one in logs, for local goods and for inactive markets.
    fn refresh_weight_prices(&mut self) {
        let (ng, nt) = (self.inputs.n_goods(), self.inputs.goods.traded);
        self.weight_price.clear();
        self.weight_price.extend((0..self.smooth.len()).map(|cell| {
            let g = cell % ng;
            let p = self.markets.price[cell];
            if g >= nt || p <= 0.0 || !self.markets.active[cell] {
                return p;
            }
            let theta = self.kernel.theta(g);
            if self.smooth[cell] == -theta * libm::log(p) {
                p
            } else {
                libm::exp(-self.smooth[cell] / theta)
            }
        }));
    }

    /// E0: under partial adjustment (S12), moves the lagged crossings toward the target
    /// kernel by their classes' rates (P53, `Kernel::adjust_toward`). Instant adjustment
    /// has nothing to do.
    pub fn advance_kernel(&mut self) {
        if !self.trade.partial_adjustment() {
            self.smooth.clear();
            self.weight_price.clear();
            return;
        }
        self.advance_smoothing();
        let trade = &self.trade;
        Arc::make_mut(&mut self.kernel).adjust_toward(&self.target, |g| trade.rho_of(g));
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

    /// Replaces the applied tariffs between ticks, an ex-post policy edit: the kernel is
    /// rebuilt as in [`EconWorld::set_network`] and the digest follows.
    pub fn set_wedges(&mut self, wedges: ceteris_trade::network::Wedges) {
        let mut inputs = (*self.inputs).clone();
        inputs.wedges = wedges;
        self.configure(
            Arc::new(inputs),
            self.network_digest,
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
        let kernel_held = network == self.network_digest
            && trade == self.trade
            && self
                .scratch
                .channels
                .is_shut(crate::channels::Channel::TradeShares);
        if !kernel_held
            && (network != self.network_digest
                || trade != self.trade
                || !Arc::ptr_eq(&inputs, &self.inputs))
        {
            let target = Arc::new(Kernel::build(
                &inputs.network,
                &inputs.weight,
                &inputs.wedges,
                &trade,
            ));
            // a tariff edit leaves the lagged crossings behind their new target; any other
            // change starts the kernel at its target
            if trade.partial_adjustment() && trade == self.trade && self.kernel.same_legs(&target) {
                Arc::make_mut(&mut self.kernel).adjust_toward(&target, |_| 0.0);
            } else {
                self.kernel = Arc::clone(&target);
            }
            self.target = target;
        }
        let mut accounts = self.layout.accounts.clone();
        if accounts.transfer.is_none() && transfers_on(&inputs, &params) {
            accounts.transfer = Some(self.ledger.open_wide());
        }
        if accounts.foreign.is_none() && inputs.has_foreign() {
            accounts.foreign = Some(self.ledger.open_wide());
        }
        if params != self.params || trade != self.trade || accounts != self.layout.accounts {
            self.layout = Arc::new(Layout::build(&inputs, &params, &trade, accounts));
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

/// `params` as snapshots carry them; S09 only where the inputs have more than one vintage, S22
/// only where they have transfer shares and S23 and P153 only where they have drifts, as in
/// [`static_digest`], since without them every setting runs identically.
fn encode_params(
    e: &mut Encoder,
    params: &EconParams,
    one_vintage: bool,
    transfers_on: bool,
    has_drift: bool,
) {
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
        transfers,
        drift,
        drift_half_life,
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
    if transfers_on {
        e.u8(*transfers as u8);
    }
    if has_drift {
        e.bools(&[*drift]);
        e.f64(*drift_half_life);
    }
}

fn decode_array<const N: usize>(d: &mut Decoder, what: &str) -> Result<[f64; N], DecodeError> {
    let v = d.f64s()?;
    let n = v.len();
    v.try_into()
        .map_err(|_| DecodeError(format!("{what}: {n} entries, want {N}")))
}

/// Decodes [`encode_params`]; with one vintage S09 is `technology`, without transfer shares S22
/// is `transfers`, and without drifts S23 and P153 are `drift`, the restoring world's.
fn decode_params(
    d: &mut Decoder,
    (one_vintage, technology): (bool, Technology),
    (transfers_on, transfers): (bool, Transfers),
    (has_drift, drift): (bool, (bool, f64)),
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
        transfers: if transfers_on {
            match d.u8()? {
                0 => Transfers::Off,
                1 => Transfers::FixedShares,
                2 => Transfers::ObservedPath,
                b => return Err(DecodeError(format!("transfers {b}"))),
            }
        } else {
            transfers
        },
        drift: if has_drift {
            let [on] = decode_array_bool(d)?;
            on
        } else {
            drift.0
        },
        drift_half_life: if has_drift { d.f64()? } else { drift.1 },
    })
}

fn decode_array_bool(d: &mut Decoder) -> Result<[bool; 1], DecodeError> {
    let v = d.bools()?;
    let n = v.len();
    v.try_into()
        .map_err(|_| DecodeError(format!("switch: {n} entries, want 1")))
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
        rho,
        storage_loss,
        storage_cost,
        speculation,
        convenience,
        storage_gain,
        anchor,
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
    e.f64s(rho);
    // the snapshot is restored into a world with the same setting, so off writes nothing
    if let Some(a) = anchor {
        let gains = [a.kappa_r_traded, a.kappa_r_local, a.kappa_b, a.kappa_m];
        // a fifth entry marks the E3 package, so worlds without it keep their bytes
        if a.e3 {
            e.f64s(&[gains[0], gains[1], gains[2], gains[3], 1.0]);
        } else {
            e.f64s(&gains);
        }
    }
}

fn decode_trade(
    d: &mut Decoder,
    goods: &Goods,
    anchored: bool,
) -> Result<TradeParams, DecodeError> {
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
        rho: decode_array(d, "short-run adjustment rates")?,
        storage_loss,
        storage_cost,
        speculation,
        convenience,
        storage_gain,
        anchor: if anchored {
            let (gains, e3) = match *d.f64s()?.as_slice() {
                [a, b, c, m] => ([a, b, c, m], false),
                [a, b, c, m, _] => ([a, b, c, m], true),
                _ => return Err(DecodeError("price anchor".into())),
            };
            let [kappa_r_traded, kappa_r_local, kappa_b, kappa_m] = gains;
            Some(PriceAnchor {
                kappa_r_traded,
                kappa_r_local,
                kappa_b,
                kappa_m,
                e3,
            })
        } else {
            None
        },
    };
    trade.check_goods().map_err(DecodeError)?;
    Ok(trade)
}

impl Encode for EconWorld {
    /// The inputs' digest without the network, the network's digest, the applied tariffs
    /// (edits change them mid-run) and both parameter sets, then the dynamic state.
    fn encode(&self, e: &mut Encoder) {
        encode_hash(e, &self.base_digest);
        encode_hash(e, &self.network_digest);
        let tariff: Vec<u32> = self
            .inputs
            .wedges
            .tariff
            .iter()
            .chain(&self.inputs.wedges.ntb)
            .map(|t| t.to_bits())
            .collect();
        e.u32s(&tariff);
        encode_params(
            e,
            &self.params,
            self.inputs.one_vintage(),
            self.inputs.has_transfers(),
            self.inputs.has_drift(),
        );
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
        if self.trade.anchor.is_some() {
            match self.anchor_base {
                Some((i0, m0)) => e.f64s(&[i0, m0]),
                None => e.f64s(&[]),
            }
        }
        if self.trade.anchor.is_some_and(|a| a.e3) {
            e.f64s(&self.consigned);
            match &self.anchor_chain {
                Some(c) => {
                    e.f64s(&c.price);
                    e.bools(&c.bought);
                    e.f64s(&[c.level]);
                }
                None => {
                    e.f64s(&[]);
                    e.bools(&[]);
                    e.f64s(&[]);
                }
            }
        }
        e.u32s(&self.wage_clip_ticks);
        // the lagged crossings are state; under instant adjustment there are none
        if self.trade.partial_adjustment() {
            e.f64s(&self.kernel.crossing_k());
        } else {
            e.f64s(&[]);
        }
        e.f64s(&self.smooth);
        if let Some((demography, cohorts)) = &self.pop {
            demography.encode(cohorts, e);
            if let Some((dis, cov)) = &self.disease {
                dis.encode(cov, e);
            }
            if let Some(m) = &self.migration {
                m.encode(e);
                if let Some(s) = &self.settlement {
                    s.encode(e);
                }
            }
        }
        if let Some(g) = &self.wage_grid {
            g.encode(e);
        }
        if let Some(m) = &self.monetary {
            m.encode(e);
        }
        if let Some(g) = &self.fiscal {
            g.encode(e);
        }
        if let Some(p) = &self.energy {
            p.encode(e);
        }
    }
}

fn monetary_next(d: &Decoder) -> bool {
    d.peek_count() == Ok(crate::monetary::SNAPSHOT_TAG as usize)
}

/// Whether one of the tagged sections that close the snapshot comes next.
fn tagged_next(d: &Decoder) -> bool {
    wage_grid_next(d) || monetary_next(d) || fiscal_next(d) || energy_next(d)
}

fn fiscal_next(d: &Decoder) -> bool {
    d.peek_count() == Ok(crate::fiscal::SNAPSHOT_TAG as usize)
}

fn energy_next(d: &Decoder) -> bool {
    d.peek_count() == Ok(crate::energy_policy::SNAPSHOT_TAG as usize)
}

fn wage_grid_next(d: &Decoder) -> bool {
    d.peek_count() == Ok(crate::wagegrid::SNAPSHOT_TAG as usize)
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
    /// its recorded income, exact in minor units); and the world transfer account at zero
    /// (ADR 0100, exact).
    fn end_tick(&self, tick: u32) -> Result<(), Violation> {
        let (ng, nt) = (self.inputs.n_goods(), self.inputs.goods.traded);
        let span = timing::span("end.ledger", timing::NONE);
        self.ledger.check(tick)?;
        drop(span);
        let span = timing::span("end.columns", timing::NONE);
        let (inp, lay) = (&*self.inputs, &*self.layout);
        let fail = |invariant, detail| Err(Violation::new(tick, invariant, detail));

        if self.pop_drift != 0 {
            return fail(
                "cohort conservation",
                format!("{} persons beyond births less deaths", self.pop_drift),
            );
        }

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

        drop(span);
        let span = timing::span("end.bounds", timing::NONE);
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

        drop(span);
        let span = timing::span("end.inventory", timing::NONE);
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
        drop(span);
        let span = timing::span("end.transit", timing::NONE);
        let in_flight = self.pipeline.in_flight_cached();
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
        drop(span);
        let span = timing::span("end.gateway", timing::NONE);
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
        if let Some(a) = lay.accounts.transfer
            && self.ledger.money_wide(a) != 0
        {
            let detail = format!("world transfer account holds {}", self.ledger.money_wide(a));
            return fail("transfers", detail);
        }
        if let Some(a) = lay.accounts.foreign
            && self.ledger.money_wide(a) != 0
        {
            let detail = format!("world dividend account holds {}", self.ledger.money_wide(a));
            return fail("foreign dividends", detail);
        }

        drop(span);
        let span = timing::span("end.leontief", timing::NONE);
        let sc = &self.scratch;
        // the first failing enterprise in index order, as the serial scan reports it
        let leontief = (0..inp.n_enterprises())
            .into_par_iter()
            .find_map_first(|e| {
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
                        return Some(format!(
                            "enterprise {e} used {used} of good {k}, a Y = {want}"
                        ));
                    }
                }
                None
            });
        if let Some(detail) = leontief {
            return fail("leontief", detail);
        }

        drop(span);
        let span = timing::span("end.dispatch", timing::NONE);
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
        drop(span);
        let _span = timing::span("end.income", timing::NONE);
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
        let mut tariff: Vec<f32> = d.u32s()?.into_iter().map(f32::from_bits).collect();
        let n_tariff = self.inputs.wedges.tariff.len();
        if tariff.len() != n_tariff {
            expect_len("ntb", tariff.len(), 2 * n_tariff)?;
        }
        let ntb = tariff.split_off(n_tariff);
        let params = decode_params(
            d,
            (self.inputs.one_vintage(), self.params.technology),
            (self.inputs.has_transfers(), self.params.transfers),
            (
                self.inputs.has_drift(),
                (self.params.drift, self.params.drift_half_life),
            ),
        )?;
        let trade = decode_trade(d, &self.inputs.goods, self.trade.anchor.is_some())?;
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
        let anchor_base = if trade.anchor.is_some() {
            match d.f64s()?.as_slice() {
                [] => None,
                &[i0, m0] => Some((i0, m0)),
                _ => return Err(DecodeError("anchor base".into())),
            }
        } else {
            None
        };
        let (consigned, anchor_chain) = if trade.anchor.is_some_and(|a| a.e3) {
            let consigned = d.f64s()?;
            let (price, bought, level) = (d.f64s()?, d.bools()?, d.f64s()?);
            let chain = match level.as_slice() {
                [] => None,
                &[level] => Some(AnchorChain {
                    price,
                    bought,
                    level,
                }),
                _ => return Err(DecodeError("anchor chain".into())),
            };
            (consigned, chain)
        } else {
            (vec![0.0; sales.len()], None)
        };
        let wage_clip_ticks = d.u32s()?;
        let lagged = d.f64s()?;
        let smooth_state = d.f64s()?;
        let pop = if d.at_end() || tagged_next(d) {
            None
        } else {
            Some(Demography::decode(d)?)
        };
        let disease = if pop.is_some()
            && !d.at_end()
            && d.peek_count()? == crate::disease::SNAPSHOT_TAG as usize
        {
            Some(Disease::decode(d)?)
        } else {
            None
        };
        let migration = if pop.is_some() && !d.at_end() && !tagged_next(d) {
            Some(Migration::decode(d)?)
        } else {
            None
        };
        let settlement = if migration.is_some() && !d.at_end() && !tagged_next(d) {
            Some(Settlement::decode(d)?)
        } else {
            None
        };
        let wage_grid = if wage_grid_next(d) {
            Some(WageGrid::decode(d)?)
        } else {
            None
        };
        let monetary = if monetary_next(d) {
            Some(crate::monetary::Policy::decode(d)?)
        } else {
            None
        };
        let fiscal = if !fiscal_next(d) {
            None
        } else {
            let g = crate::fiscal::Government::decode(d)?;
            expect_len(
                "fiscal countries",
                g.fiscal.len(),
                self.inputs.n_countries(),
            )?;
            Some(g)
        };
        let energy = if energy_next(d) {
            let p = crate::energy_policy::EnergyPolicy::decode(d)?;
            expect_len(
                "energy countries",
                p.countries.len(),
                self.inputs.n_countries(),
            )?;
            Some(p)
        } else {
            None
        };

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
        if let Some((_, c)) = &pop {
            expect_len("cohort provinces", c.n_provinces(), np)?;
        }
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
            target: Arc::clone(&self.target),
            smooth: smooth_state,
            weight_price: Vec::new(),
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
            anchor_base,
            employment,
            employed,
            income,
            output,
            consigned,
            anchor_chain,
            sales,
            profit,
            pop,
            migration,
            settlement,
            wage_grid,
            monetary,
            fiscal,
            energy,
            disease,
            pop_drift: 0,
            scratch: Scratch::new(nc, np, n_pops, n_ent, &inp.goods),
        };
        let same_tariff = tariff
            .iter()
            .zip(&self.inputs.wedges.tariff)
            .all(|(a, b)| a.to_bits() == b.to_bits())
            && ntb.len() == self.inputs.wedges.ntb.len()
            && ntb
                .iter()
                .zip(&self.inputs.wedges.ntb)
                .all(|(a, b)| a.to_bits() == b.to_bits());
        if network != self.network_digest
            || !same_tariff
            || params != self.params
            || trade != self.trade
        {
            let mut inputs = (*self.inputs).clone();
            if network != self.network_digest {
                let (_, net) = self
                    .networks
                    .iter()
                    .find(|(d, _)| *d == network)
                    .ok_or_else(|| {
                        DecodeError("snapshot names a network this run has not had".into())
                    })?;
                inputs.network = (**net).clone();
            }
            inputs.wedges.tariff = tariff;
            inputs.wedges.ntb = ntb;
            let inputs = Arc::new(inputs);
            restored.configure(inputs, network, params, trade);
        }
        if restored.trade.partial_adjustment() {
            let mut kernel = (*restored.target).clone();
            if !kernel.set_crossing_k(&lagged) {
                return Err(DecodeError(
                    "lagged crossings do not fit the kernel of the snapshot".into(),
                ));
            }
            restored.kernel = Arc::new(kernel);
        } else {
            expect_len("lagged crossings", lagged.len(), 0)?;
            expect_len("smoothed prices", restored.smooth.len(), 0)?;
            restored.kernel = Arc::clone(&restored.target);
        }
        // the shares' prices are rebuilt from the smoothed state at the next tick's start
        if !restored.smooth.is_empty() {
            expect_len("smoothed prices", restored.smooth.len(), np * ng)?;
        }
        Ok(restored)
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
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
        all.extend(acc.government.iter().map(|a| a.0));
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
            let pool: f64 = (0..inp.n_enterprises())
                .filter(|&e| inp.country_of(inp.enterprises.province[e] as usize) == c)
                .map(|e| (1.0 - inp.ent_s_l(e)) * inp.enterprises.va0[e])
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

    fn population_sim() -> Sim<EconWorld> {
        let (d, c) = crate::pop::toy(world().inputs().n_provinces());
        Sim::new(
            world().with_population(d, c),
            crate::systems::schedule(),
            crate::systems::SEED,
        )
    }

    #[test]
    fn population_runs_conserved_and_changes_the_hash() {
        let mut on = population_sim();
        let before = on.world().pop.as_ref().unwrap().1.total();
        on.run_until(25).unwrap_or_else(|v| panic!("{v}"));
        let after = on.world().pop.as_ref().unwrap().1.total();
        assert_ne!(before, after);
        let mut off = Sim::new(world(), crate::systems::schedule(), crate::systems::SEED);
        off.run_until(25).unwrap_or_else(|v| panic!("{v}"));
        assert_ne!(on.state_hash(), off.state_hash());
        let mut again = population_sim();
        again.run_until(25).unwrap_or_else(|v| panic!("{v}"));
        assert_eq!(on.state_hash(), again.state_hash());
    }

    #[test]
    fn population_off_adds_nothing_to_the_snapshot() {
        let off = world();
        let on = population_sim();
        assert!(canonical(0, on.world()).len() > canonical(0, &off).len());
        // the economy's own columns are untouched by the extra system
        let mut a = Sim::new(world(), crate::systems::schedule(), crate::systems::SEED);
        a.run_until(6).unwrap_or_else(|v| panic!("{v}"));
        let mut b = population_sim();
        b.run_until(6).unwrap_or_else(|v| panic!("{v}"));
        assert_eq!(a.world().wage, b.world().wage);
        assert_eq!(a.world().income, b.world().income);
    }

    #[test]
    fn population_state_round_trips() {
        let mut sim = population_sim();
        sim.run_until(13).unwrap_or_else(|v| panic!("{v}"));
        let w = sim.world();
        let (bytes, back) = round_trip(w);
        assert_eq!(canonical(12, &back), bytes);
        assert_eq!(back.pop, w.pop);
    }

    fn disease_sim(incidence: f64) -> Sim<EconWorld> {
        use crate::disease::{N_DISEASE, tests::params};
        let mut sim = population_sim();
        let np = sim.world().inputs().n_provinces();
        let mut par = params();
        par.iter_mut().for_each(|k| k.incidence = incidence);
        let mut dis = Disease::zero(np, par);
        dis.prevalence.fill(0.02);
        let w = std::mem::replace(sim.world_mut(), world());
        *sim.world_mut() = EconWorld {
            disease: Some((dis, vec![[0.3; N_DISEASE]; np])),
            ..w
        };
        sim
    }

    /// With disease off the population run is unchanged; on, prevalence kills beyond the cause
    /// group path, conservation holds and the section survives a snapshot.
    #[test]
    fn disease_off_equals_status_quo_and_on_adds_deaths() {
        let mut off = population_sim();
        off.run_until(14).unwrap_or_else(|v| panic!("{v}"));
        let mut on = disease_sim(0.002);
        on.run_until(14).unwrap_or_else(|v| panic!("{v}"));
        let total = |s: &Sim<EconWorld>| s.world().pop.as_ref().unwrap().1.total();
        assert!(total(&on) < total(&off));
        let (dis, _) = on.world().disease.as_ref().unwrap();
        assert!(dis.prevalence.iter().any(|&x| x != 0.02));
        let (bytes, back) = round_trip(on.world());
        assert_eq!(canonical(12, &back), bytes);
        assert_eq!(back.disease, on.world().disease);

        let mut none = disease_sim(0.002);
        none.world_mut().disease = None;
        none.run_until(14).unwrap_or_else(|v| panic!("{v}"));
        assert_eq!(none.state_hash(), off.state_hash());
    }

    fn migration_sim(cap: u64) -> Sim<EconWorld> {
        let w = world();
        let (np, nc) = (w.inputs().n_provinces(), w.inputs().n_countries());
        let (d, c) = crate::pop::toy(np);
        let cost: Vec<f64> = (0..np * np)
            .map(|k| if k / np == k % np { 0.0 } else { 0.5 })
            .collect();
        let choice = crate::migration::ChoiceSet::dense(&cost, np);
        let mut m = Migration::new(choice, nc, 0.95, 0.7, 0.02);
        m.chi = 1.0;
        m.z = 0.3;
        m.accounts.iter_mut().for_each(|a| a.cap = cap);
        Sim::new(
            w.with_population(d, c).with_migration(m),
            crate::systems::schedule(),
            crate::systems::SEED,
        )
    }

    /// Migration moves persons between provinces only: with no births or deaths the total is
    /// fixed, the cells change, and the end-of-tick conservation check stays green.
    #[test]
    fn migration_conserves_persons_and_moves_cells() {
        for cap in [u64::MAX, 50] {
            let mut sim = migration_sim(cap);
            sim.world_mut().pop.as_mut().unwrap().0 = Default::default();
            let before = sim.world().pop.as_ref().unwrap().1.clone();
            sim.run_until(14).unwrap_or_else(|v| panic!("{v}"));
            let after = &sim.world().pop.as_ref().unwrap().1;
            assert_eq!(before.total(), after.total());
            assert_ne!(&before, after);
        }
    }

    #[test]
    fn migration_is_deterministic_and_round_trips() {
        let (mut a, mut b) = (migration_sim(50), migration_sim(50));
        a.run_until(14).unwrap_or_else(|v| panic!("{v}"));
        b.run_until(14).unwrap_or_else(|v| panic!("{v}"));
        assert_eq!(a.state_hash(), b.state_hash());
        let (bytes, back) = round_trip(a.world());
        assert_eq!(canonical(12, &back), bytes);
        assert_eq!(back.migration, a.world().migration);
        assert!(back.migration.unwrap().diaspora.iter().sum::<u64>() > 0);
    }

    fn settlement_sim() -> Sim<EconWorld> {
        use crate::settlement::{N_CLASS, Settlement};
        let mut sim = migration_sim(50);
        let (np, cells) = (sim.world().inputs().n_provinces(), crate::pop::CELLS);
        let counts = sim.world().pop.as_ref().unwrap().1.counts().to_vec();
        let persons = (0..np)
            .flat_map(|p| {
                let t: u64 = counts[p * cells..(p + 1) * cells].iter().sum();
                [t / 2, t / 4, t - t / 2 - t / 4]
            })
            .collect();
        let mut st = Settlement::new(persons, vec![1e6; np * N_CLASS]);
        st.enabled = true;
        st.mu_move = 0.05;
        st.agglomeration = [0.0, 0.05, 0.1];
        st.cost = [0.0, 0.1, 0.2, 0.1, 0.0, 0.1, 0.2, 0.1, 0.0];
        sim.world_mut().settlement = Some(st);
        sim
    }

    /// With settlement off (`None`) the migration run is unchanged; on, classes follow the
    /// cohort totals per province, move persons, and the section survives a snapshot.
    #[test]
    fn settlement_off_equals_status_quo_and_on_moves_classes_within_provinces() {
        let mut off = migration_sim(50);
        let mut none = settlement_sim();
        none.world_mut().settlement = None;
        off.run_until(14).unwrap_or_else(|v| panic!("{v}"));
        none.run_until(14).unwrap_or_else(|v| panic!("{v}"));
        assert_eq!(off.state_hash(), none.state_hash());

        let mut on = settlement_sim();
        let before = on.world().settlement.clone().unwrap();
        on.run_until(14).unwrap_or_else(|v| panic!("{v}"));
        let w = on.world();
        let st = w.settlement.as_ref().unwrap();
        let cohorts = &w.pop.as_ref().unwrap().1;
        assert_ne!(st.persons, before.persons);
        for p in 0..st.n_provinces() {
            let class: u64 = st.persons[p * 3..p * 3 + 3].iter().sum();
            let cell: u64 = cohorts.counts()[p * crate::pop::CELLS..(p + 1) * crate::pop::CELLS]
                .iter()
                .sum();
            assert_eq!(class, cell);
        }
        let (bytes, back) = round_trip(w);
        assert_eq!(canonical(12, &back), bytes);
        assert_eq!(back.settlement.as_ref(), Some(st));
    }

    fn wage_grid_sim(enabled: bool) -> Sim<EconWorld> {
        use crate::skills::SkillCes;
        let w = world();
        let (np, ns) = (w.inputs().n_provinces(), w.inputs().n_goods());
        let low = (0..np * ns).map(|i| 4000 + 100 * (i % 7) as u64).collect();
        let high = (0..np * ns).map(|i| 1000 + 300 * (i % 5) as u64).collect();
        let mut g = crate::wagegrid::WageGrid::new(SkillCes::new(1.6, 0.4), ns, low, high);
        g.enabled = enabled;
        g.skills.enabled = true;
        g.switching.iter_mut().for_each(|k| {
            k.enabled = true;
            k.m = 0.1;
            k.omega = 0.5;
        });
        Sim::new(
            w.with_wage_grid(g),
            crate::systems::schedule(),
            crate::systems::SEED,
        )
    }

    /// With the wage grid off (`None`) the run is the status quo; on, it leaves the labour market
    /// as it was, gives each (province, sector, skill) its own wage, moves workers between
    /// sectors, and the section survives a snapshot.
    #[test]
    fn wage_grid_off_equals_status_quo_and_on_has_per_cell_wages() {
        let mut off = Sim::new(world(), crate::systems::schedule(), crate::systems::SEED);
        let mut none = wage_grid_sim(true);
        none.world_mut().wage_grid = None;
        off.run_until(14).unwrap_or_else(|v| panic!("{v}"));
        none.run_until(14).unwrap_or_else(|v| panic!("{v}"));
        assert_eq!(off.state_hash(), none.state_hash());

        let mut on = wage_grid_sim(true);
        let before = on.world().wage_grid.clone().unwrap();
        on.run_until(14).unwrap_or_else(|v| panic!("{v}"));
        let w = on.world();
        assert_eq!(w.wage, off.world().wage);
        assert_eq!(w.employment, off.world().employment);
        let g = w.wage_grid.as_ref().unwrap();
        let ns = g.n_sectors();
        for p in 0..g.n_provinces() {
            for s in 0..ns {
                let (wl, wh) = (g.wage[(p * ns + s) * 2], g.wage[(p * ns + s) * 2 + 1]);
                assert!(wh > wl);
            }
            for k in 0..2 {
                let now: u64 = g.switching[k].workers[p * ns..(p + 1) * ns].iter().sum();
                let was: u64 = before.switching[k].workers[p * ns..(p + 1) * ns]
                    .iter()
                    .sum();
                assert_eq!(now, was);
            }
        }
        assert_ne!(g.switching[0].workers, before.switching[0].workers);
        let (bytes, back) = round_trip(w);
        assert_eq!(canonical(12, &back), bytes);
        assert_eq!(back.wage_grid.as_ref(), Some(g));
    }

    fn monetary_sim() -> Sim<EconWorld> {
        use crate::monetary::{Policy, Rule};
        let w = world();
        let rule = Rule {
            r_star: 0.02,
            a_pi: 0.5,
            a_y: 0.5,
            pi_star: 0.02,
            lower_bound: 0.0,
        };
        let nc = w.inputs().n_countries();
        Sim::new(
            w.with_monetary(Policy::floating(nc, rule)),
            crate::systems::schedule(),
            crate::systems::SEED,
        )
    }

    /// With policy off the run is the status quo; on, the real economy is untouched, each country
    /// gets a rate that moves off its start value, and the section survives a snapshot.
    #[test]
    fn monetary_off_equals_status_quo_and_on_sets_policy_rates() {
        let mut off = Sim::new(world(), crate::systems::schedule(), crate::systems::SEED);
        let mut on = monetary_sim();
        let start = on.world().monetary.clone().unwrap().rates;
        off.run_until(14).unwrap_or_else(|v| panic!("{v}"));
        on.run_until(14).unwrap_or_else(|v| panic!("{v}"));
        let w = on.world();
        assert_eq!(w.wage, off.world().wage);
        assert_eq!(w.employment, off.world().employment);
        let m = w.monetary.as_ref().unwrap();
        assert_ne!(m.rates, start);
        assert!(m.rates.iter().all(|r| r.is_finite()));
        let (bytes, back) = round_trip(w);
        assert_eq!(canonical(12, &back), bytes);
        assert_eq!(back.monetary.as_ref(), Some(m));
    }

    /// With the filter on, r* is re-estimated each year, so the policy rates differ from the
    /// fixed-intercept run while the real economy is untouched; replay and snapshot are exact.
    #[test]
    fn hlw_on_updates_r_star_annually_and_off_changes_nothing() {
        use crate::hlw::HlwParams;
        let sim = |hlw: bool| {
            let mut s = monetary_sim();
            if hlw {
                let w = s.world().clone();
                let m = w.monetary.clone().unwrap().with_hlw(
                    HlwParams {
                        a1: 1.0,
                        a2: -0.2,
                        a_r: -0.3,
                        b_pi: 0.7,
                        b_y: 0.1,
                        c: 1.0,
                        sigma_y_gap: 0.005,
                        sigma_pi: 0.005,
                        sigma_y_star: 0.004,
                        sigma_g: 0.0005,
                        sigma_z: 0.0015,
                    },
                    0.02,
                );
                s = Sim::new(
                    w.with_monetary(m),
                    crate::systems::schedule(),
                    crate::systems::SEED,
                );
            }
            s.run_until(26).unwrap_or_else(|v| panic!("{v}"));
            s
        };
        let (off, on, again) = (sim(false), sim(true), sim(true));
        let (m0, m1) = (
            off.world().monetary.as_ref().unwrap(),
            on.world().monetary.as_ref().unwrap(),
        );
        assert_eq!(on.world().wage, off.world().wage);
        let star = |m: &crate::monetary::Policy| match m.banks[0].regime {
            crate::monetary::Regime::Float(r) => r.r_star,
            _ => unreachable!(),
        };
        assert_eq!(star(m0), 0.02);
        assert_ne!(star(m1), 0.02);
        assert_ne!(m1.rates, m0.rates);
        assert_eq!(on.world().monetary, again.world().monetary);
        let (bytes, back) = round_trip(on.world());
        assert_eq!(canonical(12, &back), bytes);
        assert_eq!(back.monetary.as_ref(), Some(m1));
    }

    fn hazard_sim(scale: f64) -> Sim<EconWorld> {
        use crate::monetary::{Policy, Regime, RegimeHazards, Rule};
        use crate::regime_hazard::TransitionHazards;
        let w = world();
        let rule = Rule {
            r_star: 0.02,
            a_pi: 0.5,
            a_y: 0.5,
            pi_star: 0.02,
            lower_bound: 0.0,
        };
        let nc = w.inputs().n_countries();
        let mut policy = Policy::floating(nc, rule);
        for b in policy.banks.iter_mut().skip(1) {
            b.regime = Regime::HardPeg {
                anchor: 0,
                fallback: rule,
                defence_premium: 0.0,
            };
            b.reserves = 1.0;
        }
        // Only the move to a free float has a large intercept: every peg breaks within a few months.
        let mut beta = [[-20.0, 0.0, 0.0, 0.0, 0.0, 0.0]; 4];
        beta[3][0] = 2.0;
        let policy = policy.with_hazards(RegimeHazards {
            model: TransitionHazards { scale, beta },
            anchor: 0,
            defence_premium: 0.0,
        });
        Sim::new(
            w.with_monetary(policy),
            crate::systems::schedule(),
            crate::systems::SEED,
        )
    }

    /// Hazards at zero keep every peg; switched on, pegs break to floats, the break is recorded,
    /// the real economy is untouched, and the section survives a snapshot.
    #[test]
    fn regime_hazards_off_keep_pegs_and_on_break_them() {
        use crate::monetary::Regime;
        let mut off = hazard_sim(0.0);
        let mut on = hazard_sim(1.0);
        off.run_until(14).unwrap_or_else(|v| panic!("{v}"));
        on.run_until(14).unwrap_or_else(|v| panic!("{v}"));
        let pegs = |s: &Sim<EconWorld>| {
            s.world().monetary.as_ref().unwrap().banks[1..]
                .iter()
                .filter(|b| matches!(b.regime, Regime::HardPeg { .. }))
                .count()
        };
        assert!(pegs(&off) > 0);
        assert_eq!(pegs(&on), 0);
        let m = on.world().monetary.as_ref().unwrap();
        assert!(m.peg_break_month[1] != u32::MAX);
        assert_eq!(on.world().wage, off.world().wage);
        let w = on.world();
        let (bytes, back) = round_trip(w);
        assert_eq!(canonical(12, &back), bytes);
        assert_eq!(back.monetary, w.monetary);
    }

    fn government(nc: usize) -> crate::fiscal::Government {
        use crate::fiscal::{
            CORPORATE, Fiscal, Government, Instrument, N_REVENUE, PERSONAL_INCOME, VAT,
        };
        let mut instruments = [Instrument {
            rate: 0.0,
            base: 0,
            informality: 0.0,
            capacity: 0.0,
        }; N_REVENUE];
        for (i, rate) in [(PERSONAL_INCOME, 0.2), (VAT, 0.15), (CORPORATE, 0.25)] {
            instruments[i] = Instrument {
                rate,
                base: 0,
                informality: 0.2,
                capacity: 0.6,
            };
        }
        let fiscal = Fiscal {
            instruments,
            compliance: 0.9,
            depreciation: 0.001,
            build_rate: 0.5,
            debt: 0,
        };
        let mut capacity = [0.0; N_REVENUE];
        capacity[VAT] = 0.02;
        Government::uniform(
            nc,
            fiscal,
            [0.3, 0.2, 0.2, 0.2, 0.1, 0.05, 0.05],
            capacity,
            0.004,
        )
    }

    /// With the fiscal system off the government accounts stay at zero; on, each country collects
    /// revenue from the tick's bases, runs the 10% deficit its spending shares imply into debt
    /// that its ledger account holds as a negative balance and its balance sheet as bonds,
    /// builds VAT capacity, moves pops' money and so the real economy, and the section survives a
    /// snapshot next to the monetary one.
    #[test]
    fn fiscal_off_equals_status_quo_and_on_books_the_government_account() {
        use crate::fiscal::{CORPORATE, PERSONAL_INCOME, VAT};
        use crate::sfc::{BONDS, GOVERNMENT};
        let mut off = Sim::new(world(), crate::systems::schedule(), crate::systems::SEED);
        let w = monetary_sim().world().clone();
        let nc = w.inputs().n_countries();
        let mut on = Sim::new(
            w.with_fiscal(government(nc)),
            crate::systems::schedule(),
            crate::systems::SEED,
        );
        off.run_until(14).unwrap_or_else(|v| panic!("{v}"));
        on.run_until(14).unwrap_or_else(|v| panic!("{v}"));
        let w = on.world();
        let quo = off.world();
        for &a in &quo.layout.accounts.government {
            assert_eq!(quo.ledger.money_wide(a), 0);
        }
        let pops = |w: &EconWorld| -> Vec<i64> {
            w.layout
                .accounts
                .pop
                .iter()
                .map(|&a| w.ledger.money(a))
                .collect()
        };
        assert_ne!(pops(w), pops(quo));
        assert_ne!(w.employment, quo.employment);
        let g = w.fiscal.as_ref().unwrap();
        for ((f, sheet), &a) in g
            .fiscal
            .iter()
            .zip(&g.sheets)
            .zip(&w.layout.accounts.government)
        {
            assert_eq!(w.ledger.money_wide(a), -f.debt);
            for i in [PERSONAL_INCOME, VAT, CORPORATE] {
                assert!(f.instruments[i].base > 0, "instrument {i} has no base");
            }
            assert!(f.total_revenue() > 0);
            assert!(f.debt > 0);
            assert_eq!(sheet.positions[BONDS][GOVERNMENT], -f.debt);
            assert!(sheet.rows_close());
            assert!(f.instruments[VAT].capacity > 0.6);
            assert!(f.instruments[PERSONAL_INCOME].capacity < 0.6);
        }
        let (bytes, back) = round_trip(w);
        assert_eq!(canonical(12, &back), bytes);
        assert_eq!(back.fiscal.as_ref(), Some(g));
        assert_eq!(back.monetary, w.monetary);
    }

    /// With the energy policy off the government accounts stay at zero and the snapshot is the
    /// status quo's; on, an excise books revenue to each government account from the buyers, a
    /// subsidy books a cost, households' demand moves with the buyer price, and the section
    /// survives a snapshot.
    #[test]
    fn energy_wedges_off_equals_status_quo_and_on_books_the_government_account() {
        use crate::energy_policy::{EnergyPolicy, EnergyWedges};
        let nc = world().inputs().n_countries();
        let run = |wedge: [i128; 3]| {
            let policy = EnergyPolicy {
                goods: [1, 2, 7],
                countries: vec![EnergyWedges { wedge }; nc],
            };
            let mut sim = Sim::new(
                world().with_energy_policy(policy),
                crate::systems::schedule(),
                crate::systems::SEED,
            );
            sim.run_until(14).unwrap_or_else(|v| panic!("{v}"));
            sim
        };
        let mut off = Sim::new(world(), crate::systems::schedule(), crate::systems::SEED);
        off.run_until(14).unwrap_or_else(|v| panic!("{v}"));
        let unit = MINOR_PER_UNIT as i128 / 20;
        let excise = run([unit, 0, unit]);
        let subsidy = run([-unit, -unit, 0]);
        let gov = |w: &EconWorld| -> Vec<i128> {
            w.layout
                .accounts
                .government
                .iter()
                .map(|&a| w.ledger.money_wide(a))
                .collect()
        };
        assert!(gov(off.world()).iter().all(|&m| m == 0));
        assert!(gov(excise.world()).iter().sum::<i128>() > 0);
        assert!(gov(subsidy.world()).iter().sum::<i128>() < 0);
        let pops = |w: &EconWorld| -> Vec<i64> {
            w.layout
                .accounts
                .pop
                .iter()
                .map(|&a| w.ledger.money(a))
                .collect()
        };
        assert_ne!(pops(excise.world()), pops(off.world()));
        assert_ne!(
            excise.world().scratch.pop_demand,
            off.world().scratch.pop_demand
        );
        let (bytes, back) = round_trip(excise.world());
        assert_eq!(canonical(12, &back), bytes);
        assert_eq!(back.energy, excise.world().energy);
    }

    #[test]
    fn energy_off_adds_nothing_to_the_snapshot() {
        let a = canonical(12, &world());
        let mut w = world();
        w.energy = None;
        assert_eq!(canonical(12, &w), a);
    }

    #[test]
    fn fiscal_off_adds_nothing_to_the_snapshot() {
        let a = canonical(12, &world());
        let mut w = world();
        w.fiscal = None;
        assert_eq!(canonical(12, &w), a);
    }

    #[test]
    fn monetary_off_adds_nothing_to_the_snapshot() {
        let a = canonical(12, &world());
        let mut w = world();
        w.monetary = None;
        assert_eq!(canonical(12, &w), a);
    }

    #[test]
    fn wage_grid_off_adds_nothing_to_the_snapshot() {
        let a = canonical(12, &world());
        let mut w = world();
        w.wage_grid = None;
        assert_eq!(canonical(12, &w), a);
    }

    #[test]
    fn migration_off_leaves_the_population_run_unchanged() {
        let mut on = migration_sim(u64::MAX);
        on.world_mut().migration = None;
        let mut off = population_sim();
        on.run_until(14).unwrap_or_else(|v| panic!("{v}"));
        off.run_until(14).unwrap_or_else(|v| panic!("{v}"));
        assert_eq!(on.state_hash(), off.state_hash());
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

    fn slow_world(rho: f64) -> EconWorld {
        let trade = TradeParams {
            rho: [rho; 4],
            ..TradeParams::default()
        };
        EconWorld::new(Arc::new(small_world()), EconParams::default(), trade)
    }

    fn tariffs_up(w: &mut EconWorld) {
        let mut wedges = w.inputs().wedges.clone();
        wedges.tariff.iter_mut().for_each(|t| *t = 0.25);
        w.set_wedges(wedges);
    }

    /// ADR 0027: a settled lagged kernel is its target bit for bit, so at any rho the steady
    /// state and every tick without a policy change equal the instant path's.
    #[test]
    fn lagged_kernel_without_a_change_equals_instant() {
        let mut slow = slow_world(0.1);
        let mut fast = world();
        for _ in 0..3 {
            slow.advance_kernel();
        }
        assert_eq!(slow.kernel().crossing_k(), fast.kernel().crossing_k());
        assert_eq!(slow.kernel().crossing_k(), slow.target.crossing_k());
        fast.advance_kernel();
    }

    /// ADR 0027, closed form: after a tariff edit the k of a crossing at tick n is
    /// `ln K* + (1 - rho)^n (ln K0 - ln K*)`; the other crossing fields follow at once.
    #[test]
    fn tariff_edit_moves_crossings_along_the_closed_form() {
        let rho = 0.2;
        let mut w = slow_world(rho);
        let before = w.kernel().crossing_k();
        tariffs_up(&mut w);
        assert_eq!(w.kernel().crossing_k(), before, "k lags the edit");
        let target = w.target.crossing_k();
        assert!(
            before.iter().zip(&target).any(|(a, b)| a != b),
            "the edit moves some k"
        );
        let duty = w.target.good(6).crossings.row(6)[0].inv_duty;
        assert_eq!(w.kernel().good(6).crossings.row(6)[0].inv_duty, duty);
        for n in 1..=6 {
            w.advance_kernel();
            let decay = (1.0 - rho).powi(n);
            for ((&k, &k0), &kt) in w.kernel().crossing_k().iter().zip(&before).zip(&target) {
                if k0 > 0.0 && kt > 0.0 {
                    let want = kt.ln() + decay * (k0.ln() - kt.ln());
                    assert!(
                        (k.ln() - want).abs() < 1e-9,
                        "tick {n}: {} vs {want}",
                        k.ln()
                    );
                }
            }
        }
        for _ in 0..400 {
            w.advance_kernel();
        }
        assert_eq!(
            w.kernel().crossing_k(),
            target,
            "settles on the target exactly"
        );
    }

    fn price_index_at(k: &Kernel, g: usize, d: usize) -> f64 {
        let np = k.n_provinces();
        let mut out = Sourced::new();
        ceteris_trade::source(
            k,
            g,
            &vec![1.0; np],
            &vec![true; np],
            &vec![1.0; np],
            &vec![[1.0, 0.0]; np],
            &mut out,
        );
        out.price_index[d]
    }

    /// ADR 0027, the elasticity path: with every border at one tariff the foreign part of a
    /// destination's access scales with `f = ((1 + t1) / (1 + t0))^-theta` at the target, and
    /// at tick n by `f^(1 - (1 - rho)^n)`, so the short-run elasticity is rho theta in month
    /// one and theta in the limit.
    #[test]
    fn tariff_elasticity_path_matches_closed_form() {
        let (rho, g, d) = (0.25, 0, 0);
        let mut w = slow_world(rho);
        let flat = |w: &mut EconWorld, t: f32| {
            let mut wedges = w.inputs().wedges.clone();
            wedges.tariff.iter_mut().for_each(|x| *x = t);
            w.set_wedges(wedges);
        };
        flat(&mut w, 0.0);
        for _ in 0..400 {
            w.advance_kernel();
        }
        let theta = w.kernel().theta(g);
        let phi0 = price_index_at(w.kernel(), g, d).powf(-theta);
        flat(&mut w, 0.25);
        let phi_end = price_index_at(&w.target, g, d).powf(-theta);
        let f = 1.25f64.powf(-theta);
        // phi = home + abroad f^s: two points fix both parts
        let abroad = (phi0 - phi_end) / (1.0 - f);
        let home = phi0 - abroad;
        assert!(abroad > 0.0 && home > 0.0, "home {home} abroad {abroad}");
        for n in 1..=8 {
            w.advance_kernel();
            let s = 1.0 - (1.0 - rho).powi(n);
            let want = home + abroad * f.powf(s);
            let got = price_index_at(w.kernel(), g, d).powf(-theta);
            assert!((got / want - 1.0).abs() < 1e-9, "tick {n}: {got} vs {want}");
        }
    }

    /// Sticky sourcing keeps every invariant while a tariff is phased in, and settles on the
    /// target's prices.
    #[test]
    fn sticky_sourcing_runs_through_a_tariff_edit() {
        let trade = TradeParams {
            rho: [0.2; 4],
            ..TradeParams::default()
        };
        let mut sim = EconWorld::sim(Arc::new(small_world()), EconParams::default(), trade);
        sim.run_until(6).unwrap_or_else(|v| panic!("{v}"));
        let mut wedges = sim.world_mut().inputs().wedges.clone();
        wedges.tariff.iter_mut().for_each(|x| *x = 0.25);
        sim.world_mut().set_wedges(wedges);
        sim.run_until(200).unwrap_or_else(|v| panic!("{v}"));
        let w = sim.world_mut();
        assert_eq!(w.kernel().crossing_k(), w.target.crossing_k());
        assert!(!w.smooth.is_empty());
    }

    /// The smoothed prices are snapshot state; the prices the shares read follow from them.
    #[test]
    fn smoothed_prices_round_trip() {
        let trade = TradeParams {
            rho: [0.2; 4],
            ..TradeParams::default()
        };
        let mut sim = EconWorld::sim(Arc::new(small_world()), EconParams::default(), trade);
        sim.run_until(10).unwrap_or_else(|v| panic!("{v}"));
        let w = sim.world_mut();
        assert!(
            w.weight_price
                .iter()
                .zip(&w.markets.price)
                .any(|(a, b)| a != b)
        );
        let (bytes, mut back) = round_trip(w);
        assert_eq!(canonical(12, &back), bytes);
        assert_eq!(back.smooth, w.smooth);
        // the next tick start rebuilds the prices of the shares from the state
        w.advance_kernel();
        back.advance_kernel();
        assert_eq!(back.weight_price, w.weight_price);
    }

    /// Lagged crossings are snapshot state.
    #[test]
    fn lagged_kernel_round_trips() {
        let mut w = slow_world(0.2);
        tariffs_up(&mut w);
        w.advance_kernel();
        assert_ne!(w.kernel().crossing_k(), w.target.crossing_k());
        let (bytes, back) = round_trip(&w);
        assert_eq!(canonical(12, &back), bytes);
        assert_eq!(back.kernel().crossing_k(), w.kernel().crossing_k());
        assert_eq!(back.target.crossing_k(), w.target.crossing_k());
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
