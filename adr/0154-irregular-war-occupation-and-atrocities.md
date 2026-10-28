# 0154. Irregular war, occupation, civilian harm and atrocities

Status: proposed
Date: 2026-10-28
Design sections: 11.5, 11.6, 23.6, 23.7; ADRs 0029, 0033, 0051, 0060, 0069, 0072, 0115, 0132, 0145
Phase: P7 (2.0-beta)

## Context

Civil conflict is the dominant form of organized violence since 2000 (11). 0069 gives armed groups
revenue and attack hazards, and 0115 lets control move by cell. Missing: how control and violence
interact inside a province, how occupation works, how civilian harm is counted, and whether
atrocities and banned weapons exist as actions.

## Options

Atrocities and banned weapons: AI-side events with costs; player and AI orders with full
consequences; left out entirely.

## Decision

Player and AI orders with full consequences (owner, 2026-10-28).

- **Zones of control.** Per cell, control is a share held by the state, insurgents and criminal
  groups (0069, 0115). Selective violence concentrates where control is contested but leaning,
  indiscriminate violence where it is weak (Kalyvas 2006).
- **Counterinsurgency.** Force density per population, posture (population-centric or
  enemy-centric), local recruitment, development spending and governance set the trajectory. State
  civilian harm raises insurgent recruitment (Condra and Shapiro 2012). Ambushes and raids use the
  Deitchman asymmetric form (0149 switch).
- **Proxies and contractors.** Sponsors send arms, money and advisers through arms transfers with
  23.6 exposure rules; private military companies as in 0069.
- **Occupation** writes control labels, never ownership (0115). The occupier taxes in proportion to
  control share, pays administration and garrisons, and faces resistance hazards rising with
  repression, exclusion and duration.
- **Civilian harm.** Direct: fires and strikes in populated cells by density, munition, precision,
  shelter and urban share. Indirect: excess mortality from health-system collapse (0033), famine
  (0029), disease and displacement, produced by those modules rather than a fixed multiplier.
- **Atrocities and banned weapons** (chemical and biological weapons, deliberate attacks on
  civilians, forced deportation) are orders in the action space, open to player and AI alike. Each
  carries: legal exposure (0072 courts and referrals), sanctions and alliance hazards, legitimacy
  loss at home and abroad, identity salience and recruitment against the perpetrator (0058, 0069),
  and its deaths on the cost screen (0132). The AI chooses them only when its estimated payoff
  exceeds those costs; the 2013 Ghouta attack and the ISIS campaigns are calibration cases for
  rates and responses.
- **War ledger.** Losses by side, civilians killed directly and indirectly, displaced people,
  capital destroyed, years to recover the pre-war output path, and debt taken on, paired against
  the AI baseline as in 0132.

## Migration

- P7 stage W2 (zones of control, occupation, ledger) and W5 (atrocity orders with planner support),
  in `ceteris-war` (irregular).
- Data: UCDP GED (CC BY 4.0) per cell-month; EPR/GeoEPR (accepted NC licence).

## Tests

- Simulated violence per cell-month matches UCDP GED patterns on the selection window.
- A mineral price boom raises conflict in mining cells with the sign and rough size of Berman et al.
  (2017) (0069).
- An atrocity order applies every listed consequence; none applies without it.

## Consequences

The period's wars are representable in full, including their worst acts, with their legal,
political and human costs explicit. Presentation stays plain: the chronicle and cost screen report
what happened and who did it, without spectacle.
