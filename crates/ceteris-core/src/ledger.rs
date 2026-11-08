//! Double-entry ledger for money (fixed-point minor units) and goods (`f64`), Section 4.6.
//! Money only moves between accounts; goods appear and disappear only through
//! `create` and `destroy`, which are tallied and checked every tick. An account is narrow
//! (balance within `i64`) or wide (`i128`, for accounts that aggregate many others; ADR 0106),
//! chosen when it is opened.

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

/// One good's holdings of every account and its tallies, taken out of a [`Ledger`] by
/// [`Ledger::take_columns`]: the ledger's goods operations on that good, with the same
/// arithmetic in the same order.
#[derive(Clone, Debug, Default)]
pub struct GoodColumn {
    good: usize,
    goods: Vec<f64>,
    created: f64,
    destroyed: f64,
}

impl GoodColumn {
    pub fn good(&self) -> usize {
        self.good
    }

    pub fn goods(&self, a: AccountId) -> f64 {
        self.goods[a.0 as usize]
    }

    pub fn ship(&mut self, from: AccountId, to: AccountId, qty: f64) {
        self.goods[from.0 as usize] -= qty;
        self.goods[to.0 as usize] += qty;
    }

    pub fn create(&mut self, a: AccountId, qty: f64) {
        self.goods[a.0 as usize] += qty;
        self.created += qty;
    }

    pub fn destroy(&mut self, a: AccountId, qty: f64) {
        self.goods[a.0 as usize] -= qty;
        self.destroyed += qty;
    }
}

/// The accounts of one province for [`Ledger::province_slices`]: ids of its pops, of its
/// enterprises and of its market.
#[derive(Clone, Debug)]
pub struct ProvinceAccounts {
    pub pops: std::ops::Range<u32>,
    pub enterprises: std::ops::Range<u32>,
    pub market: AccountId,
}

/// What a [`ProvinceLedger`] leaves for [`Ledger::put_province_slices`]: its journal block and
/// its `created` and `destroyed` partials.
#[derive(Debug)]
pub struct ProvinceDone {
    journal: Vec<Entry>,
    created: Vec<f64>,
    destroyed: Vec<f64>,
}

