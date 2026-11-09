//! Version-control verbs on branches: rebase, cherry-pick and bisect (ADR 0116).
//!
//! All three work on the scenario's branch tree, so what they produce is again a scenario whose
//! run hash names the result. Edits that cannot apply at their new place are reported, not
//! lost silently.

use std::collections::BTreeMap;

use crate::scenario::{Edit, Hash, Scenario, ScenarioError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// The edit's tick precedes the new fork tick, so the branch never reaches it.
    BeforeFork,
    /// The edit's target is absent in the new parent.
    TargetAbsent,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Skipped {
    pub edit: Edit,
    pub reason: SkipReason,
}

fn partition(
    edits: impl IntoIterator<Item = Edit>,
    fork_tick: u32,
    target_exists: &impl Fn(&Edit) -> bool,
) -> (Vec<Edit>, Vec<Skipped>) {
    let (mut kept, mut skipped) = (Vec::new(), Vec::new());
    for edit in edits {
        let reason = if edit.tick < fork_tick {
            Some(SkipReason::BeforeFork)
        } else if !target_exists(&edit) {
            Some(SkipReason::TargetAbsent)
        } else {
            None
        };
        match reason {
            Some(reason) => skipped.push(Skipped { edit, reason }),
            None => kept.push(edit),
        }
    }
    (kept, skipped)
}

/// Moves `branch` to fork from `onto` at `onto_tick`, keeping its edits that still apply.
pub fn rebase(
    s: &Scenario,
    branch: &str,
    onto: &str,
    onto_tick: u32,
    target_exists: impl Fn(&Edit) -> bool,
) -> Result<(Scenario, Vec<Skipped>), ScenarioError> {
    s.branch(branch)?;
    s.branch(onto)?;
    let mut out = s.clone();
    let b = out
        .branches
        .iter_mut()
        .find(|b| b.id == branch)
        .expect("checked above");
    let (kept, skipped) = partition(std::mem::take(&mut b.edits), onto_tick, &target_exists);
    b.parent = Some(onto.to_string());
    b.fork_tick = Some(onto_tick);
    b.edits = kept;
    out.validate()?;
    Ok((out, skipped))
}

/// Appends the edits of `from` at positions `picks` (in log order) to `onto`'s own edits.
pub fn cherry_pick(
    s: &Scenario,
    from: &str,
    picks: &[usize],
    onto: &str,
    target_exists: impl Fn(&Edit) -> bool,
) -> Result<(Scenario, Vec<Skipped>), ScenarioError> {
    let source = s.branch(from)?;
    let fork = s.branch(onto)?.fork_tick.unwrap_or(0);
    let mut picks = picks.to_vec();
    picks.sort_unstable();
    picks.dedup();
    let chosen: Vec<Edit> = picks
        .iter()
        .filter_map(|&i| source.edits.get(i).cloned())
        .collect();
    let (kept, skipped) = partition(chosen, fork, &target_exists);
    let mut out = s.clone();
    out.branches
        .iter_mut()
        .find(|b| b.id == onto)
        .expect("checked above")
        .edits
        .extend(kept);
    Ok((out, skipped))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bisected {
    /// Position in the branch's own edit list of the first edit after which the probe is bad.
    pub culprit: Option<usize>,
    /// Probe runs made, cache hits excluded.
    pub probes: usize,
}

/// Finds the first edit of `branch` after which `bad` holds, assuming the branch with none of
/// its own edits is good. A probe is the branch's scenario cut to a prefix of its edits; it is
/// cached by the prefix's run hash, so a repeated subset costs nothing. At most
/// ceil(log2 n) + 1 probes for n edits.
pub fn bisect(
    s: &Scenario,
    branch: &str,
    seed: u64,
    mut bad: impl FnMut(&Scenario, &str) -> bool,
) -> Result<Bisected, ScenarioError> {
    let n = s.branch(branch)?.edits.len();
    let mut cache: BTreeMap<Hash, bool> = BTreeMap::new();
    let mut probes = 0;
    let mut probe = |k: usize| -> Result<bool, ScenarioError> {
        let mut cut = s.clone();
        cut.branches
            .iter_mut()
            .find(|b| b.id == branch)
            .expect("checked above")
            .edits
            .truncate(k);
        let key = cut.run_hash(branch, seed)?;
        if let Some(&hit) = cache.get(&key) {
            return Ok(hit);
        }
        probes += 1;
        let verdict = bad(&cut, branch);
        cache.insert(key, verdict);
        Ok(verdict)
    };
    if n == 0 || !probe(n)? {
        return Ok(Bisected {
            culprit: None,
            probes,
        });
    }
    let (mut good, mut worse) = (0, n);
    while worse - good > 1 {
        let mid = good + (worse - good) / 2;
        if probe(mid)? {
            worse = mid;
        } else {
            good = mid;
        }
    }
    Ok(Bisected {
        culprit: Some(worse - 1),
        probes,
    })
}

#[cfg(test)]
mod tests {
    use ceteris_core::rng::Seed;
    use ceteris_core::toy::Toy;

    use super::*;
    use crate::branch::Workbench;
    use crate::scenario::{Branch, EditKind, EditSource, ReplaySel};
    use crate::store::ChunkStore;

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

    fn branch(id: &str, parent: Option<(&str, u32)>, edits: Vec<Edit>) -> Branch {
        Branch {
            id: id.to_string(),
            parent: parent.map(|(p, _)| p.to_string()),
            fork_tick: parent.map(|(_, t)| t),
            edits,
        }
    }

    fn scenario(branches: Vec<Branch>) -> Scenario {
        Scenario {
            pack_hash: "p".into(),
            engine: "test+1".into(),
            start_year: 2000,
            replay: ReplaySel::Off,
            seeds: vec![3],
            params: Default::default(),
            branches,
        }
    }

    fn apply(w: &mut Toy, e: &Edit) -> Result<(), String> {
        let p: usize = e.target.trim_start_matches("price.").parse().unwrap();
        w.price[p] = e.value;
        Ok(())
    }

    fn bench(s: &Scenario) -> (tempfile::TempDir, Workbench<Toy>) {
        let dir = tempfile::tempdir().unwrap();
        let store = ChunkStore::open(dir.path().join("cache"), "test+1").unwrap();
        let wb = Workbench::new(s.clone(), Seed(3), |s| Toy::sim(6, s), apply, store);
        (dir, wb)
    }

    fn hash(s: &Scenario, id: &str, tick: u32) -> [u8; 32] {
        let (_dir, mut wb) = bench(s);
        wb.at(id, tick).unwrap().state_hash()
    }

    #[test]
    fn bisect_finds_planted_edit_in_log2_probes() {
        // 20 benign edits on provinces 1 to 5, one of them replaced by a price crash on 0.
        for planted in 0..20 {
            let edits: Vec<Edit> = (0..20)
                .map(|i| {
                    if i == planted {
                        edit(i as u32, 0, 1.0)
                    } else {
                        edit(i as u32, 1 + i % 5, 12_000.0)
                    }
                })
                .collect();
            let s = scenario(vec![branch("b", None, edits)]);
            let found = bisect(&s, "b", 3, |cut, id| {
                let (_dir, mut wb) = bench(cut);
                wb.at(id, 21).unwrap().world().price[0] < 100.0
            })
            .unwrap();
            assert_eq!(found.culprit, Some(planted), "planted {planted}");
            assert!(found.probes <= 6, "{} probes", found.probes);
        }
    }

    #[test]
    fn bisect_reports_no_culprit_when_the_branch_is_good() {
        let s = scenario(vec![branch("b", None, vec![edit(2, 1, 1.0)])]);
        let found = bisect(&s, "b", 3, |_, _| false).unwrap();
        assert_eq!(found.culprit, None);
    }

    #[test]
    fn rebase_onto_own_parent_reproduces() {
        let s = scenario(vec![
            branch("base", None, vec![edit(3, 0, 2.0)]),
            branch(
                "c",
                Some(("base", 8)),
                vec![edit(10, 1, 0.5), edit(12, 2, 3.0)],
            ),
        ]);
        let (rebased, skipped) = rebase(&s, "c", "base", 8, |_| true).unwrap();
        assert!(skipped.is_empty());
        assert_eq!(rebased, s);
        assert_eq!(hash(&rebased, "c", 20), hash(&s, "c", 20));
    }

    #[test]
    fn rebase_reports_edits_it_cannot_keep() {
        let s = scenario(vec![
            branch("base", None, vec![]),
            branch("other", None, vec![]),
            branch(
                "c",
                Some(("base", 8)),
                vec![edit(9, 1, 0.5), edit(12, 7, 3.0), edit(14, 2, 1.5)],
            ),
        ]);
        let exists = |e: &Edit| e.target != "price.7";
        let (rebased, skipped) = rebase(&s, "c", "other", 10, exists).unwrap();
        assert_eq!(skipped.len(), 2);
        assert_eq!(skipped[0].reason, SkipReason::BeforeFork);
        assert_eq!(skipped[1].reason, SkipReason::TargetAbsent);
        let c = rebased.branch("c").unwrap();
        assert_eq!(c.parent.as_deref(), Some("other"));
        assert_eq!(c.edits, vec![edit(14, 2, 1.5)]);
    }

    #[test]
    fn cherry_pick_all_onto_parent_reproduces() {
        let edits = vec![edit(10, 1, 0.5), edit(12, 2, 3.0), edit(15, 0, 2.0)];
        let s = scenario(vec![
            branch("base", None, vec![edit(3, 0, 2.0)]),
            branch("c", Some(("base", 8)), edits),
            branch("n", Some(("base", 8)), vec![]),
        ]);
        let (picked, skipped) = cherry_pick(&s, "c", &[2, 0, 1], "n", |_| true).unwrap();
        assert!(skipped.is_empty());
        assert_eq!(
            picked.branch("n").unwrap().edits,
            s.branch("c").unwrap().edits
        );
        assert_eq!(hash(&picked, "n", 24), hash(&s, "c", 24));
    }
}
