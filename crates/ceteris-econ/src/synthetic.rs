//! Synthetic goods count for benchmarks (ADR 0235, task G2): the goods of a world resampled to
//! `n` goods, never a calibration input. Each new good copies a base good (traded from traded,
//! local from local, in turn); a base good copied `m` times splits its value added, final
//! demand and input use evenly over its copies, so the world's value added, spending shares and
//! input cost shares stay those of the base. The result starts from the data state: the
//! calibrated steady state, border friction and elasticities do not carry over.

use ceteris_core::rows::Rows;
use ceteris_trade::{GoodLegs, Goods, LegTaus, TradeParams};

use crate::demand::PiglWeights;
use crate::inputs::{Countries, EconInputs, EntData};

struct Plan {
    /// Base good of each new good.
    src: Vec<usize>,
    /// Copies of each base good.
    mult: Vec<usize>,
    traded: usize,
}

impl Plan {
    fn new(goods: &Goods, n: usize) -> Self {
        let (base, traded_b) = (goods.n(), goods.traded);
        let local_b = base - traded_b;
        let traded = if local_b == 0 {
            n
        } else {
            (traded_b * n).div_ceil(base).clamp(1, n - 1)
        };
        let src: Vec<usize> = (0..n)
            .map(|j| {
                if j < traded {
                    j % traded_b
                } else {
                    traded_b + (j - traded) % local_b
                }
            })
            .collect();
        let mut mult = vec![0; base];
        for &s in &src {
            mult[s] += 1;
        }
        assert!(mult.iter().all(|&m| m > 0), "{n} goods cannot cover {base}");
        Self { src, mult, traded }
    }

    /// `f(base value, copies)` for every new good.
    fn pick<T: Clone>(&self, v: &[T]) -> Vec<T> {
        self.src.iter().map(|&s| v[s].clone()).collect()
    }

    /// A row of per-good values shared out evenly over the copies.
    fn split(&self, row: &[f64]) -> Vec<f64> {
        self.src
            .iter()
            .map(|&s| row[s] / self.mult[s] as f64)
            .collect()
    }

    fn pick_rows(&self, rows: &Rows<f64>) -> Rows<f64> {
        Rows::from_rows(self.src.len(), rows.iter().map(|r| self.pick(r)))
    }

    fn split_rows(&self, rows: &Rows<f64>) -> Rows<f64> {
        Rows::from_rows(self.src.len(), rows.iter().map(|r| self.split(r)))
    }

    fn goods(&self, g: &Goods) -> Goods {
        let mut seen = vec![0; g.n()];
        let name = self
            .src
            .iter()
            .map(|&s| {
                seen[s] += 1;
                if seen[s] == 1 {
                    g.name[s].clone()
                } else {
                    format!("{}-{}", g.name[s], seen[s])
                }
            })
            .collect();
        Goods::new(
            name,
            self.traded,
            self.pick(&g.storable),
            self.pick(&g.group),
            self.pick(&g.category),
        )
    }

    fn countries(&self, c: &Countries) -> Countries {
        let n = self.src.len();
        let nb = self.mult.len();
        let io = Rows::from_rows(
            n * n,
            c.io.iter().map(|r| {
                let mut out = vec![0.0; n * n];
                for k in 0..n {
                    for g in 0..n {
                        out[k * n + g] =
                            r[self.src[k] * nb + self.src[g]] / self.mult[self.src[k]] as f64;
                    }
                }
                out
            }),
        );
        let pigl = c.pigl.as_ref().map(|p| {
            p.iter()
                .map(|w| PiglWeights {
                    within: self.split(&w.within),
                    category: self.pick(&w.category),
                    gov: if w.gov.is_empty() {
                        vec![]
                    } else {
                        self.split(&w.gov)
                    },
                    inv: if w.inv.is_empty() {
                        vec![]
                    } else {
                        self.split(&w.inv)
                    },
                    ..w.clone()
                })
                .collect()
        });
        let opt = |r: &Rows<f64>, f: fn(&Plan, &Rows<f64>) -> Rows<f64>| {
            if r.is_empty() { r.clone() } else { f(self, r) }
        };
        Countries {
            omega: opt(&c.omega, Plan::split_rows),
            gov: opt(&c.gov, Plan::split_rows),
            inv: opt(&c.inv, Plan::split_rows),
            io,
            s_l_good: opt(&c.s_l_good, Plan::pick_rows),
            foreign: opt(&c.foreign, Plan::pick_rows),
            pigl,
            ..c.clone()
        }
    }

