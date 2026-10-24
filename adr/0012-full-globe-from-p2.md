# 0012. Simulate the full globe from P2

Status: accepted (owner, 2026-10-23)
Date: 2026-10-23
Design sections: 21 (development slice), 21.1 (P2, P4), 22.1 (Map scope), 8.3, 19.2

## Context

Section 21 develops P1 to P3 on a European slice of about 600 provinces, the rest of the world
reduced to trade gateways, and switches to the full globe in P4. P1 already built the full
4,796-province pack, the transport graph and the map. The P2 economic core is the first phase
whose systems run per tick.

## Options

1. European slice in P2 and P3, as designed; full globe in P4.
2. Full globe in P2.
3. Slice for development, full globe required at the P2 gate.

## Decision

Option 2 (owner decision). P2 runs on all 4,796 provinces with 2000 data for every country
(ICIO economies directly, the rest imputed, ADR 0014). The `dev_slice` flag stays in the pack
for ad hoc fast runs but no gate depends on it.

## Consequences

- The two-level trade kernel (ADR 0013) and the 19.2 budgets are exercised at full scale from
  the first economic tick, so performance problems surface in P2, not P4.
- The P2 data pipeline must cover every country, including imputation for economies outside
  ICIO, earlier than planned.
- CI cannot hold the pack; full-globe acceptance runs locally, as in P1, while CI runs the
  generated small-world golden.
- Section 21's development-slice paragraph and 22.1's map-scope row change at the next
  design-doc revision.
