//! Double-entry ledger for money (fixed-point `i64`) and goods (`f64`), Section 4.6.
//! Money only moves between accounts; goods appear and disappear only through
//! `create` and `destroy`, which are tallied and checked every tick.

use rayon::prelude::*;

use crate::invariant::Violation;
use crate::reduce::fixed_sum;
use crate::snapshot::{Decode, DecodeError, Decoder, Encode, Encoder};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AccountId(pub u32);

/// Tag on a money transfer, for sector flow matrices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flow(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub flow: Flow,
    pub from: AccountId,
    pub to: AccountId,
    pub amount: i64,
}

pub const GOODS_TOLERANCE: f64 = 1e-9;

/// Entries per parallel chunk of [`Ledger::post`].
const POST_CHUNK: usize = 1 << 17;

#[derive(Clone, Debug, PartialEq)]
pub struct Ledger {
    n_goods: usize,
    money: Vec<i64>,
    goods: Vec<f64>,
    open: Vec<bool>,
    journal: Vec<Entry>,
    created: Vec<f64>,
    destroyed: Vec<f64>,
    opening: Vec<f64>,
}

impl Ledger {
    pub fn new(n_goods: usize) -> Self {
        Self {
            n_goods,
            money: Vec::new(),
            goods: Vec::new(),
            open: Vec::new(),
            journal: Vec::new(),
            created: vec![0.0; n_goods],
            destroyed: vec![0.0; n_goods],
            opening: vec![0.0; n_goods],
        }
    }

    pub fn open(&mut self) -> AccountId {
        let id = AccountId(u32::try_from(self.money.len()).expect("account space exhausted"));
        self.money.push(0);
        self.goods.extend(std::iter::repeat_n(0.0, self.n_goods));
        self.open.push(true);
        id
    }

    /// Closes an empty account. Its ID is never reused.
    pub fn close(&mut self, a: AccountId) {
        assert!(self.is_open(a), "closing closed account {a:?}");
        assert_eq!(self.money(a), 0, "closing account {a:?} with money");
        assert!(
            self.holdings(a).iter().all(|&q| q == 0.0),
            "closing account {a:?} with goods"
        );
        self.open[a.0 as usize] = false;
    }

    /// Accounts ever opened, closed ones included.
    pub fn n_accounts(&self) -> usize {
        self.money.len()
    }

    pub fn is_open(&self, a: AccountId) -> bool {
        self.open[a.0 as usize]
    }

    pub fn money(&self, a: AccountId) -> i64 {
        self.money[a.0 as usize]
    }

    pub fn goods(&self, a: AccountId, good: usize) -> f64 {
        self.holdings(a)[good]
    }

    fn holdings(&self, a: AccountId) -> &[f64] {
        let start = a.0 as usize * self.n_goods;
        &self.goods[start..start + self.n_goods]
    }

    fn slot(&mut self, a: AccountId, good: usize) -> &mut f64 {
        debug_assert!(self.is_open(a), "goods on closed account {a:?}");
        &mut self.goods[a.0 as usize * self.n_goods + good]
    }

    pub fn transfer(&mut self, flow: Flow, from: AccountId, to: AccountId, amount: i64) {
        debug_assert!(self.is_open(from) && self.is_open(to));
        let (f, t) = (from.0 as usize, to.0 as usize);
        self.money[f] = self.money[f].checked_sub(amount).expect("ledger overflow");
        self.money[t] = self.money[t].checked_add(amount).expect("ledger overflow");
        self.journal.push(Entry {
            flow,
            from,
            to,
            amount,
        });
    }

