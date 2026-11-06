use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

pub type Hash = [u8; 32];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScenarioError {
    Parse(String),
    UnknownEditKind(String),
    DuplicateBranch(String),
    UnknownParent { branch: String, parent: String },
    ParentCycle(String),
    RootWithForkTick(String),
    ChildWithoutForkTick(String),
    UnknownBranch(String),
}

impl fmt::Display for ScenarioError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "scenario: {e}"),
            Self::UnknownEditKind(k) => write!(f, "scenario: unknown edit kind `{k}`"),
            Self::DuplicateBranch(b) => write!(f, "scenario: branch `{b}` appears twice"),
            Self::UnknownParent { branch, parent } => {
                write!(
                    f,
                    "scenario: branch `{branch}` has unknown parent `{parent}`"
                )
            }
            Self::ParentCycle(b) => write!(f, "scenario: branch `{b}` is its own ancestor"),
            Self::RootWithForkTick(b) => {
                write!(f, "scenario: root branch `{b}` must not set fork_tick")
            }
            Self::ChildWithoutForkTick(b) => {
                write!(f, "scenario: branch `{b}` has a parent but no fork_tick")
            }
            Self::UnknownBranch(b) => write!(f, "scenario: no branch `{b}`"),
        }
    }
}

impl std::error::Error for ScenarioError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EditSource {
    User,
    Cli,
    Api,
    Replay,
    Commitment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EditKind {
    Param,
    State,
    Policy,
    Event,
    Override,
    Replay,
    Switch,
}

impl EditKind {
    const ALL: [Self; 7] = [
        Self::Param,
        Self::State,
        Self::Policy,
        Self::Event,
        Self::Override,
        Self::Replay,
        Self::Switch,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Param => "param",
            Self::State => "state",
            Self::Policy => "policy",
            Self::Event => "event",
            Self::Override => "override",
            Self::Replay => "replay",
            Self::Switch => "switch",
        }
    }
}

