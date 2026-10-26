# 0067. State capacity: building, eroding and collapsing

Status: proposed
Date: 2026-10-24
Design sections: 9.6, 10.1, 11.6, ADR 0042
Phase: P6

## Context

Fiscal capacity is built by investment and depreciates (9.6, Besley and Persson), but coercive
and administrative capacity are V-Dem indices fixed or drifting slowly, and nothing makes a
state fail. State collapse (Somalia, Libya after 2011, Yemen, the Sahel) is a defining
2000-2024 phenomenon: when capacity falls below a threshold, taxes and services stop, which
erodes legitimacy and capacity further. Measurement exists for the components (Hanson and
Sigman 2021).

## Options

1. Status quo: fiscal capacity only.
2. A capacity triple per state (fiscal, coercive, administrative), each built by investment and
   lost to depreciation, conflict, elite fragmentation and capture, with a collapse regime
   below thresholds.

## Decision

Option 2.

- Fiscal capacity stays as designed; coercive capacity (control of territory and violence) and
  administrative capacity (implementation, statistics, services) become stocks with investment
  and depreciation (P114).
- Below a threshold in coercive or administrative capacity, a province enters a
  failed-governance regime: tax collection and services stop, armed actors (0060, 0069) expand,
  statistics degrade (0065), and legitimacy falls, a feedback that can tip a state into
  collapse; recovery needs external support or a winning coalition that reinvests (P115).
- Administrative capacity scales the implementation of every policy (0083) and the efficiency
  of public investment (0042).

## Migration

- P6.
- Data: Hanson-Sigman capacity estimates (licence to audit), V-Dem (CC BY-SA 4.0), World Bank
  governance indicators (CC BY 4.0).

## Tests

- Collapse emerges in ensembles for states with the 2000 profiles of Somalia or Yemen at
  plausible frequencies, and not for high-capacity states without extreme shocks.

## Consequences

State failure becomes an emergent outcome rather than an absent one, and capacity building
becomes a policy with long payoffs.
