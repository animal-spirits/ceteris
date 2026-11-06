//! History retention tiers (Section 14.3; ADR 0212).
//!
//! One tick is one month. The live branch keeps its recorded series monthly for the most
//! recent `window` ticks and annually before; other branches and ensemble members keep their
//! declared outcome series monthly and nothing else. Both keep snapshots every 12 ticks inside
//! the window and every 60 after it, and any other series is recomputed from the nearest
//! snapshot, bit for bit, by replaying.

use std::collections::BTreeMap;

use ceteris_core::sim::{Sim, SimError, State};
use ceteris_core::snapshot::canonical;

use crate::store::{ChunkStore, Manifest, StoreError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Retention {
    /// Ticks of monthly history kept on the live branch (25 simulated years).
    pub window: u32,
    /// Spacing of kept samples older than the window.
    pub annual: u32,
    /// Snapshot spacing inside the window, and after it.
    pub snapshot_early: u32,
    pub snapshot_late: u32,
}

impl Default for Retention {
    fn default() -> Self {
        Self {
            window: 25 * 12,
            annual: 12,
            snapshot_early: 12,
            snapshot_late: 60,
        }
    }
}

impl Retention {
    /// Whether the live branch, now at tick `now`, keeps the sample taken at `tick`.
    pub fn keeps_sample(&self, tick: u32, now: u32) -> bool {
        now.saturating_sub(tick) <= self.window || tick.is_multiple_of(self.annual)
    }

    pub fn keeps_snapshot(&self, tick: u32) -> bool {
        let every = if tick <= self.window {
            self.snapshot_early
        } else {
            self.snapshot_late
        };
        tick.is_multiple_of(every)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Live,
    /// A non-live branch or an ensemble member.
    Member,
}

/// One sampled quantity of the world, by name.
pub struct Metric<W> {
    pub name: &'static str,
    pub sample: fn(&W) -> f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Series {
    pub ticks: Vec<u32>,
    pub values: Vec<f64>,
}

impl Series {
    fn push(&mut self, tick: u32, value: f64) {
        self.ticks.push(tick);
        self.values.push(value);
    }

    fn thin(&mut self, r: &Retention, now: u32) {
        let keep: Vec<bool> = self.ticks.iter().map(|&t| r.keeps_sample(t, now)).collect();
        let mut k = keep.iter();
        self.ticks.retain(|_| *k.next().expect("one flag per tick"));
        let mut k = keep.iter();
        self.values
            .retain(|_| *k.next().expect("one flag per tick"));
    }

    pub fn bytes(&self) -> usize {
        self.ticks.len() * (size_of::<u32>() + size_of::<f64>())
    }
}

#[derive(Debug)]
pub enum RecomputeError {
    Store(StoreError),
    Sim(SimError),
    NoSnapshot(u32),
}

impl std::fmt::Display for RecomputeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Store(e) => e.fmt(f),
            Self::Sim(e) => e.fmt(f),
            Self::NoSnapshot(t) => write!(f, "retention: no snapshot at or before tick {t}"),
        }
    }
}

impl std::error::Error for RecomputeError {}

impl From<StoreError> for RecomputeError {
    fn from(e: StoreError) -> Self {
        Self::Store(e)
    }
}

impl From<SimError> for RecomputeError {
    fn from(e: SimError) -> Self {
        Self::Sim(e)
    }
}

/// Bytes held by a recorder, by tier component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Footprint {
    pub series_bytes: u64,
    pub snapshot_bytes: u64,
    pub snapshots: usize,
}

/// Records the declared metrics and the snapshots of one branch or member.
pub struct Recorder<W> {
    retention: Retention,
    tier: Tier,
    store: ChunkStore,
    manifests: BTreeMap<u32, Manifest>,
    metrics: Vec<Metric<W>>,
    series: Vec<Series>,
}

impl<W: State> Recorder<W> {
    pub fn new(
        retention: Retention,
        tier: Tier,
        store: ChunkStore,
        metrics: Vec<Metric<W>>,
    ) -> Self {
        let series = metrics.iter().map(|_| Series::default()).collect();
        Self {
            retention,
            tier,
            store,
            manifests: BTreeMap::new(),
            metrics,
            series,
        }
    }

    /// Records the world after `tick` completed (tick 0 is the initial state).
    pub fn record(&mut self, tick: u32, world: &W) -> Result<(), StoreError> {
        for (m, s) in self.metrics.iter().zip(&mut self.series) {
            s.push(tick, (m.sample)(world));
            if self.tier == Tier::Live {
                s.thin(&self.retention, tick);
            }
        }
        if self.retention.keeps_snapshot(tick) {
            let manifest = self.store.put(&canonical(tick, world))?;
            self.manifests.insert(tick, manifest);
        }
        Ok(())
    }

