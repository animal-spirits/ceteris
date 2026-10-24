//! Province markets: origin clearing of traded goods with rationing between buyer classes
//! (ADR 0016), perishing of local goods and the stock-flow price rule (`specs/trade/SPEC.md`,
//! Equations).
//!
//! All arrays are `[province * N_GOODS + g]`. A traded cell is the origin market of the
//! province's output: the caller sets `supply` (Q, this tick's output) and the orders placed
//! at it by every destination (`demand_int`, `demand_hh`) before [`Markets::clear`]. A local
//! cell is the province market: the caller sets `supply` (local output) and `demand` before
//! [`Markets::clear_local`]. `active` means the cell has a market: a traded cell with output
//! capacity, a local cell anyone in the province buys. Prices of inactive cells are not read;
//! `clear`, `clear_local` and `reprice` skip inactive cells.

use ceteris_core::snapshot::{Decode, DecodeError, Decoder, Encode, Encoder};

use crate::goods::{N_GOODS, TRADED};
use crate::params::TradeParams;

/// Q_floor as a fraction of the good's world mean Q over active cells.
const Q_FLOOR_FRACTION: f64 = 1e-9;

/// S11: how a shortfall is shared between the two buyer classes (`specs/trade/SPEC.md`,
/// Deviations, rationing row). Index 0 of a fill pair is intermediate users, index 1
/// households.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rationing {
    /// DESIGN 8.2: households' essential consumption, then intermediate users pro rata, then
    /// the rest. P2 has no essential consumption levels (P4), so it fills as `Mixed`.
    Priority,
    /// One fill ratio for both classes (Pichler and Farmer 2022, Algorithm 1).
    Proportional,
    /// Intermediate users pro rata at min(1, A / D_int), households the residual
    /// max(A - D_int, 0) / D_hh (Pichler and Farmer 2022, Algorithm 2, eqs. (12) and (15)).
    Mixed,
}

