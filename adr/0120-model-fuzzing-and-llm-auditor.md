# 0120. Fuzzing the model against stylized facts

Status: proposed
Date: 2026-10-28
Design sections: 4.6, 15.3, 15.6
Phase: P3 (harness), P9 (stylized-fact library)

## Context

Invariants (4.6) catch accounting bugs. Nothing searches for runs that are consistent but
economically absurd: gravity coefficients drifting out of range, price explosions without a
shock, city sizes leaving Zipf's law. Brainstorm:
https://claude.ai/artifact/N9zi5kznSaqvjg9ztw4WMT (ideas 7 and 21).

## Options

1. Status quo: the validation ladder (15.3) on fixed scenarios.
2. Property-based search over seeds, shock tapes and parameter draws for violations of a
stylized-fact library.
3. Option 2 plus an LLM auditor outside the binary that proposes scenarios likely to break the
model.

## Decision

Option 3.

- Stylized-fact library: each fact is a predicate on run output with a tolerance and a source
  (for example the distance elasticity of trade in a published range, no price above k times
  its 2000 level without a logged shock).
- Fuzzer: draws seeds, shock tapes and parameters from their priors, runs short horizons, and
  shrinks failing cases to a minimal manifest. Failures become regression scenarios in tests.
- LLM auditor: a script outside the repository's binaries reads the specs, proposes scenario
  manifests, runs them through the CLI and files findings with the manifest hash. It never
  changes code or parameters.

## Migration

- P3: fuzzer harness on the CLI. P9: the full fact library alongside the scorecard.

## Tests

- A planted bug that doubles one sector's trade cost after 2010 is found by the fuzzer within a
  fixed budget.
- Every shrunk failure reproduces from its manifest.

## Consequences

The model gets tested against economics as well as accounting. Fact tolerances are new
judgement calls and are listed with their sources.
