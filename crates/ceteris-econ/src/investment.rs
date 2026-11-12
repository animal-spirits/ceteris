//! Capital formation pipeline for one (cell, asset class) (P5 Branch D1, ADR 0204, DESIGN 7.4).
//! An order waits out the permit lag, then passes `k` Erlang stages of mean total time `T`:
//! `W[s](t+1) = W[s](t) + flow[s-1](t) - flow[s](t)`, `flow[s] = (k/T) W[s] fill`,
//! `K(t+1) = (1-delta) K(t) + flow[k-1](t)`. Each stage does `1/k` of the work, paid in goods
//! at the order's locked input mix. Orders enter the first stage the tick they leave the permit
//! queue, so `k = 1, T = 1, lag = 0` is the instant rule. The kernel is not yet in the tick;
//! paused work-in-progress depreciation, cash calls and write-offs wait on Branch F.
//! In the tick as system 234, off by default (`EconWorld::with_capital`, #534): one pipeline per
//! enterprise, fully filled. Production reads the capital: A scales by `(K/K0)^(1-s_L)`
//! (`Capital::factor`), K0 the stock at switch-on.

use std::collections::VecDeque;

use ceteris_core::snapshot::{DecodeError, Decoder, Encoder};

use crate::inputs::EconInputs;

/// Opens the capital section of a snapshot, which follows the state cells one.
pub const SNAPSHOT_TAG: u32 = 0x3A6E_6F04;

#[derive(Clone, Debug, PartialEq)]
pub struct Order {
    pub size: f64,
    /// Goods per unit of capital, fixed when the order is placed.
    pub mix: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pipeline {
    k: usize,
    mean_time: f64,
    delta: f64,
    capital: f64,
    stages: Vec<f64>,
    stage_mix: Vec<Vec<f64>>,
    permit: VecDeque<Option<Order>>,
}

impl Pipeline {
    pub fn new(
        k: usize,
        mean_time: f64,
        delta: f64,
        permit_lag: usize,
        capital: f64,
        goods: usize,
    ) -> Self {
        Self {
            k,
            mean_time,
            delta,
            capital,
            stages: vec![0.0; k],
            stage_mix: vec![vec![0.0; goods]; k],
            permit: (0..permit_lag).map(|_| None).collect(),
        }
    }

    /// The data state of ADR 0089: work in progress per stage is the fixed point of the step
    /// (the order joins stage 0 before it flows: `I/r` per later stage, `I(1-r)/r` in stage 0 with
    /// `r = k/T`), and capital replaces its own depreciation.
    pub fn steady_state(
        k: usize,
        mean_time: f64,
        delta: f64,
        permit_lag: usize,
        investment: f64,
        mix: &[f64],
    ) -> Self {
        let mut p = Self::new(
            k,
            mean_time,
            delta,
            permit_lag,
            investment / delta,
            mix.len(),
        );
        let r = k as f64 / mean_time;
        p.stages.fill(investment / r);
        p.stages[0] *= 1.0 - r;
        p.stage_mix.fill(mix.to_vec());
        p.permit = (0..permit_lag)
            .map(|_| {
                Some(Order {
                    size: investment,
                    mix: mix.to_vec(),
                })
            })
            .collect();
        p
    }

    fn encode(&self, e: &mut Encoder) {
        e.count(self.k);
        e.f64(self.mean_time);
        e.f64(self.delta);
        e.f64(self.capital);
        e.f64s(&self.stages);
        for m in &self.stage_mix {
            e.f64s(m);
        }
        e.count(self.permit.len());
        for o in &self.permit {
            match o {
                Some(o) => {
                    e.u8(1);
                    e.f64(o.size);
                    e.f64s(&o.mix);
                }
                None => e.u8(0),
            }
        }
    }

    fn decode(d: &mut Decoder) -> Result<Self, DecodeError> {
        let k = d.count()?;
        let (mean_time, delta, capital) = (d.f64()?, d.f64()?, d.f64()?);
        let stages = d.f64s()?;
        if stages.len() != k || k == 0 {
            return Err(DecodeError("capital stages".into()));
        }
        let stage_mix = (0..k).map(|_| d.f64s()).collect::<Result<_, _>>()?;
        let lag = d.count()?;
        let permit = (0..lag)
            .map(|_| match d.u8()? {
                0 => Ok(None),
                1 => Ok(Some(Order {
                    size: d.f64()?,
                    mix: d.f64s()?,
                })),
                _ => Err(DecodeError("capital permit flag".into())),
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            k,
            mean_time,
            delta,
            capital,
            stages,
            stage_mix,
            permit,
        })
    }

    pub fn capital(&self) -> f64 {
        self.capital
    }

