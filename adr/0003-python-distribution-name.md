# 0003. Python distribution name

Status: proposed (needs owner decision before P1 ships Python code)
Date: 2026-10-23
Design sections: 22.1 (Name: Python package ceteris)

## Context

Section 22.1 asks to check crates.io, PyPI and GitHub before P0. Results on
2026-10-23:

- crates.io: `ceteris`, `ceteris-core`, `ceteris-py` are free.
- PyPI: `ceteris` is taken by an unrelated, maintained project (v0.4.1, "capture
  benchmark run environment fingerprints").
- GitHub: `ceterisParibus` and `pyCeterisParibus` (ModelOriented) exist for
  what-if model explanation plots; no conflict on the simulator name.

## Options

1. Distribution `ceteris-paribus` (free on PyPI), import name `ceteris`.
2. Distribution `ceteris-sim` (free), import name `ceteris`.
3. Ask the owner of `ceteris` on PyPI to transfer the name.

## Decision

Pending. Proposed: option 1, which matches the project name and keeps
`import ceteris`.

## Consequences

Option 1 or 2 means `pip install ceteris-paribus` but `import ceteris`, a common
pattern (for example `scikit-learn` and `sklearn`). Rust crate names are unaffected.
