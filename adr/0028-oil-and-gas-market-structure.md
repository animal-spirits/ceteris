# 0028. Oil and gas market structure

Status: proposed
Date: 2026-10-24
Design sections: 7.1, 7.7, 8.2, 8.4, 15.4, 17.1, 17.2
Phase: P2 (crude oil), P8 (natural gas and LNG)

## Context

Oil price shocks are among the best-identified macro shocks of 2000-2024 (2008, 2014-16, 2020,
2022), and their transmission is a core validation target. In the design, crude is one more
good priced by the province stock-flow rule, with no producer market power, no spare capacity
and no speculative storage. The size of the short-run supply elasticity is contested:
Baumeister and Hamilton (2019, AER) estimate about 0.15 and find supply shocks important, while
Kilian and coauthors argue it is close to zero and demand shocks dominate. Natural gas is
regional, constrained by pipelines and liquefaction, and drove the 2021-22 European energy
shock.

## Options

1. Status quo: crude and gas as ordinary goods.
2. A cartel producer: OPEC members' output is set by a rule that targets a price band and holds
   spare capacity; others are price takers with a short-run supply elasticity. Competitive
   storage holds inventories.
3. A full game-theoretic oil market with strategic producers.

## Decision

Option 2.

- OPEC (membership a state attribute, from 2000 data, changeable by law) sets quarterly
  production by a reaction rule on the price relative to a target band and on OECD commercial
  inventories, subject to capacity. Spare capacity is capacity minus production and is
  reported. Saudi spare capacity and the 2014 and 2020 price wars are the test cases.
- Non-OPEC supply responds to price with a short-run elasticity (P54, prior covering 0 to 0.2,
  deliberately wide given the dispute) and a long-run response through investment and depletion
  (7.4, 13.4).
- Crude trades on the gravity kernel like other goods, so the world price emerges with regional
  basis differentials from transport costs and sanctions (Urals discount after 2022 as a test).
- Inventories follow the competitive storage rule of ADR 0091 (which took over 0029's storage
  part), shared with food and metals.
- Natural gas (P8) clears in regional markets linked by pipelines and LNG shipping with
  capacity constraints; liquefaction and regasification capacity are capital stocks. Built
  pipelines arrive with the pipeline mode (5.3).
- P55 holds the OPEC rule's coefficients. Switch S13: competitive supply or the cartel rule.

## Migration

- P2: crude oil (an ICIO split good) gains the OPEC rule and storage; no new crates.
- P8: gas markets, LNG capacity, pipelines.
- Data: OPEC membership and quotas from public OPEC reports; capacity from Global Energy
  Monitor oil and gas trackers (already in the manifest); EIA international data (public
  domain) for production and inventories; Baumeister's structural shock series as validation
  (licence to audit, ADR 0050).

## Tests

- With S13 competitive and no storage, results equal the status quo.
- A toy supply disruption is smaller in price impact when spare capacity is high, with the
  elasticity implied by P54.
- Historical shock tape: simulated oil price path 2000-2010 within the calibration bands;
  Baumeister-Hamilton shock decomposition used as a pattern target, not a fit.

## Consequences

Oil shocks get a structural source and a buffer. The contested supply elasticity is a free
parameter with a wide prior rather than a hidden assumption, and its posterior is reported.
