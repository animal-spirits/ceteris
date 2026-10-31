# 0247. Bulk freight rate per theta class

Status: proposed (owner ruling 1B, 2026-10-30: add a bulk freight rate for class 3 inside pre-P3
B2, then refit nu); built, and on by default for class 3 by owner ruling (2026-10-30): criterion 3
of the ablation test was ill-posed (As built)
Date: 2026-10-30
Design sections: 5.3 (maritime container, bulk and tanker; freight rates), 5.4, 15.7 (ablation),
17.1 (P04; new P154, P155), 21.2 item 7; amends ADR 0022 (P04 mode costs); ADR 0092 (nu per
class); trade SPEC Leg factors, Parameters; P154 rescaled to the new P04c by ADR 0269

## Context

- Class 3 (goods 1 and 2: crude oil, gas and refined fuels; coal and other mining; theta 16.91)
  cannot reach its ICIO 2000 cross-border share at any border friction nu >= 0 on the 2000 pack:
  0.2007 at nu 0 against a target of 0.2964, so ADR 0092's fit has no root for it (pre-P3 B2).
- The diagnosis (`.superpowers/sdd/PLAN-pre-p3-rust/class3-diagnosis.md`) found the cause in leg
  cost times theta, not in a basis or data error. Every good is shipped at P04's general-cargo
  sea rate (0.008 USD per tonne-km) and port fees (8 USD/t at a listed port, 20 at a minor one).
  At class 3's weight per value, that makes crossings cost tau - 1 of about 0.8 (good 1) and 3
  (good 2). At theta 16.91, the border is shut to everything but net imports.
- In 2000, crude oil, coal and ores did not move at general-cargo rates. They moved in tankers
  and bulk carriers at rates per tonne an order of magnitude below the P04 sea prior. DESIGN
  5.3 lists maritime transport as "container, bulk, tanker", with freight rates as a cost
  driver; the code had only one sea rate.

## Options

1. Clamp class 3 at nu = 0 and record the miss (ADR 0092 amendment, no new mechanism).
2. Lower class 3's theta. Rejected: P01 is Caliendo and Parro under ADR 0101, and even class
   2's theta (7.68) leaves the nu-0 share at 0.274.
3. A bulk freight rate: per-class multipliers on the sea cost per tonne-km and on the port fee,
   sourced from 2000 tanker and dry-bulk rates.

## Decision

Option 3 (owner, 2026-10-30), with option 1 as the fallback if the ablation test fails. The
test's criterion 3 failed, but it was ill-posed (As built); by owner ruling (2026-10-30, "Do the
recommended") the rate is on by default: P154 0.026 and P155 0.40 for class 3, 1 for classes 0
to 2.

- **Mechanism.** Two per-class multipliers in `TradeParams`: `bulk_sea[c]` (P154) on the sea
  cost per tonne-km (P04c, sea and canal edges) and `bulk_port[c]` (P155) on the port handling fees
  (P04m, listed and minor ports). They are 1 for classes 0 to 2. Rail, road, air, the canal
  fee, the hours and P43 are unchanged.
- **Legs.** A class with multipliers other than 1 is routed on its own least-cost paths: the
  transport graph is repriced at the bulk rates and the same legs (same endpoints, same
  reachability) get their cost per tonne and hours from those paths (`Network.bulk`,
  `ceteris_econ::EconInputs::network_from_graph`). Everything downstream of the leg factor
  `tau = 1 + c w_g + lambda_g T` is unchanged. All classes that use the bulk rate share one pair
  of multipliers (only class 3 does).