/// One province's accounts taken out of a [`Ledger`] by [`Ledger::province_slices`]: the
/// ledger's `transfer`, `ship`, `create`, `destroy` and `goods` on those accounts with the
/// same arithmetic, so that provinces run in parallel. Kind 0 holds the pops, 1 the
/// enterprises, 2 the market. The journal entries go to the province's own block and the
/// created and destroyed quantities to its own partials. A call that names an account of
/// another province panics.
#[derive(Debug)]
pub struct ProvinceLedger<'a> {
    n_goods: usize,
    tracked: u64,
    base: [u32; 3],
    len: [u32; 3],
    money: [&'a mut [i128]; 3],
    wide: [&'a [bool]; 3],
    goods: [&'a mut [f64]; 3],
    tally: [&'a mut [i64]; 3],
    journal: Vec<Entry>,
    created: Vec<f64>,
    destroyed: Vec<f64>,
}

impl ProvinceLedger<'_> {
    fn locate(&self, a: AccountId) -> (usize, usize) {
        for k in 0..3 {
            let d = a.0.wrapping_sub(self.base[k]);
            if d < self.len[k] {
                return (k, d as usize);
            }
        }
        panic!("account {a:?} is not in this province");
    }

    fn slot(&mut self, a: AccountId, good: usize) -> &mut f64 {
        let (k, i) = self.locate(a);
        &mut self.goods[k][i * self.n_goods + good]
    }

    pub fn goods(&self, a: AccountId, good: usize) -> f64 {
        let (k, i) = self.locate(a);
        self.goods[k][i * self.n_goods + good]
    }

    pub fn transfer(&mut self, flow: Flow, from: AccountId, to: AccountId, amount: i64) {
        let ((fk, f), (tk, t)) = (self.locate(from), self.locate(to));
        let amount_wide = i128::from(amount);
        self.money[fk][f] = moved(self.money[fk][f], self.wide[fk][f], -amount_wide);
        self.money[tk][t] = moved(self.money[tk][t], self.wide[tk][t], amount_wide);
        self.journal.push(Entry {
            flow,
            from,
            to,
            amount,
        });
        if tracks(self.tracked, flow) {
            self.tally[tk][t] += amount;
            self.tally[fk][f] -= amount;
        }
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

    /// Ends the province's work, releasing the borrow of the ledger.
    pub fn finish(self) -> ProvinceDone {
        ProvinceDone {
            journal: self.journal,
            created: self.created,
            destroyed: self.destroyed,
        }
    }
}

/// Cuts `v` (whose first element is index `at` of the whole) into one run of `stride` elements
/// per index of each of `spans`, which ascend without overlap.
fn carve<'a, T>(
    mut rest: &'a mut [T],
    mut at: usize,
    spans: &[std::ops::Range<usize>],
    stride: usize,
) -> Vec<&'a mut [T]> {
    spans
        .iter()
        .map(|r| {
            assert!(
                r.start >= at && r.start <= r.end,
                "province accounts out of order"
            );
            let (_, tail) = std::mem::take(&mut rest).split_at_mut((r.start - at) * stride);
            let (head, tail) = tail.split_at_mut(r.len() * stride);
            (rest, at) = (tail, r.end);
            head
        })
        .collect()
}

/// Entries per parallel chunk of [`Ledger::post`].
const POST_CHUNK: usize = 1 << 17;

/// Lanes of [`Ledger::post`]: chunk `i` sums into lane `i % POST_LANES`, a lane taking its
/// chunks in order. Fixed, not per thread, so an overflow is caught at the same lane addition
/// at any thread count.
const POST_LANES: usize = 32;

/// Accounts per parallel task when [`Ledger::post`] applies its sums.
const ACCOUNT_CHUNK: usize = 1 << 12;

/// Whether `flow` is in the tracked set `mask` ([`Ledger::track`]).
fn tracks(mask: u64, flow: Flow) -> bool {
    flow.0 < 64 && mask >> flow.0 & 1 == 1
}

/// `balance + delta` for an account of the given kind, panicking past its range.
fn moved(balance: i128, wide: bool, delta: i128) -> i128 {
    let next = balance.checked_add(delta).expect("ledger overflow");
    assert!(wide || i64::try_from(next).is_ok(), "ledger overflow");
    next
}

/// Working space of [`Ledger::post`], kept by the caller between calls: one sum per account
/// for each lane, zero between calls. `dirty` marks a call that did not finish (an overflow
/// panic), whose sums the next call clears first.
#[derive(Clone, Debug, Default)]
pub struct PostScratch {
    deltas: Vec<Vec<i128>>,
    dirty: bool,
    /// Per lane, its entries of tallied flows.
    tallied: Vec<Vec<Entry>>,
}

/// The entries of a tick as the blocks they were posted in, in posting order. Posted blocks
/// are kept, not copied; `pool` holds emptied buffers for the next tick.
#[derive(Clone, Debug, Default)]
struct Journal {
    blocks: Vec<Vec<Entry>>,
    pool: Vec<Vec<Entry>>,
    /// Whether the last block takes single transfers.
    open_tail: bool,
}

impl Journal {
    fn buffer(&mut self) -> Vec<Entry> {
        self.pool.pop().unwrap_or_default()
    }

    fn push(&mut self, e: Entry) {
        if !self.open_tail {
            let b = self.buffer();
            self.blocks.push(b);
            self.open_tail = true;
        }
        self.blocks.last_mut().expect("tail block").push(e);
    }

    fn seal(&mut self, block: Vec<Entry>) {
        self.blocks.push(block);
        self.open_tail = false;
    }

    fn clear(&mut self) {
        for mut b in self.blocks.drain(..) {
            b.clear();
            self.pool.push(b);
        }
        self.open_tail = false;
    }

    fn iter(&self) -> impl Iterator<Item = &Entry> + Clone {
        self.blocks.iter().flatten()
    }

    fn len(&self) -> usize {
        self.blocks.iter().map(Vec::len).sum()
    }
}

impl PartialEq for Journal {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

/// A tick's journal, read in posting order ([`Ledger::journal`]).
#[derive(Clone, Copy)]
pub struct JournalView<'a>(&'a Journal);

impl<'a> JournalView<'a> {
    pub fn iter(&self) -> impl Iterator<Item = &'a Entry> + Clone {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The entries from position `start` on.
    pub fn since(&self, start: usize) -> impl Iterator<Item = &'a Entry> + Clone {
        self.0.iter().skip(start)
    }
}

