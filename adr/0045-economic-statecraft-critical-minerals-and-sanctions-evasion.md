# 0045. Economic statecraft: critical minerals, export controls and sanctions evasion

Status: proposed
Date: 2026-10-24
Design sections: 8.5, 9.12, 11.8, 7.1
Phase: P5 (trade side), P6 (state decisions)

## Context

Since 2018, trade policy has turned into statecraft: export controls on semiconductors and
their inputs, Chinese controls on gallium, germanium and graphite (2023), and the 2022
sanctions on Russia with large rerouting through third countries. Supply of many transition
minerals is highly concentrated by country. Clayton, Maggiori and Schreger (2023) model
geoeconomic power as leverage over the input-output dependencies of other states. The design
has sanctions and export controls as wedges (8.5) and route choice (0017), which already
reroutes around them, but no evasion cost, no mineral detail and no measure of leverage.

## Options

1. Status quo.
2. Critical minerals as sub-goods of mining with concentration data; export controls by good
   and destination; sanctions evasion as an indirect-routing cost on third-country gateway
   links; a leverage index computed from the input-output network.
3. A separate geoeconomic game.

## Decision

Option 2.

- Critical minerals (lithium, cobalt, nickel, rare earths, gallium, germanium, graphite) as
  sub-goods of non-energy mining with production by country from USGS data, without adding ICIO
  sectors: their flows are shares of the mining good.
- Export controls and sanctions as wedges by good, origin and destination; evasion through
  third countries costs an extra wedge on transshipment links (P83), so rerouting is partial,
  as observed after 2022.
- Leverage index per state pair: the output loss in the target from cutting the controlled
  flows, computed from the input-output matrices; it feeds the state planner's diplomacy scores
  (12.3) in P6.

## Migration

- P5: mineral shares and transshipment wedges in ceteris-trade.
- P6: leverage in the planner.
- Data: USGS Mineral Commodity Summaries (public domain, partly in the manifest as usgs-ds140);
  Global Sanctions Database (Felbermayr et al.; licence to audit).

## Tests

- Replay of the 2022 sanctions: direct trade falls and third-country trade rises with the
  pattern in post-2022 data (H10).
- With P83 zero, sanctions are fully evaded through third countries in a toy world; with it
  large, not at all.

## Consequences

Sanctions, export controls and supply-chain security become quantitative experiments, and P6
diplomacy gets an economic measure of power.
