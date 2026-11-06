//! The counterfactual workbench's run model: scenario files, branches, run hashes
//! (Sections 14.1, 14.3; ADRs 0128, 0211). See `specs/workbench/SPEC.md`.

pub mod branch;
pub mod retention;
pub mod scenario;
pub mod store;

pub use branch::{RunError, Workbench, due};
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