impl<'a> IntoIterator for JournalView<'a> {
    type Item = &'a Entry;
    type IntoIter = std::iter::Flatten<std::slice::Iter<'a, Vec<Entry>>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.blocks.iter().flatten()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Ledger {
    n_goods: usize,
    /// Balances of every account; a narrow account's stays within `i64`.
    money: Vec<i128>,
    wide: Vec<bool>,
    goods: Vec<f64>,
    open: Vec<bool>,
    journal: Journal,
    /// Flows whose net amount per account this tick is kept in `tally` ([`Ledger::track`]).
    tracked: u64,
    tally: Vec<i64>,
    created: Vec<f64>,
    destroyed: Vec<f64>,
    opening: Vec<f64>,
}

impl Ledger {
    pub fn new(n_goods: usize) -> Self {
        Self {
            n_goods,
            money: Vec::new(),
            wide: Vec::new(),
            goods: Vec::new(),
            open: Vec::new(),
            journal: Journal::default(),
            tracked: 0,
            tally: Vec::new(),
            created: vec![0.0; n_goods],
            destroyed: vec![0.0; n_goods],
            opening: vec![0.0; n_goods],
        }
    }

    /// Opens a narrow account, whose balance stays within `i64`.
    pub fn open(&mut self) -> AccountId {
        self.open_as(false)
    }

    /// Opens a wide account, whose balance is an `i128`.
    pub fn open_wide(&mut self) -> AccountId {
        self.open_as(true)
    }

    fn open_as(&mut self, wide: bool) -> AccountId {
        let id = AccountId(u32::try_from(self.money.len()).expect("account space exhausted"));
        self.money.push(0);
        self.tally.push(0);
        self.wide.push(wide);
        self.goods.extend(std::iter::repeat_n(0.0, self.n_goods));
        self.open.push(true);
        id
    }

