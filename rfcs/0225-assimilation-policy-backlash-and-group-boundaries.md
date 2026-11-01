# 0225. Assimilation policy, backlash and group boundaries

Status: proposed
Date: 2026-10-28
Design sections: 6.1, 6.4, 10.1, 10.6, 10.7; ADRs 0055, 0058, 0131, 0189, 0223, 0224
Phase: P6 (2.0-alpha)

## Context

States push language and religion through schooling, official status, citizenship, autonomy and
bans. Effects are not always the intended sign: Fouka (2020) finds US state bans on German-language
schooling after 1917 raised German identification among affected children; Bazzi et al. (2019)
find Indonesian transmigration raised national-language use and intermarriage at moderate
diversity; Laitin (1998) models post-Soviet Russian speakers' language choice as a tipping game.
Self-identification also moves: Posner (2005), Eifert, Miguel and Posner (2010) on identification
near elections, Nagel (1995) on census ethnic renewal, Ukraine 2001-2022. The pop key fixes the
EPR group list at 2000.

## Options

1. Fixed EPR list with re-identification flows between existing groups.
2. Option 1, and new groups may form during a run.
3. Fully fixed membership.

## Decision

Option 2 (owner, 2026-10-28).

- **Levers.** Medium of instruction, official language status and media quotas, citizenship and
  naturalization law, regional autonomy and mother-tongue rights, bans on language or religious
  practice, and forced assimilation (internment, child removal), each a policy variable feeding
  0223 and 0224 terms.
- **Backlash.** A policy that targets a group's language or religion raises that group's salience
  (0058), which raises transmission (k_f in 0223) and retention (0224). Weight prior from Fouka
  (2020); a lane sets it to zero.
- **Re-identification.** Slow yearly flows of people between existing groups' pops in the same
  province, driven by salience gaps, status gaps (EPR status, income) and intermarriage. Children
  of mixed marriages are assigned by a homogamy-weighted rule.
- **New groups.** A subgroup of a pop may split into a new group when its salience, language or
  religion diverge from the parent group beyond thresholds for a sustained period, or merge back
  when they converge. New groups inherit the parent's EPR status, name from the province or
  language, and are recorded in the chronicle. No calibration data exist for formation within
  2000-2100, so this stays off by default until it passes the ablation test (15.7); ADR 0131 deep
  history is where it gets data.
- **Parameters.** About eight register entries: backlash weight, re-identification rate and
  slopes, formation and merge thresholds and duration.

## Migration

- P6, in the population crate, after 0223 and 0224.
- Validation: Ukraine identification surveys 2001-2023, census ethnic-response changes where
  IPUMS has repeated questions, Fouka's estimate as the backlash test case.

## Tests

- With all levers at 2000 values, no backlash and no re-identification, results equal 0223 and
  0224 alone.
- Group population totals are conserved across re-identification, splits and merges (proptest).
- A language ban with backlash on raises heritage transmission in the targeted group.

## Consequences

Assimilation is a policy with costs and possible reversal, and the group list can change in long
runs. Pop count can grow when groups form, bounded by the thresholds; the pop budget (6.1) is
checked in the ablation run.
