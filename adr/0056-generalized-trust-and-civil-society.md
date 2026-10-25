# 0056. Generalized trust and civil society

Status: proposed
Date: 2026-10-24
Design sections: 9.6, 10.7, 10.8, 13.3
Phase: P6 (P4 for the pandemic compliance link)

## Context

Generalized trust conditions tax compliance, cooperation, contract enforcement, pandemic
compliance and the legitimacy channel of 10.7, and its cross-country ordering is highly
persistent. Civil society density lowers the cost of coordinating protest and raises the share
of campaigns that stay nonviolent. The design has neither as a state: compliance in 9.6 is a
state parameter, and protest thresholds (10.8) do not depend on organizational capacity.

## Options

1. Status quo.
2. A trust stock per (province, ethnic group) and a civil society stock per state, each with
   slow dynamics and explicit uses.

## Decision

Option 2.

- Trust falls with crime victimization (0060), corruption exposure (0046), ethnic polarization
  and inequality, and rebuilds slowly (P94). It scales tax compliance (9.6), pandemic
  compliance (13.3) and the weight of performance in approval (10.7).
- Civil society density, initialized from V-Dem's civil society indices, lowers coordination
  costs in the protest threshold model and raises the probability that a campaign is nonviolent
  (P95); repression and closing civic space lower it.

## Migration

- P6, with the pandemic link in P4.
- Data: V-Dem civil society indices (CC BY-SA 4.0); trust levels from WVS and regional
  barometers for calibration only (non-commercial terms; Afrobarometer geocoded data carry a
  non-commercial pledge).

## Tests

- With trust and civil society held constant, results equal the status quo.
- The cross-national ordering of trust is persistent over 2000-2024 in simulation, as in the
  data.

## Consequences

Fiscal capacity, pandemic response and protest dynamics share a social foundation that can
erode and recover.