    /// Closes an empty account. Its ID is never reused.
    pub fn close(&mut self, a: AccountId) {
        assert!(self.is_open(a), "closing closed account {a:?}");
        assert_eq!(self.money_wide(a), 0, "closing account {a:?} with money");
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

    pub fn is_wide(&self, a: AccountId) -> bool {
        self.wide[a.0 as usize]
    }

    /// The balance of `a`; panics where it does not fit an `i64` (only a wide account's can
    /// leave that range: read it with [`Ledger::money_wide`]).
    pub fn money(&self, a: AccountId) -> i64 {
        i64::try_from(self.money[a.0 as usize])
            .unwrap_or_else(|_| panic!("account {a:?} holds beyond i64; read it wide"))
    }

    pub fn money_wide(&self, a: AccountId) -> i128 {
        self.money[a.0 as usize]
    }

    /// `balance + delta` for account index `i`, panicking past its kind's range.
    fn moved(&self, i: usize, delta: i128) -> i128 {
        moved(self.money[i], self.wide[i], delta)
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
        let amount_wide = i128::from(amount);
        self.money[f] = self.moved(f, -amount_wide);
        self.money[t] = self.moved(t, amount_wide);
        self.journal.push(Entry {
            flow,
            from,
            to,
            amount,
        });
        if tracks(self.tracked, flow) {
            self.tally[t] += amount;
            self.tally[f] -= amount;
        }
    }

    /// Keeps, from the next [`Ledger::begin_tick`] on, each account's net amount this tick over
    /// the entries of `flows`: the sum the journal gives, without reading it
    /// ([`Ledger::tally`]).
    /// Flow ids below 64 only.
    pub fn track(&mut self, flows: &[Flow]) {
        self.tracked = flows.iter().fold(0, |mask, f| {
            assert!(f.0 < 64, "tracked flow {} is 64 or more", f.0);
            mask | 1 << f.0
        });
    }

    /// Account `a`'s credits less debits this tick in the tracked flows.
    pub fn tally(&self, a: AccountId) -> i64 {
        self.tally[a.0 as usize]
    }

    /// [`Ledger::transfer`] of each entry in turn: the same balances and journal. Each
    /// account's money moves by the integer sum of its entries, summed in `i128` over chunks
    /// in parallel and added in chunk order, so an overflow is caught at a chunk's addition to
    /// the balance rather than at the step that would cross the bound. `scratch` keeps the
    /// chunks' sums between calls.
    pub fn post(&mut self, entries: &[Entry], scratch: &mut PostScratch) {
        self.post_all(&[entries], scratch);
    }

    /// [`Ledger::post`] of the blocks in turn, in one pass: the same balances and journal,
    /// with each block cut into chunks of its own.
    pub fn post_all(&mut self, blocks: &[&[Entry]], scratch: &mut PostScratch) {
        self.apply(blocks, scratch);
        for block in blocks {
            let mut kept = self.journal.buffer();
            // an indexed parallel extend writes in place, in order
            kept.par_extend(block.par_iter().copied());
            self.journal.seal(kept);
        }
    }

    /// [`Ledger::post_all`] without copying the journal: each block is swapped for an empty
    /// buffer of the ledger's pool and kept as it is until [`Ledger::begin_tick`] empties it
    /// into the pool again.
    pub fn post_blocks(&mut self, blocks: &mut [&mut Vec<Entry>], scratch: &mut PostScratch) {
        let refs: Vec<&[Entry]> = blocks.iter().map(|b| &b[..]).collect();
        self.apply(&refs, scratch);
        for block in blocks {
            let kept = std::mem::replace(&mut **block, self.journal.buffer());
            self.journal.seal(kept);
        }
    }

    /// Moves the money of `blocks`: each account by the integer sum of its entries, summed in
    /// `i128` over `POST_LANES` lanes in parallel and added in lane order.
    fn apply(&mut self, blocks: &[&[Entry]], scratch: &mut PostScratch) {
        let all: Vec<&[Entry]> = blocks.iter().flat_map(|b| b.chunks(POST_CHUNK)).collect();
        let n = self.money.len();
        let lanes = POST_LANES.min(all.len());
        if scratch.dirty {
            scratch.deltas.iter_mut().for_each(|d| d.fill(0));
        }
        if scratch.deltas.len() < lanes {
            scratch.deltas.resize_with(lanes, Vec::new);
        }
        if scratch.tallied.len() < lanes {
            scratch.tallied.resize_with(lanes, Vec::new);
        }
        let deltas = &mut scratch.deltas[..lanes];
        // every delta is zero between calls, so this only sizes them
        deltas.iter_mut().for_each(|d| d.resize(n, 0));
        let (open, tracked) = (&self.open, self.tracked);
        scratch.dirty = true;
        let all = &all;
        deltas
            .par_iter_mut()
            .zip(scratch.tallied[..lanes].par_iter_mut())
            .enumerate()
            .for_each(|(lane, (delta, tallied))| {
                tallied.clear();
                for chunk in all.iter().skip(lane).step_by(POST_LANES) {
                    for e in *chunk {
                        debug_assert!(open[e.from.0 as usize] && open[e.to.0 as usize]);
                        let (f, t) = (e.from.0 as usize, e.to.0 as usize);
                        delta[f] -= i128::from(e.amount);
                        delta[t] += i128::from(e.amount);
                        if tracks(tracked, e.flow) {
                            tallied.push(*e);
                        }
                    }
                }
            });
        for e in scratch.tallied[..lanes].iter().flatten() {
            self.tally[e.to.0 as usize] += e.amount;
            self.tally[e.from.0 as usize] -= e.amount;
        }
        // each account adds its lane sums in lane order; accounts are independent, so they
        // run in parallel
        let (deltas, wide) = (&*deltas, &self.wide);
        self.money
            .par_chunks_mut(ACCOUNT_CHUNK)
            .zip(wide.par_chunks(ACCOUNT_CHUNK))
            .enumerate()
            .for_each(|(k, (money, wide))| {
                let lo = k * ACCOUNT_CHUNK;
                for (j, (m, &wide)) in money.iter_mut().zip(wide).enumerate() {
                    for delta in deltas {
                        let d = delta[lo + j];
                        if d != 0 {
                            *m = moved(*m, wide, d);
                        }
                    }
                }
            });
        scratch.deltas[..lanes]
            .par_iter_mut()
            .for_each(|d| d.fill(0));
        scratch.dirty = false;
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

    /// Copies goods `goods` out into `cols`, one column per good, so that each good's moves can
    /// run on their own ([`GoodColumn`]); `cols` keeps its buffers between calls. Until
    /// [`Ledger::put_columns`] the ledger holds stale quantities of those goods.
    pub fn take_columns(&self, goods: std::ops::Range<usize>, cols: &mut Vec<GoodColumn>) {
        assert!(
            goods.end <= self.n_goods,
            "goods {goods:?} of {}",
            self.n_goods
        );
        cols.resize_with(goods.len(), GoodColumn::default);
        cols.par_iter_mut().enumerate().for_each(|(i, c)| {
            let g = goods.start + i;
            c.good = g;
            c.created = self.created[g];
            c.destroyed = self.destroyed[g];
            c.goods.clear();
            c.goods
                .extend(self.goods.chunks_exact(self.n_goods).map(|h| h[g]));
        });
    }

    /// Writes back the columns of [`Ledger::take_columns`].
    pub fn put_columns(&mut self, cols: &[GoodColumn]) {
        for c in cols {
            self.created[c.good] = c.created;
            self.destroyed[c.good] = c.destroyed;
        }
        let ng = self.n_goods;
        self.goods
            .par_chunks_mut(ACCOUNT_CHUNK * ng)
            .enumerate()
            .for_each(|(k, chunk)| {
                let lo = k * ACCOUNT_CHUNK;
                for (j, h) in chunk.chunks_exact_mut(ng).enumerate() {
                    for c in cols {
                        h[c.good] = c.goods[lo + j];
                    }
                }
            });
    }

    /// One [`ProvinceLedger`] per province for their accounts, which must ascend in province
    /// order within each kind (pops, then enterprises, then markets, as opened); no two
    /// provinces share an account. Each takes a journal block from the pool.
    /// [`Ledger::put_province_slices`] writes the results back.
    pub fn province_slices(&mut self, provinces: &[ProvinceAccounts]) -> Vec<ProvinceLedger<'_>> {
        let Some(first) = provinces.first() else {
            return Vec::new();
        };
        let ng = self.n_goods;
        let cuts = [first.enterprises.start as usize, first.market.0 as usize];
        let spans: [Vec<std::ops::Range<usize>>; 3] = [
            provinces
                .iter()
                .map(|p| p.pops.start as usize..p.pops.end as usize)
                .collect(),
            provinces
                .iter()
                .map(|p| p.enterprises.start as usize..p.enterprises.end as usize)
                .collect(),
            provinces
                .iter()
                .map(|p| p.market.0 as usize..p.market.0 as usize + 1)
                .collect(),
        ];
        let n = self.money.len();
        let bounds = [0, cuts[0], cuts[1], n];

        fn by_kind<'a, T>(
            v: &'a mut [T],
            bounds: [usize; 4],
            spans: &[Vec<std::ops::Range<usize>>; 3],
            stride: usize,
        ) -> [std::vec::IntoIter<&'a mut [T]>; 3] {
            let (pops, rest) = v.split_at_mut(bounds[1] * stride);
            let (ents, markets) = rest.split_at_mut((bounds[2] - bounds[1]) * stride);
            [
                carve(pops, 0, &spans[0], stride).into_iter(),
                carve(ents, bounds[1], &spans[1], stride).into_iter(),
                carve(markets, bounds[2], &spans[2], stride).into_iter(),
            ]
        }
        let [mut m0, mut m1, mut m2] = by_kind(&mut self.money, bounds, &spans, 1);
        let [mut g0, mut g1, mut g2] = by_kind(&mut self.goods, bounds, &spans, ng);
        let [mut t0, mut t1, mut t2] = by_kind(&mut self.tally, bounds, &spans, 1);
        let wide = &self.wide;
        let (tracked, journal) = (self.tracked, &mut self.journal);
        provinces
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let r = |k: usize| spans[k][i].clone();
                ProvinceLedger {
                    n_goods: ng,
                    tracked,
                    base: [p.pops.start, p.enterprises.start, p.market.0],
                    len: [
                        p.pops.end - p.pops.start,
                        p.enterprises.end - p.enterprises.start,
                        1,
                    ],
                    money: [
                        m0.next().expect("pops"),
                        m1.next().expect("enterprises"),
                        m2.next().expect("market"),
                    ],
                    wide: [&wide[r(0)], &wide[r(1)], &wide[r(2)]],
                    goods: [
                        g0.next().expect("pops"),
                        g1.next().expect("enterprises"),
                        g2.next().expect("market"),
                    ],
                    tally: [
                        t0.next().expect("pops"),
                        t1.next().expect("enterprises"),
                        t2.next().expect("market"),
                    ],
                    journal: journal.buffer(),
                    created: vec![0.0; ng],
                    destroyed: vec![0.0; ng],
                }
            })
            .collect()
    }

    /// Writes back what the provinces of [`Ledger::province_slices`] did: their journal blocks
    /// appended in the order given and their `created` and `destroyed` partials added in that
    /// order. Pass the results in province order; that order fixes the journal and the sums at
    /// any thread count.
    pub fn put_province_slices(&mut self, done: Vec<ProvinceDone>) {
        for d in done {
            for (total, part) in self.created.iter_mut().zip(&d.created) {
                *total += part;
            }
            for (total, part) in self.destroyed.iter_mut().zip(&d.destroyed) {
                *total += part;
            }
            if d.journal.is_empty() {
                self.journal.pool.push(d.journal);
            } else {
                self.journal.seal(d.journal);
            }
        }
    }

    /// Quantity of `good` destroyed this tick (the tally the goods identity checks).
    pub fn destroyed(&self, good: usize) -> f64 {
        self.destroyed[good]
    }

    pub fn journal(&self) -> JournalView<'_> {
        JournalView(&self.journal)
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
        self.tally.fill(0);
        self.created.fill(0.0);
        self.destroyed.fill(0.0);
        self.opening = self.good_totals();
    }

    pub fn check(&self, tick: u32) -> Result<(), Violation> {
        let net = self
            .money
            .iter()
            .try_fold(0i128, |acc, &m| acc.checked_add(m));
        let Some(net) = net else {
            return Err(Violation::new(
                tick,
                "money",
                "balances overflow i128".into(),
            ));
        };
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
    /// Each account's kind, then its balance as `i64` (narrow) or `i128` (wide).
    fn encode(&self, e: &mut Encoder) {
        e.count(self.n_goods);
        e.bools(&self.wide);
        for (&m, &wide) in self.money.iter().zip(&self.wide) {
            if wide {
                e.i128(m);
            } else {
                e.i64(i64::try_from(m).expect("narrow balance within i64"));
            }
        }
        e.f64s(&self.goods);
        e.bools(&self.open);
    }
}