impl Rationing {
    /// Fill ratios `[intermediate, household]` of orders `d_int` and `d_hh` against `avail`;
    /// 1 for a class with no orders.
    pub fn fill(self, avail: f64, d_int: f64, d_hh: f64) -> [f64; 2] {
        let d = d_int + d_hh;
        if d <= avail {
            return [1.0, 1.0];
        }
        match self {
            Rationing::Proportional => [avail / d; 2],
            // no essential consumption before P4: Priority is Mixed
            Rationing::Priority | Rationing::Mixed if d_int > avail => {
                [avail / d_int, if d_hh > 0.0 { 0.0 } else { 1.0 }]
            }
            Rationing::Priority | Rationing::Mixed => [1.0, (avail - d_int) / d_hh],
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Markets {
    pub price: Vec<f64>,
    /// Merchant stock S carried into the next tick; always 0 for local goods.
    pub inventory: Vec<f64>,
    /// Total orders D, read by `reprice`. [`Markets::clear`] writes it as
    /// `demand_int + demand_hh` on the cells it clears; for local cells the caller sets it.
    pub demand: Vec<f64>,
    /// Orders by intermediate users at the origin, in units at the origin.
    pub demand_int: Vec<f64>,
    /// Orders by households at the origin, in units at the origin.
    pub demand_hh: Vec<f64>,
    /// Supply Q: this tick's output (traded) or local output.
    pub supply: Vec<f64>,
    pub sales: Vec<f64>,
    pub active: Vec<bool>,
    /// Ticks at which the step clip c_max (P03c) bound at the cell, counted by `reprice`
    /// since the start of the run.
    pub clip_ticks: Vec<u32>,
}

impl Markets {
    /// All cells active at price 1 with no stock, supply or demand.
    pub fn new(provinces: usize) -> Self {
        let n = provinces * N_GOODS;
        Self {
            price: vec![1.0; n],
            inventory: vec![0.0; n],
            demand: vec![0.0; n],
            demand_int: vec![0.0; n],
            demand_hh: vec![0.0; n],
            supply: vec![0.0; n],
            sales: vec![0.0; n],
            active: vec![true; n],
            clip_ticks: vec![0; n],
        }
    }

    pub fn n_provinces(&self) -> usize {
        self.price.len() / N_GOODS
    }

    /// Clears the active traded cells at the origin: D = `demand_int + demand_hh` (written to
    /// `demand`), sales = min(D, S + Q), a shortfall shared by `rationing`
    /// ([`Rationing::fill`]), S' = S + Q - sales. Writes the fill ratios `[intermediate,
    /// household]` into `fill[i]` for the cells it clears, 1 for a class with no orders; other
    /// cells of `fill` are untouched.
    pub fn clear(&mut self, rationing: Rationing, fill: &mut [[f64; 2]]) {
        assert_eq!(fill.len(), self.price.len());
        for i in 0..self.price.len() {
            if !self.active[i] || !TRADED[i % N_GOODS] {
                continue;
            }
            let (s, q) = (self.inventory[i], self.supply[i]);
            let (d_int, d_hh) = (self.demand_int[i], self.demand_hh[i]);
            let d = d_int + d_hh;
            self.demand[i] = d;
            let avail = s + q;
            fill[i] = rationing.fill(avail, d_int, d_hh);
            if d <= avail {
                self.sales[i] = d;
                // S + (Q - D) rather than (S + Q) - D keeps S bit for bit when D = Q
                self.inventory[i] = (s + (q - d)).max(0.0);
            } else {
                self.sales[i] = avail;
                self.inventory[i] = 0.0;
            }
        }
    }

    /// Clears the active local cells: sales = min(D, Q), the unsold rest perishes and is
    /// written to `spoiled[i]` (0 for every other cell) so the caller can destroy it in the
    /// ledger. Inventory stays 0.
    pub fn clear_local(&mut self, spoiled: &mut [f64]) {
        assert_eq!(spoiled.len(), self.price.len());
        spoiled.fill(0.0);
        for i in 0..self.price.len() {
            if !self.active[i] || TRADED[i % N_GOODS] {
                continue;
            }
            let (q, d) = (self.supply[i], self.demand[i]);
            let sold = d.min(q);
            self.sales[i] = sold;
            spoiled[i] = q - sold;
            self.inventory[i] = 0.0;
        }
    }

    /// Price step of spec 8.2 for every active cell, after clearing, with the merchants'
    /// expected inflation pi^e (ADR 0019):
    /// dlnp = clip(pi^e + kappa1 (D - Q) / Q_eff - kappa2 (S' - S*) / S*_eff, -c_max, c_max),
    /// S* = coverage D, Q_eff and S*_eff floored at 1e-9 of the good's world mean Q over
    /// active cells. Local goods have coverage 0 so the stock term is absent. A good whose
    /// world mean Q is 0 keeps its prices. Each cell whose step is clipped counts the tick in
    /// `clip_ticks`.
    pub fn reprice(&mut self, p: &TradeParams, expected_inflation: f64) {
        let mut floor = [0.0; N_GOODS];
        let mut count = [0usize; N_GOODS];
        for (i, &q) in self.supply.iter().enumerate() {
            if self.active[i] {
                floor[i % N_GOODS] += q;
                count[i % N_GOODS] += 1;
            }
        }
        for g in 0..N_GOODS {
            floor[g] = Q_FLOOR_FRACTION * floor[g] / count[g].max(1) as f64;
        }
        for i in 0..self.price.len() {
            let g = i % N_GOODS;
            if !self.active[i] || floor[g] <= 0.0 {
                continue;
            }
            let (q, d) = (self.supply[i], self.demand[i]);
            let mut dlnp = expected_inflation + p.kappa1 * (d - q) / q.max(floor[g]);
            if p.coverage[g] > 0.0 {
                let target = p.coverage[g] * d;
                dlnp -= p.kappa2 * (self.inventory[i] - target) / target.max(floor[g]);
            }
            if dlnp.abs() > p.c_max {
                self.clip_ticks[i] += 1;
            }
            self.price[i] *= libm::exp(dlnp.clamp(-p.c_max, p.c_max));
        }
    }
}

impl Encode for Markets {
    fn encode(&self, e: &mut Encoder) {
        e.f64s(&self.price);
        e.f64s(&self.inventory);
        e.f64s(&self.demand);
        e.f64s(&self.demand_int);
        e.f64s(&self.demand_hh);
        e.f64s(&self.supply);
        e.f64s(&self.sales);
        e.bools(&self.active);
        e.u32s(&self.clip_ticks);
    }
}

impl Decode for Markets {
    fn decode(d: &mut Decoder) -> Result<Self, DecodeError> {
        let m = Self {
            price: d.f64s()?,
            inventory: d.f64s()?,
            demand: d.f64s()?,
            demand_int: d.f64s()?,
            demand_hh: d.f64s()?,
            supply: d.f64s()?,
            sales: d.f64s()?,
            active: d.bools()?,
            clip_ticks: d.u32s()?,
        };
        let n = m.price.len();
        let lens = [
            m.inventory.len(),
            m.demand.len(),
            m.demand_int.len(),
            m.demand_hh.len(),
            m.supply.len(),
            m.sales.len(),
            m.active.len(),
            m.clip_ticks.len(),
        ];
        if !n.is_multiple_of(N_GOODS) || lens.iter().any(|&l| l != n) {
            return Err(DecodeError(format!("market arrays {n} and {lens:?}")));
        }
        Ok(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ceteris_core::snapshot::canonical;

    const G: usize = 3; // food, traded
    const L: usize = 9; // services, local

    /// Both terms of the rule on, at the speeds these tests were written for (the defaults
    /// switch the inventory term off since Task 7b).
    fn rule() -> TradeParams {
        TradeParams {
            kappa1: 0.2,
            kappa2: 0.1,
            ..TradeParams::default()
        }
    }

    fn one() -> (Markets, TradeParams) {
        (Markets::new(1), rule())
    }

    /// Stock S, supply Q and demand D at a cell; on a traded cell D is household orders.
    fn set(m: &mut Markets, g: usize, s: f64, q: f64, d: f64) {
        m.inventory[g] = s;
        m.supply[g] = q;
        m.demand[g] = d;
        m.demand_int[g] = 0.0;
        m.demand_hh[g] = d;
    }

    /// Origin clearing with one fill ratio; the household fill of every cell it clears.
    fn clear(m: &mut Markets) -> Vec<f64> {
        let mut fill = vec![[f64::NAN; 2]; m.price.len()];
        m.clear(Rationing::Proportional, &mut fill);
        fill.iter().map(|f| f[1]).collect()
    }

    #[test]
    fn inventory_absorbs_glut() {
        let (mut m, _) = one();
        set(&mut m, G, 1.0, 5.0, 2.0);
        let fill = clear(&mut m);
        assert_eq!(fill[G], 1.0);
        assert_eq!(m.sales[G], 2.0);
        assert_eq!(m.inventory[G], 4.0);
    }

    #[test]
    fn zero_demand_fills_fully_and_stocks_supply() {
        let (mut m, _) = one();
        set(&mut m, G, 1.0, 2.0, 0.0);
        let fill = clear(&mut m);
        assert_eq!((fill[G], m.sales[G], m.inventory[G]), (1.0, 0.0, 3.0));
    }

    #[test]
    fn stock_alone_serves_demand_without_supply() {
        let (mut m, _) = one();
        set(&mut m, G, 3.0, 0.0, 2.0);
        let fill = clear(&mut m);
        assert_eq!((fill[G], m.sales[G], m.inventory[G]), (1.0, 2.0, 1.0));
    }

    #[test]
    fn price_rises_with_excess_demand_and_clips() {
        let (mut m, p) = one();
        // unclipped: D = 1.1 Q with the stock ending on target S* = 2 D
        set(&mut m, G, 2.3, 1.0, 1.1);
        clear(&mut m);
        // S' = 2.3 + 1.0 - 1.1 = 2.2 = S*, so only the flow term: 0.2 * 0.1
        let before = m.price[G];
        m.reprice(&p, 0.0);
        let want = before * libm::exp(0.2 * (1.1 - 1.0));
        assert!((m.price[G] - want).abs() < 1e-12, "{}", m.price[G]);
        assert!(m.price[G] > before);

        // clipped: huge excess demand rises by exactly exp(c_max)
        let (mut m, p) = one();
        set(&mut m, G, 0.0, 1.0, 50.0);
        clear(&mut m);
        m.reprice(&p, 0.0);
        assert_eq!(m.price[G], libm::exp(p.c_max));

        // clipped below: glut with stock far above target falls by exactly exp(-c_max)
        let (mut m, p) = one();
        set(&mut m, G, 100.0, 10.0, 0.1);
        clear(&mut m);
        m.reprice(&p, 0.0);
        assert_eq!(m.price[G], libm::exp(-p.c_max));
    }

    #[test]
    fn reprice_adds_expected_inflation() {
        // traded: D = Q and S' = S* = 2 D; local: D = Q
        let (mut m, p) = one();
        set(&mut m, G, 2.0, 1.0, 1.0);
        set(&mut m, L, 0.0, 1.0, 1.0);
        clear(&mut m);
        let mut spoiled = vec![0.0; N_GOODS];
        m.clear_local(&mut spoiled);
        assert_eq!(m.inventory[G], p.coverage[G] * m.demand[G]);
        let pi = 0.0123;
        m.reprice(&p, pi);
        for c in [G, L] {
            assert_eq!(m.price[c], libm::exp(pi), "cell {c}");
        }
        assert_eq!(m.clip_ticks[G], 0);
        // deflation expected: the same, below 1
        let (mut m, p) = one();
        set(&mut m, G, 2.0, 1.0, 1.0);
        clear(&mut m);
        m.reprice(&p, -0.004);
        assert_eq!(m.price[G], libm::exp(-0.004));
    }

    #[test]
    fn clip_ticks_count_bindings() {
        let mut m = Markets::new(2);
        let p = rule();
        let cells = [G, N_GOODS + G, L, N_GOODS + L];
        // excess demand clips up, a glut clips down, a balanced cell does not clip
        let tick = |m: &mut Markets, pi: f64| {
            set(m, G, 0.0, 1.0, 50.0);
            set(m, N_GOODS + G, 100.0, 10.0, 0.1);
            set(m, L, 0.0, 1.0, 1.0);
            set(m, N_GOODS + L, 0.0, 1.0, 1.0);
            clear(m);
            let mut spoiled = vec![0.0; 2 * N_GOODS];
            m.clear_local(&mut spoiled);
            m.reprice(&p, pi);
            cells.map(|c| m.clip_ticks[c])
        };
        assert_eq!(tick(&mut m, 0.0), [1, 1, 0, 0]);
        assert_eq!(tick(&mut m, 0.0), [2, 2, 0, 0]);
        // expected inflation beyond the clip binds on the balanced cells too
        assert_eq!(tick(&mut m, 0.5), [3, 3, 1, 1]);
        assert_eq!(m.price[L], libm::exp(p.c_max));
        // an inactive cell never counts
        m.active[N_GOODS + L] = false;
        assert_eq!(tick(&mut m, -0.5), [4, 4, 2, 1]);
        let others: u32 = (0..2 * N_GOODS)
            .filter(|c| !cells.contains(c))
            .map(|c| m.clip_ticks[c])
            .sum();
        assert_eq!(others, 0);
    }

    #[test]
    fn local_good_ignores_inventory_term() {
        let (mut m, p) = one();
        set(&mut m, L, 0.0, 1.0, 1.1);
        // a stale stock must not enter the local price rule
        m.inventory[L] = 1e6;
        let mut spoiled = vec![0.0; N_GOODS];
        m.clear_local(&mut spoiled);
        assert_eq!(m.inventory[L], 0.0);
        m.reprice(&p, 0.0);
        assert!((m.price[L] - libm::exp(0.2 * 0.1)).abs() < 1e-12);
    }

    #[test]
    fn local_goods_perish_what_is_unsold() {
        let (mut m, _) = one();
        set(&mut m, L, 0.0, 5.0, 2.0);
        set(&mut m, L - 1, 0.0, 1.0, 3.0);
        set(&mut m, G, 0.0, 9.0, 1.0);
        let mut spoiled = vec![7.0; N_GOODS];
        m.clear_local(&mut spoiled);
        assert_eq!((m.sales[L], spoiled[L]), (2.0, 3.0));
        assert_eq!((m.sales[L - 1], spoiled[L - 1]), (1.0, 0.0));
        assert_eq!(spoiled[G], 0.0);
        assert_eq!(m.sales[G], 0.0);
    }

    #[test]
    fn clear_leaves_local_and_inactive_cells_alone() {
        let mut m = Markets::new(2);
        set(&mut m, G, 0.0, 1.0, 2.0);
        set(&mut m, N_GOODS + G, 0.0, 1.0, 2.0);
        set(&mut m, L, 0.0, 1.0, 2.0);
        m.active[N_GOODS + G] = false;
        m.price[N_GOODS + G] = 3.0;
        let fill = clear(&mut m);
        assert_eq!(fill[G], 0.5);
        assert!(fill[N_GOODS + G].is_nan() && fill[L].is_nan());
        assert_eq!((m.sales[N_GOODS + G], m.sales[L]), (0.0, 0.0));
        m.reprice(&rule(), 0.0);
        assert_eq!(m.price[N_GOODS + G], 3.0);
    }

    #[test]
    fn all_import_province_with_zero_q_stays_finite() {
        // province 0 produces and supplies; province 1 only imports, so Q = 0, D > 0
        let mut m = Markets::new(2);
        let p = rule();
        for g in 0..7 {
            set(&mut m, g, 0.0, 10.0, 1.0);
            set(&mut m, N_GOODS + g, 0.0, 0.0, 4.0);
        }
        let mut fill = vec![0.0; 2 * N_GOODS];
        for _ in 0..50 {
            fill = clear(&mut m);
            m.reprice(&p, 0.0);
            assert!(m.price.iter().all(|x| x.is_finite() && *x > 0.0));
            assert!(m.inventory.iter().all(|&s| s >= 0.0));
            for g in 0..7 {
                m.supply[N_GOODS + g] = 0.0;
                m.inventory[N_GOODS + g] = 0.0;
            }
        }
        // no supply and rising demand pressure: the clip bounds each step
        assert_eq!(fill[N_GOODS + G], 0.0);
        let want = libm::exp(p.c_max * 50.0);
        assert!((m.price[N_GOODS + G] / want - 1.0).abs() < 1e-9);
    }

    #[test]
    fn world_with_no_supply_of_a_good_keeps_its_price() {
        let (mut m, p) = one();
        set(&mut m, G, 0.0, 0.0, 4.0);
        m.price[G] = 2.5;
        clear(&mut m);
        m.reprice(&p, 0.0);
        assert_eq!(m.price[G], 2.5);
    }

    #[test]
    fn floor_uses_the_mean_over_active_cells_only() {
        let mut m = Markets::new(3);
        let p = rule();
        // provinces 0 and 1 active with Q 4 and 0; province 2 inactive with huge Q
        set(&mut m, G, 0.0, 4.0, 4.0);
        set(&mut m, N_GOODS + G, 0.0, 0.0, 1.0);
        set(&mut m, 2 * N_GOODS + G, 0.0, 1e12, 1.0);
        m.active[2 * N_GOODS + G] = false;
        clear(&mut m);
        m.reprice(&p, 0.0);
        // province 1: floor = 1e-9 * 2, flow term 0.2 * 1 / 2e-9 clips; stock term pushes down
        // from S' = 0 vs S* = 2 (kappa2 * 1) but the clip still binds upward
        assert_eq!(m.price[N_GOODS + G], libm::exp(p.c_max));
    }

    #[test]
    fn steady_state_is_fixed_point() {
        let mut m = Markets::new(2);
        let p = rule();
        // awkward values: (S + Q) - D is not S in floating point for these
        let d = 0.1 + 0.2;
        for pi in 0..2 {
            for g in 0..N_GOODS {
                let i = pi * N_GOODS + g;
                let dd = d * (1.0 + g as f64 / 7.0);
                m.demand[i] = dd;
                m.demand_hh[i] = dd;
                m.supply[i] = dd;
                m.inventory[i] = p.coverage[g] * dd;
                m.price[i] = 1.0 + 0.1 * (pi + g) as f64;
            }
        }
        let before = m.price.clone();
        let stock = m.inventory.clone();
        let mut spoiled = vec![0.0; 2 * N_GOODS];
        for _ in 0..10 {
            clear(&mut m);
            m.clear_local(&mut spoiled);
            m.reprice(&p, 0.0);
        }
        for (a, b) in m.price.iter().zip(&before) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
        for (a, b) in m.inventory.iter().zip(&stock) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
        assert!(spoiled.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn goods_are_conserved_by_clearing() {
        let mut m = Markets::new(1);
        for (g, (s, q, d)) in [(0.3, 2.0, 7.0), (4.0, 1.0, 0.5), (0.0, 0.0, 0.0)]
            .into_iter()
            .enumerate()
        {
            set(&mut m, g, s, q, d);
        }
        let before: Vec<f64> = (0..3).map(|g| m.inventory[g] + m.supply[g]).collect();
        clear(&mut m);
        for (g, b) in before.iter().enumerate() {
            assert!((m.sales[g] + m.inventory[g] - b).abs() < 1e-12);
        }
    }

    /// Inventory S, output Q and the orders of both classes at an origin cell.
    fn order(m: &mut Markets, g: usize, s: f64, q: f64, int: f64, hh: f64) {
        m.inventory[g] = s;
        m.supply[g] = q;
        m.demand_int[g] = int;
        m.demand_hh[g] = hh;
    }

    #[test]
    fn origin_clears_orders_against_output_and_inventory() {
        for rationing in [Rationing::Priority, Rationing::Proportional] {
            // short: output and stock are all sold, nothing is carried
            let (mut m, _) = one();
            order(&mut m, G, 1.0, 2.0, 1.0, 5.0);
            let mut fill = vec![[-1.0; 2]; N_GOODS];
            m.clear(rationing, &mut fill);
            assert_eq!((m.sales[G], m.inventory[G], m.demand[G]), (3.0, 0.0, 6.0));
            assert_eq!(fill[G][0] * 1.0 + fill[G][1] * 5.0, 3.0);
            // enough: every order is filled from output and stock
            let (mut m, _) = one();
            order(&mut m, G, 2.0, 1.0, 1.0, 1.5);
            m.clear(rationing, &mut fill);
            assert_eq!(fill[G], [1.0, 1.0]);
            assert_eq!((m.sales[G], m.inventory[G], m.demand[G]), (2.5, 0.5, 2.5));
        }
    }

    #[test]
    fn excess_supply_goes_to_inventory() {
        let (mut m, _) = one();
        order(&mut m, G, 1.0, 5.0, 1.0, 1.0);
        let mut fill = vec![[0.0; 2]; N_GOODS];
        m.clear(Rationing::Priority, &mut fill);
        assert_eq!(fill[G], [1.0, 1.0]);
        assert_eq!((m.sales[G], m.inventory[G]), (2.0, 4.0));
        // no orders at all: output goes to stock whole
        order(&mut m, G, 1.0, 2.0, 0.0, 0.0);
        m.clear(Rationing::Proportional, &mut fill);
        assert_eq!(fill[G], [1.0, 1.0]);
        assert_eq!((m.sales[G], m.inventory[G]), (0.0, 3.0));
        // S + (Q - D) keeps S bit for bit when the orders equal output
        let (d_int, d_hh) = (0.1, 0.2);
        order(&mut m, G, 0.7, d_int + d_hh, d_int, d_hh);
        m.clear(Rationing::Priority, &mut fill);
        assert_eq!(m.inventory[G].to_bits(), 0.7f64.to_bits());
    }

    #[test]
    fn priority_fills_intermediate_first() {
        let (mut m, _) = one();
        let mut fill = vec![[0.0; 2]; N_GOODS];
        // intermediate orders exceed what is available: households get nothing
        order(&mut m, G, 0.5, 1.5, 4.0, 3.0);
        m.clear(Rationing::Priority, &mut fill);
        assert_eq!(fill[G], [0.5, 0.0]);
        assert_eq!((m.sales[G], m.inventory[G]), (2.0, 0.0));
        // intermediate filled, households share the rest
        order(&mut m, G, 0.5, 1.5, 1.0, 4.0);
        m.clear(Rationing::Priority, &mut fill);
        assert_eq!(fill[G], [1.0, 0.25]);
        assert_eq!((m.sales[G], m.inventory[G]), (2.0, 0.0));
        // no household orders while intermediate users are short
        order(&mut m, G, 0.0, 1.0, 4.0, 0.0);
        m.clear(Rationing::Priority, &mut fill);
        assert_eq!(fill[G], [0.25, 1.0]);
    }

    #[test]
    fn proportional_fills_one_ratio() {
        let (mut m, _) = one();
        let mut fill = vec![[0.0; 2]; N_GOODS];
        order(&mut m, G, 0.5, 2.5, 1.0, 5.0);
        m.clear(Rationing::Proportional, &mut fill);
        assert_eq!(fill[G], [0.5, 0.5]);
        assert_eq!((m.sales[G], m.inventory[G]), (3.0, 0.0));
    }

    /// Mixed proportional/priority rationing of Pichler and Farmer (2022, Algorithm 2, eqs.
    /// (12) and (15)) at an origin with A = S + Q: intermediate users are filled pro rata at
    /// min(1, A / D_int), households take the residual max(A - D_int, 0) / D_hh. Without
    /// essential consumption levels (P4), Priority fills the same.
    #[test]
    fn mixed_rationing_matches_definition() {
        let mut fill = vec![[0.0; 2]; N_GOODS];
        // A = 0.5 + 1.5 = 2 against intermediate orders 1.6: filled, households share 0.4
        // of 2.0; against 2.5: r = 0.8, households get nothing
        for (int, hh, want) in [(1.6, 2.0, [1.0, 0.2]), (2.5, 3.0, [0.8, 0.0])] {
            for rationing in [Rationing::Mixed, Rationing::Priority] {
                let (mut m, _) = one();
                order(&mut m, G, 0.5, 1.5, int, hh);
                m.clear(rationing, &mut fill);
                for c in 0..2 {
                    assert!((fill[G][c] - want[c]).abs() < 1e-15, "{rationing:?} {c}");
                }
                assert_eq!((m.sales[G], m.inventory[G]), (2.0, 0.0));
            }
        }
        // no household orders: the intermediate fill alone
        let (mut m, _) = one();
        order(&mut m, G, 0.0, 1.0, 4.0, 0.0);
        m.clear(Rationing::Mixed, &mut fill);
        assert_eq!(fill[G], [0.25, 1.0]);
    }

    #[test]
    fn origin_clearing_leaves_local_and_inactive_cells_alone() {
        let mut m = Markets::new(2);
        order(&mut m, G, 0.0, 1.0, 1.0, 1.0);
        order(&mut m, N_GOODS + G, 0.0, 1.0, 1.0, 1.0);
        order(&mut m, L, 0.0, 1.0, 1.0, 1.0);
        m.active[N_GOODS + G] = false;
        let mut fill = vec![[-1.0; 2]; 2 * N_GOODS];
        m.clear(Rationing::Priority, &mut fill);
        assert_eq!(fill[G], [1.0, 0.0]);
        assert_eq!((fill[N_GOODS + G], fill[L]), ([-1.0; 2], [-1.0; 2]));
        assert_eq!((m.sales[N_GOODS + G], m.sales[L]), (0.0, 0.0));
    }

    #[test]
    fn round_trips_through_snapshot_bytes() {
        let mut m = Markets::new(2);
        set(&mut m, G, 1.0, 2.0, 3.0);
        m.active[5] = false;
        clear(&mut m);
        order(&mut m, N_GOODS + G, 0.5, 1.0, 2.0, 0.25);
        let mut e = Encoder::default();
        m.encode(&mut e);
        let bytes = e.into_bytes();
        let mut d = Decoder::new(&bytes);
        let back = Markets::decode(&mut d).unwrap();
        d.finish().unwrap();
        assert_eq!(back, m);
        assert_eq!(canonical(3, &back), canonical(3, &m));
    }

    #[test]
    fn decode_rejects_mismatched_lengths() {
        let mut e = Encoder::default();
        e.f64s(&[1.0; N_GOODS]);
        for _ in 0..6 {
            e.f64s(&[0.0; N_GOODS]);
        }
        e.bools(&[true; 3]);
        e.u32s(&[0; N_GOODS]);
        let bytes = e.into_bytes();
        assert!(Markets::decode(&mut Decoder::new(&bytes)).is_err());
    }
}
