# 0035. Technology diffusion and energy policy instruments

Status: proposed
Date: 2026-10-24
Design sections: 7.4, 7.5, 7.7, 13.2, 17.1 (P32), 17.2
Phase: P8

## Context

Section 7.5 uses Wright's law for learning curves. Learning sets costs but not how fast new
capacity is adopted: with cost-minimizing investment, technologies switch almost instantly at
cost parity, which no historical transition shows. Energy-economy models have also persistently
underestimated renewable deployment and overestimated its costs (Way, Ives, Mealy and Farmer
2022), so the learning process needs its uncertainty, not just its mean. FTT models (Mercure
2012; Mercure et al. 2018) describe adoption with an empirically grounded replicator dynamic:
new capacity shares shift by pairwise comparisons of levelized costs, at a rate limited by the
existing share of each technology (industry capacity, supply chains, skills). Carbon pricing
and the EU carbon border adjustment mechanism (CBAM definitive phase from January 2026) need to
act on these choices.

## Options

1. Cost-minimizing choice at investment with Wright's-law costs (status quo).
2. FTT-style replicator dynamics on shares of new capacity, with stochastic experience curves.
3. Calibrated logit choice without share dependence.

## Decision

Option 2, applied to power generation classes (0034), road vehicles and industrial heat, the
three areas where transition counterfactuals are asked.

- New-capacity shares evolve by pairwise logistic substitution on levelized cost differences,
  scaled by existing shares and a technology timescale (P68). The quarterly investment step
  allocates capacity by these shares.
- Experience curves are stochastic, following Way et al.: each doubling of cumulative output
  lowers cost by the learning rate (P32) plus noise (P69), drawn per run on the shock tape, so
  ensembles carry technology uncertainty.
- Policy instruments act on levelized costs and allowed choices: carbon prices and emissions
  trading caps (EU ETS from 2005), CBAM on embodied emissions of imports from the input-output
  flows (7.7), fuel taxes, subsidies, efficiency standards and phase-out mandates.
- Switch S16: cost-minimizing or FTT.

## Migration

- P8: diffusion system in ceteris-econ, a replicator update for about ten classes per state per
  quarter.
- Data: historical capacity by technology (Global Energy Monitor, IRENA statistics; licences to
  audit), Way et al. learning-rate estimates, EU ETS prices.

## Tests

- With S16 cost-minimizing, golden hashes equal the status quo.
- Replay of 2000-2024 solar and wind capacity growth by region: FTT reproduces the S-curve
  timing within tolerance where cost-minimization switches too early.
- The noise process reproduces Way et al.'s forecast error distribution on held-out years.

## Consequences

Transition timing becomes realistic and uncertain in the right way. H12 (learning curves make
clean energy dominate new capacity) becomes testable with honest uncertainty.