impl Decode for Ledger {
    fn decode(d: &mut Decoder) -> Result<Self, DecodeError> {
        let mut ledger = Ledger::new(d.count()?);
        ledger.wide = d.bools()?;
        ledger.money = ledger
            .wide
            .iter()
            .map(|&wide| {
                if wide {
                    d.i128()
                } else {
                    d.i64().map(i128::from)
                }
            })
            .collect::<Result<_, _>>()?;
        ledger.goods = d.f64s()?;
        ledger.open = d.bools()?;
        let n = ledger.money.len();
        ledger.tally = vec![0; n];
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
    fn tally_is_the_journal_sum_of_tracked_flows() {
        let mut l = Ledger::new(1);
        let ids: Vec<AccountId> = (0..4).map(|_| l.open()).collect();
        l.track(&[Flow(1), Flow(3)]);
        l.begin_tick();
        let entry = |flow, f: usize, t: usize, amount| Entry {
            flow: Flow(flow),
            from: ids[f],
            to: ids[t],
            amount,
        };
        l.transfer(Flow(1), ids[0], ids[1], 9);
        l.transfer(Flow(2), ids[1], ids[2], 4);
        let block = [entry(3, 2, 0, 5), entry(2, 0, 3, 8), entry(1, 3, 1, -2)];
        l.post_all(&[&block, &block[..1]], &mut PostScratch::default());
        for &a in &ids {
            let want: i64 = l
                .journal()
                .iter()
                .filter(|e| [Flow(1), Flow(3)].contains(&e.flow))
                .map(|e| i64::from(e.to == a) * e.amount - i64::from(e.from == a) * e.amount)
                .sum();
            assert_eq!(l.tally(a), want, "account {a:?}");
        }
        l.begin_tick();
        assert!(ids.iter().all(|&a| l.tally(a) == 0));
    }

    #[test]
    fn columns_are_goods_operations_in_turn() {
        let (mut a, mut b) = (Ledger::new(3), Ledger::new(3));
        let ids: Vec<AccountId> = (0..4).map(|_| (a.open(), b.open()).0).collect();
        for l in [&mut a, &mut b] {
            l.begin_tick();
            l.create(ids[0], 1, 2.5);
            l.create(ids[2], 0, 0.75);
        }
        let ops = [(0, 1, 0.3), (1, 3, 0.1), (2, 2, 0.0), (3, 0, 1e-17)];
        for &(f, t, q) in &ops {
            for g in 0..2 {
                a.ship(ids[f], ids[t], g, q);
                a.destroy(ids[t], g, q / 3.0);
                a.create(ids[f], g, q * 0.7);
            }
        }
        let mut cols = Vec::new();
        b.take_columns(0..2, &mut cols);
        for c in &mut cols {
            for &(f, t, q) in &ops {
                c.ship(ids[f], ids[t], q);
                c.destroy(ids[t], q / 3.0);
                c.create(ids[f], q * 0.7);
            }
        }
        b.put_columns(&cols);
        assert_eq!(a, b);
    }

    /// Two provinces of (2 pops, 1 enterprise, 1 market) each, in the world's account order.
    fn two_provinces() -> (Ledger, Vec<ProvinceAccounts>) {
        let mut l = Ledger::new(3);
        for _ in 0..4 {
            l.open();
        }
        for _ in 0..2 {
            l.open();
        }
        for _ in 0..2 {
            l.open_wide();
        }
        let province = |p: u32| ProvinceAccounts {
            pops: 2 * p..2 * p + 2,
            enterprises: 4 + p..5 + p,
            market: AccountId(6 + p),
        };
        (l, vec![province(0), province(1)])
    }

    fn province_ops(l: &mut ProvinceLedger<'_>, p: u32) {
        let (pop, ent, mkt) = (AccountId(2 * p + 1), AccountId(4 + p), AccountId(6 + p));
        l.create(ent, 1, 2.5 + f64::from(p));
        l.ship(ent, mkt, 1, 1.25);
        l.destroy(ent, 1, 0.5);
        l.transfer(Flow(2), mkt, ent, 700 + i64::from(p));
        l.transfer(Flow(1), ent, pop, 300);
        assert_eq!(l.goods(mkt, 1), 1.25);
    }

    #[test]
    fn province_slices_are_the_ledger_operations_in_province_order() {
        let (mut serial, provinces) = two_provinces();
        let (mut sliced, _) = two_provinces();
        serial.track(&[Flow(2)]);
        sliced.track(&[Flow(2)]);
        for p in 0..2 {
            let (pop, ent, mkt) = (AccountId(2 * p + 1), AccountId(4 + p), AccountId(6 + p));
            serial.create(ent, 1, 2.5 + f64::from(p));
            serial.ship(ent, mkt, 1, 1.25);
            serial.destroy(ent, 1, 0.5);
            serial.transfer(Flow(2), mkt, ent, 700 + i64::from(p));
            serial.transfer(Flow(1), ent, pop, 300);
        }
        let mut slices = sliced.province_slices(&provinces);
        for (p, s) in slices.iter_mut().enumerate() {
            province_ops(s, p as u32);
        }
        let done: Vec<_> = slices.into_iter().map(ProvinceLedger::finish).collect();
        sliced.put_province_slices(done);
        assert!(serial.journal().iter().eq(sliced.journal().iter()));
        for a in 0..serial.n_accounts() {
            let id = AccountId(a as u32);
            assert_eq!(serial.money_wide(id), sliced.money_wide(id));
            assert_eq!(serial.tally(id), sliced.tally(id));
            assert_eq!(serial.goods(id, 1), sliced.goods(id, 1));
        }
        assert_eq!(serial.destroyed(1), sliced.destroyed(1));
        assert_eq!(serial.created, sliced.created);
    }

    #[test]
    #[should_panic(expected = "not in this province")]
    fn a_province_slice_refuses_another_provinces_account() {
        let (mut l, provinces) = two_provinces();
        let mut slices = l.province_slices(&provinces);
        slices[0].transfer(Flow(1), AccountId(4), AccountId(5), 1);
    }

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
        b.post(&entries, &mut PostScratch::default());
        assert_eq!(a, b);
    }

