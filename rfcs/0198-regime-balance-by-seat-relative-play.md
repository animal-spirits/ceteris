# 0198. Regime balance by seat-relative play

Status: proposed
Date: 2026-10-28
Design sections: 10.6, 10.8, 23.4; ADRs 0130, 0140, 0161, 0164, 0192, 0193, 0194, 0195, 0199
Phase: P6 (2.0-alpha)

## Context

The owner asked whether dictatorships will be strictly easier and more fun than democracies.
Paradox games never remove the player from power, so they balance regimes with flat modifiers
(Stellaris ethics, EU4 reforms), and autocracy usually wins on convenience (HOI4). Ceteris has
mandate loss (23.4), so the real costs of autocracy can stand in for modifiers: falsified support
(0192, 0193), the selectorate's private goods (10.6), loyalty over competence (0195), coups
(10.8), succession. The risk is that these costs arrive late and unseen, so autocracy feels
smoother even when it scores no better. Analysis:
https://claude.ai/artifact/EHruH8W4Trtq7YD6N2DhqX

## Options

1. Keep 23.4 as written.
2. Flat regime modifiers tuned for parity.
3. Balance from per-seat scoring, legible costs, symmetric fail states and an audit, with no
   parameter tuned for fun.

## Decision

Option 3. Option 2 is rejected: it is the silent tuning DESIGN 17 forbids.

- **Score stays per seat.** Objectives and ambitions (0161) are judged against the AI baseline of
  the same seat under CRN (23.4), so regime class never changes the score scale. Objective
  presets per regime class: "survive and hand over power" for autocrats, "re-election plus one
  reform" for democrats, the private ledger (0164) for either.
- **Fail states after a coup** (owner, 2026-10-28). The player continues as the successor regime
  by default (the opt-in of 23.4 becomes the default). Ironman scenarios may set "run ends". A beaten democrat
  keeps opposition play as in 23.4.
- **Phase order.** The democrat's negotiating verbs (whip count 0164, coalition seat 0130, bills
  0195) ship no later than the autocrat's levers in 0199.
- **Backsliding** by bill (0195) carries its legitimacy (0192), sanctions (11.1) and credibility
  (0162) costs through existing mechanisms; nothing is added to make it cheaper or dearer.
- **Regime balance audit**, run before each release and summarized in the CHANGELOG; it never
  gates a release and does not run in CI. Per regime class of 0140, over
  an ensemble: score dispersion of the AI baseline and of simple scripted players, mandate-loss
  rate by year, actions refused per game year, and leader exit hazards against Archigos. A
  degenerate class (one dominant strategy, near-zero refusals, near-certain early loss) files a
  mechanism gap; no parameter moves to fix it.

## Tests

- Changing a seat's objective preset changes no state hash.
- A coup with the default fail state continues the run with the successor's seat; the state hash
  matches a run where the coup happened with no player seat.
- The audit report reproduces bit for bit from its seed list.

## Consequences

Regimes differ in how they are hard rather than how hard they are: autocracy is easy to act and
hard to know, democracy the reverse. Cost about 2-3 part-time weeks after P6, mostly the audit.
