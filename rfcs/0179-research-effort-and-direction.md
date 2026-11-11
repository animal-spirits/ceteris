# 0179. Research effort and its direction

Status: proposed
Date: 2026-10-28
Design sections: 7.5, 15.4; ADRs 0116-0135, 0136
Phase: country R&D as a 1.x minor after P8; direction as part of the 3.0 technology major

## Context

Technology progresses with "world research effort" (7.5), which is never defined. R&D is about
2.5% of world GDP, concentrated in a dozen countries, and its direction responds to prices:
energy prices shifted patenting toward energy-saving technology (Popp 2002), and fuel prices
shifted car-industry patenting from dirty to clean engines (Aghion et al. 2016). Knowledge
spillovers decay with distance and language. Directed technical change belongs in the frozen
technology block that ADR 0136 makes the 3.0 major.
Gap audit (2026-10-28)

## Options

1. Status quo: undefined world effort.
2. Country R&D investment as a minor, with direction exogenous.
3. Country R&D investment with direction chosen by expected profit, as one module.

## Decision

Option 2 now, and option 3 as the lab default at the 3.0 major.

- 1.x minor: enterprises and governments invest in R&D per country, enterprises on expected
  profit and governments by a budget lever. Effort raises the TFP growth of the sectors it
  targets, with spillovers that decay by distance and language. Data: OECD MSTI and UNESCO UIS
  for R&D, PatentsView and REGPAT for spillover fits (ADR 0182).
- 3.0 major: R&D is allocated across sectors and technology classes by expected profit, which
  makes the direction of technical change respond to prices and policy. This joins
  capabilities and emerging sectors in the 3.0 technology major (0136).

## Migration

- After P8: an R&D stock per country and sector; spillover kernel.
- 3.0: direction choice per enterprise.

## Tests

- With R&D frozen at its 2000 shares, golden hashes are unchanged.
- Spillover elasticities fitted to patent citations by distance within the published range.
- At 3.0: a carbon-price shock shifts R&D toward clean technology with the sign and order of
  magnitude of Aghion et al. 2016.

## Consequences

Long-run growth gains its main driver. This is the only gap-audit item that is a major. The
cost is about 2-3 months. Register: 3-5 entries.
