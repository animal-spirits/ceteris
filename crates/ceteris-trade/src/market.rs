//! Province markets: clearing with pro rata rationing, perishing of local goods and the
//! stock-flow price rule (`specs/trade/SPEC.md`, Equations).
//!
//! All arrays are `[province * N_GOODS + g]`. The caller sets `supply` (Q) and `demand` (D)
//! before clearing and sets the price of inactive cells from netback; `clear`, `clear_local`
//! and `reprice` skip inactive cells.

use ceteris_core::snapshot::{Decode, DecodeError, Decoder, Encode, Encoder};

use crate::goods::{N_GOODS, TRADED};
use crate::params::TradeParams;

/// Q_floor as a fraction of the good's world mean Q over active cells.
const Q_FLOOR_FRACTION: f64 = 1e-9;

#[derive(Clone, Debug, PartialEq)]
pub struct Markets {
    pub price: Vec<f64>,
    /// Stock S carried into the next tick; always 0 for local goods.
    pub inventory: Vec<f64>,
    /// Notional demand D posted by buyers.
    pub demand: Vec<f64>,
    /// Supply Q: home-routed supply plus arrivals, or local output.
    pub supply: Vec<f64>,
    pub sales: Vec<f64>,
    pub active: Vec<bool>,
}

impl Markets {
    /// All cells active at price 1 with no stock, supply or demand.
    pub fn new(provinces: usize) -> Self {
        let n = provinces * N_GOODS;
        Self {
            price: vec![1.0; n],
            inventory: vec![0.0; n],
            demand: vec![0.0; n],
            supply: vec![0.0; n],
            sales: vec![0.0; n],
            active: vec![true; n],
        }
    }

    pub fn n_provinces(&self) -> usize {
        self.price.len() / N_GOODS
    }

