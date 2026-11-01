# 0155. Economic lenses

Status: proposed
Date: 2026-10-28
Design sections: 9.2, 9.11, 9.13, 20.1, 20.2, 23.8; ADRs 0017, 0020, 0029, 0096, 0118, 0124
Phase: P3 (route inspector, map numbers, edits by query), P3-P5 (bottleneck lens, peer radar), P5 (headroom gauges)

## Context

The Victoria 3 comparison named legibility as the largest risk to Ceteris. The model already
computes several things the UI does not show: which input rationed a firm this tick (0020), which
origin-destination pairs use each transport edge (0017), and the thresholds of its own regime
switches (9.2, 9.11, 9.13, 0029). Other games turn exactly these into views: Kerbal Space
Program's delta-v readout, Factorio's production statistics and alt-mode, Cities: Skylines' route
view on a road segment, Football Manager's percentile Data Hub, and Europa Universalis IV's macro
builder. Survey of other games' mechanics (2026-10-28): https://claude.ai/artifact/LcDMgZuczpSefSVeadiZWT

## Options

1. Leave these to the explainer (20.2) and alerts (23.8).
2. Add dedicated views over quantities the model already computes, plus one editor convenience.

## Decision

Option 2. None of the views writes state.

- Headroom gauges: per state, the distance to each regime switch the model contains (months of
  reserves to the peg-break region of 9.2, debt and spread to the crisis region of 9.13, bank
  capital to the run threshold of 9.11, food stocks to the famine rule of 0029), with the burn
  rate from ADR 0096 tangents. Where the switch is a hazard rather than a threshold, the gauge
  shows the hazard curve, never a cliff.
- Bottleneck lens: a map mode for cells where putty-clay rationing (0020) binds this tick, and a
  panel that walks the shortfall upstream through IO coefficients and trade shares to the origin
  market. Every series gains a window toggle (month, year, decade, run).
- Route inspector: selecting a transport edge, port or chokepoint lists the origin-destination
  pairs and goods routed through it, from the route-choice shares of 0017. A "close this edge"
  action forks a branch with that edit.
- Peer radar: a state's headline series as percentiles against a user-chosen peer set (income
  group, region, commodity exporters, currency regime), with the AI-baseline branch as a ghost
  outline. Peer sets are saved queries.
- Map numbers: holding a key prints the active map mode's value on each province.
- Edits by query: select provinces by a predicate ("port provinces with a food deficit") and apply
  one edit to all. The predicate is stored with the edit and re-evaluated on replay at the edit's
  tick, so the edit list stays the only causal record (14.2).

## Tests

- No view changes a state hash.
- Route inspector shares for an edge sum to the edge's flow from 0017 within rounding.
- An edit by query replays to the same cell set on another machine.

## Consequences

The workbench explains itself at the level a player or reader asks questions. Cost about 8-12
part-time weeks in total; the gauges wait for P5 thresholds, the rest can ship with P3.
