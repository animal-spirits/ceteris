# 0273. Mining support services are a local good

Status: accepted
Date: 2026-10-31
Design sections: 7.1, 8.3, 8.6; amends ADR 0271

## Context

ADR 0271 makes the 50 ICIO 2025 industries the goods and calls A01 to C31T33 traded (27),
including B09, mining support services. A traded good needs HS lines: its weight w_g is BACI
tonnes over BACI value, and its theta is the BACI-weighted mean of the Caliendo-Parro estimates
over its lines (ADR 0023). The OECD HS to CPA key gives B09 no HS line, so both are 0/0.

ICIO 2000 (SML, 81 economies): B09 output 103 bn USD, VA 50 bn, exports 6.8% of output, against
50% for B06 and 13% for G. The work is done at the field.

## Options

1. B09 local.
2. B09 merged into B06, 49 goods (B06 keeps today's B06 plus B09 column).
3. B09 traded with a weight and theta borrowed from B06.

## Decision

Option 1 (owner, 2026-10-31). 26 traded goods (A01 to C31T33 without B09) and 24 local (B09, D,
E, F, G to T). Traded goods stay first in the goods order, so B09 takes id 26; the order is
otherwise ICIO's. B09 is in the industry group for WDI shares outside ICIO (ADR 0249) and in the
PIGL category S (ADR 0021). Its province VA is the population base point of the local goods;
calibration stage 3 then places its output where local demand is, which B06's input column puts
in oil provinces.

## Consequences

- B09's exports (7 bn USD in 2000) are dropped, as are all services exports (P2 convention).
- ADR 0256's world column and the rents rule of ADR 0249 apply to B06 alone.
- The theta classes are drawn over 26 goods (ADR 0274).
