//! W1 branch manager data (specs/p3/PLAN.md Branch W): the tree of branches as indented rows,
//! the edit log lines with source and god flag, and the seek clamp for stepping back. Plain
//! data in and out, so it is tested without the scenario or run crates or a GPU.

use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub id: String,
    pub parent: Option<String>,
    pub fork_tick: Option<u32>,
    pub edits: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub id: String,
    pub depth: usize,
    pub fork_tick: Option<u32>,
    pub edits: usize,
}

/// Depth-first rows, children ordered by fork tick then id. A branch whose parent is missing
/// from the list is a root, so a partial list still renders; ids already placed are skipped, so
/// a cycle or a duplicate id cannot loop or repeat a row.
pub fn tree_rows(nodes: &[Node]) -> Vec<Row> {
    let known: BTreeSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    let mut children: BTreeMap<Option<&str>, Vec<&Node>> = BTreeMap::new();
    for n in nodes {
        let parent = n
            .parent
            .as_deref()
            .filter(|p| known.contains(p) && *p != n.id);
        children.entry(parent).or_default().push(n);
    }
    for list in children.values_mut() {
        list.sort_by(|a, b| (a.fork_tick, &a.id).cmp(&(b.fork_tick, &b.id)));
    }
    let mut rows = Vec::new();
    let mut placed = BTreeSet::new();
    let mut stack: Vec<(&Node, usize)> = children
        .get(&None)
        .map(|r| r.iter().rev().map(|n| (*n, 0)).collect())
        .unwrap_or_default();
    while let Some((n, depth)) = stack.pop() {
        if !placed.insert(n.id.as_str()) {
            continue;
        }
        rows.push(Row {
            id: n.id.clone(),
            depth,
            fork_tick: n.fork_tick,
            edits: n.edits,
        });
        if let Some(kids) = children.get(&Some(n.id.as_str())) {
            stack.extend(kids.iter().rev().map(|k| (*k, depth + 1)));
        }
    }
    rows
}

/// One edit log line: tick, source, a god mark, target and value, and the query if any.
pub fn log_line(
    tick: u32,
    source: &str,
    god: bool,
    target: &str,
    value: f64,
    query: Option<&str>,
) -> String {
    let mut s = format!("t{tick} [{source}]");
    if god {
        s.push_str(" GOD");
    }
    s.push_str(&format!(" {target} = {value}"));
    if let Some(q) = query {
        s.push_str(&format!(" where {q}"));
    }
    s
}

/// Where a seek lands: never before the branch's fork tick (earlier ticks belong to its
/// parent) and never after the last run tick.
pub fn clamp_seek(target: u32, fork_tick: Option<u32>, last_tick: u32) -> u32 {
    target.max(fork_tick.unwrap_or(0)).min(last_tick)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, parent: Option<&str>, fork: Option<u32>) -> Node {
        Node {
            id: id.into(),
            parent: parent.map(Into::into),
            fork_tick: fork,
            edits: 0,
        }
    }

    fn ids(rows: &[Row]) -> Vec<(&str, usize)> {
        rows.iter().map(|r| (r.id.as_str(), r.depth)).collect()
    }

    #[test]
    fn children_follow_their_parent_ordered_by_fork_tick() {
        let nodes = [
            node("c", Some("base"), Some(30)),
            node("base", None, None),
            node("a", Some("base"), Some(12)),
            node("a2", Some("a"), Some(40)),
        ];
        assert_eq!(
            ids(&tree_rows(&nodes)),
            [("base", 0), ("a", 1), ("a2", 2), ("c", 1)]
        );
    }

    #[test]
    fn orphans_cycles_and_duplicates_do_not_loop_or_repeat() {
        let nodes = [
            node("x", Some("gone"), Some(1)),
            node("p", Some("q"), Some(1)),
            node("q", Some("p"), Some(2)),
            node("x", None, None),
            node("self", Some("self"), None),
        ];
        let rows = tree_rows(&nodes);
        let seen: BTreeSet<_> = rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(rows.len(), seen.len());
        assert!(seen.contains("x") && seen.contains("self"));
        // The p/q cycle has no root, so it is not drawn rather than looped over.
        assert!(!seen.contains("p"));
    }

    #[test]
    fn log_lines_carry_source_god_and_query() {
        assert_eq!(
            log_line(12, "user", false, "tariff.US.CN", 0.25, None),
            "t12 [user] tariff.US.CN = 0.25"
        );
        assert_eq!(
            log_line(3, "replay", true, "wage", 1.0, Some("country = 'DE'")),
            "t3 [replay] GOD wage = 1 where country = 'DE'"
        );
    }

    #[test]
    fn seek_stays_between_fork_and_last_tick() {
        assert_eq!(clamp_seek(5, Some(12), 100), 12);
        assert_eq!(clamp_seek(50, Some(12), 100), 50);
        assert_eq!(clamp_seek(500, Some(12), 100), 100);
        assert_eq!(clamp_seek(0, None, 100), 0);
    }
}
