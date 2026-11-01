# 0096. Forward-mode derivatives in the Rust core

Status: proposed; amended per ADR 0113 (2026-10-29)
Date: 2026-10-24
Design sections: 15.2, 15.7, 17, 19.4

Numbering: proposed as 0023; renumbered because p2-econ took 0023 for trade elasticity classes.

Timing: P2's kernels shipped in f64 on p2-econ without this ADR, so adopting it now means
retrofitting those kernels.

## Context

Section 15.2 planned gradient-based tuning on JAX relaxations of the price, demand and trade
kernels: a second implementation of the core, in Python, that would drift from the first.
Gradient-based calibration of agent-based models is now practical: Hamid et al. (2025)
calibrate a differentiable supply-chain ABM by variational inference and reach in 300 model
evaluations a loss that approximate Bayesian computation had not reached in 30,000. After ADRs
0015 to 0021 the per-tick economic kernels are smooth and closed form: the gravity
normalization, route-choice solves, PIGL demand, and the price and wage rules.

## Options

(1) Reimplement the kernels in JAX. (2) Reverse-mode automatic differentiation in Rust with
Enzyme. (3) Forward-mode dual numbers through a float trait. (4) Finite differences under
common random numbers.

## Decision

Option 3, with option 4 as its test. Calibration needs derivatives with respect to about 10
parameters (the top Sobol indices), where forward mode is cheap and needs no tape. Reverse mode
over a 300-tick run with 100,000 pops would store the whole trajectory, and Enzyme needs a
nightly LLVM plugin against the pinned stable toolchain. Option 1 duplicates the core.

## Migration

- Add a Real trait in ceteris-core (arithmetic, mul_add, exp, ln, powf, smooth max and min),
  implemented by f64 and by `Dual<N>`: a value plus N tangents, N up to 16, stored contiguously
  for SIMD.
- Write the P2 economic kernels and the route-choice solve generic over Real. Release builds
  instantiate f64 only, so derivatives cost nothing outside calibration.
- Give non-smooth operations smooth versions in calibration builds only: the price-step clip
  becomes a soft clip, and priority rationing a softmax ordering with a temperature. Discrete
  events (default, entry, exit) are held at their primal outcome, and gradients are reported as
  conditional on that event path.
- Ledger balances stay fixed-point i64 without tangents; tangents live on prices, quantities
  and parameters.
- Gradients of summary statistics reach Python through the same Arrow boundary as other
  outputs.

## Tests

- The f64 instantiation reproduces the non-generic golden hashes bit for bit.
- Dual gradients match central finite differences under common random numbers to 1e-6 relative
  on the P2 golden scenario.
- A criterion benchmark of `Dual<10>` against f64 sets the calibration-build budget in the P2
  spec.

## Consequences

Variational inference and gradient-based MCMC become available in P9 for the smooth blocks
without emulators; JAX stays only in the GE reference model. Calibration builds differ from
release builds in the soft clips and rationing, so each posterior is checked by re-running its
mean in release mode.

## Amendment (ADR 0113, 2026-10-29)

The JAX relaxations are gone: the GE reference model is the `ceteris-reference` crate and no JAX
model remains, so "JAX stays only in the GE reference model" no longer holds. The finite-difference
test of the dual numbers runs against `ceteris-reference`. Gradients of summary statistics reach
analysis through the Arrow and Parquet outputs, not Python.
