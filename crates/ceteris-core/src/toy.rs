//! P0 fixture (specs/core/SPEC.md, Toy world): a ring of provinces, each with a
//! household and a merchant, plus a churning table of firms; one good; a mint issues
//! the initial money. It exercises every core service and is replaced by the economic
//! core in P2. Constants are fixture values, not registered parameters.

use rayon::prelude::*;

use crate::entity::{EntityId, IdAlloc, Kind, compact};
use crate::event::{Event, EventKind, EventLog};
use crate::invariant::Violation;
use crate::ledger::{AccountId, Flow, Ledger};
use crate::reduce::fixed_sum;
use crate::rng::Seed;
use crate::schedule::{Cadence, Ctx, Schedule, SystemDef, SystemId};
use crate::sim::{Sim, State};
use crate::snapshot::{Decode, DecodeError, Decoder, Encode, Encoder};

const GOOD: usize = 0;

const PROVINCE: Kind = Kind(1);
const FIRM: Kind = Kind(2);

pub const OUTPUT_SHOCK: EventKind = EventKind(1);
pub const FIRM_ENTRY: EventKind = EventKind(2);
pub const FIRM_EXIT: EventKind = EventKind(3);
/// World mean price; subject 0 stands for the world.
pub const CENSUS_PRICE: EventKind = EventKind(4);

const ISSUE: Flow = Flow(1);
const WHOLESALE: Flow = Flow(2);
const WAGES: Flow = Flow(3);
const RETAIL: Flow = Flow(4);
const TRADE: Flow = Flow(5);
const SETUP: Flow = Flow(6);
const CLOSURE: Flow = Flow(7);
const DIVIDENDS: Flow = Flow(8);

const HOUSEHOLD_CASH: i64 = 100_000_000;
const MERCHANT_CASH: i64 = 50_000_000;
const INITIAL_PRICE: f64 = 10_000.0;
const FIRMS_PER_PROVINCE: usize = 3;
const FIRM_CAPACITY: f64 = 1_000.0;
const OUTPUT_SD: f64 = 0.1;
const SHOCK_Z: f64 = -2.5;
const WAGE_SHARE: f64 = 0.9;
const SPEND_SHARE: f64 = 0.3;
const MARGIN: f64 = 0.1;
const RESERVE_MONTHS: f64 = 3.0;
const SPOILAGE: f64 = 0.02;
const TARGET_COVER: f64 = 2.0;
const K_EXCESS: f64 = 0.1;
const K_STOCK: f64 = 0.05;
const MAX_STEP: f64 = 0.1;
const TRADE_SHARE: f64 = 0.2;
const TRADE_COST: f64 = 0.05;
const EXIT_CASH: f64 = 0.05;
const ENTRY_RATE: f64 = 0.5;
const ENTRY_MAX: f64 = 0.5;
const SETUP_MONTHS: f64 = 0.5;

#[derive(Clone, Debug, PartialEq)]
pub struct Toy {
    pub ledger: Ledger,
    pub events: EventLog,
    mint: AccountId,
    pub household: Vec<AccountId>,
    pub merchant: Vec<AccountId>,
    pub price: Vec<f64>,
    /// Desired retail purchases last month.
    pub demand: Vec<f64>,
    /// Wholesale purchases last month.
    pub supply: Vec<f64>,
    firm_ids: IdAlloc,
    pub firm_id: Vec<EntityId>,
    pub firm_province: Vec<u32>,
    pub firm_account: Vec<AccountId>,
    pub firm_capacity: Vec<f64>,
}

impl Toy {
    pub fn new(provinces: usize) -> Self {
        let mut ledger = Ledger::new(1);
        let mint = ledger.open();
        let mut toy = Toy {
            mint,
            household: Vec::new(),
            merchant: Vec::new(),
            price: vec![INITIAL_PRICE; provinces],
            demand: vec![0.0; provinces],
            supply: vec![0.0; provinces],
            firm_ids: IdAlloc::default(),
            firm_id: Vec::new(),
            firm_province: Vec::new(),
            firm_account: Vec::new(),
            firm_capacity: Vec::new(),
            ledger,
            events: EventLog::default(),
        };
        for p in 0..provinces {
            let (h, m) = (toy.ledger.open(), toy.ledger.open());
            toy.ledger.transfer(ISSUE, mint, h, HOUSEHOLD_CASH);
            toy.ledger.transfer(ISSUE, mint, m, MERCHANT_CASH);
            toy.household.push(h);
            toy.merchant.push(m);
            for k in 0..FIRMS_PER_PROVINCE {
                toy.add_firm(p, FIRM_CAPACITY * (1.0 + 0.1 * k as f64));
            }
        }
        toy
    }

