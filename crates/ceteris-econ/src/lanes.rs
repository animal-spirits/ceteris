//! Ensemble lanes (ADR 0024): `L` worlds that share their read-only parts and run a tick
//! together, so that the trade kernels read their rows once for all lanes. Lane `l` ends every
//! tick exactly as a lone world with its parameters and seed does.

use ceteris_core::invariant::Violation;
use ceteris_core::sim::State;
use ceteris_trade::Sourced;
use ceteris_trade::source::{LaneInput, dispatch_lanes, source_lanes};
use rayon::prelude::*;

use crate::flows::FlowWindow;
use crate::systems;
use crate::world::{EconWorld, Payments};
use ceteris_core::ledger::PostScratch;

pub struct EconLanes<const L: usize> {
    worlds: [EconWorld; L],
    tick: u32,
    /// Working buffers of the systems that finish within one call, swapped into each lane in
    /// turn so that lanes do not each hold their own: the money legs of dispatch and the
    /// ledger's posting space. A lane's own entries stay empty.
    payments: Vec<Payments>,
    post: PostScratch,
}

impl<const L: usize> EconLanes<L> {
    /// Lanes from `L` worlds at the same tick (the data state) over the same inputs.
    pub fn new(worlds: [EconWorld; L]) -> Self {
        assert!(L > 0);
        assert!(
            worlds
                .iter()
                .all(|w| std::sync::Arc::ptr_eq(&w.inputs, &worlds[0].inputs)),
            "lanes share one inputs allocation"
        );
        let payments = worlds[0].scratch.payments.clone();
        Self {
            worlds,
            tick: 0,
            payments,
            post: PostScratch::default(),
        }
    }

    /// Lanes over `inputs` with per-lane parameters. Lanes with equal trade parameters share
    /// one kernel; a lane that differs gets its own.
    pub fn build(
        inputs: &std::sync::Arc<crate::inputs::EconInputs>,
        params: [(crate::params::EconParams, ceteris_trade::TradeParams); L],
    ) -> Self {
        let mut kernels: Vec<(
            ceteris_trade::TradeParams,
            std::sync::Arc<ceteris_trade::Kernel>,
        )> = Vec::new();
        let worlds = params.map(|(p, trade)| {
            let kernel = match kernels.iter().find(|(t, _)| *t == trade) {
                Some((_, k)) => std::sync::Arc::clone(k),
                None => {
                    let k = std::sync::Arc::new(ceteris_trade::Kernel::build(
                        &inputs.network,
                        &inputs.weight,
                        &inputs.wedges,
                        &trade,
                    ));
                    kernels.push((trade.clone(), std::sync::Arc::clone(&k)));
                    k
                }
            };
            EconWorld::with_kernel(std::sync::Arc::clone(inputs), p, trade, kernel)
        });
        Self::new(worlds)
    }

    /// These lanes with the country-pair flow window of every lane switched off: the export
    /// of trade flows is empty, and each lane holds that much less memory.
    #[must_use]
    pub fn without_flows(mut self) -> Self {
        for w in &mut self.worlds {
            let (ng, nt) = (w.inputs.n_goods(), w.scratch.sourced.len());
            w.scratch.flows = FlowWindow::off(ng, nt);
        }
        self
    }

    pub fn worlds(&self) -> &[EconWorld; L] {
        &self.worlds
    }

    pub fn tick(&self) -> u32 {
        self.tick
    }

    /// The world of lane `l` between ticks, for changes the schedule does not make (an edit
    /// of one lane).
    pub fn lane_mut(&mut self, l: usize) -> &mut EconWorld {
        &mut self.worlds[l]
    }

    /// Canonical snapshot of lane `l`: the same bytes a lone world at this tick writes, so a
    /// lane restores into a scalar `Sim` (ADR 0024 Consequences).
    pub fn snapshot(&self, l: usize) -> Vec<u8> {
        ceteris_core::snapshot::canonical(self.tick, &self.worlds[l])
    }

    /// One state hash per lane.
    pub fn state_hashes(&self) -> [ceteris_core::snapshot::Hash; L] {
        std::array::from_fn(|l| ceteris_core::snapshot::state_hash(&self.snapshot(l)))
    }

    /// One tick of the P2 schedule on every lane; `source` and `dispatch` run the lane kernels, the other
    /// systems run lane by lane.
    pub fn step(&mut self) -> Result<(), Violation> {
        let t = self.tick;
        for w in &mut self.worlds {
            w.begin_tick();
        }
        for w in &mut self.worlds {
            w.advance_kernel();
            systems::drift(w, t);
            systems::deliver(w);
            systems::labour(w);
            systems::produce(w);
            systems::demand(w);
        }
        self.source();
        for w in &mut self.worlds {
            std::mem::swap(&mut w.scratch.post, &mut self.post);
            systems::clear(w);
            std::mem::swap(&mut w.scratch.post, &mut self.post);
        }
        self.dispatch();
        for w in &mut self.worlds {
            systems::reprice(w);
            systems::distribute(w, t);
            systems::assert_invariants(w, t);
        }
        for w in &self.worlds {
            w.end_tick(t)?;
        }
        self.tick = t + 1;
        Ok(())
    }