    /// Clears the active traded cells: sales = min(D, S + Q), a shortfall rationed pro rata,
    /// S' = S + Q - sales. Writes the fill ratio min(1, (S + Q) / D), 1 when D = 0, into
    /// `fill[i]` for the cells it clears; other cells of `fill` are untouched.
    pub fn clear(&mut self, fill: &mut [f64]) {
        assert_eq!(fill.len(), self.price.len());
        for i in 0..self.price.len() {
            if !self.active[i] || !TRADED[i % N_GOODS] {
                continue;
            }
            let (s, q, d) = (self.inventory[i], self.supply[i], self.demand[i]);
            let avail = s + q;
            if d > avail {
                fill[i] = avail / d;
                self.sales[i] = avail;
                self.inventory[i] = 0.0;
            } else {
                fill[i] = 1.0;
                self.sales[i] = d;
                // S + (Q - D) rather than (S + Q) - D keeps S bit for bit when D = Q
                self.inventory[i] = (s + (q - d)).max(0.0);
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

    /// Price step of spec 8.2 for every active cell, after clearing:
    /// dlnp = clip(kappa1 (D - Q) / Q_eff - kappa2 (S' - S*) / S*_eff, -c_max, c_max),
    /// S* = coverage D, Q_eff and S*_eff floored at 1e-9 of the good's world mean Q over
    /// active cells. Local goods have coverage 0 so the stock term is absent. A good whose
    /// world mean Q is 0 keeps its prices.
    pub fn reprice(&mut self, p: &TradeParams) {
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
            let mut dlnp = p.kappa1 * (d - q) / q.max(floor[g]);
            if p.coverage[g] > 0.0 {
                let target = p.coverage[g] * d;
                dlnp -= p.kappa2 * (self.inventory[i] - target) / target.max(floor[g]);
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
        e.f64s(&self.supply);
        e.f64s(&self.sales);
        e.bools(&self.active);
    }
}

impl Decode for Markets {
    fn decode(d: &mut Decoder) -> Result<Self, DecodeError> {
        let m = Self {
            price: d.f64s()?,
            inventory: d.f64s()?,
            demand: d.f64s()?,
            supply: d.f64s()?,
            sales: d.f64s()?,
            active: d.bools()?,
        };
        let n = m.price.len();
        let lens = [
            m.inventory.len(),
            m.demand.len(),
            m.supply.len(),
            m.sales.len(),
            m.active.len(),
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

    fn set(m: &mut Markets, g: usize, s: f64, q: f64, d: f64) {
        m.inventory[g] = s;
        m.supply[g] = q;
        m.demand[g] = d;
    }

    #[test]
    fn pro_rata_fill_when_short() {
        let (mut m, _) = one();
        set(&mut m, G, 0.5, 1.5, 4.0);
        let mut fill = vec![0.0; N_GOODS];
        m.clear(&mut fill);
        assert_eq!(fill[G], 0.5);
        assert_eq!(m.sales[G], 2.0);
        assert_eq!(m.inventory[G], 0.0);
        // a buyer's allocation is fill x notional demand
        assert_eq!(fill[G] * 3.0, 1.5);
    }

    #[test]
    fn inventory_absorbs_glut() {
        let (mut m, _) = one();
        set(&mut m, G, 1.0, 5.0, 2.0);
        let mut fill = vec![0.0; N_GOODS];
        m.clear(&mut fill);
        assert_eq!(fill[G], 1.0);
        assert_eq!(m.sales[G], 2.0);
        assert_eq!(m.inventory[G], 4.0);
    }

    #[test]
    fn zero_demand_fills_fully_and_stocks_supply() {
        let (mut m, _) = one();
        set(&mut m, G, 1.0, 2.0, 0.0);
        let mut fill = vec![0.0; N_GOODS];
        m.clear(&mut fill);
        assert_eq!((fill[G], m.sales[G], m.inventory[G]), (1.0, 0.0, 3.0));
    }

    #[test]
    fn stock_alone_serves_demand_without_supply() {
        let (mut m, _) = one();
        set(&mut m, G, 3.0, 0.0, 2.0);
        let mut fill = vec![0.0; N_GOODS];
        m.clear(&mut fill);
        assert_eq!((fill[G], m.sales[G], m.inventory[G]), (1.0, 2.0, 1.0));
    }

    #[test]
    fn price_rises_with_excess_demand_and_clips() {
        let (mut m, p) = one();
        // unclipped: D = 1.1 Q with the stock ending on target S* = 2 D
        set(&mut m, G, 2.3, 1.0, 1.1);
        let mut fill = vec![0.0; N_GOODS];
        m.clear(&mut fill);
        // S' = 2.3 + 1.0 - 1.1 = 2.2 = S*, so only the flow term: 0.2 * 0.1
        let before = m.price[G];
        m.reprice(&p);
        let want = before * libm::exp(0.2 * (1.1 - 1.0));
        assert!((m.price[G] - want).abs() < 1e-12, "{}", m.price[G]);
        assert!(m.price[G] > before);

        // clipped: huge excess demand rises by exactly exp(c_max)
        let (mut m, p) = one();
        set(&mut m, G, 0.0, 1.0, 50.0);
        m.clear(&mut fill);
        m.reprice(&p);
        assert_eq!(m.price[G], libm::exp(p.c_max));

        // clipped below: glut with stock far above target falls by exactly exp(-c_max)
        let (mut m, p) = one();
        set(&mut m, G, 100.0, 10.0, 0.1);
        m.clear(&mut fill);
        m.reprice(&p);
        assert_eq!(m.price[G], libm::exp(-p.c_max));
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
        m.reprice(&p);
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
        let mut fill = vec![-1.0; 2 * N_GOODS];
        m.clear(&mut fill);
        assert_eq!(fill[G], 0.5);
        assert_eq!((fill[N_GOODS + G], fill[L]), (-1.0, -1.0));
        assert_eq!((m.sales[N_GOODS + G], m.sales[L]), (0.0, 0.0));
        m.reprice(&rule());
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
            m.clear(&mut fill);
            m.reprice(&p);
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
        m.clear(&mut [0.0; N_GOODS]);
        m.reprice(&p);
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
        m.clear(&mut vec![0.0; 3 * N_GOODS]);
        m.reprice(&p);
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
                m.supply[i] = dd;
                m.inventory[i] = p.coverage[g] * dd;
                m.price[i] = 1.0 + 0.1 * (pi + g) as f64;
            }
        }
        let before = m.price.clone();
        let stock = m.inventory.clone();
        let mut fill = vec![0.0; 2 * N_GOODS];
        let mut spoiled = vec![0.0; 2 * N_GOODS];
        for _ in 0..10 {
            m.clear(&mut fill);
            m.clear_local(&mut spoiled);
            m.reprice(&p);
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
        m.clear(&mut [0.0; N_GOODS]);
        for (g, b) in before.iter().enumerate() {
            assert!((m.sales[g] + m.inventory[g] - b).abs() < 1e-12);
        }
    }

    #[test]
    fn round_trips_through_snapshot_bytes() {
        let mut m = Markets::new(2);
        set(&mut m, G, 1.0, 2.0, 3.0);
        m.active[5] = false;
        m.clear(&mut [0.0; 2 * N_GOODS]);
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
        for _ in 0..4 {
            e.f64s(&[0.0; N_GOODS]);
        }
        e.bools(&[true; 3]);
        let bytes = e.into_bytes();
        assert!(Markets::decode(&mut Decoder::new(&bytes)).is_err());
    }
}
