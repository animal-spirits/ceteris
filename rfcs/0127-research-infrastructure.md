# 0127. Research infrastructure: living papers, parameter wiki, volunteer ensembles, model court

Status: proposed
Date: 2026-10-28
Design sections: 15.6, 15.7, 17; ADRs 0052, 0119
Phase: P9

## Context

Published results from a model that keeps changing go stale silently. Parameters have
provenance in the register (17) but no public, citable home. Ensembles and sweeps are limited
by one machine. Disputes between schools of modelling have no shared test. Brainstorm:
https://claude.ai/artifact/N9zi5kznSaqvjg9ztw4WMT (ideas 25 to 28).

## Options

1. Status quo.
2. Four pieces of research infrastructure, each optional and independent.

## Decision

Option 2.

- Living papers: published work registers its manifest in a `papers/` index. A scheduled job
  reruns all of them on each release and reports results that moved, with a diff. Authors keep
  a badge while results hold.
- Parameter wiki: the degrees-of-freedom register rendered as a static site, one page per
  parameter: estimates with sources, a small meta-analysis, the runs most sensitive to it (from
  Sobol indices), and its change history. Generated from the repository; no server.
- Volunteer ensembles: a client that runs lanes for a published experiment and returns state
  hashes and outputs. Each lane is computed twice on different machines; results count only
  when hashes agree.
- Model court: rival modules behind one structural switch, with tests agreed and preregistered
  (ADR 0119) before either side sees results; the scorecard (ADR 0052) is published for both.

## Migration

- P9: living papers and the parameter wiki with the scorecard. Volunteer ensembles and model
  court after 1.0.

## Tests

- A planted parameter change that moves a registered result is flagged by the living-papers
  job.
- A volunteer result with a mismatched hash is rejected.

## Consequences

Results stay honest across releases. Volunteer ensembles need a coordinating service, which
conflicts with the no-cloud non-goal unless a volunteer hosts it; that is the open question.