    pub fn run_until(&mut self, tick: u32) -> Result<(), Violation> {
        while self.tick < tick {
            self.step()?;
        }
        Ok(())
    }

    /// Runs `kernel` once per good over the lanes' `sourced` columns, which move into one
    /// array per good for the call and back after.
    fn per_good(&mut self, kernel: impl Fn(usize, &[EconWorld; L], &mut [Sourced; L]) + Sync) {
        let ng = self.worlds[0].scratch.sourced.len();
        let mut by_good: Vec<[Sourced; L]> = (0..ng)
            .map(|_| std::array::from_fn(|_| Sourced::new()))
            .collect();
        for (l, w) in self.worlds.iter_mut().enumerate() {
            for (g, s) in std::mem::take(&mut w.scratch.sourced)
                .into_iter()
                .enumerate()
            {
                by_good[g][l] = s;
            }
        }
        let worlds = &self.worlds;
        by_good
            .par_iter_mut()
            .enumerate()
            .for_each(|(g, outs)| kernel(g, worlds, outs));
        for outs in by_good {
            for (l, s) in outs.into_iter().enumerate() {
                self.worlds[l].scratch.sourced.push(s);
            }
        }
    }

    fn shared_kernel(&self) -> bool {
        let id = self.worlds[0].kernel.id();
        self.worlds.iter().all(|w| w.kernel.id() == id)
    }

    /// T2 for all lanes: each good's sourcing runs once over the lanes that share its kernel.
    fn source(&mut self) {
        for w in &mut self.worlds {
            systems::source_inputs(w);
        }
        if self.shared_kernel() {
            let kernel = std::sync::Arc::clone(&self.worlds[0].kernel);
            self.per_good(|g, worlds, outs| {
                let inputs = std::array::from_fn(|l| {
                    let w = &worlds[l];
                    let col = &w.scratch.columns[g];
                    LaneInput {
                        price: &col.price,
                        weight_price: (!w.weight_price.is_empty()).then_some(&col.weight_price[..]),
                        active: &col.active,
                        gamma: &w.layout.gamma[g],
                        spend: &col.spend,
                    }
                });
                source_lanes(&kernel, g, inputs, outs);
            });
        } else {
            for w in &mut self.worlds {
                systems::source_kernel(w);
            }
        }
        for w in &mut self.worlds {
            systems::source_commit(w);
        }
    }

