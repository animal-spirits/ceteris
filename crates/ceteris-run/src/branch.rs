//! Running branches: forks load the nearest earlier snapshot and replay (Section 14.3).
//!
//! A branch is its ancestors' edits before its fork tick plus its own. Edits at tick `t` apply
//! to the snapshot of tick `t` (taken before them), so a child forked at `t` does not inherit
//! the parent's edits at `t`. Snapshots go to the content-addressed store, so a child shares
//! the parent's chunks up to the fork.

use std::collections::BTreeMap;
use std::fmt;

use ceteris_core::rng::Seed;
use ceteris_core::sim::{SNAPSHOT_EVERY, Sim, SimError, State};
use ceteris_core::snapshot::canonical;

use crate::scenario::{Edit, Scenario, ScenarioError};
use crate::store::{ChunkStore, Manifest, StoreError};

#[derive(Debug)]
pub enum RunError {
    Scenario(ScenarioError),
    Store(StoreError),
    Sim(SimError),
    Edit(String),
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Scenario(e) => e.fmt(f),
            Self::Store(e) => e.fmt(f),
            Self::Sim(e) => e.fmt(f),
            Self::Edit(e) => f.write_str(e),
        }
    }
}

impl std::error::Error for RunError {}

impl From<ScenarioError> for RunError {
    fn from(e: ScenarioError) -> Self {
        Self::Scenario(e)
    }
}
impl From<StoreError> for RunError {
    fn from(e: StoreError) -> Self {
        Self::Store(e)
    }
}
impl From<SimError> for RunError {
    fn from(e: SimError) -> Self {
        Self::Sim(e)
    }
}

/// The edits due at `tick`, in (source, log sequence) order.
pub fn due(edits: &[Edit], tick: u32) -> Vec<&Edit> {
    let mut due: Vec<(usize, &Edit)> = edits
        .iter()
        .enumerate()
        .filter(|(_, e)| e.tick == tick)
        .collect();
    due.sort_by_key(|(seq, e)| (e.source, *seq));
    due.into_iter().map(|(_, e)| e).collect()
}

pub struct Workbench<W> {
    scenario: Scenario,
    seed: Seed,
    build: Box<dyn Fn(Seed) -> Sim<W>>,
    apply: fn(&mut W, &Edit) -> Result<(), String>,
    store: ChunkStore,
    manifests: BTreeMap<(String, u32), Manifest>,
}

impl<W: State> Workbench<W> {
    /// `build` makes the root branch's world at tick 0; `apply` applies one edit to a world.
    pub fn new(
        scenario: Scenario,
        seed: Seed,
        build: impl Fn(Seed) -> Sim<W> + 'static,
        apply: fn(&mut W, &Edit) -> Result<(), String>,
        store: ChunkStore,
    ) -> Self {
        Self {
            scenario,
            seed,
            build: Box::new(build),
            apply,
            store,
            manifests: BTreeMap::new(),
        }
    }

    pub fn store(&self) -> &ChunkStore {
        &self.store
    }

    /// The sim of `branch` at `tick`, before that tick's edits.
    pub fn at(&mut self, branch: &str, tick: u32) -> Result<Sim<W>, RunError> {
        let b = self.scenario.branch(branch)?.clone();
        let from = b.fork_tick.unwrap_or(0);
        if let Some(parent) = &b.parent
            && tick < from
        {
            return self.at(parent, tick);
        }
        let mut sim = match self.start_from_snapshot(branch, from, tick)? {
            Some(s) => s,
            None => match &b.parent {
                Some(parent) => {
                    let s = self.at(parent, from)?;
                    self.record(branch, from, &s)?;
                    s
                }
                None => (self.build)(self.seed),
            },
        };
        while sim.tick() < tick {
            let t = sim.tick();
            if t.is_multiple_of(SNAPSHOT_EVERY) {
                self.record(branch, t, &sim)?;
            }
            for e in due(&b.edits, t) {
                (self.apply)(sim.world_mut(), e).map_err(RunError::Edit)?;
            }
            sim.step().map_err(|v| RunError::Sim(v.into()))?;
        }
        Ok(sim)
    }

    fn record(&mut self, branch: &str, tick: u32, sim: &Sim<W>) -> Result<(), RunError> {
        let key = (branch.to_string(), tick);
        if !self.manifests.contains_key(&key) {
            let m = self.store.put(&canonical(tick, sim.world()))?;
            self.manifests.insert(key, m);
        }
        Ok(())
    }

    /// The branch's own latest snapshot in `from..=tick`, loaded into a fresh sim.
    fn start_from_snapshot(
        &self,
        branch: &str,
        from: u32,
        tick: u32,
    ) -> Result<Option<Sim<W>>, RunError> {
        let Some((_, m)) = self
            .manifests
            .range((branch.to_string(), from)..=(branch.to_string(), tick))
            .next_back()
        else {
            return Ok(None);
        };
        let mut sim = (self.build)(self.seed);
        sim.load(&self.store.get(m)?)
            .map_err(|e| RunError::Sim(e.into()))?;
        Ok(Some(sim))
    }
}

