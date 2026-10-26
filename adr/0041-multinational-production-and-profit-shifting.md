# 0041. Multinational production and profit shifting

Status: proposed
Date: 2026-10-24
Design sections: 7.4, 9.1, 9.6, 15.4
Phase: P5

## Context

Foreign ownership is already a cell dimension (3.2) and FDI is ordinary capital flow (7.4). But
where profits are booked differs from where production happens: Torslov, Wier and Zucman (2023,
REStud) estimate that about 36% of multinational profits were shifted to tax havens in 2015.
This inflates measured GDP and the corporate tax base of hubs (Ireland's 2015 GDP jump),
shrinks the base elsewhere, and distorts FDI positions in CDIS. Without it, the model cannot
match the national accounts and FDI data of the hubs, and minimum-tax policy (OECD Pillar Two,
from 2024) has nothing to act on.

## Options

1. Status quo: profits booked where produced.
2. A profit-shifting wedge: a share of foreign affiliates' profits is booked in haven states,
   responding to tax-rate differences; booked profits enter the haven's GDP, tax base and FDI
   income.
3. Firm-level transfer pricing.

## Decision

Option 2, as an accounting layer, on top of the foreign share of profits that ADR 0104 pays
abroad. Each foreign-owned cell shifts a share of profits to haven
states that rises with the tax-rate gap (semi-elasticity P76, from the profit-shifting
literature). Booked profits count in the haven's GDP and corporate tax revenue, and as FDI
income in the balance of payments; real production, employment and trade stay where they are.
Minimum taxes cap the gap.

## Migration

- P5: the wedge in ceteris-finance's income accounts.
- Data: Missing Profits of Nations estimates and OECD activities of multinational enterprises
  tables (licences to audit, ADR 0050); IMF CDIS (already in the manifest under imf-data).

## Tests

- With P76 zero, results equal the status quo.
- Simulated GDP-to-GNI gaps of Ireland and Luxembourg and their FDI positions within tolerance
  in the calibration window.

## Consequences

Hub economies' accounts become matchable, and international tax policy becomes a lever.