- **Values (sourced, not fitted to the share target).** A least-squares line through the
  2000 voyage rates per tonne that UNCTAD's Review of Maritime Transport quotes for class-3
  cargoes, against the routes' sea-lane km on the pack's own graph (so the per-km slope prices
  the same km the model charges):

  ```
  rate_r = F + s * km_r,     bulk_sea = s / 0.008,     bulk_port = (F / 2) / 8
  ```

  F is the fixed part of a voyage rate, paid once per end (two port calls); 8 USD/t is P04m's
  listed-port fee. The routes, each the mean of the 2000 quotes the reports give:

  | Route (cargo, ship) | Source | 2000 quotes, USD/t | Mean | Pack sea km |
  |---|---|---|---|---|
  | Brazil (Tubarao) to Europe (Rotterdam), iron ore, Capesize | RMT 2001 para. 96 p. 54; RMT 2002 ch. VII p. 107 | 6.15, 7.95; 7.50, 8.70 | 7.575 | 10,273 |
  | Australia (Port Hedland) to Europe, iron ore, Capesize | RMT 2001 para. 96 p. 54 | 8.50, 10.70 | 9.60 | 19,450 (Cape) |
  | South Africa (Richards Bay) to Antwerp-Rotterdam-Amsterdam, coal, Capesize | RMT 2001 para. 96 p. 54 | 7.60, 10.10 | 8.85 | 14,125 |
  | East Coast North America (Hampton Roads) to ARA, coal, Capesize | RMT 2001 para. 96 p. 54 | 5.90, 7.70 | 6.80 | 7,097 |
  | Brazil (Tubarao) to China (Beilun), iron ore | RMT 2002 ch. VII p. 107 | 11.50, 13.50 | 12.50 | 21,087 |
  | Puerto Bolivar (Colombia) to Northern Europe, coal, Capesize | RMT 2002 ch. VII p. 108 | 7.70, 8.40 | 8.05 | 8,455 |
  | Curacao to Houston, dirty, 70,000-100,000 t tanker | RMT 2001 para. 82 p. 51 | 4.70, 13.20 | 8.95 | 3,840 |
  | Curacao to Houston, clean products, 25,000-35,000 dwt tanker | RMT 2002 ch. VII p. 107 | 7.50 (March) | 7.50 | 3,840 |

  Sources: UNCTAD, *Review of Maritime Transport 2001* (UNCTAD/RMT/2001), chapter IV, sections
  A(b) and B(b); UNCTAD, *Review of Maritime Transport 2002* (UNCTAD/RMT/2002), chapter VII,
  Latin America. Each route takes the 2000 quotes as given (start and end of the year, or the
  months named), averaged. Grain routes are left out (class 2). Every class-3 rate in USD per tonne
  for 2000 that the two reports give is in the table. Sea km: least-cost sea path between the
  ports nearest the named terminals, sea and canal edges only (P04 circuity 1), from the probe
  run with the canal fee at 0 (canals free) and again with it at 1e9 (canals barred). The two
  runs agree on every route but Port Hedland to Rotterdam: 19,136 km through Suez, 19,450 km
  round the Cape. A loaded Capesize could not transit Suez in 2000, so the quote is a Cape
  voyage and the table uses 19,450 km. No route in the table is then canal-routed, so no
  canal fee (P04o, 5 USD/t) is netted out of a quote; the bulk legs keep P04o unscaled.

  Fit (8 routes): F = 6.47 USD/t (s.e. 0.89), s = 2.05e-4 USD per tonne-km (s.e. 7.0e-5).
  **P154 bulk_sea = 0.026** (2.05e-4 / 0.008), **P155 bulk_port = 0.40** (3.23 / 8), for class
  3; 1 for classes 0 to 2. Sensitivity: the six dry-bulk routes alone give 0.039 and 0.29.
  With Port Hedland at its Suez km (the fit before the routing check, 2026-10-30): F = 6.45,
  s = 2.08e-4, the same 0.026 and 0.40, dry bulk alone 0.040 and 0.29.
  The quoted rates are port to port: what they include at the terminals (cargo handling) is not
  stated, so the bulk port fee is the rate's fixed part only.
- **Ablation test (DESIGN 15.7, falsification branch; owner criterion, 2026-10-30).** With no
  holdout scoring before P3, the rate is compared off (multipliers 1, the code before this ADR)
  against on:
  1. off, the class-3 nu fit has no root; on, it has one;
  2. H2's class-3 elasticity is inside its band (theta_3 within 2 pair-clustered s.e. of the
     class-3 PPML estimate) on, and outside it off;
  3. H2's distance elasticity stays inside -1.1 to -0.7 with the rate on.

  If 2 or 3 fails, the rate stays off by default (P154 and P155 at 1 for every class), class 3
  is clamped at nu = 0 and the miss is recorded in ADR 0092. Nothing is tuned to pass. (Owner
  ruling 2026-10-30: criterion 3 was ill-posed and the rate is on; see As built.)

## Consequences

- With the rate on, class 3's sea cost per tonne-km is 1/38 of P04c's and its port fees 0.40
  of P04m's, and the class can trade two-way, as ICIO says it does in 2000. This is the default
  (owner ruling 2026-10-30). With P154 and P155 at 1 for every class (off), nothing changes.
- A second shortest-path pass over all sources at load, on a repriced copy of the graph (pack
  load time; the kernel and the tick are unchanged).
- The export's legs table carries the bulk costs and hours (export format 12); the reference
  reads the kernel factors and needs no change.
- Pipelines, the other bulk mode for crude and gas, are still not built (DESIGN 5.3, P8).
- ADR 0017's route choice does not see the bulk rate; a network with route taus and bulk legs is
  rejected.