    #[test]
    fn post_over_several_chunks_is_transfer_in_turn() {
        let n = 1000;
        let (mut a, mut b) = (Ledger::new(1), Ledger::new(1));
        let ids: Vec<AccountId> = (0..n).map(|_| (a.open(), b.open()).0).collect();
        let mut x = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = move || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        let mut scratch = PostScratch::default();
        // three and a half chunks, then one chunk on the same scratch
        for len in [7 * POST_CHUNK / 2, POST_CHUNK / 3] {
            let entries: Vec<Entry> = (0..len)
                .map(|_| Entry {
                    flow: Flow(4),
                    from: ids[next() as usize % n],
                    to: ids[next() as usize % n],
                    amount: (next() % 2_000_001) as i64 - 1_000_000,
                })
                .collect();
            for e in &entries {
                a.transfer(e.flow, e.from, e.to, e.amount);
            }
            b.post(&entries, &mut scratch);
            assert_eq!(a, b);
        }
        assert!(scratch.deltas.iter().flatten().all(|&d| d == 0));
    }

    #[test]
    fn post_all_is_post_of_each_block_in_turn() {
        // more accounts than one apply task takes, and blocks across chunk boundaries
        let n = 3 * ACCOUNT_CHUNK + 5;
        let (mut a, mut b) = (Ledger::new(1), Ledger::new(1));
        let ids: Vec<AccountId> = (0..n)
            .map(|i| (a.open_as(i % 9 == 0), b.open_as(i % 9 == 0)).0)
            .collect();
        let mut x = 0x2545_f491_4f6c_dd1du64;
        let mut next = move || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        let blocks: Vec<Vec<Entry>> = [POST_CHUNK + 3, 0, POST_CHUNK / 2, 2 * POST_CHUNK]
            .iter()
            .map(|&len| {
                (0..len)
                    .map(|_| Entry {
                        flow: Flow(2),
                        from: ids[next() as usize % n],
                        to: ids[next() as usize % n],
                        amount: (next() % 2_000_001) as i64 - 1_000_000,
                    })
                    .collect()
            })
            .collect();
        let (mut sa, mut sb) = (PostScratch::default(), PostScratch::default());
        for block in &blocks {
            a.post(block, &mut sa);
        }
        let refs: Vec<&[Entry]> = blocks.iter().map(|b| &b[..]).collect();
        b.post_all(&refs, &mut sb);
        assert_eq!(a, b);
        assert!(sb.deltas.iter().flatten().all(|&d| d == 0));
    }

