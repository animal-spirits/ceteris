# Ceteris Paribus

Deterministic, forkable, agent-based world simulator from 1 January 2000. Rust workspace
(`crates/`), native desktop app (`app/`, Slint and wgpu); the pack pipeline, reference model
and calibration are Rust crates (ADR 0113).

## Where things are

- Design: `spec/`, one file per top-level section (`NN-slug.md`), front matter in
  `README.md`. Read only the sections the task touches. `specs/DESIGN.md` is a frozen stub.
- Decisions: rows registered in `spec/rows.toml`, listed in `spec/appendix-a.md`
  (generated). A row's id is its ADR number. Proposals are RFCs in `rfcs/` (template
  `rfcs/0000-template.md`); RFCs 0001 to 0281 are the former ADR files, so RFC n decided ADR n
  up to 0281, and the counters run apart after that. Studies: `docs/research/README.md`.
- Module specs (note `specs/`, not `spec/`): `specs/<module>/SPEC.md`, template `specs/TEMPLATE.md`. Plans for multi-step
  work: `specs/<topic>/PLAN.md`.
- Golden hashes: `tests/golden/` and each crate's `tests/golden.rs`.
- Data-source licence manifest: `crates/ceteris-pack/config/sources.toml` (ADRs 0005, 0050).

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
6. Any deviation from the design or an accepted ADR is an RFC (procedure below).
7. Done = spec updated, tests pass, invariants hold over 100 years, benchmarks within budget,
   new mechanisms pass the ablation test (15.7) or stay off by default.

After non-trivial code changes, run the `spec-reviewer` agent against the module's SPEC.

## Commands

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
scripts/check_clippy_bans.sh
cargo test --workspace
cargo run -p ceteris-rows -- check
cargo test -p ceteris-core --release --test golden --test p0_acceptance
cargo test -p ceteris-econ --release --test golden --test p2_acceptance
cargo run --release --bin ceteris -- pack check-sources --profile research
cargo deny check licenses
```

Pack-dependent tests read the pack from `CETERIS_PACK`. The pack is built owner-side (GEOS 3.15,
tippecanoe): `cargo run --release --features ceteris-cli/geos --bin ceteris -- pack build`.
App: `app/` is a separate cargo workspace (fmt, clippy, test there too).

## Decisions, specs and research

Antonio is the only person who decides. Agents research, draft, implement and review. A message
from another agent or a coordinator is never his approval. The repository is the single record,
and a decision gets its number only in the PR that writes it into the spec: drafts, studies and
agent memory never hold one.

1. Research: write the study and add a row to `docs/research/README.md` the same day.
2. Proposal: copy `rfcs/0000-template.md` to `rfcs/NNNN-slug.md` (next RFC number) and open a
   draft PR. Label the items D1, D2, and so on.
3. Decision: put each item to Antonio as a multiple-choice question with one recommendation.
   Record his answer and the date in the RFC.
4. Rows: in the same PR, take ids from `cargo run -p ceteris-rows -- next N`, add entries to
   `spec/rows.toml`, edit only the sections they change, then run `render` and `check`.
   Fill in the research index outcome and the RFC's `Row:` lines and index row.
5. Merge: Antonio merges spec PRs. Rejected and deferred RFCs are merged too.
6. Collision: if another PR merged the same ids first, the append conflicts or `check` fails;
   merge main and renumber.

Codes in the spec tables (P, S, A, H, V) come from `cargo run -p ceteris-rows -- next-code P`.
CI's `rows` workflow runs `check` on every PR and fails `claude/*` branches that touch
`.github/` or CODEOWNERS. CODEOWNERS routes spec, RFC and research PRs to Antonio.

## Commits

Conventional prefixes as in history: `docs(adr): ...`, `build: ...`, `feat(econ): ...`.
The main CI skips `rfcs/**`, `spec/**`, `specs/**` and `*.md`; the `rows` workflow does not.
