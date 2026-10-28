# 0199. Loyalty belief state and coup-proofing

Status: proposed
Date: 2026-10-28
Design sections: 10.1, 10.6, 10.8, 11.4, 23.5; ADRs 0065, 0153, 0164, 0185, 0186, 0187, 0192, 0195, 0198
Phase: P6 (2.0-alpha)

## Context

A democrat sees a whip count (0164) and honest polls (0193); an autocrat sees falsified support
(0192) and nothing on the people who can depose them. DESIGN 10.8 calibrates coup frequencies but
gives the ruler no verbs against coups, so a coup reads as dice. About two thirds of dictators who
lost power irregularly were removed by regime insiders (Svolik 2012). Analysis:
https://claude.ai/artifact/EHruH8W4Trtq7YD6N2DhqX

## Options

1. Keep coups as a hazard with no ruler-side view or levers.
2. A domestic belief state over loyalty and a small set of coup-proofing levers, both on existing
   bodies and characters.

## Decision

Option 2, extended profile until it passes the ablation test (15.7).

- **Loyalty belief state.** The domestic form of 0153: the ruler holds a belief over each member
  of the selectorate body and security command (characters, 0185) and over true approval. It is
  shown as a dossier view (0187) with intervals, the autocrat's analogue of the whip count.
- **Internal intelligence** is a budget line that narrows those intervals. It raises repression
  (10.7), and so falsification (0192), and builds a security service whose own coup capacity
  enters the defection calculus.
- **Coup-proofing levers**, as settings on bodies and appointment policies (0195): parallel
  security forces (counterbalancing), rotation of commanders, ethnic or family stacking, and pay.
  Each lowers coup hazard and costs military effectiveness, officer competence or budget, after
  Quinlivan (1999) and Talmadge (2015). Coefficients enter the X-register (23.1) with priors.
- The AI uses the same levers (12.6), so the baseline of 0198 faces the same trade-off.

## Tests

- With all levers at their defaults and intelligence spending zero, coup hazards match the lab
  model of 10.8 (equilibrium nesting).
- The belief view never changes a state hash; only lever and budget edits do.
- Coup frequencies by regime class stay within the coup-dataset bands of 10.8 calibration.

## Consequences

Autocratic survival becomes a game of information and trade-offs, not a hazard rate, and its
costs show before they land. Cost about 4-6 part-time weeks after 0195 is built.
