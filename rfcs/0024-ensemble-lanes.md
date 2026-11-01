# 0024. Ensemble lanes

Status: proposed
Date: 2026-10-24
Design sections: 4.5, 14.1, 14.4, 15.2, 19.4

Timing: P2's kernels shipped in f64 on p2-econ without this ADR, so adopting it now means
retrofitting those kernels.

## Context

Section 19.4 ran one single-threaded run per process. Ensembles (32 seeds by default),
common-random-number pairs and calibration batches are most of the compute, and every process
reads the same kernels and route-choice factors from memory again. Current agent-based
frameworks vectorize across runs instead; ABMax, for example, runs many similar models in
parallel with JAX's vmap.

## Options

(1) One process per run. (2) Lanes: an inner array dimension indexing ensemble members,
advanced in lockstep in one process. (3) GPU-resident ensembles.

## Decision

Option 2, with the lane count L a const generic: L = 1 for interactive play, 8 to 32 for
ensembles. Option 3 stays the Metal fallback for the trade products only, under the GPU rules
of 19.4.

## Migration

- State arrays gain lane as the innermost dimension, after goods. P2's entities are static, so
  every table is rectangular in lanes.
- RNG streams already key on the seed, so lane k uses seed k and no stream changes.
- Read-only data (pack, kernels, route-choice factors) is stored once; a lane whose edges
  differ after an intervention gets a copy-on-write factor.
- Trade products widen from (provinces x goods) to (provinces x goods x lanes), so gateway
  products become larger matrix-matrix products.
- From P4, lanes can diverge in their entity sets (enterprise entry, wars): entity tables carry
  per-lane existence masks, and work for absent entities is masked, not skipped.
- Lanes compose with 0096: tangents and lanes are both inner dimensions, and a calibration
  build uses lanes for parameter draws or tangents, not both at once.

## Tests

- Lane k of a vectorized run has the same state hashes as a scalar run with seed k at every
  snapshot.
- A common-random-number pair in two lanes reproduces the paired effects of two scalar runs.
- A benchmark of run-ticks per second against the process-per-run baseline.

## Consequences

Snapshots store lanes as separate branches. The throughput gain is measured in P2; it is
expected from memory traffic rather than arithmetic.
