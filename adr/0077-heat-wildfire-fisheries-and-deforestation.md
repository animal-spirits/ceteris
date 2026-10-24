# 0077. Heat, wildfire, fisheries and deforestation frontiers

Status: proposed
Date: 2026-10-24
Design sections: 13.2, 13.4, 7.1, 6.2, ADR 0037, ADR 0038
Phase: P8

## Context

Four environmental channels with good data complete the climate block. Heat mortality and
labour losses are larger in cities because of the urban heat island, and rise with heatwave
frequency (Gasparrini et al. 2017). Burned area in the western United States roughly doubled
because of warming (Abatzoglou and Williams 2016), with smoke adding to pollution mortality.
Fish stocks respond to harvest and warming, which shifts stocks poleward (Free et al. 2019) and
fuels disputes over illegal fishing. Deforestation follows road building and enforcement:
Brazil's Amazon clearing fell sharply in 2004-2012 with enforcement and rose again later
(Assuncao, Gandour and Rocha 2015), and the Amazon dieback tipping element (0037) should
respond to clearing as well as temperature.

## Options

1. Status quo: heat in damages only, no fires, fisheries or frontier dynamics.
2. An urban heat premium and heatwave mortality; a wildfire hazard; fish biomass per sea zone;
   deforestation driven by roads, prices and enforcement and coupled to the Amazon tipping
   element.

## Decision

Option 2.

- Heat: heatwave days per province from MESMER extremes; mortality and labour losses rise
  faster in the city class (0074) (P133, from published exposure-response curves); adaptation
  (air conditioning) rises with income and raises power demand.
- Wildfire: a hazard per province rising with temperature and dryness and fuel load, destroying
  capital and adding smoke to PM2.5 (0038).
- Fisheries: logistic biomass per sea zone with harvest effort, warming-driven range shifts and
  quotas (P131); catches enter food supply; illegal fishing raises maritime tension.
- Deforestation: land conversion at the forest frontier rises with new roads (5.5),
  agricultural prices and weak enforcement, and falls with enforcement policy (P132);
  cumulative clearing raises the Amazon dieback hazard.

## Migration

- P8.
- Data: FAO fisheries statistics (CC BY 4.0, to add), Global Forest Watch tree-cover loss (CC
  BY 4.0), MODIS burned area (public domain).

## Tests

- Simulated Amazon clearing 2004-2012 falls with the enforcement policy and rises after its
  weakening.
- Simulated global capture fisheries stay near the observed plateau 2000-2020.

## Consequences

Climate damages arrive through the channels people experience, and land and ocean policy become
levers.
