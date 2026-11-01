//! What the app shows of the economy (econ SPEC, App): a view of one finished tick, copied on
//! the sim thread so the frame loop never touches the world (ADR 0208), and the P2 map-mode
//! columns per pack province.

use std::sync::Arc;

use ceteris_core::sim::Sim;
use ceteris_econ::EconWorld;
use ceteris_trade::Goods;

/// What does not change over a run.
pub struct Layout {
    /// Pack province of each econ province.
    pub pack: Vec<u32>,
    pub pop_offsets: Vec<u32>,
    pub ent_offsets: Vec<u32>,
    pub ent_good: Vec<u8>,
    pub pop_edu: Vec<u8>,
    pub labour_force: Vec<f64>,
    pub goods: Goods,
}

impl Layout {
    pub fn new(w: &EconWorld) -> Self {
        let inp = w.inputs();
        let n = inp.provinces.country.len();
        let pack = if inp.provinces.pack.is_empty() {
            (0..n as u32).collect()
        } else {
            inp.provinces.pack.clone()
        };
        Self {
            pack,
            pop_offsets: w.layout().pop_offsets.clone(),
            ent_offsets: w.layout().ent_offsets.clone(),
            ent_good: inp.enterprises.good.clone(),
            pop_edu: inp.pops.edu.clone(),
            labour_force: inp.pops.labour_force.clone(),
            goods: inp.goods.clone(),
        }
    }

    pub fn provinces(&self) -> usize {
        self.pack.len()
    }

    pub fn pops_of(&self, i: usize) -> std::ops::Range<usize> {
        self.pop_offsets[i] as usize..self.pop_offsets[i + 1] as usize
    }

    pub fn ents_of(&self, i: usize) -> std::ops::Range<usize> {
        self.ent_offsets[i] as usize..self.ent_offsets[i + 1] as usize
    }
}

/// One tick's values. Cells are `[province * n_goods + g]` over econ provinces.
pub struct View {
    pub tick: u32,
    pub layout: Arc<Layout>,
    pub world_price: f64,
    /// Buyer price q_d of each good at each province.
    pub buyer_price: Vec<f64>,
    /// Market price, merchant inventory, supply and orders at each cell.
    pub price: Vec<f64>,
    pub inventory: Vec<f64>,
    pub supply: Vec<f64>,
    pub demand: Vec<f64>,
    /// Value the province's market sold this tick.
    pub sold: Vec<f64>,
    /// Value the province's market sold to all buyers less what its own buyers spent on the
    /// good at delivered prices: net shipments out, 0 for local goods.
    pub net_out: Vec<f64>,
    pub wage: Vec<f64>,
    pub employment: Vec<f64>,
    pub output: Vec<f64>,
    pub profit: Vec<f64>,
    pub employed: Vec<f64>,
    pub income: Vec<f64>,
}

impl View {
    pub fn new(sim: &Sim<EconWorld>, layout: &Arc<Layout>) -> Self {
        let w = sim.world();
        let n = layout.provinces();
        let s = &w.scratch;
        let sold: Vec<f64> = (0..n * layout.goods.n())
            .map(|c| s.dispatched.get(c).copied().unwrap_or(0.0) * w.markets.price[c])
            .collect();
        let net_out = sold
            .iter()
            .enumerate()
            .map(|(c, sold)| sold - s.class_spend.get(c).map_or(0.0, |[a, b]| a + b))
            .collect();
        Self {
            tick: sim.tick(),
            layout: Arc::clone(layout),
            world_price: w.world_price_index(),
            buyer_price: w.price_index.clone(),
            price: w.markets.price.clone(),
            inventory: w.markets.inventory.clone(),
            supply: w.markets.supply.clone(),
            demand: w.markets.demand.clone(),
            sold,
            net_out,
            wage: w.wage.clone(),
            employment: w.employment.clone(),
            output: w.output.clone(),
            profit: w.profit.clone(),
            employed: w.employed.clone(),
            income: w.income.clone(),
        }
    }

