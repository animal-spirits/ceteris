# 0134. Sandboxed mechanism modules

Status: proposed
Date: 2026-10-28
Design sections: 4.5, 23.3; ADR 0097
Phase: 2.x (with the module ABI)

## Context

The module ABI of 23.3 lets third-party crates register systems, but native code can break
determinism or invariants in ways no review catches. Brainstorm (idea 53).

## Options

1. Native modules with review.
2. Modules compiled to WebAssembly and run in a sandbox that forbids nondeterminism.

## Decision

Option 2.

- The host exposes only state components the module declares, the keyed RNG and logging. No
  clocks, threads, filesystem or network. Floating point uses the deterministic subset (no
  relaxed SIMD, canonical NaNs).
- A module must pass the V0 invariants and its own declared nesting test before it loads.
- Modules run in the extended profile only, as 23.3 already requires for mechanism mods.

## Migration

- 2.x, alongside the module ABI. The determinism lints of ADR 0097 apply to the host interface.

## Tests

- A module that reads a clock or spawns a thread fails to load.
- The same module gives identical state hashes on macOS, Linux and Windows.

## Consequences

Mechanism mods become safe to share. Sandboxed code runs slower than native, so heavy modules
may need a native path later.