    /// [`Ledger::transfer`] of each entry in turn: the same balances and journal. Each
    /// account's money moves by the integer sum of its entries, summed over chunks in
    /// parallel, so an overflow is caught in a chunk's sum or the final balance rather than
    /// at the step that would cross the bound.
    pub fn post(&mut self, entries: &[Entry]) {
        let n = self.money.len();
        let deltas: Vec<Vec<i64>> = entries
            .par_chunks(POST_CHUNK)
            .map(|chunk| {
                let mut delta = vec![0i64; n];
                for e in chunk {
                    debug_assert!(self.is_open(e.from) && self.is_open(e.to));
                    let (f, t) = (e.from.0 as usize, e.to.0 as usize);
                    delta[f] = delta[f].checked_sub(e.amount).expect("ledger overflow");
                    delta[t] = delta[t].checked_add(e.amount).expect("ledger overflow");
                }
                delta
            })
            .collect();
        for delta in &deltas {
            for (m, &d) in self.money.iter_mut().zip(delta) {
                *m = m.checked_add(d).expect("ledger overflow");
            }
        }
        self.journal.extend_from_slice(entries);
    }

    pub fn ship(&mut self, from: AccountId, to: AccountId, good: usize, qty: f64) {
        *self.slot(from, good) -= qty;
        *self.slot(to, good) += qty;
    }

    pub fn create(&mut self, a: AccountId, good: usize, qty: f64) {
        *self.slot(a, good) += qty;
        self.created[good] += qty;
    }

    pub fn destroy(&mut self, a: AccountId, good: usize, qty: f64) {
        *self.slot(a, good) -= qty;
        self.destroyed[good] += qty;
    }

    pub fn journal(&self) -> &[Entry] {
        &self.journal
    }

    /// Every good's total over the accounts, each column summed by `fixed_sum`.
    fn good_totals(&self) -> Vec<f64> {
        let n = self.n_accounts();
        let mut columns = vec![0.0; self.n_goods * n];
        for (a, holdings) in self.goods.chunks_exact(self.n_goods).enumerate() {
            for (g, &q) in holdings.iter().enumerate() {
                columns[g * n + a] = q;
            }
        }
        if n == 0 {
            return vec![fixed_sum(&[]); self.n_goods];
        }
        columns.chunks_exact(n).map(fixed_sum).collect()
    }

    pub fn begin_tick(&mut self) {
        self.journal.clear();
        self.created.fill(0.0);
        self.destroyed.fill(0.0);
        self.opening = self.good_totals();
    }

    pub fn check(&self, tick: u32) -> Result<(), Violation> {
        let net: i128 = self.money.iter().map(|&m| i128::from(m)).sum();
        if net != 0 {
            return Err(Violation::new(
                tick,
                "money",
                format!("balances sum to {net}"),
            ));
        }
        if let Some(i) = self.goods.iter().position(|&q| q.is_nan() || q < 0.0) {
            let account = i / self.n_goods;
            let detail = format!(
                "account {account} good {} holds {}",
                i % self.n_goods,
                self.goods[i]
            );
            return Err(Violation::new(tick, "goods sign", detail));
        }
        let closing = self.good_totals();
        for (g, &close) in closing.iter().enumerate() {
            let (open, made, gone) = (self.opening[g], self.created[g], self.destroyed[g]);
            let gap = close - (open + made - gone);
            if gap.abs() > GOODS_TOLERANCE * (open + made + gone).max(1.0) {
                let detail = format!(
                    "good {g}: opening {open} + created {made} - destroyed {gone} != closing {close}"
                );
                return Err(Violation::new(tick, "goods", detail));
            }
        }
        Ok(())
    }
}

impl Encode for Ledger {
    fn encode(&self, e: &mut Encoder) {
        e.count(self.n_goods);
        e.i64s(&self.money);
        e.f64s(&self.goods);
        e.bools(&self.open);
    }
}

