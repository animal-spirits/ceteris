# 0073. Proliferation, the Arctic, frozen conflicts and intelligence

Status: proposed
Date: 2026-10-24
Design sections: 11.1, 11.2, 11.4, 5.3, 13.2, roadmap P7 (dossiers)
Phase: P7, P8 (Arctic)

## Context

Several geopolitical processes of 2000-2100 sit between existing modules. Nuclear proliferation
responds to threat, alliance credibility and technical latency, and can cascade after a
neighbour crosses the threshold (Jo and Gartzke 2007). Arctic sea ice decline opens shipping
routes and resource claims. Frozen conflicts (Transnistria, Nagorno-Karabakh before 2020,
Donbas 2014-2022) are a distinct ceasefire state with low-level violence and sudden thaws.
Intelligence collection and election interference act through information.

## Options

1. Status quo: nuclear status fixed, no Arctic route change, ceasefires as peace.
2. A proliferation decision with latency; Arctic routes on the transport graph opened by
   emulated sea ice; frozen conflict as an explicit state; intelligence as dossier accuracy and
   interference as a small information shock.

## Decision

Option 2.

- Proliferation: states choose to pursue weapons when threat is high and alliance credibility
  low, subject to technical latency (enrichment capacity) and sanctions cost (P125); a
  neighbour's acquisition raises others' threat.
- Arctic: Northern Sea Route and Northwest Passage edges whose seasonal availability and cost
  follow sea-ice extent from the climate emulator (0037); claims from ICOW seed disputes.
- Frozen conflict: a ceasefire state with a low-level violence hazard and a thaw hazard rising
  with shifts in relative power.
- Intelligence: collection spending raises the accuracy of dossiers about a rival; interference
  shifts the target's information environment by small, ensembled amounts (consistent with
  0071).

## Migration

- P7 (proliferation, frozen conflicts, intelligence), P8 (Arctic routes).
- Data: proliferation latency datasets (terms to audit), NSIDC sea-ice records (public domain).

## Tests

- Arctic route usage rises with simulated ice loss in the expected season; no proliferation
  cascade occurs in ensembles under strong alliance credibility.

## Consequences

Long-run security futures carry their structural drivers rather than fixed nuclear and
geographic facts.
