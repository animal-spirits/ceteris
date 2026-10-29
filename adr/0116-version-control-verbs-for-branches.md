# 0116. Version-control verbs for branches

Status: proposed
Date: 2026-10-28
Design sections: 14.1, 14.3, 14.7, 20.2
Phase: P3

## Context

A branch is a parent plus an ordered list of timed edits (14.1), and a null edit reproduces the
parent bit for bit (14.3). That is the data model of a version-control system, but the
workbench offers only fork and compare. With twenty edits on a branch, finding which one
flipped an outcome means rerunning subsets by hand. Brainstorm:
https://claude.ai/artifact/N9zi5kznSaqvjg9ztw4WMT (idea 1).

## Options

1. Status quo: fork, compare and Shapley attribution (14.7).
2. CLI and branch-manager verbs modelled on git: bisect, rebase, cherry-pick and blame.

## Decision

Option 2.

- `ceteris bisect`: given a branch, a baseline and a predicate on an outcome (for example
  "Argentina defaults before 2003" in at least half the lanes), binary-search over prefixes of
  the edit list, then over single edits, for the smallest set that flips the predicate. Each
  probe is an ordinary ensemble run, cached by (manifest hash, edit subset).
- `ceteris rebase`: replay a branch's edits on a different parent. Edits that target entities
  absent in the new parent are reported and skipped, never guessed.
- `ceteris cherry-pick`: copy named edits into another branch at the same ticks.
- `ceteris blame`: for a series in one province, list the edits and shock-tape entries that
  moved it most, ranked by channel-shutdown effect (14.7) over a window.
- The branch manager (20.2) exposes the same verbs.

## Migration

- P3: verbs on the branch and intervention machinery; blame after channel shutdown exists.

## Tests

- Bisect on a branch with one planted decisive edit among twenty finds it in at most ceil(log2
  20) + 1 probes.
- Rebasing a branch onto its own parent reproduces it bit for bit.
- Cherry-picking every edit of branch A onto A's parent reproduces A.

## Consequences

Large counterfactual studies become navigable, and the cost of finding the edit that mattered
drops from linear to logarithmic in the number of edits.