impl Decode for Ledger {
    fn decode(d: &mut Decoder) -> Result<Self, DecodeError> {
        let mut ledger = Ledger::new(d.count()?);
        ledger.money = d.i64s()?;
        ledger.goods = d.f64s()?;
        ledger.open = d.bools()?;
        let n = ledger.money.len();
        if ledger.open.len() != n || ledger.goods.len() != n * ledger.n_goods {
            return Err(DecodeError("ledger column lengths disagree".into()));
        }
        Ok(ledger)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn post_is_transfer_in_turn() {
        let (mut a, mut b) = (Ledger::new(1), Ledger::new(1));
        let ids: Vec<AccountId> = (0..3).map(|_| (a.open(), b.open()).0).collect();
        let entries = [(0, 1, 5), (1, 2, -3), (2, 0, 7), (0, 2, 0)].map(|(f, t, amount)| Entry {
            flow: Flow(4),
            from: ids[f],
            to: ids[t],
            amount,
        });
        for e in &entries {
            a.transfer(e.flow, e.from, e.to, e.amount);
        }
        b.post(&entries);
        assert_eq!(a, b);
    }

    #[derive(Debug, Clone)]
    enum Op {
        Transfer(usize, usize, i64),
        Ship(usize, usize, f64),
        Create(usize, f64),
        Destroy(usize, f64),
    }

    fn op() -> impl Strategy<Value = Op> {
        prop_oneof![
            (0..6usize, 0..6usize, -1_000_000i64..1_000_000)
                .prop_map(|(a, b, x)| Op::Transfer(a, b, x)),
            (0..6usize, 0..6usize, 0.0..1.0f64).prop_map(|(a, b, f)| Op::Ship(a, b, f)),
            (0..6usize, 0.0..1e6f64).prop_map(|(a, q)| Op::Create(a, q)),
            (0..6usize, 0.0..1.0f64).prop_map(|(a, f)| Op::Destroy(a, f)),
        ]
    }

    proptest! {
        #[test]
        fn random_operations_conserve_money_and_goods(ops in proptest::collection::vec(op(), 0..300)) {
            let mut l = Ledger::new(2);
            let accts: Vec<AccountId> = (0..6).map(|_| l.open()).collect();
            l.begin_tick();
            for (i, op) in ops.into_iter().enumerate() {
                let g = i % 2;
                match op {
                    Op::Transfer(a, b, x) => l.transfer(Flow(0), accts[a], accts[b], x),
                    // Ship and destroy a fraction of the holding so goods stay non-negative.
                    Op::Ship(a, b, f) => { let q = f * l.goods(accts[a], g); l.ship(accts[a], accts[b], g, q) }
                    Op::Create(a, q) => l.create(accts[a], g, q),
                    Op::Destroy(a, f) => { let q = f * l.goods(accts[a], g); l.destroy(accts[a], g, q) }
                }
            }
            prop_assert!(l.check(0).is_ok(), "{:?}", l.check(0));
            let mut e = Encoder::default();
            l.encode(&mut e);
            let bytes = e.into_bytes();
            let back = Ledger::decode(&mut Decoder::new(&bytes)).unwrap();
            prop_assert_eq!((&back.money, &back.goods, &back.open), (&l.money, &l.goods, &l.open));
        }
    }

    #[test]
    fn unlogged_goods_are_caught() {
        let mut l = Ledger::new(1);
        let a = l.open();
        l.begin_tick();
        *l.slot(a, 0) += 5.0;
        assert_eq!(l.check(3).unwrap_err().invariant, "goods");
    }

    #[test]
    fn negative_goods_are_caught() {
        let mut l = Ledger::new(1);
        let (a, b) = (l.open(), l.open());
        l.begin_tick();
        l.ship(a, b, 0, 1.0);
        assert_eq!(l.check(0).unwrap_err().invariant, "goods sign");
    }

    #[test]
    fn corrupted_money_is_caught() {
        let mut l = Ledger::new(0);
        let a = l.open();
        l.money[a.0 as usize] = 1;
        assert_eq!(l.check(0).unwrap_err().invariant, "money");
    }

    #[test]
    #[should_panic(expected = "with money")]
    fn closing_a_funded_account_panics() {
        let mut l = Ledger::new(0);
        let (a, b) = (l.open(), l.open());
        l.transfer(Flow(0), a, b, 1);
        l.close(b);
    }
}