    pub fn stage_wip(&self) -> &[f64] {
        &self.stages
    }

    /// Capital-equivalent work already done on units still in the pipeline.
    pub fn wip_value(&self) -> f64 {
        let k = self.k as f64;
        self.stages
            .iter()
            .enumerate()
            .map(|(s, w)| w * s as f64 / k)
            .sum()
    }

    #[cfg(test)]
    fn permit_size(&self) -> f64 {
        self.permit.iter().flatten().map(|o| o.size).sum()
    }

    /// Goods wanted this tick, per good, if every stage progressed in full.
    pub fn goods_demand(&self, out: &mut [f64]) {
        out.fill(0.0);
        for (w, mix) in self.stages.iter().zip(&self.stage_mix) {
            for (o, m) in out.iter_mut().zip(mix) {
                *o += m * w / self.mean_time;
            }
        }
    }

    /// Advances one period with delivered share `fill` in `[0, 1]` and the new order, if any.
    /// Returns the capital-equivalent work paid for this tick.
    pub fn step(&mut self, fill: f64, order: Option<Order>) -> f64 {
        let arriving = if self.permit.is_empty() {
            order
        } else {
            self.permit.push_back(order);
            self.permit.pop_front().flatten()
        };
        if let Some(o) = arriving {
            let w = &mut self.stages[0];
            for (m, n) in self.stage_mix[0].iter_mut().zip(&o.mix) {
                *m = if *w + o.size > 0.0 {
                    (*m * *w + n * o.size) / (*w + o.size)
                } else {
                    0.0
                };
            }
            *w += o.size;
        }
        let rate = self.k as f64 / self.mean_time * fill;
        let flows: Vec<f64> = self.stages.iter().map(|w| rate * w).collect();
        let paid = flows.iter().sum::<f64>() / self.k as f64;
        for s in (0..self.k).rev() {
            self.stages[s] -= flows[s];
            if s + 1 < self.k && flows[s] > 0.0 {
                let w = self.stages[s + 1];
                let total = w + flows[s];
                for g in 0..self.stage_mix[s].len() {
                    self.stage_mix[s + 1][g] =
                        (self.stage_mix[s + 1][g] * w + self.stage_mix[s][g] * flows[s]) / total;
                }
                self.stages[s + 1] += flows[s];
            }
        }
        self.capital = (1.0 - self.delta) * self.capital + flows[self.k - 1];
        paid
    }
}

/// The capital state of the tick: one pipeline per enterprise and the order each places per tick.
/// The order is a stand-in for the 7.4 logit.
#[derive(Clone, Debug, PartialEq)]
pub struct Capital {
    pub pipelines: Vec<Pipeline>,
    pub order: Vec<f64>,
    /// Each pipeline's capital at switch-on, the reference of [`Capital::factor`].
    base: Vec<f64>,
}

impl Capital {
    pub fn new(pipelines: Vec<Pipeline>, order: Vec<f64>) -> Self {
        assert_eq!(pipelines.len(), order.len());
        let base = pipelines.iter().map(Pipeline::capital).collect();
        Self {
            pipelines,
            order,
            base,
        }
    }

    /// The data state: each enterprise orders the GFCF share of its good's final demand in
    /// its country (`inv / omega`) times its value added every tick, from the steady state that
    /// order sustains (ADR 0204; a stand-in for the 7.4 logit). Empty `inv` gives no orders.
    pub fn from_gfcf(
        inp: &EconInputs,
        stages: usize,
        mean_time: f64,
        delta: f64,
        permit_lag: usize,
    ) -> Self {
        let order: Vec<f64> = (0..inp.n_enterprises())
            .map(|e| {
                let c = inp.country_of(inp.enterprises.province[e] as usize);
                let g = usize::from(inp.enterprises.good[e]);
                let cs = &inp.countries;
                if cs.inv.is_empty() || cs.omega[c][g] <= 0.0 {
                    return 0.0;
                }
                cs.inv[c][g] / cs.omega[c][g] * inp.enterprises.va0[e]
            })
            .collect();
        let pipelines = order
            .iter()
            .map(|&i| Pipeline::steady_state(stages, mean_time, delta, permit_lag, i, &[]))
            .collect();
        Self::new(pipelines, order)
    }

    /// Enterprise `e`'s capital factor on A, `(K/K0)^(1-s_L)`; 1 without a base stock.
    pub fn factor(&self, e: usize, s_l: f64) -> f64 {
        let k0 = self.base[e];
        if k0 > 0.0 {
            libm::pow(self.pipelines[e].capital() / k0, 1.0 - s_l)
        } else {
            1.0
        }
    }