#[cfg(test)]
mod tests {
    use ceteris_core::toy::Toy;

    use super::*;
    use crate::scenario::{Branch, EditKind, EditSource, ReplaySel};

    fn edit(tick: u32, province: usize, value: f64) -> Edit {
        Edit {
            tick,
            source: EditSource::User,
            god: true,
            kind: EditKind::State,
            target: format!("price.{province}"),
            value,
            query: None,
        }
    }

    fn apply(w: &mut Toy, e: &Edit) -> Result<(), String> {
        let p: usize = e.target.trim_start_matches("price.").parse().unwrap();
        w.price[p] = e.value;
        Ok(())
    }

    fn branch(id: &str, parent: Option<(&str, u32)>, edits: Vec<Edit>) -> Branch {
        Branch {
            id: id.to_string(),
            parent: parent.map(|(p, _)| p.to_string()),
            fork_tick: parent.map(|(_, t)| t),
            edits,
        }
    }

    fn bench(dir: &tempfile::TempDir, branches: Vec<Branch>) -> Workbench<Toy> {
        let scenario = Scenario {
            pack_hash: "p".into(),
            engine: "test+1".into(),
            start_year: 2000,
            replay: ReplaySel::Off,
            seeds: vec![3],
            params: Default::default(),
            branches,
        };
        let store = ChunkStore::open(dir.path().join("cache"), "test+1").unwrap();
        Workbench::new(scenario, Seed(3), |s| Toy::sim(6, s), apply, store)
    }

    fn hash(wb: &mut Workbench<Toy>, branch: &str, tick: u32) -> [u8; 32] {
        wb.at(branch, tick).unwrap().state_hash()
    }

    #[test]
    fn null_intervention_reproduces_parent() {
        let dir = tempfile::tempdir().unwrap();
        let mut wb = bench(
            &dir,
            vec![
                branch("base", None, vec![edit(5, 0, 2.0)]),
                branch("null", Some(("base", 12)), vec![]),
            ],
        );
        let parent = hash(&mut wb, "base", 40);
        let chunks = wb.store().chunk_count();
        assert_eq!(hash(&mut wb, "null", 40), parent);
        assert_eq!(
            wb.store().chunk_count(),
            chunks,
            "child shares the parent's chunks"
        );
    }

    #[test]
    fn fork_at_any_tick_equals_continuous_run() {
        for fork in 1..=26 {
            // The parent's edit at tick 3 reaches the child only when it precedes the fork.
            let mut flat = vec![edit(fork + 2, 1, 0.5)];
            if 3 < fork {
                flat.insert(0, edit(3, 0, 2.0));
            }
            let dir = tempfile::tempdir().unwrap();
            let mut wb = bench(
                &dir,
                vec![
                    branch("base", None, vec![edit(3, 0, 2.0)]),
                    branch("child", Some(("base", fork)), vec![edit(fork + 2, 1, 0.5)]),
                    branch("flat", None, flat.clone()),
                ],
            );
            // Parent first, so the fork loads one of its snapshots.
            hash(&mut wb, "base", 30);
            assert_eq!(
                hash(&mut wb, "child", 30),
                hash(&mut wb, "flat", 30),
                "fork {fork}"
            );
        }
    }

    #[test]
    fn child_of_child_replays_from_root() {
        let dir = tempfile::tempdir().unwrap();
        let mut wb = bench(
            &dir,
            vec![
                branch("a", None, vec![edit(2, 0, 2.0)]),
                branch("b", Some(("a", 10)), vec![edit(11, 1, 0.5)]),
                branch("c", Some(("b", 20)), vec![edit(21, 2, 3.0)]),
                branch(
                    "flat",
                    None,
                    vec![edit(2, 0, 2.0), edit(11, 1, 0.5), edit(21, 2, 3.0)],
                ),
            ],
        );
        assert_eq!(hash(&mut wb, "c", 36), hash(&mut wb, "flat", 36));
    }

    #[test]
    fn parent_edit_at_the_fork_tick_is_not_inherited() {
        let dir = tempfile::tempdir().unwrap();
        let mut wb = bench(
            &dir,
            vec![
                branch("base", None, vec![edit(8, 0, 2.0)]),
                branch("child", Some(("base", 8)), vec![]),
                branch("none", None, vec![]),
            ],
        );
        assert_eq!(hash(&mut wb, "child", 20), hash(&mut wb, "none", 20));
        assert_ne!(hash(&mut wb, "base", 20), hash(&mut wb, "none", 20));
    }
}
