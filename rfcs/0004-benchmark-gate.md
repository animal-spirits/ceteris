# 0004. Benchmark regression gate

Status: accepted
Date: 2026-10-23
Design sections: 19.4 (Tooling), 21.2 (Performance gates)

## Context

Section 21.2 requires per-system benchmarks in CI, with regressions over 20%
blocking merges. Hosted CI runners are shared virtual machines whose timings vary
by more than 20% between runs, and the budgets in 19.2 are for Apple Silicon.

## Options

1. Gate on hosted runners.
2. CI compiles the benchmarks on every push; the 20% gate runs locally or on a
   dedicated Apple Silicon runner against a saved criterion baseline.

## Decision

Option 2 until a dedicated runner exists:

```
cargo bench -p ceteris-core -- --save-baseline main     # on main
cargo bench -p ceteris-core -- --baseline main          # on a branch
```

## Consequences

CI catches broken benchmarks but not slow ones. Registering a self-hosted macOS
runner turns on the gate without code changes.
