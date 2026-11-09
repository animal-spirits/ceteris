//! The counterfactual workbench's run model: scenario files, branches, run hashes
//! (Sections 14.1, 14.3; ADRs 0128, 0211). See `specs/workbench/SPEC.md`.

pub mod attribution;
pub mod branch;
pub mod commands;
pub mod effects;
pub mod ensemble;
pub mod outputs;
pub mod query;
pub mod replay;
pub mod retention;
pub mod scenario;
pub mod serve;
pub mod store;
pub mod vcs;

pub use branch::{RunError, Workbench, due};
pub use query::{Query, QueryError};
pub use replay::{ReplayError, check_table_read, replay_edits};
pub use scenario::{Branch, Edit, EditKind, EditSource, ReplaySel, Scenario, ScenarioError};

/// Crate version plus the git commit of the build; a dirty tree appends `+dirty` and its runs
/// are not shareable.
pub fn engine_id() -> String {
    format!(
        "{}+{}",
        env!("CARGO_PKG_VERSION"),
        env!("CETERIS_BUILD_COMMIT")
    )
}