    /// T2b for all lanes: the dispatch kernel once per good over lanes that share it.
    fn dispatch(&mut self) {
        for w in &mut self.worlds {
            systems::dispatch_gather(w);
        }
        if self.shared_kernel() {
            let kernel = std::sync::Arc::clone(&self.worlds[0].kernel);
            self.per_good(|g, worlds, outs| {
                let price = std::array::from_fn(|l| worlds[l].scratch.columns[g].price.as_slice());
                let fill = std::array::from_fn(|l| worlds[l].scratch.columns[g].fill.as_slice());
                dispatch_lanes(&kernel, g, price, fill, outs);
            });
        } else {
            for w in &mut self.worlds {
                systems::dispatch_kernel(w);
            }
        }
        for w in &mut self.worlds {
            std::mem::swap(&mut w.scratch.post, &mut self.post);
            std::mem::swap(&mut w.scratch.payments, &mut self.payments);
            systems::dispatch_finish(w);
            std::mem::swap(&mut w.scratch.post, &mut self.post);
            std::mem::swap(&mut w.scratch.payments, &mut self.payments);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use ceteris_core::snapshot;
    use ceteris_trade::TradeParams;

    use super::*;
    use crate::fixture::small_world;
    use crate::params::EconParams;

    fn hash(tick: u32, w: &EconWorld) -> [u8; 32] {
        snapshot::state_hash(&snapshot::canonical(tick, w))
    }

    fn lane_params(l: usize) -> (EconParams, TradeParams) {
        let mut p = EconParams::default();
        p.kappa_w *= 1.0 + 0.25 * l as f64;
        p.sigma_demand *= 1.0 - 0.1 * l as f64;
        (p, TradeParams::default())
    }

    #[test]
    fn lanes_share_a_kernel_only_for_equal_trade_params() {
        let inputs = Arc::new(small_world());
        let mut odd = TradeParams::default();
        odd.theta_scale *= 1.1;
        let lanes = EconLanes::<3>::build(
            &inputs,
            [
                (EconParams::default(), TradeParams::default()),
                (lane_params(1).0, TradeParams::default()),
                (EconParams::default(), odd),
            ],
        );
        let ids: Vec<u64> = lanes.worlds.iter().map(|w| w.kernel.id()).collect();
        assert_eq!(ids[0], ids[1]);
        assert_ne!(ids[0], ids[2]);
        assert!(Arc::ptr_eq(
            &lanes.worlds[0].kernel,
            &lanes.worlds[1].kernel
        ));
    }

    fn assert_lanes_equal_scalar(params: [(EconParams, TradeParams); 3], ticks: u32) {
        let inputs = Arc::new(small_world());
        let mut lanes = EconLanes::<3>::build(&inputs, params.clone());
        let mut scalar: Vec<_> = params
            .into_iter()
            .map(|(p, t)| EconWorld::sim(Arc::clone(&inputs), p, t))
            .collect();
        for t in 1..=ticks {
            lanes.step().unwrap();
            for (l, s) in scalar.iter_mut().enumerate() {
                s.step().unwrap();
                assert_eq!(
                    hash(t, &lanes.worlds[l]),
                    s.state_hash(),
                    "lane {l} tick {t}"
                );
            }
        }
        assert_ne!(
            hash(ticks, &lanes.worlds[0]),
            hash(ticks, &lanes.worlds[1]),
            "lanes differ"
        );
    }

    #[test]
    fn lane_k_equals_scalar_run_with_params_k() {
        assert_lanes_equal_scalar(std::array::from_fn(lane_params), 36);
    }

    #[test]
    fn lanes_with_a_differing_kernel_equal_their_scalar_runs() {
        let mut odd = TradeParams::default();
        odd.theta_scale *= 1.1;
        let mut params: [_; 3] = std::array::from_fn(lane_params);
        params[2].1 = odd;
        assert_lanes_equal_scalar(params, 12);
    }

    #[test]
    fn lane_hashes_do_not_depend_on_the_thread_count() {
        let run = |threads: usize| {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            pool.install(|| {
                let inputs = Arc::new(small_world());
                let mut lanes = EconLanes::<3>::build(&inputs, std::array::from_fn(lane_params));
                lanes.run_until(12).unwrap();
                lanes.worlds.each_ref().map(|w| hash(12, w))
            })
        };
        assert_eq!(run(1), run(4));
    }

    #[test]
    fn snapshot_of_lane_k_restores_into_scalar_world() {
        let inputs = Arc::new(small_world());
        let mut lanes = EconLanes::<3>::build(&inputs, std::array::from_fn(lane_params));
        lanes.run_until(8).unwrap();
        let (p, t) = lane_params(2);
        let mut scalar = EconWorld::sim(Arc::clone(&inputs), p.clone(), t.clone());
        scalar.load(&lanes.snapshot(2)).unwrap();
        assert_eq!(scalar.tick(), 8);
        assert_eq!(scalar.state_hash(), lanes.state_hashes()[2]);
        // the restored scalar run continues as the lane does
        scalar.step().unwrap();
        lanes.step().unwrap();
        assert_eq!(scalar.state_hash(), lanes.state_hashes()[2]);
    }

    #[test]
    fn crn_pair_in_two_lanes_equals_two_scalar_runs() {
        // the same parameters in both lanes, one edited at tick 6: a paired run
        let inputs = Arc::new(small_world());
        let mut lanes = EconLanes::<2>::build(&inputs, std::array::from_fn(|_| lane_params(0)));
        let (p, t) = lane_params(0);
        let mut base = EconWorld::sim(Arc::clone(&inputs), p.clone(), t.clone());
        let mut edited = EconWorld::sim(Arc::clone(&inputs), p, t);
        lanes.run_until(6).unwrap();
        base.run_until(6).unwrap();
        edited.run_until(6).unwrap();
        lanes.lane_mut(1).wage[0] *= 1.5;
        edited.world_mut().wage[0] *= 1.5;
        for _ in 0..6 {
            lanes.step().unwrap();
            base.step().unwrap();
            edited.step().unwrap();
        }
        let h = lanes.state_hashes();
        assert_eq!(h[0], base.state_hash());
        assert_eq!(h[1], edited.state_hash());
        assert_ne!(h[0], h[1]);
    }

    #[test]
    fn lanes_share_working_buffers_and_run_without_flows() {
        let inputs = Arc::new(small_world());
        let mut lanes =
            EconLanes::<3>::build(&inputs, std::array::from_fn(lane_params)).without_flows();
        let (p, t) = lane_params(1);
        let mut scalar = EconWorld::sim(Arc::clone(&inputs), p, t);
        for _ in 0..6 {
            lanes.step().unwrap();
            scalar.step().unwrap();
        }
        assert_eq!(lanes.state_hashes()[1], scalar.state_hash());
        for w in lanes.worlds() {
            assert!(w.scratch.payments.iter().all(|p| p.entries.capacity() == 0));
            assert_eq!(w.scratch.flows.mean().pair_value.len(), 0);
        }
        assert!(lanes.payments.iter().any(|p| p.entries.capacity() > 0));
    }
}
