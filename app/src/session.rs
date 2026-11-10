//! W1 session: the scenario's branch tree, a cursor on one branch, and the verbs fork, seek
//! and run-to over the run crate's workbench (specs/p3/PLAN.md Branch W). Views are read off
//! `Workbench::at`, so the app sees the hashes a headless run of the same scenario gives.

use ceteris_core::sim::Sim;
use ceteris_econ::EconWorld;
use ceteris_run::{Branch, RunError, Scenario, Workbench};

use crate::branches::{self, Node, Row};
use crate::commands::BranchCommand;

pub struct Session {
    scenario: Scenario,
    make: Box<dyn Fn(Scenario) -> Workbench<EconWorld>>,
    bench: Workbench<EconWorld>,
    branch: String,
    cursor: u32,
    last_tick: u32,
}

impl Session {
    /// Starts on the first branch at tick 0. `make` builds a workbench for a scenario; it is
    /// called again whenever a fork changes the tree.
    pub fn new(
        scenario: Scenario,
        make: impl Fn(Scenario) -> Workbench<EconWorld> + 'static,
    ) -> Self {
        let branch = scenario.branches[0].id.clone();
        let bench = make(scenario.clone());
        Self {
            scenario,
            make: Box::new(make),
            bench,
            branch,
            cursor: 0,
            last_tick: 0,
        }
    }

    pub fn branch(&self) -> &str {
        &self.branch
    }

    pub fn cursor(&self) -> u32 {
        self.cursor
    }

    pub fn rows(&self) -> Vec<Row> {
        let nodes: Vec<Node> = self
            .scenario
            .branches
            .iter()
            .map(|b| Node {
                id: b.id.clone(),
                parent: b.parent.clone(),
                fork_tick: b.fork_tick,
                edits: b.edits.len(),
            })
            .collect();
        branches::tree_rows(&nodes)
    }

    /// The current branch's edit log, ancestors' edits before the fork included.
    pub fn log(&self) -> Vec<String> {
        self.scenario
            .edits_of(&self.branch)
            .unwrap_or_default()
            .iter()
            .map(|e| {
                let source = format!("{:?}", e.source).to_lowercase();
                branches::log_line(
                    e.tick,
                    &source,
                    e.god,
                    &e.target,
                    e.value,
                    e.query.as_deref(),
                )
            })
            .collect()
    }

    /// The sim of the current branch at the cursor.
    pub fn view(&mut self) -> Result<Sim<EconWorld>, RunError> {
        self.bench.at(&self.branch, self.cursor)
    }

    /// Runs one registry verb. Run-to and seek move the cursor within the branch's reach.
    pub fn execute(&mut self, cmd: BranchCommand) -> Result<(), String> {
        match cmd {
            BranchCommand::Fork(id) => {
                if self.scenario.branch(id).is_ok() {
                    return Err(format!("branch {id} exists"));
                }
                self.scenario.branches.push(Branch {
                    id: id.into(),
                    parent: Some(self.branch.clone()),
                    fork_tick: Some(self.cursor),
                    edits: vec![],
                });
                self.bench = (self.make)(self.scenario.clone());
                self.branch = id.into();
            }
            BranchCommand::Switch(id) => {
                let b = self.scenario.branch(id).map_err(|e| e.to_string())?;
                self.cursor = branches::clamp_seek(self.cursor, b.fork_tick, u32::MAX);
                self.branch = id.into();
            }
            BranchCommand::Seek(t) => {
                let fork = self
                    .scenario
                    .branch(&self.branch)
                    .map_err(|e| e.to_string())?
                    .fork_tick;
                self.cursor = branches::clamp_seek(t, fork, self.last_tick.max(self.cursor));
            }
            BranchCommand::RunTo(t) => {
                self.bench.at(&self.branch, t).map_err(|e| e.to_string())?;
                self.cursor = t;
                self.last_tick = self.last_tick.max(t);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use ceteris_core::rng::Seed;
    use ceteris_econ::EconParams;
    use ceteris_econ::edit::apply_edit;
    use ceteris_econ::fixture::small_world;
    use ceteris_run::Edit;
    use ceteris_run::store::ChunkStore;
    use ceteris_trade::TradeParams;

    use super::*;

    fn apply(w: &mut EconWorld, e: &Edit) -> Result<(), String> {
        apply_edit(w, e).map_err(|e| e.to_string())
    }

    fn scenario() -> Scenario {
        Scenario {
            pack_hash: "fixture".into(),
            engine: "test+1".into(),
            start_year: 2000,
            replay: Default::default(),
            seeds: vec![1],
            params: Default::default(),
            branches: vec![Branch {
                id: "main".into(),
                parent: None,
                fork_tick: None,
                edits: vec![],
            }],
        }
    }

    fn bench(s: Scenario) -> Workbench<EconWorld> {
        let dir = tempfile::tempdir().unwrap().keep();
        let store = ChunkStore::open(dir.join("cache"), "test+1").unwrap();
        Workbench::new(
            s,
            Seed(1),
            |_| {
                EconWorld::sim(
                    Arc::new(small_world()),
                    EconParams::default(),
                    TradeParams::default(),
                )
            },
            apply,
            store,
        )
    }

    #[test]
    fn app_session_hash_equals_headless_run_and_fork_seek_clamp() {
        let mut headless = bench(scenario());
        let want = headless.at("main", 12).unwrap().state_hash();

        let mut s = Session::new(scenario(), bench);
        s.execute(BranchCommand::RunTo(12)).unwrap();
        assert_eq!(s.view().unwrap().state_hash(), want);

        s.execute(BranchCommand::Fork("null")).unwrap();
        assert_eq!(s.branch(), "null");
        assert_eq!(s.view().unwrap().state_hash(), want);
        assert!(s.execute(BranchCommand::Fork("null")).is_err());

        s.execute(BranchCommand::Seek(3)).unwrap();
        assert_eq!(s.cursor(), 12, "seek never goes before the fork tick");
        let rows = s.rows();
        assert_eq!(
            rows.iter()
                .map(|r| (r.id.as_str(), r.depth))
                .collect::<Vec<_>>(),
            [("main", 0), ("null", 1)]
        );
    }
}
