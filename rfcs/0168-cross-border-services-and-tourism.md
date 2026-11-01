# 0168. Cross-border services trade and tourism

Status: proposed
Date: 2026-10-28
Design sections: 7.1, 8.1, 8.3, 9.5, 15.4, 15.5; ADRs 0081, 0100, 0111
Phase: P5

## Context

Services are local goods: they trade between provinces and countries only through demand
frictions that were never specified (8.1, assumption audit T8). ADR 0111 raised services trade
and deferred it "until goods grow toward 45". Services are about a fifth of world trade, and the
current accounts of Spain, Greece, Thailand, the Caribbean, Ireland, Luxembourg and India depend
on them. The COVID 2020 case of 15.5 is largely a collapse of travel. Services trade follows
gravity with larger border effects than goods (Anderson, Borchert, Mattoo and Yotov 2018).
Gap audit (2026-10-28): https://claude.ai/artifact/HRDQVbjJz8MrcDuPphPyYs

## Options

1. Status quo: services are local.
2. Exogenous services balances as transfers, like ADR 0100.
3. A services gravity kernel between countries without gateways or shipments, plus tourism as
   spending by visitors on local services.

## Decision

Option 3.

- Traded services (finance, IT and business services, transport services): a country-pair
  gravity kernel without physical routes, with its own trade elasticity and border friction
  estimated by PPML on BaTiS (ADR 0022 direct estimation). Flows settle through the current
  account; nothing is shipped.
- Tourism: pops spend a share of income on travel abroad by a logit over destinations on
  price, distance and an amenity term fitted to 2000 arrivals. The spending buys the
  destination's local services at its local price, so it is an export of the destination.
  Seasonality comes from ADR 0081's factors.
- Border closures and travel advisories (13.3, ADR 0182's OxCGRT tape) are wedges on the
  tourism logit.

## Migration

- P5: a country-level services kernel in ceteris-trade; tourism demand in the household block.
- Pack: bilateral services 2005 nearest to 2000 (BaTiS), tourism receipts and arrivals from WDI
  (already in the manifest).

## Tests

- With the kernel and tourism off, results equal the status quo.
- PPML on simulated 2005-2010 services flows recovers the estimated elasticity and border
  friction within their bands.
- Replaying 2020 border closures reproduces the fall in travel receipts by country within
  tolerance.

## Consequences

Current accounts of services exporters and tourism economies become modelled rather than
residual. Register: 2 entries (services elasticity and border friction, both estimated).
