# Ceteris Paribus

Deterministic, forkable, agent-based world simulator from 1 January 2000. Rust workspace
(`crates/`), Tauri desktop app (`app/`), Python pack pipeline being removed (ADR 0113).

## Where things are

- Design: `specs/DESIGN.md`. Read only the sections the task touches; never load it whole.
- Decisions: `adr/NNNN-*.md`, template `adr/TEMPLATE.md`. Check for ADRs that amend a design
  section before relying on it (`grep -l "Design sections:.*<n>" adr/`).
- Module specs: `specs/<module>/SPEC.md`, template `specs/TEMPLATE.md`. Plans for multi-step
  work: `specs/<topic>/PLAN.md`.
- Golden hashes: `tests/golden/` and each crate's `tests/golden.rs`.
- Data-source licence manifest: `python/ceteris/packs/sources.toml` (ADRs 0005, 0050).

## Workflow (DESIGN 21.2)

1. Spec first: update `specs/<module>/SPEC.md` before code. The owner approves specs for
   P2, P5 and P6 modules; do not start code on an unapproved spec there.
2. Tests before code: unit tests per equation, proptest for invariants, an equilibrium-nesting
   test where it applies.
3. One system or one data table per task; the world stays runnable after each.
4. A change that moves a golden hash says why in the commit and the CHANGELOG
   (`### BEHAVIOUR: ...`), and updates the hashes in the same commit.
5. No silent tuning: a parameter changed to pass a test is recorded in the register
   (DESIGN 17) with its justification.
6. Any deviation from DESIGN.md or an accepted ADR is a new ADR. ADR numbers collide across
   parallel branches: check `main` for the highest number right before pushing.
7. Done = spec updated, tests pass, invariants hold over 100 years, benchmarks within budget,
   new mechanisms pass the ablation test (15.7) or stay off by default.

After non-trivial code changes, run the `spec-reviewer` agent against the module's SPEC.

## Commands

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
scripts/check_clippy_bans.sh
cargo test --workspace
cargo test -p ceteris-core --release --test golden --test p0_acceptance
cargo test -p ceteris-econ --release --test golden --test p2_acceptance
cargo run --release --bin ceteris -- pack check-sources --profile research
cargo deny check licenses
```

Pack-dependent tests read the pack from `CETERIS_PACK`. App: `cd app && npm run build`;
`app/src-tauri` is a separate cargo workspace (fmt, clippy, test there too).

## Commits

Conventional prefixes as in history: `docs(adr): ...`, `build: ...`, `feat(econ): ...`.
CI skips `adr/**`, `specs/**` and `*.md`; PRs that change only ADRs merge once reviewed.
