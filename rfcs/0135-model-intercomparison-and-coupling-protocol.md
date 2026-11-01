# 0135. Model intercomparison protocol and deterministic coupling

Status: proposed
Date: 2026-10-28
Design sections: 14.2, 15.4, 16; ADRs 0052, 0113
Phase: P9 (protocol), after 1.0 (coupling)

## Context

Climate modelling converged partly through CMIP: fixed experiments every model runs, with
outputs in a shared format. Trade and macro modelling has no equivalent. Separately, Ceteris
will not model every domain itself; climate, epidemic and energy-system models already exist.
Brainstorm: https://claude.ai/artifact/N9zi5kznSaqvjg9ztw4WMT (ideas 60 and 61).

## Options

1. Status quo.
2. Publish an intercomparison protocol and invite other models to run it.
3. Option 2 plus a coupling protocol for running Ceteris in step with outside models.

## Decision

Option 3.

- Intercomparison: a small set of protocol experiments (the 2018 tariffs, a 2008-style credit
  shock, a 30% oil price shock, a Suez closure), each with a data pack, metrics and an Arrow or
  Parquet output schema. CGE, DSGE and agent-based teams run them on their own models; Ceteris
  is one participant. Outputs are compared on the scorecard of ADR 0052.
- Coupling: an open standard in the spirit of the Functional Mock-up Interface. Each side
  declares exchanged variables, units and time step. Exchange order and rounding are fixed by
  the protocol, so a coupled run stays bit-reproducible when both sides are deterministic.

## Migration

- P9: protocol experiments and schema with the scorecard. After 1.0: coupling, starting with a
  one-way export of prices and flows.

## Tests

- The protocol experiments run end to end from their manifests through the CLI.
- A coupled run with a deterministic stub model reproduces its state hashes across two runs.

## Consequences

Ceteris can become a reference point for the field rather than one more model, and it can be
the economic layer of other simulations. Both depend on outreach as much as code.