    /// One value per pack province (`provinces` of them), NaN where the economy has none.
    fn per_pack(&self, provinces: usize, value: impl Fn(usize) -> f64) -> Vec<f64> {
        let mut out = vec![f64::NAN; provinces];
        for (i, &p) in self.layout.pack.iter().enumerate() {
            if let Some(slot) = out.get_mut(p as usize) {
                *slot = value(i);
            }
        }
        out
    }

    /// Buyer price of good `g` over the world price index.
    pub fn price_of(&self, provinces: usize, g: usize) -> Vec<f64> {
        self.per_pack(provinces, |i| {
            self.buyer_price[i * self.layout.goods.n() + g] / self.world_price
        })
    }

    /// Output of sector `g`, at its market price.
    pub fn output_of(&self, provinces: usize, g: usize) -> Vec<f64> {
        self.per_pack(provinces, |i| {
            self.layout
                .ents_of(i)
                .filter(|&e| usize::from(self.layout.ent_good[e]) == g)
                .map(|e| self.output[e] * self.price[i * self.layout.goods.n() + g])
                .sum()
        })
    }

    pub fn net_out_of(&self, provinces: usize, g: usize) -> Vec<f64> {
        let n_goods = self.layout.goods.n();
        self.per_pack(provinces, |i| self.net_out[i * n_goods + g])
    }

    pub fn wages(&self, provinces: usize) -> Vec<f64> {
        self.per_pack(provinces, |i| self.wage[i])
    }

    /// Share of the labour force not employed.
    pub fn unemployment(&self, provinces: usize) -> Vec<f64> {
        self.per_pack(provinces, |i| {
            let pops = self.layout.pops_of(i);
            let force: f64 = self.layout.labour_force[pops.clone()].iter().sum();
            let employed: f64 = self.employed[pops].iter().sum();
            if force > 0.0 {
                (1.0 - employed / force).max(0.0)
            } else {
                f64::NAN
            }
        })
    }

    /// Econ province of a pack province.
    pub fn province(&self, pack: u32) -> Option<usize> {
        self.layout.pack.iter().position(|&p| p == pack)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ceteris_econ::EconParams;
    use ceteris_econ::fixture::small_world;
    use ceteris_trade::TradeParams;

    fn view(ticks: u32) -> View {
        let mut sim = EconWorld::sim(
            Arc::new(small_world()),
            EconParams::default(),
            TradeParams::default(),
        );
        sim.run_until(ticks).expect("no violation");
        let layout = Arc::new(Layout::new(sim.world()));
        View::new(&sim, &layout)
    }

    #[test]
    fn columns_cover_every_province_and_unemployment_is_a_share() {
        let v = view(12);
        let n = v.layout.provinces();
        for u in v.unemployment(n) {
            assert!((0.0..=1.0).contains(&u), "{u}");
        }
        assert!(v.wages(n).iter().all(|w| w.is_finite() && *w > 0.0));
        assert!(v.price_of(n, 3).iter().all(|p| p.is_finite() && *p > 0.0));
    }

    #[test]
    fn net_shipments_net_out_over_the_world_but_for_delivery_margins() {
        let v = view(24);
        let n = v.layout.provinces();
        let n_goods = v.layout.goods.n();
        for g in v.layout.goods.traded_goods() {
            let net: f64 = v.net_out_of(n, g).iter().sum();
            let sold: f64 = (0..n).map(|i| v.sold[i * n_goods + g]).sum();
            assert!(
                net.abs() <= 0.15 * sold.max(1e-9),
                "good {g}: net {net}, sold {sold}"
            );
        }
    }

    #[test]
    fn provinces_outside_the_economy_have_no_value() {
        let v = view(0);
        let n = v.layout.provinces();
        let w = v.wages(n + 2);
        assert!(w[n].is_nan() && w[n + 1].is_nan());
    }
}
