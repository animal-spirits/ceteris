# 0119. Multiverse results and built-in preregistration

Status: proposed
Date: 2026-10-28
Design sections: 14.5, 15.6, 17.2; ADR 0053
Phase: P3 (preregistration), P9 (multiverse)

## Context

A counterfactual effect depends on structural switches (17.2) as much as on seeds, but results
are reported for one model variant. DESIGN 21.1 puts preregistration in P9 without saying how a
reader verifies it. Brainstorm (ideas 5 and
8).

## Options

1. Status quo.
2. Report every experiment's effect across the switch set as a specification curve, and give
   manifests a verifiable registration step.

## Decision

Option 2.

- Multiverse: an experiment can declare `multiverse: [switches]`; the CLI runs the full
  factorial (or a fractional design above 64 variants) and outputs a specification curve
  (Simonsohn, Simmons and Nelson 2020): the effect and interval per variant, sorted, with the
  switches marked. The summary reads "effect positive in n of m variants".
- Preregistration: `ceteris preregister` hashes an experiment manifest (branches, metrics,
  seeds, switches, analysis) and records the hash in a signed git tag before any run. Result
  files carry the hash; `ceteris verify` checks that the tag predates the first run of that
  manifest in the run cache.

## Migration

- P3: preregister and verify in the CLI. P9: multiverse once switches are populated.

## Tests

- Changing any field of a registered manifest changes its hash, and verify rejects results from
  the changed manifest.
- A multiverse run over one switch equals two separate runs, bit for bit.

## Consequences

Results report their dependence on modelling choices, and preregistration becomes a checkable
property, not a claim.