    pub fn sim(provinces: usize, seed: Seed) -> Sim<Toy> {
        Sim::new(Toy::new(provinces), Toy::schedule(), seed)
    }

    pub fn schedule() -> Schedule<Toy> {
        let def = |id, name, cadence, run| SystemDef {
            id: SystemId(id),
            name,
            cadence,
            run,
        };
        Schedule::new(vec![
            def(1, "produce", Cadence::Monthly, produce),
            def(2, "wholesale", Cadence::Monthly, wholesale),
            def(3, "wages", Cadence::Monthly, wages),
            def(4, "retail", Cadence::Monthly, retail),
            def(5, "trade", Cadence::Monthly, trade),
            def(9, "dividends", Cadence::Monthly, dividends),
            def(6, "prices", Cadence::Monthly, prices),
            def(7, "entry_exit", Cadence::Quarterly, entry_exit),
            def(8, "census", Cadence::Annual { month: 0 }, census),
        ])
    }

    pub fn provinces(&self) -> usize {
        self.price.len()
    }

    fn add_firm(&mut self, province: usize, capacity: f64) -> (EntityId, AccountId) {
        let (id, account) = (self.firm_ids.alloc(), self.ledger.open());
        self.firm_id.push(id);
        self.firm_province.push(province as u32);
        self.firm_account.push(account);
        self.firm_capacity.push(capacity);
        (id, account)
    }

    fn event(&mut self, ctx: &Ctx, kind: EventKind, subject: EntityId, value: f64) {
        self.events.push(Event {
            tick: ctx.tick,
            system: ctx.system,
            kind,
            subject,
            value,
        });
    }
}

/// Floor to whole minor units; never pays more than the exact amount.
fn minor_units(x: f64) -> i64 {
    libm::floor(x) as i64
}

fn produce(w: &mut Toy, ctx: &Ctx) {
    let draws: Vec<(f64, f64)> = (w.firm_id.par_iter(), w.firm_capacity.par_iter())
        .into_par_iter()
        .map(|(&id, &cap)| {
            let z = ctx.stream(FIRM, id).normal();
            (z, cap * OUTPUT_SD.mul_add(z, 1.0).max(0.0))
        })
        .collect();
    for (f, &(z, q)) in draws.iter().enumerate() {
        w.ledger.create(w.firm_account[f], GOOD, q);
        if z < SHOCK_Z {
            w.event(ctx, OUTPUT_SHOCK, w.firm_id[f], z);
        }
    }
}

fn wholesale(w: &mut Toy, _: &Ctx) {
    let mut output = vec![0.0; w.provinces()];
    for (f, &p) in w.firm_province.iter().enumerate() {
        output[p as usize] += w.ledger.goods(w.firm_account[f], GOOD);
    }
    let cost: Vec<f64> = w.price.iter().map(|&p| p / (1.0 + MARGIN)).collect();
    // Merchants buy what they can afford, up to target stock plus a month of sales.
    let bought: Vec<f64> = (0..w.provinces())
        .map(|p| {
            let m = w.merchant[p];
            let afford = w.ledger.money(m) as f64 / cost[p];
            let want = (TARGET_COVER + 1.0) * w.demand[p] - w.ledger.goods(m, GOOD);
            if output[p] > 0.0 {
                (afford.min(want.max(0.0)) / output[p]).min(1.0)
            } else {
                0.0
            }
        })
        .collect();
    w.supply.fill(0.0);
    for f in 0..w.firm_id.len() {
        let (p, firm) = (w.firm_province[f] as usize, w.firm_account[f]);
        let sold = bought[p] * w.ledger.goods(firm, GOOD);
        w.ledger.ship(firm, w.merchant[p], GOOD, sold);
        let pay = minor_units(cost[p] * sold);
        w.ledger.transfer(WHOLESALE, w.merchant[p], firm, pay);
        w.ledger.destroy(firm, GOOD, w.ledger.goods(firm, GOOD));
        w.supply[p] += sold;
    }
}

fn wages(w: &mut Toy, _: &Ctx) {
    for f in 0..w.firm_id.len() {
        let firm = w.firm_account[f];
        let pay = minor_units(WAGE_SHARE * w.ledger.money(firm) as f64);
        w.ledger
            .transfer(WAGES, firm, w.household[w.firm_province[f] as usize], pay);
    }
}