    #[test]
    fn post_after_an_overflow_starts_from_clean_sums() {
        let mut ledger = Ledger::new(1);
        let (x, y) = (ledger.open(), ledger.open());
        let entry = |amount| Entry {
            flow: Flow(4),
            from: x,
            to: y,
            amount,
        };
        let mut scratch = PostScratch::default();
        let overflow = [entry(5), entry(i64::MAX)];
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ledger.clone().post(&overflow, &mut scratch)
        }));
        assert!(caught.is_err() && scratch.dirty);
        let mut want = ledger.clone();
        want.transfer(Flow(4), x, y, 3);
        ledger.post(&[entry(3)], &mut scratch);
        assert_eq!(ledger, want);
        assert!(!scratch.dirty && scratch.deltas.iter().flatten().all(|&d| d == 0));
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

    /// A wide account holds beyond i64 and round-trips through the codec; a narrow one panics
    /// at the transfer that would take it past i64, in `transfer` and in `post`.
    #[test]
    fn wide_accounts_hold_beyond_i64() {
        let mut l = Ledger::new(1);
        let mint = l.open_wide();
        let pops: Vec<AccountId> = (0..3).map(|_| l.open()).collect();
        for &a in &pops {
            l.transfer(Flow(0), mint, a, i64::MAX);
        }
        assert_eq!(l.money_wide(mint), -3 * i128::from(i64::MAX));
        assert!(l.is_wide(mint) && !l.is_wide(pops[0]));
        assert!(l.check(0).is_ok());
        let mut e = Encoder::default();
        l.encode(&mut e);
        let bytes = e.into_bytes();
        let back = Ledger::decode(&mut Decoder::new(&bytes)).unwrap();
        assert_eq!((&back.money, &back.wide), (&l.money, &l.wide));
        let caught = std::panic::catch_unwind(|| l.money(mint));
        assert!(caught.is_err());
    }

    #[test]
    #[should_panic(expected = "ledger overflow")]
    fn narrow_side_of_a_transfer_overflows_loudly() {
        let mut l = Ledger::new(0);
        let (mint, a) = (l.open_wide(), l.open());
        l.transfer(Flow(0), mint, a, i64::MAX);
        l.transfer(Flow(0), mint, a, 1);
    }

    #[test]
    #[should_panic(expected = "ledger overflow")]
    fn narrow_side_of_a_post_overflows_loudly() {
        let mut l = Ledger::new(0);
        let (mint, a) = (l.open_wide(), l.open());
        let entry = |amount| Entry {
            flow: Flow(0),
            from: mint,
            to: a,
            amount,
        };
        l.post(&[entry(i64::MAX), entry(1)], &mut PostScratch::default());
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
