# 0083. Policy implementation and diffusion

Status: proposed
Date: 2026-10-24
Design sections: 10.1, 10.6, 12.3, 14.2, ADR 0067
Phase: P6 (P5 for fiscal programmes under the policy tape)

## Context

A law in the design takes effect when passed. In practice, policy is announced, implemented
over months to years at a speed set by administrative capacity, complied with partially, and
enforced unevenly. The same law yields different outcomes in Denmark and in a low-capacity
state, which is what state capacity (0067) is for. Policies also spread between states by
learning, emulation and competition: inflation targeting, conditional cash transfers, carbon
pricing and central bank independence diffused through peer networks (Simmons, Dobbin and
Garrett 2006; Gilardi 2010).

## Options

1. Status quo: instant, full implementation; planners choose from fixed menus.
2. An implementation pipeline (announcement, phase-in at a capacity-dependent rate, compliance
   from trust and enforcement), and a diffusion term that raises the attractiveness of policies
   adopted by a state's peers.

## Decision

Option 2.

- Every policy change has an implementation level from 0 to 1 that rises at a rate set by
  administrative capacity and the policy's complexity (P136); effective policy is the announced
  policy times implementation times compliance, with compliance from trust (0056) and
  enforcement.
- Announcements can move expectations before implementation (central bank and fiscal
  announcements).
- Diffusion: the planner's score for a candidate policy rises with the share of peer states
  that have adopted it, weighted by neighbourhood, trade, alliance and ideological ties (P137).
  Validation targets: the timing of adoption waves for inflation targeting, conditional cash
  transfers and carbon pricing.

## Migration

- P6 (P5 for fiscal programmes).

## Tests

- With implementation instant and diffusion off, results equal the status quo.
- Simulated adoption curves of inflation targeting 1990-2020 (with the 1995 pack) and carbon
  pricing 2000-2024 follow the observed S-shapes.

## Consequences

State capacity matters for every lever, and policy waves emerge rather than being scripted.