    pub fn series(&self, name: &str) -> Option<&Series> {
        let i = self.metrics.iter().position(|m| m.name == name)?;
        Some(&self.series[i])
    }

    pub fn snapshot_ticks(&self) -> impl Iterator<Item = u32> + '_ {
        self.manifests.keys().copied()
    }

    pub fn footprint(&self) -> Footprint {
        Footprint {
            series_bytes: self.series.iter().map(|s| s.bytes() as u64).sum(),
            snapshot_bytes: self.store.disk_bytes(),
            snapshots: self.manifests.len(),
        }
    }

    /// Samples `metric` monthly over `from..=to` by restoring the nearest snapshot at or
    /// before `from` into `scratch` (a fresh sim of the same world and seed) and replaying.
    pub fn recompute(
        &self,
        scratch: &mut Sim<W>,
        from: u32,
        to: u32,
        metric: fn(&W) -> f64,
    ) -> Result<Series, RecomputeError> {
        let (_, manifest) = self
            .manifests
            .range(..=from)
            .next_back()
            .ok_or(RecomputeError::NoSnapshot(from))?;
        scratch
            .load(&self.store.get(manifest)?)
            .map_err(|e| RecomputeError::Sim(e.into()))?;
        let mut out = Series::default();
        loop {
            let t = scratch.tick();
            if t >= from {
                out.push(t, metric(scratch.world()));
            }
            if t >= to {
                return Ok(out);
            }
            scratch.step().map_err(|v| RecomputeError::Sim(v.into()))?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ceteris_core::rng::Seed;
    use ceteris_core::toy::Toy;

    fn price0(w: &Toy) -> f64 {
        w.price[0]
    }

    fn price1(w: &Toy) -> f64 {
        w.price[1]
    }

    fn recorder(dir: &tempfile::TempDir, tier: Tier) -> Recorder<Toy> {
        let store = ChunkStore::open(dir.path().join("cache"), "test+1").unwrap();
        let metrics = vec![Metric {
            name: "price0",
            sample: price0,
        }];
        Recorder::new(Retention::default(), tier, store, metrics)
    }

    fn run(rec: &mut Recorder<Toy>, ticks: u32) -> Vec<(u32, f64)> {
        let mut sim = Toy::sim(20, Seed(3));
        let mut dense = Vec::new();
        loop {
            let t = sim.tick();
            dense.push((t, price1(sim.world())));
            rec.record(t, sim.world()).unwrap();
            if t == ticks {
                return dense;
            }
            sim.step().unwrap();
        }
    }

    #[test]
    fn recomputed_series_equals_dense_series() {
        let dir = tempfile::tempdir().unwrap();
        let mut rec = recorder(&dir, Tier::Member);
        let dense = run(&mut rec, 100);
        let mut scratch = Toy::sim(20, Seed(3));
        let got = rec.recompute(&mut scratch, 30, 91, price1).unwrap();
        let want: Vec<_> = dense
            .iter()
            .filter(|(t, _)| (30..=91).contains(t))
            .collect();
        assert_eq!(got.ticks.len(), want.len());
        for ((t, v), (gt, gv)) in want.iter().zip(got.ticks.iter().zip(&got.values)) {
            assert_eq!(t, gt);
            assert_eq!(v.to_bits(), gv.to_bits());
        }
    }

    #[test]
    fn retention_drops_monthly_after_25_years() {
        let dir = tempfile::tempdir().unwrap();
        let mut live = recorder(&dir, Tier::Live);
        run(&mut live, 400);
        let s = live.series("price0").unwrap();
        let (old, recent): (Vec<_>, Vec<_>) = s.ticks.iter().partition(|&&t| t < 400 - 300);
        assert!(old.iter().all(|&&t| t % 12 == 0));
        assert_eq!(old.len(), 9); // 0, 12, ..., 96
        assert_eq!(recent.len(), 301);

        let dir = tempfile::tempdir().unwrap();
        let mut member = recorder(&dir, Tier::Member);
        run(&mut member, 400);
        assert_eq!(member.series("price0").unwrap().ticks.len(), 401);
    }

    #[test]
    fn snapshots_thin_after_the_window() {
        let r = Retention::default();
        let kept: Vec<u32> = (0..=420).filter(|&t| r.keeps_snapshot(t)).collect();
        assert_eq!(kept.iter().filter(|&&t| t <= 300).count(), 26);
        assert_eq!(&kept[26..], &[360, 420]);
    }
}