    /// Advances every pipeline one tick, fully filled; returns total capital after the step.
    pub fn step(&mut self) -> f64 {
        let mut total = 0.0;
        for (p, &size) in self.pipelines.iter_mut().zip(&self.order) {
            p.step(
                1.0,
                (size > 0.0).then(|| Order {
                    size,
                    mix: Vec::new(),
                }),
            );
            total += p.capital();
        }
        total
    }

    pub fn encode(&self, e: &mut Encoder) {
        e.u32(SNAPSHOT_TAG);
        e.count(self.pipelines.len());
        for p in &self.pipelines {
            p.encode(e);
        }
        e.f64s(&self.order);
        e.f64s(&self.base);
    }

    pub fn decode(d: &mut Decoder) -> Result<Self, DecodeError> {
        if d.u32()? != SNAPSHOT_TAG {
            return Err(DecodeError("capital section tag".into()));
        }
        let n = d.count()?;
        let pipelines = (0..n)
            .map(|_| Pipeline::decode(d))
            .collect::<Result<Vec<_>, _>>()?;
        let order = d.f64s()?;
        if order.len() != n {
            return Err(DecodeError("capital orders".into()));
        }
        let base = d.f64s()?;
        if base.len() != n {
            return Err(DecodeError("capital base".into()));
        }
        Ok(Self {
            pipelines,
            order,
            base,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order(size: f64, price_shift: f64) -> Order {
        Order {
            size,
            mix: vec![0.6 + price_shift, 0.4 - price_shift],
        }
    }

    #[test]
    fn instant_orders_equal_the_instant_rule_bit_for_bit() {
        let mut p = Pipeline::new(1, 1.0, 0.05, 0, 100.0, 2);
        let mut k = 100.0_f64;
        for t in 0..400 {
            let size = 5.0 + (t % 7) as f64;
            p.step(1.0, Some(order(size, 0.0)));
            k = (1.0 - 0.05) * k + size;
            assert_eq!(p.capital().to_bits(), k.to_bits());
        }
    }

    #[test]
    fn goods_and_money_close_with_work_in_progress() {
        let mut p = Pipeline::new(3, 8.0, 0.0, 2, 0.0, 2);
        let (mut ordered, mut paid) = (0.0, 0.0);
        for t in 0..300 {
            let size = if t < 150 { 2.0 + (t % 5) as f64 } else { 0.0 };
            let fill = 0.5 + 0.5 * ((t * 37 % 11) as f64 / 10.0);
            ordered += size;
            paid += p.step(fill, (size > 0.0).then(|| order(size, 0.0)));
            let queued = p.permit_size();
            let in_stages: f64 = p.stage_wip().iter().sum();
            assert!((ordered - (p.capital() + in_stages + queued)).abs() < 1e-9);
            assert!((paid - (p.capital() + p.wip_value())).abs() < 1e-9);
        }
    }

    #[test]
    fn steady_state_2000_pipeline_holds_for_100_years() {
        let mix = [0.6, 0.4];
        let mut p = Pipeline::steady_state(3, 8.0, 0.02, 2, 10.0, &mix);
        let (k0, w0) = (p.capital(), p.stage_wip().to_vec());
        for _ in 0..400 {
            p.step(
                1.0,
                Some(Order {
                    size: 10.0,
                    mix: mix.to_vec(),
                }),
            );
        }
        assert!((p.capital() - k0).abs() < 1e-9 * k0);
        for (a, b) in p.stage_wip().iter().zip(&w0) {
            assert!((a - b).abs() < 1e-9 * b);
        }
    }

    #[test]
    fn price_shock_after_order_keeps_pre_shock_coefficients() {
        let mut p = Pipeline::new(3, 6.0, 0.0, 0, 0.0, 2);
        p.step(1.0, Some(order(10.0, 0.0)));
        p.step(1.0, None);
        // Shock: the next order is chosen at new prices; units already beyond stage 1 are untouched.
        p.step(1.0, Some(order(10.0, 0.2)));
        assert_eq!(p.stage_mix[2], vec![0.6, 0.4]);
        assert!(p.stage_mix[0][0] > 0.6);
    }

    #[test]
    fn rationing_slows_progress_in_proportion() {
        let mut full = Pipeline::new(2, 4.0, 0.0, 0, 0.0, 2);
        let mut half = full.clone();
        full.step(1.0, Some(order(8.0, 0.0)));
        half.step(0.5, Some(order(8.0, 0.0)));
        assert!((half.stage_wip()[0] - 8.0 * (1.0 - 0.5 * 0.5)).abs() < 1e-12);
        assert!(full.stage_wip()[0] < half.stage_wip()[0]);
    }
}
