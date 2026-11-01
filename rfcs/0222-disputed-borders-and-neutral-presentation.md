# 0222. Disputed borders and neutral presentation

Status: proposed
Date: 2026-10-28
Design sections: 2.4, 5.2, 22 (sensitive real-world content), 22.1 (Naming), 23.3; ADRs 0006, 0143, 0158, 0219
Phase: P3 (map and pedia entries), R (review before release)

## Context

ADR 0006 keeps Natural Earth's de facto assignment for every area not listed in
`borders_2000.toml`, which covers Kashmir, Western Sahara, Taiwan's status, Abkhazia, South
Ossetia, Transnistria, Northern Cyprus and others. The choice is reasonable but invisible to
users, and Natural Earth also ships point-of-view layers per country. DESIGN 22's risk table
promises neutral modelling and no editorial text, with no rule behind it. 1.0 holds borders fixed
(ADR 0136), so this is a presentation question. Audit gap 15.

## Options

1. Say nothing; the map shows the data.
2. State the rule, mark disputed areas, and explain each in the pedia.
3. Ship worldview switching.

## Decision

Option 2.

- **Rule.** Areas are assigned by 2000 de facto control as Natural Earth records it, except the
  overrides listed in `borders_2000.toml` with their reasons (ADR 0006). The rule is stated in the
  pedia and the model description (ADR 0219).
- **Map.** Disputed areas draw a dashed border and a hatch in the political mode; the inspector
  names the administering state and every claimant.
- **Pedia.** One entry per disputed area: claimants, the 2000 situation, the data source, and why
  the model assigns it as it does. De facto entities counted among the roughly 195 states are listed
  with their recognition status.
- **Text.** User-facing text follows the style note of ADR 0219: descriptive, sourced, no
  evaluative adjectives; groups are named as their source data names them, with the source shown.
- **Worldviews.** Not in 1.x. A data mod may replace the borders table once the mod manifest lands
  (1.4, DESIGN 23.9).

## Migration

- P3: disputed-area layer from Natural Earth's disputed-areas data, inspector field, pedia entries.
- R: a review pass of all pedia and map text against the style note.

## Tests

- Every area in Natural Earth's disputed-areas layer has a pedia entry and the dashed style.

## Consequences

Users see that a contested assignment is a modelling choice with a source. The project still takes
one position per area, and some users will object to it.
