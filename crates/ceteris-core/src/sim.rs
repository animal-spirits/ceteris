//! Run driver: tick loop, invariant checks, periodic snapshots, seek by restore and
//! replay (Sections 4.4, 14.3).

use crate::invariant::Violation;
use crate::rng::Seed;
use crate::schedule::Schedule;
use crate::snapshot::{self, DecodeError, Decoder, Encode, Hash, SnapshotStore};
use crate::timing;

pub const SNAPSHOT_EVERY: u32 = 12;

pub trait State: Encode + Sized {
    /// Resets per-tick scratch (ledger journal and tallies).
    fn begin_tick(&mut self);
    /// Asserts invariants after all systems ran.
    fn end_tick(&self, tick: u32) -> Result<(), Violation>;
    /// Rebuilds a world from its encoding. Runs on the live world so static inputs
    /// held outside snapshots (shared through an `Arc`) can be reused.
    fn restore(&self, d: &mut Decoder) -> Result<Self, DecodeError>;
}

#[derive(Debug)]
pub enum SimError {
    Violation(Violation),
    Decode(DecodeError),
}

impl std::fmt::Display for SimError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            SimError::Violation(v) => v.fmt(f),
            SimError::Decode(d) => d.fmt(f),
        }
    }
}

impl std::error::Error for SimError {}

impl From<Violation> for SimError {
    fn from(v: Violation) -> Self {
        SimError::Violation(v)
    }
}

impl From<DecodeError> for SimError {
    fn from(d: DecodeError) -> Self {
        SimError::Decode(d)
    }
}

/// A change to the world between ticks, replayed by seek; it must be deterministic.
pub type Intervention<W> = Box<dyn Fn(&mut W) + Send + Sync>;

pub struct Sim<W> {
    world: W,
    schedule: Schedule<W>,
    seed: Seed,
    tick: u32,
    store: SnapshotStore,
    /// Interventions by the tick they apply at, in the order made. A snapshot at a tick is
    /// taken before that tick's interventions.
    interventions: Vec<(u32, Intervention<W>)>,
}

impl<W: State> Sim<W> {
    pub fn new(world: W, schedule: Schedule<W>, seed: Seed) -> Self {
        let mut sim = Self {
            world,
            schedule,
            seed,
            tick: 0,
            store: SnapshotStore::default(),
            interventions: Vec::new(),
        };
        sim.snapshot();
        sim
    }

    pub fn world(&self) -> &W {
        &self.world
    }

    /// The world between ticks, for changes the schedule does not make (a shock). Snapshots
    /// taken before the change keep the world as it was, and a seek does not replay it; see
    /// [`Sim::intervene`].
    pub fn world_mut(&mut self) -> &mut W {
        &mut self.world
    }

    /// Applies `change` to the world now and records it at this tick, so that a seek across
    /// it replays it. After a seek back, the snapshots and interventions later than this tick
    /// belong to the timeline being left and are dropped.
    pub fn intervene(&mut self, change: impl Fn(&mut W) + Send + Sync + 'static) {
        let t = self.tick;
        self.store.truncate_after(t);
        self.interventions.retain(|(at, _)| *at <= t);
        change(&mut self.world);
        self.interventions.push((t, Box::new(change)));
    }

    fn apply_interventions(&mut self) {
        let t = self.tick;
        for (_, change) in self.interventions.iter().filter(|(at, _)| *at == t) {
            change(&mut self.world);
        }
    }

    /// Number of completed ticks.
    pub fn tick(&self) -> u32 {
        self.tick
    }

    pub fn state_hash(&self) -> Hash {
        snapshot::state_hash(&snapshot::canonical(self.tick, &self.world))
    }

    /// Snapshot hashes by tick, in tick order.
    pub fn snapshot_hashes(&self) -> Vec<(u32, Hash)> {
        self.store.hashes().collect()
    }

    fn snapshot(&mut self) {
        let bytes = {
            let _span = timing::span("snap.encode", timing::NONE);
            snapshot::canonical(self.tick, &self.world)
        };
        self.store.insert(self.tick, &bytes);
    }

    pub fn step(&mut self) -> Result<(), Violation> {
        let t = self.tick;
        timing::set_tick(t);
        let _tick = timing::span("tick", timing::NONE);
        {
            let _span = timing::span("begin_tick", timing::NONE);
            self.world.begin_tick();
        }
        self.schedule.run_tick(&mut self.world, self.seed, t);
        {
            let _span = timing::span("end_tick", timing::NONE);
            self.world.end_tick(t)?;
        }
        self.tick = t + 1;
        if self.tick.is_multiple_of(SNAPSHOT_EVERY) {
            let _span = timing::span("snapshot", timing::NONE);
            self.snapshot();
        }
        self.apply_interventions();
        Ok(())
    }

    pub fn run_until(&mut self, tick: u32) -> Result<(), Violation> {
        while self.tick < tick {
            self.step()?;
        }
        Ok(())
    }

    /// Replaces the world and the tick with a canonical snapshot kept elsewhere (the run
    /// store). Snapshots and interventions held by this sim are untouched, so use it on a
    /// scratch sim that replays forward, as the run store's recompute does.
    pub fn load(&mut self, canonical: &[u8]) -> Result<(), DecodeError> {
        let (tick, world) = snapshot::decode_canonical_with(canonical, |d| self.world.restore(d))?;
        self.world = world;
        self.tick = tick;
        Ok(())
    }

    /// Moves to any tick: restores the nearest snapshot at or before it and replays, with the
    /// interventions recorded on the way.
    pub fn seek(&mut self, tick: u32) -> Result<(), SimError> {
        let (t, bytes) = self
            .store
            .at_or_before(tick)
            .expect("tick 0 is always snapshotted");
        let (decoded_tick, world) =
            snapshot::decode_canonical_with(&bytes, |d| self.world.restore(d))?;
        debug_assert_eq!(decoded_tick, t);
        self.world = world;
        self.tick = t;
        self.apply_interventions();
        self.run_until(tick)?;
        Ok(())
    }
}
