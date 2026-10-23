//! Run driver: tick loop, invariant checks, periodic snapshots, seek by restore and
//! replay (Sections 4.4, 14.3).

use crate::invariant::Violation;
use crate::rng::Seed;
use crate::schedule::Schedule;
use crate::snapshot::{self, Decode, DecodeError, Encode, Hash, SnapshotStore};

pub const SNAPSHOT_EVERY: u32 = 12;

pub trait State: Encode + Decode {
    /// Resets per-tick scratch (ledger journal and tallies).
    fn begin_tick(&mut self);
    /// Asserts invariants after all systems ran.
    fn end_tick(&self, tick: u32) -> Result<(), Violation>;
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

pub struct Sim<W> {
    world: W,
    schedule: Schedule<W>,
    seed: Seed,
    tick: u32,
    store: SnapshotStore,
}

impl<W: State> Sim<W> {
    pub fn new(world: W, schedule: Schedule<W>, seed: Seed) -> Self {
        let mut sim = Self {
            world,
            schedule,
            seed,
            tick: 0,
            store: SnapshotStore::default(),
        };
        sim.snapshot();
        sim
    }

    pub fn world(&self) -> &W {
        &self.world
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
        self.store
            .insert(self.tick, &snapshot::canonical(self.tick, &self.world));
    }

    pub fn step(&mut self) -> Result<(), Violation> {
        let t = self.tick;
        self.world.begin_tick();
        self.schedule.run_tick(&mut self.world, self.seed, t);
        self.world.end_tick(t)?;
        self.tick = t + 1;
        if self.tick.is_multiple_of(SNAPSHOT_EVERY) {
            self.snapshot();
        }
        Ok(())
    }

    pub fn run_until(&mut self, tick: u32) -> Result<(), Violation> {
        while self.tick < tick {
            self.step()?;
        }
        Ok(())
    }

    /// Moves to any tick: restores the nearest snapshot at or before it and replays.
    pub fn seek(&mut self, tick: u32) -> Result<(), SimError> {
        let (t, bytes) = self
            .store
            .at_or_before(tick)
            .expect("tick 0 is always snapshotted");
        let (decoded_tick, world) = snapshot::decode_canonical(&bytes)?;
        debug_assert_eq!(decoded_tick, t);
        self.world = world;
        self.tick = t;
        self.run_until(tick)?;
        Ok(())
    }
}