    fn enterprises(&self, e: &EntData) -> EntData {
        let mut out = EntData {
            coef: Rows::new(self.src.len()),
            column: Rows::new(self.src.len()),
            ..EntData::default()
        };
        let mut i = 0;
        while i < e.province.len() {
            // enterprises of one province, sorted by sector
            let p = e.province[i];
            let end = (i..e.province.len())
                .find(|&j| e.province[j] != p)
                .unwrap_or(e.province.len());
            let mut cells: Vec<(usize, usize)> = Vec::new();
            for r in i..end {
                for (j, &s) in self.src.iter().enumerate() {
                    if usize::from(e.good[r]) == s {
                        cells.push((j, r));
                    }
                }
            }
            cells.sort_unstable();
            for (j, r) in cells {
                let m = self.mult[self.src[j]] as f64;
                out.province.push(p);
                out.good.push(u8::try_from(j).expect("at most 256 goods"));
                out.a.push(e.a[r]);
                out.va0.push(e.va0[r] / m);
                out.l0.push(e.l0[r] / m);
                out.va_target.push(e.va_target[r] / m);
                out.coef.push(&self.split(&e.coef[r]));
                out.column.push(&self.split(&e.column[r]));
                out.a_va.push(e.a_va[r]);
            }
            i = end;
        }
        out
    }

    fn trade(&self, t: &TradeParams) -> TradeParams {
        TradeParams {
            goods: self.goods(&t.goods),
            theta: self.pick(&t.theta),
            theta_class: self.pick(&t.theta_class),
            coverage: self.pick(&t.coverage),
            border_nu: self.pick(&t.border_nu),
            time_cost: self.pick(&t.time_cost),
            storage_loss: self.pick(&t.storage_loss),
            storage_cost: self.pick(&t.storage_cost),
            speculation: self.pick(&t.speculation),
            convenience: self.pick(&t.convenience),
            ..t.clone()
        }
    }
}

/// `inputs` and `trade` with `n` goods. Every base good must keep at least one copy, so `n`
/// is at least the base count, and at most 256.
pub fn with_goods(inputs: &EconInputs, trade: &TradeParams, n: usize) -> (EconInputs, TradeParams) {
    let plan = Plan::new(&inputs.goods, n);
    let mut out = inputs.clone();
    out.goods = plan.goods(&inputs.goods);
    out.weight = plan.pick(&inputs.weight);
    out.countries = plan.countries(&inputs.countries);
    out.provinces.base_price = plan.pick_rows(&inputs.provinces.base_price);
    out.provinces.gamma = plan.pick_rows(&inputs.provinces.gamma);
    out.enterprises = plan.enterprises(&inputs.enterprises);
    out.network.route_taus = inputs.network.route_taus.as_ref().map(|t| LegTaus {
        domestic: plan.pick_rows(&t.domestic),
        access: plan.pick_rows(&t.access),
        crossings: plan.pick_rows(&t.crossings),
        domestic_hours: plan.pick_rows(&t.domestic_hours),
        access_hours: plan.pick_rows(&t.access_hours),
        crossing_hours: plan.pick_rows(&t.crossing_hours),
    });
    out.network.good_legs = inputs.network.good_legs.as_ref().map(|g| GoodLegs {
        set: plan.src[..plan.traded].iter().map(|&s| g.set[s]).collect(),
        kappa: plan.src[..plan.traded]
            .iter()
            .map(|&s| g.kappa[s])
            .collect(),
        sets: g.sets.clone(),
    });
    out.steady = None;
    out.border_nu = inputs.border_nu.as_ref().map(|v| plan.pick(v));
    out.market_elasticity = None;
    (out, plan.trade(trade))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::fixture::small_world;
    use crate::params::EconParams;
    use crate::world::EconWorld;

    #[test]
    fn same_count_keeps_the_goods() {
        let base = small_world();
        let (same, _) = with_goods(&base, &TradeParams::default(), base.n_goods());
        assert_eq!(same.goods, base.goods);
        assert_eq!(same.enterprises, base.enterprises);
        assert_eq!(same.countries.io, base.countries.io);
    }

    #[test]
    fn value_added_and_final_demand_are_conserved() {
        let base = small_world();
        for n in [20, 45] {
            let (w, _) = with_goods(&base, &TradeParams::default(), n);
            w.validate();
            assert_eq!(w.n_goods(), n);
            let sum = |v: &[f64]| v.iter().sum::<f64>();
            let (a, b) = (sum(&w.enterprises.va0), sum(&base.enterprises.va0));
            assert!((a - b).abs() < 1e-9 * b);
            assert!((sum(&w.countries.omega[0]) - sum(&base.countries.omega[0])).abs() < 1e-12);
        }
    }

    #[test]
    fn synthetic_worlds_run_without_violations() {
        let base = small_world();
        for n in [20, 45] {
            let (w, trade) = with_goods(&base, &TradeParams::default(), n);
            let mut sim = EconWorld::sim(Arc::new(w), EconParams::default(), trade);
            sim.run_until(24).expect("invariants hold");
        }
    }
}
