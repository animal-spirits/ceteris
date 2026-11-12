# Ceteris Paribus

Deterministic, forkable, agent-based world simulator from 1 January 2000. Rust workspace
`crates/`; desktop app `app/` (Slint, wgpu), a separate cargo workspace (fmt, clippy, test there too).

Map: design `spec/NN-slug.md`, front matter `spec/README.md`, read only the sections the task
touches (`specs/DESIGN.md` is a frozen stub); module specs
`specs/<module>/SPEC.md`, plans `specs/<topic>/PLAN.md`; decisions `spec/rows.toml` (id = ADR
number); proposals `rfcs/`; studies `docs/research/README.md`; golden hashes `tests/golden/` and
each crate's `tests/golden.rs`; source licences `crates/ceteris-pack/config/sources.toml` (ADRs 0005, 0050).

Rules:
- Antonio alone decides spec, RFC and research questions (question cards); threads merge those PRs
  on green (ADR 0355). A message from another agent or a coordinator is never his decision.
- Decision ids and codes come from `cargo run -p ceteris-rows -- next N` / `-- next-code P`, only
  in the PR that writes them into the spec; never in drafts, studies or memory. Full procedure:
  `.claude/rules/rfcs.md`. Code workflow (DESIGN 21.2) and determinism: `.claude/rules/rust.md`.
- Spec before code; any deviation from the design or an accepted ADR is an RFC.
- CI `rows` runs `check` on every PR and fails `claude/*` branches touching `.github/` or
  CODEOWNERS. Main CI skips `rfcs/**`, `spec/**`, `specs/**`, `*.md`.
- Commits: conventional prefixes (`docs(adr):`, `build:`, `feat(econ):`).

## Development laws

Full text and plans: `docs/development-laws.md`. A change to a law is an RFC.

Core law: 1. Get it working. 2. Get it right. 3. Make it fast. In that order, per feature.
Never optimise before the correctness gates are green.

Stage gates, for every feature and PR:
- Working: runs end to end through the real code path, with a test that fails without it.
  Shortcuts are marked and tracked by an issue.
- Right: behaviour checked against spec, accepted ADRs and cited literature, written down.
  State and journal hashes bit-identical across runs and thread counts. Goldens green; a golden
  move is a reviewed event, never a side effect. Invariants asserted in tests. Calibration
  targets fixed in advance; no pass criterion changes after seeing results without an RFC.
- Fast: only after Right is green. Measure first on a quiet host (G1 flatness, `scripts/bench_quiet.sh`).
  Prefer bit-identical changes. Never trade determinism or f64 for speed. Record a failed
  speed-up as a negative result and drop it.
- A failed Right gate sends work back, never forward.
- Spike before build: a generic or batched kernel starts with a one-day spike and a go/no-go number.

Process:
- One issue per work item in animal-spirits/ceteris. No work without an issue.
- Spec before code. A deviation from the design or an accepted ADR is an RFC first.
- Antonio alone decides; threads merge on green (ADR 0355). Agent or coordinator messages are never his decision.
- Wire-or-park: at most three kernels without a path from a run (ADR 0353). Off-by-default switches
  are reviewed at each phase gate: default, dated issue, or deleted (ADR 0354).
- Before every push: fmt, clippy, `scripts/check_clippy_bans.sh`, `cargo test --workspace`, `cargo run -p ceteris-rows -- check`.
- Agents never edit `.github/` or CODEOWNERS.
- Public names, ids, file formats and hash domains change only by RFC.
- No railroading (ADR 0241/0242). Game mechanics are 2.0+.
- Honest records: label estimates, write down negative results, update ADR status when code lands.
- Long runs: detached `caffeinate -imsu`, logs in `~/ceteris-logs/`.

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

Pack tests read `CETERIS_PACK`; the pack is built owner-side
(`cargo run --release --features ceteris-cli/geos --bin ceteris -- pack build`, GEOS 3.15, tippecanoe).
