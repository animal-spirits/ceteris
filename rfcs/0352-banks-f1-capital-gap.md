# RFC 0352. Banks F1: five archetypes, and the capital gap that feeds the zombie band

Status: accepted
Date: 2026-11-13
Design sections: 9.11, 9.3, 7.4
Release: 1.0 (P5 Branch F, F1; unblocks #535)
Research: none; follows ADR 0172, ADR 0202 and `specs/p5/PLAN.md` D2 and F1

## Summary

`entry_exit::step` takes a zombie share from `z_b`, the bank capital gap of ADR 0172, and `z_b` has
no source because no bank balance sheet exists (#535). F1 builds the bank sector. This RFC fixes
its structure, how `z_b` is defined and where 2000 balance sheets come from. It covers F1 only:
the monthly fixed-point loop, the four contagion channels, deposit insurance and the safety net
(later F1 slices) keep their own wiring issues under ADR 0353. Banks are off by default and with
them off no state is added and goldens do not move (ADR 0354).

## Decisions

### D1. What bank structure does F1 build first?

Options:

- A. One aggregate bank per state: smallest slice, archetypes later.
- B. All five archetypes of 9.11 per state from the start (global systemic, domestic universal,
  regional savings and cooperative, state-owned, nonbank lenders).
- C. `z_b` as an exogenous input with no balance sheet.

Recommendation: A, because it is the smallest slice that gives `z_b` a source.

Decision: B, Antonio, 2026-11-13 (card "What bank structure does F1 build first for the capital gap?").
Row: ADR 0367

### D2. How is `z_b` defined?

Options:

- A. Shortfall of the capital ratio below the minimum over the minimum, clipped.
- B. Logistic in the capital buffer.
- C. The share of loans to insolvent borrowers that a bank rolls over because recognising the
  loss would push its capital below the minimum (ADR 0172).

Recommendation: A, because it needs no per-loan state.

Decision: C, Antonio, 2026-11-13 (card "How is z_b defined from the bank capital ratio?").
Row: ADR 0368

### D3. Where do the 2000 bank balance sheets come from?

Options:

- A. World Bank GFDD and BIS credit, with a labelled global median for the rest.
- B. One calibrated constant per region group.
- C. National financial accounts only (OECD, ECB, Fed Z.1) and BIS banking statistics as 9.11
  already names them; a state without them has banks off and `z_b = 0`.

Recommendation: A, because open series cover most states in 2000.

Decision: C, Antonio, 2026-11-13 (card "Where do 2000 bank capital ratios and loan books come from?").
Row: ADR 0369

## Spec changes

- 9.11: F1 builds all five archetypes per state with banks off by default (ADR 0367); `z_b` is
  the share of loans to insolvent borrowers a bank rolls over because recognition would breach
  the minimum (ADR 0368); 2000 balance sheets come from national financial accounts, and a
  state without them has banks off (ADR 0369).

## Consequences

- Wiring #535 needs F1 landed first; its forbearance switch-on test then uses `z_b` from a toy
  undercapitalised bank (ADR 0202 test).
- D2 needs per-borrower loan state: the bank step must know which loans are to insolvent
  enterprises, so F1 carries a loan book keyed by enterprise cell, not an aggregate.
- D3 leaves states without national accounts bankless in 1.0 and zombie-free. Coverage is
  smaller than the open-series option; the implementation issue lists the states covered.
- Licences of the accounts and banking statistics are checked in `sources.toml` (ADRs 0005, 0050)
  in the implementation issue, before any is packed.
- Archetype shares and balance-sheet imputation inside a state are an implementation matter
  under 9.11 and the register.