/// Which replay tape families feed edits (switch S07).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReplaySel {
    #[default]
    Off,
    Shocks,
    Policy,
    All,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edit {
    pub tick: u32,
    pub source: EditSource,
    #[serde(default)]
    pub god: bool,
    pub kind: EditKind,
    pub target: String,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Branch {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fork_tick: Option<u32>,
    #[serde(default, rename = "edit", skip_serializing_if = "Vec::is_empty")]
    pub edits: Vec<Edit>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scenario {
    pub pack_hash: String,
    pub engine: String,
    #[serde(default)]
    pub replay: ReplaySel,
    pub seeds: Vec<u64>,
    #[serde(default)]
    pub params: BTreeMap<String, f64>,
    #[serde(default, rename = "branch")]
    pub branches: Vec<Branch>,
}

impl Scenario {
    pub fn parse(text: &str) -> Result<Self, ScenarioError> {
        let value: toml::Value =
            toml::from_str(text).map_err(|e| ScenarioError::Parse(e.to_string()))?;
        reject_unknown_kinds(&value)?;
        let scenario: Self = value
            .try_into()
            .map_err(|e: toml::de::Error| ScenarioError::Parse(e.to_string()))?;
        scenario.validate()?;
        Ok(scenario)
    }

    /// Canonical text: fixed key order, no comments.
    pub fn to_toml(&self) -> String {
        toml::to_string(self).expect("scenario serializes")
    }

    fn validate(&self) -> Result<(), ScenarioError> {
        let mut seen = std::collections::BTreeSet::new();
        for b in &self.branches {
            if !seen.insert(b.id.as_str()) {
                return Err(ScenarioError::DuplicateBranch(b.id.clone()));
            }
        }
        for b in &self.branches {
            match (&b.parent, b.fork_tick) {
                (None, Some(_)) => return Err(ScenarioError::RootWithForkTick(b.id.clone())),
                (Some(_), None) => return Err(ScenarioError::ChildWithoutForkTick(b.id.clone())),
                (Some(p), Some(_)) if !seen.contains(p.as_str()) => {
                    return Err(ScenarioError::UnknownParent {
                        branch: b.id.clone(),
                        parent: p.clone(),
                    });
                }
                _ => {}
            }
            self.lineage(&b.id)?;
        }
        Ok(())
    }

    pub fn branch(&self, id: &str) -> Result<&Branch, ScenarioError> {
        self.branches
            .iter()
            .find(|b| b.id == id)
            .ok_or_else(|| ScenarioError::UnknownBranch(id.to_string()))
    }

    /// The branch and its ancestors, root first.
    pub fn lineage(&self, id: &str) -> Result<Vec<&Branch>, ScenarioError> {
        let mut chain = vec![self.branch(id)?];
        while let Some(parent) = &chain.last().expect("non-empty").parent {
            if chain.len() > self.branches.len() {
                return Err(ScenarioError::ParentCycle(id.to_string()));
            }
            chain.push(self.branch(parent)?);
        }
        chain.reverse();
        Ok(chain)
    }

    /// Hash of everything that determines the run: pack, engine, parameters, replay choice,
    /// the seed, and the edits of the branch and its ancestors with their fork ticks. Comments,
    /// key order and unrelated branches do not enter it.
    pub fn run_hash(&self, branch: &str, seed: u64) -> Result<Hash, ScenarioError> {
        let lineage = self.lineage(branch)?;
        let mut h = Hasher::default();
        h.str(&self.pack_hash);
        h.str(&self.engine);
        h.str(match self.replay {
            ReplaySel::Off => "off",
            ReplaySel::Shocks => "shocks",
            ReplaySel::Policy => "policy",
            ReplaySel::All => "all",
        });
        h.u64(seed);
        h.u64(self.params.len() as u64);
        for (k, v) in &self.params {
            h.str(k);
            h.f64(*v);
        }
        h.u64(lineage.len() as u64);
        for b in lineage {
            h.u64(u64::from(b.fork_tick.unwrap_or(0)));
            h.u64(b.edits.len() as u64);
            for e in &b.edits {
                h.u64(u64::from(e.tick));
                h.str(match e.source {
                    EditSource::User => "user",
                    EditSource::Cli => "cli",
                    EditSource::Api => "api",
                    EditSource::Replay => "replay",
                    EditSource::Commitment => "commitment",
                });
                h.u64(u64::from(e.god));
                h.str(e.kind.name());
                h.str(&e.target);
                h.f64(e.value);
            }
        }
        Ok(h.finish())
    }
}

#[derive(Default)]
struct Hasher(blake3::Hasher);

impl Hasher {
    fn u64(&mut self, v: u64) {
        self.0.update(&v.to_le_bytes());
    }
    fn f64(&mut self, v: f64) {
        self.u64(v.to_bits());
    }
    fn str(&mut self, s: &str) {
        self.u64(s.len() as u64);
        self.0.update(s.as_bytes());
    }
    fn finish(self) -> Hash {
        *self.0.finalize().as_bytes()
    }
}

fn reject_unknown_kinds(value: &toml::Value) -> Result<(), ScenarioError> {
    let branches = value.get("branch").and_then(toml::Value::as_array);
    for edit in branches
        .into_iter()
        .flatten()
        .filter_map(|b| b.get("edit").and_then(toml::Value::as_array))
        .flatten()
    {
        if let Some(kind) = edit.get("kind").and_then(toml::Value::as_str)
            && !EditKind::ALL.iter().any(|k| k.name() == kind)
        {
            return Err(ScenarioError::UnknownEditKind(kind.to_string()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = r#"
# a comment
pack_hash = "abc"
engine = "0.0.1+x"
seeds = [1, 2]
replay = "policy"

[params]
P01 = 4.5
P02 = 0.25

[[branch]]
id = "base"

[[branch]]
id = "tariff"
parent = "base"
fork_tick = 24

[[branch.edit]]
tick = 24
source = "user"
kind = "policy"
target = "tariff.US.CN"
value = 0.25
"#;

    fn scenario() -> Scenario {
        Scenario::parse(TEXT).unwrap()
    }

    fn hash(s: &Scenario) -> Hash {
        s.run_hash("tariff", 1).unwrap()
    }

    #[test]
    fn scenario_roundtrip_is_byte_stable() {
        let a = scenario();
        let text = a.to_toml();
        let b = Scenario::parse(&text).unwrap();
        assert_eq!(a, b);
        assert_eq!(text, b.to_toml());
    }

    #[test]
    fn run_hash_changes_with_any_field() {
        let base = hash(&scenario());
        let mutations: [fn(&mut Scenario); 11] = [
            |s| s.pack_hash.push('x'),
            |s| s.engine.push('x'),
            |s| s.replay = ReplaySel::All,
            |s| *s.params.get_mut("P01").unwrap() = 4.6,
            |s| {
                s.params.insert("P03".into(), 1.0);
            },
            |s| s.branches[1].fork_tick = Some(25),
            |s| s.branches[1].edits[0].tick = 25,
            |s| s.branches[1].edits[0].source = EditSource::Cli,
            |s| s.branches[1].edits[0].god = true,
            |s| s.branches[1].edits[0].value = 0.3,
            |s| {
                let e = s.branches[1].edits[0].clone();
                s.branches[0].edits.push(e);
            },
        ];
        for (i, m) in mutations.iter().enumerate() {
            let mut s = scenario();
            m(&mut s);
            assert_ne!(hash(&s), base, "mutation {i}");
        }
        assert_ne!(scenario().run_hash("tariff", 2).unwrap(), base);
        assert_ne!(scenario().run_hash("base", 1).unwrap(), base);
    }

    #[test]
    fn run_hash_ignores_comments_key_order_and_other_branches() {
        let base = hash(&scenario());
        let reordered = r#"
seeds = [1, 2]
engine = "0.0.1+x"
pack_hash = "abc"
replay = "policy"
[params]
P02 = 0.25
P01 = 4.5
[[branch]]
id = "base"
[[branch]]
id = "other"
parent = "base"
fork_tick = 3
[[branch]]
parent = "base"
id = "tariff"
fork_tick = 24
[[branch.edit]]
value = 0.25
target = "tariff.US.CN"
kind = "policy"
source = "user"
tick = 24
"#;
        assert_eq!(hash(&Scenario::parse(reordered).unwrap()), base);
    }

    #[test]
    fn unknown_edit_kind_is_named_error() {
        let text = TEXT.replace("kind = \"policy\"", "kind = \"teleport\"");
        assert_eq!(
            Scenario::parse(&text),
            Err(ScenarioError::UnknownEditKind("teleport".into()))
        );
    }

    #[test]
    fn branch_tree_errors_are_named() {
        let dup = format!("{TEXT}\n[[branch]]\nid = \"base\"\n");
        assert_eq!(
            Scenario::parse(&dup),
            Err(ScenarioError::DuplicateBranch("base".into()))
        );
        let orphan = TEXT.replace("parent = \"base\"", "parent = \"nope\"");
        assert!(matches!(
            Scenario::parse(&orphan),
            Err(ScenarioError::UnknownParent { .. })
        ));
        let no_fork = TEXT.replace("fork_tick = 24\n", "");
        assert_eq!(
            Scenario::parse(&no_fork),
            Err(ScenarioError::ChildWithoutForkTick("tariff".into()))
        );
    }
}