fn retail(w: &mut Toy, _: &Ctx) {
    for p in 0..w.provinces() {
        let (h, m) = (w.household[p], w.merchant[p]);
        let budget = minor_units(SPEND_SHARE * w.ledger.money(h) as f64);
        w.demand[p] = budget as f64 / w.price[p];
        let sold = w.demand[p].min(w.ledger.goods(m, GOOD));
        w.ledger.ship(m, h, GOOD, sold);
        w.ledger.destroy(h, GOOD, sold);
        w.ledger
            .transfer(RETAIL, h, m, minor_units(w.price[p] * sold));
    }
}

/// Merchants on a ring ship toward the dearer neighbour when the price gap beats the
/// trade cost. Proposals use start-of-system state; application clamps to stock and cash.
fn trade(w: &mut Toy, _: &Ctx) {
    let n = w.provinces();
    if n < 2 {
        return;
    }
    let proposals: Vec<(usize, usize, f64)> = (0..n)
        .into_par_iter()
        .map(|a| {
            let b = (a + 1) % n;
            let (from, to) = if w.price[b] > w.price[a] {
                (a, b)
            } else {
                (b, a)
            };
            let margin = w.price[to] / w.price[from] - 1.0 - TRADE_COST;
            let q = TRADE_SHARE * margin.clamp(0.0, 1.0) * w.ledger.goods(w.merchant[from], GOOD);
            (from, to, q)
        })
        .collect();
    for (from, to, q) in proposals {
        let (mf, mt) = (w.merchant[from], w.merchant[to]);
        let q = q
            .min(w.ledger.goods(mf, GOOD))
            .min(w.ledger.money(mt) as f64 / w.price[from]);
        if q > 0.0 {
            w.ledger.ship(mf, mt, GOOD, q);
            w.ledger
                .transfer(TRADE, mt, mf, minor_units(w.price[from] * q));
        }
    }
}

/// Merchants pay out cash above a reserve of a few months of sales.
fn dividends(w: &mut Toy, _: &Ctx) {
    for p in 0..w.provinces() {
        let (h, m) = (w.household[p], w.merchant[p]);
        let reserve = minor_units(RESERVE_MONTHS * w.price[p] * w.demand[p]);
        let surplus = w.ledger.money(m) - reserve;
        if surplus > 0 {
            w.ledger.transfer(DIVIDENDS, m, h, surplus);
        }
    }
}

/// Stock-flow price rule of Section 8.2, then merchant spoilage.
fn prices(w: &mut Toy, _: &Ctx) {
    let next: Vec<f64> = (0..w.provinces())
        .into_par_iter()
        .map(|p| {
            let (d, q, s) = (
                w.demand[p],
                w.supply[p],
                w.ledger.goods(w.merchant[p], GOOD),
            );
            let target = TARGET_COVER * d;
            let excess = (d - q) / q.max(1.0);
            let glut = (s - target) / target.max(1.0);
            let step = K_EXCESS
                .mul_add(excess, -K_STOCK * glut)
                .clamp(-MAX_STEP, MAX_STEP);
            w.price[p] * libm::exp(step)
        })
        .collect();
    w.price = next;
    for &m in &w.merchant {
        w.ledger
            .destroy(m, GOOD, SPOILAGE * w.ledger.goods(m, GOOD));
    }
}

/// Each quarter a province loses at most its poorest firm, if that firm's cash is below
/// the exit threshold and it is not the province's last firm, and gains at most one.
fn entry_exit(w: &mut Toy, ctx: &Ctx) {
    let mut firms = vec![0usize; w.provinces()];
    let mut poorest: Vec<Option<(usize, f64)>> = vec![None; w.provinces()];
    for f in 0..w.firm_id.len() {
        let p = w.firm_province[f] as usize;
        let cover = w.ledger.money(w.firm_account[f]) as f64 / (w.price[p] * w.firm_capacity[f]);
        firms[p] += 1;
        if poorest[p].is_none_or(|(_, c)| cover < c) {
            poorest[p] = Some((f, cover));
        }
    }
    let mut keep = vec![true; w.firm_id.len()];
    for p in 0..w.provinces() {
        if let Some((f, cover)) = poorest[p]
            && cover < EXIT_CASH
            && firms[p] > 1
        {
            keep[f] = false;
            firms[p] -= 1;
        }
    }
    for f in (0..keep.len()).filter(|&f| !keep[f]) {
        let (firm, household) = (w.firm_account[f], w.household[w.firm_province[f] as usize]);
        let cash = w.ledger.money(firm);
        w.ledger.transfer(CLOSURE, firm, household, cash);
        w.ledger.close(firm);
        w.event(ctx, FIRM_EXIT, w.firm_id[f], cash as f64);
    }
    compact(&mut w.firm_id, &keep);
    compact(&mut w.firm_province, &keep);
    compact(&mut w.firm_account, &keep);
    compact(&mut w.firm_capacity, &keep);

    let mean_price = fixed_sum(&w.price) / w.provinces() as f64;
    for p in 0..w.provinces() {
        let chance = (ENTRY_RATE * (w.price[p] / mean_price - 1.0)).clamp(0.0, ENTRY_MAX);
        let setup = minor_units(SETUP_MONTHS * w.price[p] * FIRM_CAPACITY);
        let draw = ctx.stream(PROVINCE, EntityId(p as u32)).uniform();
        if draw < chance && setup <= w.ledger.money(w.household[p]) {
            let (id, account) = w.add_firm(p, FIRM_CAPACITY);
            w.ledger.transfer(SETUP, w.household[p], account, setup);
            w.event(ctx, FIRM_ENTRY, id, setup as f64);
        }
    }
}

