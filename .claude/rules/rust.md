---
paths:
  - "crates/**/*.rs"
  - "app/**/*.rs"
---

# Rust rules (DESIGN 4.5, 19.4; ADR 0097)

Determinism is the product. A state hash must not depend on run, thread count, architecture
or optimizer.

- RNG: Philox counter-based only; every draw is a function of (seed, tick, system, entity,
  draw index). No global or thread-local RNG.
- Float reductions in parallel: `ceteris_core::reduce::fixed_sum`, or write to fixed slots and
  combine in index order, or the exact accumulator of ADR 0298 where order must be free.
  rayon `sum`/`reduce`/`fold` and friends are banned by `clippy.toml`.
- No `HashMap`/`HashSet`: use `BTreeMap`/`BTreeSet` or a `Vec` indexed by id.
- No fast-math, no `algebraic_*`, no implicit FMA; use `mul_add` only where the spec says so.
  Transcendentals go through the pure-Rust libm the core already uses; `clippy.toml` bans the
  std ones (tests, the reference oracle and offline `ceteris-calibrate` allow them). Portable
  golden gate: `scripts/check_golden.sh`.
- Iterate entities in stable-id order; break ties by id.
- Iterative solves must not warm-start from state that a snapshot does not carry (ADR 0097).
- Hot loops: no allocation, structure-of-arrays, `u32` indices.
- `unsafe` is forbidden except unchecked indexing in a crate's `hot` module under ADR 0295
  (`SAFETY:` comment, `debug_assert!` of the bound, safe twin compared in CI, at least 2% of
  the tick).
- Money moves are exact `i64` with largest-remainder splits; never round floats into the ledger.

Adding a new banned path to `clippy.toml` also updates the fixture count in
`scripts/check_clippy_bans.sh`.

## Workflow (DESIGN 21.2)

1. Spec first: update `specs/<module>/SPEC.md` (template `specs/TEMPLATE.md`) before code. The
   owner approves specs for P2, P5 and P6 modules; do not start code on an unapproved spec there.
2. Tests before code: unit tests per equation, proptest for invariants, an equilibrium-nesting
   test where it applies.
3. One system or one data table per task; the world stays runnable after each.
4. A change that moves a golden hash says why in the commit and the CHANGELOG
   (`### BEHAVIOUR: ...`), and updates the hashes in the same commit.
5. No silent tuning: a parameter changed to pass a test is recorded in the register
   (DESIGN 17) with its justification.
6. Any deviation from the design or an accepted ADR is an RFC.
7. Done = spec updated, tests pass, invariants hold over 100 years, benchmarks within budget,
   new mechanisms pass the ablation test (15.7) or stay off by default.

After non-trivial code changes, run the `spec-reviewer` agent against the module's SPEC.