- A network built without a graph (the fixture, hand-built test networks) has no repriced paths.
  The fixture builds its bulk legs itself: its sea crossings at P154 times its sea cost per
  tonne-km; it has no ports, so P155 has nothing to scale. Hand-built test networks without bulk
  legs, and the fixture's unreachable-island variant (route taus), run with the rate off
  (`TradeParams::bulk_off`); the variant folds the bulk legs into its route taus for the goods of
  a bulk class, so its kernel is the default fixture's but for the cut.
- The gravity estimation's S05 costs between countries (`ceteris gravity`, ADR 0022;
  `ceteris_calibrate::costs::CostInputs::country_costs`) take the same bulk legs for the goods of
  a bulk class as the kernel (2026-10-30; they were base legs only before): the chains run on the
  graph repriced at the rate, and the path quantities carry the sea km and port fees scaled by
  P154 and P155, so the P04 fit prices them at the rate (`bulk_class_country_costs_match_the_kernel`).

## As built

Built on 2026-10-30 (pre-P3 B2, task b2-1b): `TransportParams::bulk`, `TradeParams::{bulk_sea,
bulk_port}` (P154, P155), `Network::bulk` (`BulkLegs`), built by `network_from_graph` and used by
the kernel per class; export format 12. Every class off 1 must share one rate
(`TradeParams::bulk_rate`). `BulkLegs` records the multipliers it was built with, and
`Kernel::build` refuses parameters whose P154 or P155 differ from them (review fix, 2026-10-30):
the network is built once at load, so a run cannot change the rate on its own.

**Ablation test** on the 2000 pack (B1 econ tables, gateway masses, home shares; scratch packs
`b2/pack-bulk` and `b2/pack-off`), each fitted (ADR 0092) and calibrated from cold:

| | Off (P154, P155 at 1) | On (0.026, 0.40 for class 3) |
|---|---|---|
| Class-3 share at nu 0 (target 0.2964) | 0.2007 | 0.3037 |
| Fitted nu, classes 0 to 3 | 2.6767, 0.8032, 0.4003, no root (clamped at 0) | 2.6767, 0.8032, 0.4003, 0.0078 |
| Fit evaluations | 17 | 18 |
| Calibration passes (cold) | 70 (final change 3.7e-13) | 64 (9.4e-13) |
| Fit and calibration, wall time | 712 s | 578 s |
| Shares at the calibrated steady state (coupled check) | 0.1545, 0.3108, 0.1428, 0.1963 | 0.1545, 0.3108, 0.1428, 0.2954 |
| H2 class 3: PPML theta (pair-clustered s.e.), band | 12.386 (1.350), 9.686 to 15.086 | 16.432 (0.370), 15.692 to 17.171 |
| H2 distance elasticity (s.e.), band -1.1 to -0.7 | -0.509 (0.014) | -0.463 (0.011) |

1. Off, no root; on, a root (nu 0.0078). **Pass.**
2. theta_3 16.91 is outside the class-3 band off and inside it on. **Pass.**
3. H2 distance is outside -1.1 to -0.7 with the rate on (-0.463), and the rate moves it further
   from the band than off (-0.509). **Fail.**

By the criterion as written, the rate would stay off by default, and it did from 2026-10-30
(task b2-1b) until the owner ruling below: P154 and P155 at 1 for every class, class 3 clamped at
nu = 0 (ADR 0092, Amendment 2026-10-30).

**Criterion 3 was ill-posed.** It assumed the distance elasticity was inside -1.1 to -0.7 before
the rate. It was not: -0.509 with the rate off, -0.463 with it on. The miss comes from the leg
costs and the masses (ADR 0092, As built), not from this rate, so the rate cannot be blamed for
it. Criteria 1 and 2 pass. The numbers above stand as measured.

**Owner ruling (2026-10-30, "Do the recommended"):** the rate is on by default. P154 is 0.026 and
P155 0.40 for class 3, 1 for classes 0 to 2 (`TradeParams::default`, pre-P3 task B2c). Class 3
has a root with the rate on (nu 0.0078 on the scratch pack of task b2-1b), so ADR 0092's clamp
no longer applies to it; the clamp stays the general rule. The H2 distance miss stays open as a
P2 gate question (B8). Status stays proposed.

H2 per class at the fitted nu, both runs: classes 0 (2.393, pair-clustered s.e. 0.086, band 2.221
to 2.566 against theta 2.91) and 1 (3.507, 0.257, 2.993 to 4.021 against 4.14) miss; class 2 (7.61,
0.19, 7.23 to 8.00 against 7.68) passes; they move by less than 0.003 between off and on.
