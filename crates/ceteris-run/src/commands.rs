//! The command registry of the workbench (ADR 0188): every verb once, with its arguments and
//! one line of help, for the CLI, `serve` and the app's palette to share. The scenario-file
//! verbs are functions here; verbs that run worlds are registered and built by later tasks.

use crate::scenario::{Branch, Edit, EditKind, EditSource, Scenario, ScenarioError};

pub struct Command {
    pub name: &'static str,
    pub usage: &'static str,
    pub summary: &'static str,
    /// Whether the verb exists yet.
    pub built: bool,
}

const fn cmd(
    name: &'static str,
    usage: &'static str,
    summary: &'static str,
    built: bool,
) -> Command {
    Command {
        name,
        usage,
        summary,
        built,
    }
}

pub const REGISTRY: &[Command] = &[
    cmd(
        "new",
        "new FILE --pack-hash H --engine E [--seeds 1,2]",
        "write a scenario with a root branch `main`",
        true,
    ),
    cmd(
        "fork",
        "fork FILE --from BRANCH --at TICK --id ID",
        "add a branch forked from another at a tick",
        true,
    ),
    cmd(
        "edit",
        "edit FILE --branch B --at TICK --kind K --target T --value V [--query Q]",
        "append an edit to a branch",
        true,
    ),
    cmd(
        "diff",
        "diff FILE A B",
        "list the edits of two branches that differ",
        true,
    ),
    cmd("run", "run FILE --branch B --seed S", "run a branch", false),
    cmd(
        "ensemble",
        "ensemble FILE --branch B",
        "run a branch over its seeds",
        false,
    ),
    cmd(
        "effects",
        "effects FILE --branch B --baseline A",
        "paired effects of a branch",
        false,
    ),
    cmd(
        "explain",
        "explain FILE --branch B",
        "channel decomposition of an effect",
        false,
    ),
    cmd(
        "blame",
        "blame FILE --branch B",
        "attribute an effect to edits",
        false,
    ),
    cmd(
        "bisect",
        "bisect FILE --good A --bad B",
        "find the first edit that changes an outcome",
        false,
    ),
    cmd(
        "rebase",
        "rebase FILE --branch B --onto O --at TICK",
        "move a branch to another parent",
        false,
    ),
    cmd(
        "cherry-pick",
        "cherry-pick FILE --from B --onto O --edits 0,1",
        "copy edits between branches",
        false,
    ),
    cmd(
        "share",
        "share FILE --branch B --seed S",
        "print the run hash",
        true,
    ),
    cmd(
        "fetch",
        "fetch HASH",
        "recompute or pull a shared run",
        false,
    ),
];

pub fn new_scenario(pack_hash: &str, engine: &str, seeds: Vec<u64>) -> Scenario {
    Scenario {
        pack_hash: pack_hash.to_string(),
        engine: engine.to_string(),
        start_year: 2000,
        replay: Default::default(),
        seeds,
        params: Default::default(),
        branches: vec![Branch {
            id: "main".to_string(),
            parent: None,
            fork_tick: None,
            edits: vec![],
        }],
    }
}

pub fn share(s: &Scenario, branch: &str, seed: u64) -> Result<String, ScenarioError> {
    let hash = s.run_hash(branch, seed)?;
    Ok(hash.iter().map(|b| format!("{b:02x}")).collect())
}

pub fn fork(s: &Scenario, from: &str, at: u32, id: &str) -> Result<Scenario, ScenarioError> {
    s.branch(from)?;
    let mut out = s.clone();
    out.branches.push(Branch {
        id: id.to_string(),
        parent: Some(from.to_string()),
        fork_tick: Some(at),
        edits: vec![],
    });
    out.validate()?;
    Ok(out)
}

pub fn add_edit(s: &Scenario, branch: &str, edit: Edit) -> Result<Scenario, ScenarioError> {
    s.branch(branch)?;
    let mut out = s.clone();
    let b = out
        .branches
        .iter_mut()
        .find(|b| b.id == branch)
        .expect("checked above");
    b.edits.push(edit);
    Ok(out)
}

pub fn user_edit(
    tick: u32,
    kind: EditKind,
    target: &str,
    value: f64,
    query: Option<String>,
) -> Edit {
    Edit {
        tick,
        source: EditSource::Cli,
        god: kind == EditKind::State,
        kind,
        target: target.to_string(),
        value,
        query,
    }
}

/// Edits only in `a` (`-`) or only in `b` (`+`), over the whole lineage with fork ticks.
pub fn diff(s: &Scenario, a: &str, b: &str) -> Result<Vec<String>, ScenarioError> {
    let flat = |id: &str| -> Result<Vec<String>, ScenarioError> {
        let mut out = Vec::new();
        for br in s.lineage(id)? {
            for e in &br.edits {
                let q = e
                    .query
                    .as_deref()
                    .map_or(String::new(), |q| format!(" where {q}"));
                out.push(format!(
                    "{} {:?} {} = {}{}",
                    e.tick, e.kind, e.target, e.value, q
                ));
            }
        }
        Ok(out)
    };
    let (fa, fb) = (flat(a)?, flat(b)?);
    let mut lines: Vec<String> = fa
        .iter()
        .filter(|x| !fb.contains(x))
        .map(|x| format!("- {x}"))
        .collect();
    lines.extend(
        fb.iter()
            .filter(|x| !fa.contains(x))
            .map(|x| format!("+ {x}")),
    );
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_names_are_unique_and_cover_the_verbs() {
        let mut names: Vec<_> = REGISTRY.iter().map(|c| c.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 14);
        assert!(REGISTRY.iter().all(|c| c.usage.starts_with(c.name)));
    }

    #[test]
    fn new_fork_edit_diff_compose() {
        let s = new_scenario("p", "e", vec![1]);
        let s = fork(&s, "main", 12, "tariff").unwrap();
        let hash = s.run_hash("tariff", 1).unwrap();
        let s = add_edit(
            &s,
            "tariff",
            user_edit(13, EditKind::Policy, "tariff.0.1.0", 0.25, None),
        )
        .unwrap();
        assert_ne!(s.run_hash("tariff", 1).unwrap(), hash);
        assert_eq!(
            diff(&s, "main", "tariff").unwrap(),
            ["+ 13 Policy tariff.0.1.0 = 0.25"]
        );
        assert_eq!(Scenario::parse(&s.to_toml()).unwrap(), s);
        assert!(fork(&s, "nope", 1, "x").is_err());
    }
}
