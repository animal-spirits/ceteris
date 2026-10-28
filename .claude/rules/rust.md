---
paths:
  - "crates/**/*.rs"
  - "app/src-tauri/**/*.rs"
---

# Rust rules (DESIGN 4.5, 19.4; ADR 0097)

Determinism is the product. A state hash must not depend on run, thread count, architecture
or optimizer.

- RNG: Philox counter-based only; every draw is a function of (seed, tick, system, entity,
  draw index). No global or thread-local RNG.
- Float reductions in parallel: `ceteris_core::reduce::fixed_sum`, or write to fixed slots and
  combine in index order. rayon `sum`/`reduce`/`fold` and friends are banned by `clippy.toml`.
- No `HashMap`/`HashSet`: use `BTreeMap`/`BTreeSet` or a `Vec` indexed by id.
- No fast-math, no `algebraic_*`, no implicit FMA; use `mul_add` only where the spec says so.
  Transcendentals go through the pure-Rust libm the core already uses.
- Iterate entities in stable-id order; break ties by id.
- Iterative solves must not warm-start from state that a snapshot does not carry (ADR 0097).
- Hot loops: no allocation, structure-of-arrays, `u32` indices.
- `unsafe` is forbidden workspace-wide.
- Money moves are exact `i64` with largest-remainder splits; never round floats into the ledger.

Adding a new banned path to `clippy.toml` also updates the fixture count in
`scripts/check_clippy_bans.sh`.