fn census(w: &mut Toy, ctx: &Ctx) {
    let mean_price = fixed_sum(&w.price) / w.provinces() as f64;
    w.event(ctx, CENSUS_PRICE, EntityId(0), mean_price);
}

impl State for Toy {
    fn begin_tick(&mut self) {
        self.ledger.begin_tick();
    }

    fn end_tick(&self, tick: u32) -> Result<(), Violation> {
        self.ledger.check(tick)?;
        let accounts = self
            .household
            .iter()
            .chain(&self.merchant)
            .chain(&self.firm_account);
        if let Some(a) = accounts.clone().find(|&&a| self.ledger.money(a) < 0) {
            let detail = format!("account {a:?} holds {}", self.ledger.money(*a));
            return Err(Violation::new(tick, "non-negative money", detail));
        }
        if let Some(p) = self.price.iter().position(|&p| !(p.is_finite() && p > 0.0)) {
            let detail = format!("province {p} price {}", self.price[p]);
            return Err(Violation::new(tick, "positive finite price", detail));
        }
        if let Some(a) = accounts.clone().find(|&&a| !self.ledger.is_open(a)) {
            return Err(Violation::new(
                tick,
                "open accounts",
                format!("{a:?} is closed"),
            ));
        }
        Ok(())
    }
}

fn account_ids(ids: &[AccountId]) -> Vec<u32> {
    ids.iter().map(|a| a.0).collect()
}

impl Encode for Toy {
    fn encode(&self, e: &mut Encoder) {
        self.ledger.encode(e);
        self.events.encode(e);
        e.u32(self.mint.0);
        e.u32s(&account_ids(&self.household));
        e.u32s(&account_ids(&self.merchant));
        e.f64s(&self.price);
        e.f64s(&self.demand);
        e.f64s(&self.supply);
        self.firm_ids.encode(e);
        e.u32s(&self.firm_id.iter().map(|f| f.0).collect::<Vec<_>>());
        e.u32s(&self.firm_province);
        e.u32s(&account_ids(&self.firm_account));
        e.f64s(&self.firm_capacity);
    }
}

impl Decode for Toy {
    fn decode(d: &mut Decoder) -> Result<Self, DecodeError> {
        let accounts = |v: Vec<u32>| v.into_iter().map(AccountId).collect();
        let toy = Toy {
            ledger: Ledger::decode(d)?,
            events: EventLog::decode(d)?,
            mint: AccountId(d.u32()?),
            household: accounts(d.u32s()?),
            merchant: accounts(d.u32s()?),
            price: d.f64s()?,
            demand: d.f64s()?,
            supply: d.f64s()?,
            firm_ids: IdAlloc::decode(d)?,
            firm_id: d.u32s()?.into_iter().map(EntityId).collect(),
            firm_province: d.u32s()?,
            firm_account: accounts(d.u32s()?),
            firm_capacity: d.f64s()?,
        };
        let n = toy.provinces();
        let f = toy.firm_id.len();
        if [
            toy.household.len(),
            toy.merchant.len(),
            toy.demand.len(),
            toy.supply.len(),
        ] != [n; 4]
            || [
                toy.firm_province.len(),
                toy.firm_account.len(),
                toy.firm_capacity.len(),
            ] != [f; 3]
        {
            return Err(DecodeError("toy column lengths disagree".into()));
        }
        Ok(toy)
    }
}
