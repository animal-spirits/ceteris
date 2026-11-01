# 0047. Trade agreements and international organizations

Status: proposed
Date: 2026-10-24
Design sections: 8.5, 10.3, 11.1, 9.4
Phase: P6

## Context

Over 25-year horizons, trade agreements are a first-order shifter of trade costs: Baier and
Bergstrand (2007) find that an FTA roughly doubles members' bilateral trade after ten years,
and their formation is predictable from economic size, distance and similarity (Baier and
Bergstrand 2004). Agreement depth varies widely (World Bank Deep Trade Agreements database). In
the design, agreements exist as treaties but their formation is left to the generic planner.
International organizations act as constraints: WTO dispute settlement has lacked a functioning
Appellate Body since December 2019; EU accession requires convergence to the acquis; UN
Security Council permanent members can veto sanctions coalitions.

## Options

1. Status quo: the generic planner proposes treaties.
2. An FTA formation hazard estimated on observed formations (economic determinants plus
   political alignment), with depth by provisions; explicit procedural rules for WTO, EU
   accession and UNSC.
3. A full negotiation game per agreement.

## Decision

Option 2.

- FTA formation: a dyadic hazard with Baier-Bergstrand determinants plus alignment and existing
  memberships (P88), estimated on DESTA and Deep Trade Agreements data; the planner can
  accelerate or block it. Agreements lower tariffs on a schedule and non-tariff frictions by
  depth.
- WTO: MFN tariffs bind; disputes resolve by panel unless appealed into the void after 2019,
  which the state can choose.
- EU: accession as a multi-year process with acquis convergence of institutions and regulation;
  membership transfers trade policy (10.3).
- UNSC: veto holders can block multilateral sanctions; unilateral sanctions remain available.

## Migration

- P6, in ceteris-polity and ceteris-trade (agreement wedges).
- Data: World Bank DTA (already in the manifest, wb-dta), DESTA (licence to audit).

## Tests

- In-sample: FTA formations 2000-2010 by hazard score; out of sample: 2011-2024 formations
  scored by AUC against a size-and-distance baseline.
- Simulated trade creation of a new FTA follows the Baier-Bergstrand phase-in.

## Consequences

The trade-policy landscape evolves endogenously and plausibly, so long-run trade
counterfactuals do not freeze 2000 agreements.
