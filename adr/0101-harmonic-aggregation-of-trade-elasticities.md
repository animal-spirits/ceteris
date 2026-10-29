# 0101. Harmonic aggregation of trade elasticities

Status: proposed
Date: 2026-10-26
Design sections: 8.3, 15.5, 17.1 (P01), 18 (H2); amends ADRs 0018, 0023

## Context

The three theta classes (2.62, 5.19, 10.57) are trade-weighted arithmetic means of
Caliendo-Parro values over the HS lines mapped to each class. Gains from trade and the
response of trade shares to a cost change run through 1/theta (Arkolakis, Costinot and
Rodriguez-Clare 2012), so the class value that preserves those aggregates is closer to the
harmonic mean. Mixing large and small values makes the two diverge: in good 6, office
equipment (about 13) sits with motor vehicles (about 1.8), and the harmonic mean is roughly 3
against the arithmetic 5.19. Class 2 at 10.57 is above the range most of the literature
reports for broad aggregates (about 4 to 6; Head and Mayer 2014, Simonovska and Waugh 2014).

Direction: gains from trade are understated and trade diversion after tariffs is overstated,
the latter being the headline output of P3 policy experiments.

## Options

1. Keep arithmetic means.
2. Trade-weighted harmonic means per class.
3. Re-cluster goods into classes by theta, not by sector.
4. Estimate class theta directly by PPML (0022), which is planned.

## Decision

Option 2 now, option 4 when 0022 runs. Option 3 is what ADR 0023's rule already does; this ADR
changes only the mean inside it, as an amendment to 0023 (owner, 2026-10-26). Owner decision D3
(ADR 0092) chose the Caliendo-Parro source, not the mean type, and stands.

- theta_class = (sum_l w_l / theta_l)^-1, w_l the 2000 BACI trade weight of HS line l in the
  class.
- A lane over theta scale {0.6, 1, 1.4} (applied to all classes) is reported with every tariff
  experiment until 0022 estimates replace the literature values.
- The P01 prior widens to cover the harmonic values.
- Order: the harmonic values and the 0023 class rule are computed after ADR 0092 moves refined
  petroleum (C19) into good 1; ADR 0092's nu_c fit then runs at the new class values.

## Migration

- Pipeline: the class-theta step in the pack builder switches mean type; record both means in
  the pack metadata.
- The stability region in the trade SPEC is recomputed at the new values (lower theta
  enlarges it).

## Tests

- On a two-line toy class, the welfare change from a uniform cost shock at the harmonic class
  theta equals the line-level sum to first order.
- H2 distance elasticity gate (-1.1 to -0.7) still passes, or the change is reported to the
  owner.

## Consequences

- Trade responses to tariffs fall in the classes with mixed values; welfare gains from trade
  rise.
- The distance elasticity at fixed P04 changes; 0092's nu fit absorbs part of it.

## As built (2026-10-29)

Built with ADR 0092's C19 move, on the Caliendo-Parro replication vector, in which Auto and Other
Transport are 8.22 (owner ruling; ADR 0023, Amendment 2026-10-29). The rule draws four classes:
2.91, 4.14, 7.68 and 16.91. The P01 prior is 2.91 to 16.91. The theta scale is P150
(`TradeParams.theta_scale`).

The welfare test above cannot fail: under a uniform shock, theta cancels from each line's first-order
welfare change. It is replaced by a non-uniform shock that lowers every line's domestic share by
the same log amount. The class-level change at the class theta then equals the line-level sum only
at the harmonic mean (ADR 0023, Amendment). The response of the trade share to a uniform tariff
runs through the arithmetic mean instead, so the Context's claim about trade diversion holds for
welfare read from observed shares but not for that response.
