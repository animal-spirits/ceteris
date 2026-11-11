# 0181. Small endogenizations: invoicing, piracy, housing regulation, pandemic spillover

Status: proposed
Date: 2026-10-28
Design sections: 8.8, 8.3, 6.4, 13.3; ADRs 0040, 0051, 0067, 0077
Phase: as listed

## Context

Four inputs are fixed that could be outcomes. Invoicing currencies are fixed shares (8.8), but
firms choose them, and dollar dominance follows from that choice (Gopinath and Stein 2021). The
piracy attribute of sea lanes is static, although Somali piracy rose and fell with state failure
and naval patrols. ADR 0040's housing regulation index is fixed, but incumbent homeowners vote
for restrictions (Fischel 2001). Pandemic arrivals come from a fixed historical rate (Marani et
al. 2021 via 0051), though spillover risk rises with deforestation, livestock density and the
wildlife trade.
Gap audit (2026-10-28)

## Options

1. Keep all four fixed.
2. Endogenize each as its own module on its own schedule.

## Decision

Option 2.

- Invoicing choice (1.x, after P5): exporters choose the invoicing currency by strategic
  complementarity in pricing and the currency's use in funding, after Gopinath and Stein.
  Data: Boz et al. invoicing shares (ADR 0182).
- Piracy hazard (after 2.0): the hazard on a lane rises with state fragility of the coast
  (0067) and falls with naval patrols. Data: NGA ASAM incidents.
- Homevoter regulation (after P6): the regulation index moves with the homeowner share of the
  electorate and house-price growth.
- Pandemic spillover (extended minor): the arrival rate scales with forest loss and livestock
  density near forest edges, using forest loss from 0077.

## Migration

- Each module ships separately on the schedule above; each is off by default until it passes
  the ablation rule.

## Tests

- Each off leaves golden hashes unchanged.
- Invoicing: the dollar share of invoicing by country 2000-2015 within the published band.
- Piracy: Gulf of Aden incidents 2005-2015 rise and fall in sequence with fragility and patrols.

## Consequences

Small gains in realism; each is cheap. Register: 1 entry per module at most.
