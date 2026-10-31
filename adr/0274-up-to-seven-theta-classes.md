# 0274. Up to seven theta classes

Status: accepted
Date: 2026-10-31
Design sections: 8.3, 17.1 (P01, P05), 18 (H2); amends ADRs 0018 and 0023

## Context

ADR 0023 splits the sorted traded thetas into the fewest classes, 3 to 5, of consecutive goods
within a factor 1.5, and says that if 5 classes cannot meet the bound the rule fails and the owner
decides. With the 26 traded goods of ADRs 0271 and 0273, the thetas (BACI 2000 weights through the
OECD key, as `theta.by_good` computes them) run from 0.39 to 64.85 and need 7 classes:

| Class | Goods | theta |
|---|---|---|
| 0 | C301, C302T309 | 0.39 |
| 1 | C28, C22, C29 | 1.67 |
| 2 | C23, C10T12, C20, C21, C24A, C24B | 3.01 |
| 3 | C31T33, C25 | 5.65 |
| 4 | C26, C13T15, A01, A02, A03 | 7.73 |
| 5 | C16, C27, B05, B06, B07, B08, C17_18 | 13.72 |
| 6 | C19 | 64.85 |

Spread 2.0 gives 6 classes and 2.5 gives 5.

## Options

1. Allow 3 to 7 classes, spread 1.5.
2. Keep 3 to 5 classes, spread 2.5.
3. One class per good.

## Decision

Option 1 (owner, 2026-10-31). `CLASS_COUNTS` becomes 3 to 7; the spread, the tie rules and the
class value (trade-weighted mean) are unchanged. nu (P05) is fitted per class as in ADR 0092, so
seven values; H2 tests each class against its prior band (ADR 0023).

Class 0's estimate (Other Transport, 0.39, s.e. 1.08) is not distinct from zero, so its band
includes negative values and cannot fail. Its PPML estimate is reported and not gated on; the
Caliendo-Parro table stays as printed.

## Consequences

- Kernels are already per good (ADR 0023), so the class count changes parameter count, nu fits
  and H2 tests, not dispatch cost.
- Widening the spread would have put Machinery (1.45) with Food (2.62) and Paper (16.52) with
  Mining (13.53), the averaging ADR 0271 removes.
- At theta 64.85 the sourcing weight floor (1e-100) drops origins priced above 35 times the best.
- The classes above are the expected draw; the pack build recomputes them by the rule.
