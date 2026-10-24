# Trade SPEC

Status: approved (owner, 2026-10-23)
Phase: P2
Design sections: 3.4, 4.4, 4.6, 5.4, 8.1, 8.2, 8.3, 8.4, 8.5, 17 (P01-P05, S01, S02, S05), 19.4, 21.1 (P2); ADRs 0012 and 0013

## Purpose

`ceteris-trade` owns province markets and the movement of goods between them: one price and
one inventory per (province, good), the stock-flow price rule, the two-level trade kernel built
from `ceteris-geo` path costs, merchant routing, the transit pipeline, and the payments that go
with shipments. It does not own production, labour or demand (`specs/econ/SPEC.md`); it sees
them only as quantities offered to and demanded from each market.

## Deviations from DESIGN.md (approved with the spec)

| Design | P2 | Why |
|---|---|---|
| Develop P1-P3 on a European slice (21) | Full globe, 4,796 provinces | Owner decision; ADR 0012 |
| Per-tick unbalanced Sinkhorn over both marginals (8.3) | Posted-price merchant logit: each origin routes its supply over destinations by a logit in log netback prices. Closed form on the two-level kernel, no per-tick iteration. Sinkhorn and Anderson acceleration stay in the GE reference and calibration | The same entropic transport family with the demand-side penalty at zero: flows keep the gravity form, steady state is the same equilibrium, and supply that is not profitable to ship stays home through the home route. Cost per tick falls from iterations to a few sparse products. ADR 0013 |
| Per-unit plus iceberg costs (5.4, A7) | Iceberg only (S05 = iceberg): tau = 1 + c w_g + lambda_g T at reference prices, the second term the time cost of transit (Hummels and Schaur 2013) standing in for 5.4's carrying cost; risk and per-unit composition effects deferred to P8 (freight) | Per-unit costs need a transport sector that owns fleets; iceberg nests exactly in the GE reference. Freight alone gives distance elasticities far below H2's range (owner decision: add the literature time cost, nothing fitted to H2) |
| Utilities, construction and services traded through demand frictions (7.1) | Not traded between provinces in P2 | Province-local markets; cross-province service frictions arrive with P8 |
| Shortage rationing by priority (8.2) | Pro rata across buyers | Priority needs subsistence levels, which arrive with P4 |
| Walrasian clearing mode (S01) | Deferred to P3 | V1 uses the stock-flow rule converging to the GE reference, which is the stronger test |

## State components

| Component | Entity | Type | Units | Initialized from |
|---|---|---|---|---|
| Price p | (province, good) | `f64` | numeraire per unit | 1.0 (2000 world reference price), then the calibrated state |
| Inventory S | (province, traded good) | `f64`, also held in the ledger | units | s_g x initial demand |
| Last demand D, last supply Q, last sales | (province, good) | `f64` | units per month | zero |
| Trade kernel legs | domestic pairs, province-gateway legs, gateway pairs, per traded good | `f32` K = tau^-theta, CSR | | path costs at load; rebuilt when the graph version moves |
| Transit lag | gateway pair | `u8` | months | gateway-pair hours |
| Transit wheel | (arrival tick slot, province, traded good) | `f64` | units | empty |
| Tariff and border wedge | (importer country, exporter country, traded good) | `f32` | ad valorem | border friction P05 plus 2000 tariffs (econ pack) |
| Market, gateway, transit accounts | ledger | money `i64`, goods `f64` | | opened at load |

A unit of good g is the quantity worth one 2000 US dollar at the world reference price. Goods
are innermost in every goods-indexed array (19.4); P2 has 10 goods, padded to 12 for `f64`
alignment where it helps vectorization.

## Equations

Symbols: o origin province, d destination province, a and b gateways, A and B countries,
g good, theta = P01, w_g weight per value (tonnes per unit, econ pack), c path cost per tonne
(`ceteris-geo`).

**Leg factors.** For a leg with least-cost path cost c (USD per tonne) and transit time T
(days, hours along the same path / 24):

```
tau_leg,g = 1 + c * w_g + lambda_g * T       (iceberg, S05 = iceberg; geo SPEC mapping at p = 1)
K_leg,g   = tau_leg,g ^ -theta
```

lambda_g is the ad valorem cost of a day in transit (P41). Inside a province trade is costless
(A2): K_oo = 1.

**Active destinations.** Market (d, g) is active if anyone at d can buy g: its country's final
demand share Omega_g > 0 or a local enterprise uses g as an input. Only active markets are
routing destinations and carry a price state. An inactive (province, traded good) cell with
output is origin-only: its producer price is last tick's realized average netback,
sum_d X_od p_d / tau_od / sum_d X_od (data state: 1), and it is left out of V1's price
comparison. The GE reference uses the same rule.

**Two-level kernel.** Domestic pairs (o and d in the same country) use the direct path:
K_od = tau_od^-theta, dense within each country. A cross-border pair is reached only through
gateways of the two countries:

```
K_od = sum over a in G(A), b in G(B) of  K_oa * K_ab * K_bd
K_ab = (tau_ab * (1 + nu_AB) * (1 + t_AB,g)) ^ -theta
```

nu is the non-tariff border friction (P05): an iceberg cost, destroyed at the crossing and
tallied as border loss. t is the applied tariff: a routing wedge only, no goods lost; its revenue
is paid in money (Payments, below). The sum over gateway pairs is
the entropic route choice of Allen and Arkolakis (2022): parallel routes add access. Gateways
belong to one country; a land crossing is two gateways, one on each side, joined by the crossing
edge.

**Routing (merchant logit).** Each tick the market at o routes this tick's supply Y_o,g (output
bought from local producers) over destinations by the expected log netback, with expected
prices equal to current posted prices (naive expectations, S04 adaptive-only in P2):

```
X_od = Y_o * K_od * p_d^theta / Pi_o,      Pi_o = sum_d K_od * p_d^theta
```

X is in origin units; the destination receives X_od / tau_route. The home route (d = o) is part
of the sum, so unprofitable supply stays home. With costs in log-iceberg units this is
structural gravity: X_od = a_o b_d tau_od^-theta with a_o = Y_o / Pi_o and b_d = p_d^theta.

**Low-rank evaluation.** International terms never materialize K_od:

```
v_b = sum_{d in B} K_bd p_d^theta          (destination access at gateway b)
u_a = sum_b K_ab v_b                        (international access at gateway a)
Pi_o = sum_{d in A} K_od p_d^theta + sum_a K_oa u_a
```

Flows split as a Markov chain: o sends Y_o K_oa u_a / Pi_o to a; a sends share K_ab v_b / u_a
of its arrivals to b; b sends share K_bd p_d^theta / v_b to d. The shares at a and b do not
depend on the origin, so aggregating at gateways is exact. Iceberg losses apply per leg.
Cost per good: O(sum_A n_A^2 + P k + G^2), with k gateways per country and G about 600.

**Payments.** The destination market pays the origin price per shipped (origin) unit, FOB, at
dispatch: domestic flows pay o directly; international value moves d to b to a to o along the
same split, so each gateway account nets to zero every tick. Tariff revenue t_AB,g times the
FOB value crossing a to b is paid by the destination market to the importer's rebate account
(`specs/econ/SPEC.md`, income).

**Transit.** Domestic shipments arrive at tick t + 1. International shipments arrive at
t + 1 + lag_ab, lag_ab = round(hours_ab / 730), capped at 3. Goods in transit sit in a transit
account per good; the iceberg loss is destroyed at dispatch (tallied as transport).

**Province market, traded good.** At tick t, supply Q = home-routed supply plus arrivals.
Buyers post notional demand D at the posted price: households for consumption and enterprises
for next tick's inputs (econ SPEC). Sales =
min(D, S + Q); a shortfall is rationed pro rata. S' = S + Q - sales. Then (8.2):

```
dlnp = clip( kappa1 (D - Q)/Q  -  kappa2 (S' - S*)/S*,  -c_max, c_max ),   S* = s_g D
```

**Province market, local good** (utilities, construction, services): Q = local output; unsold
output perishes (destroyed, tallied as spoilage); s_g = 0, so only the first term applies.

Q or S* of zero (a province with no supply of g) uses the denominator max(Q, Q_floor) with
Q_floor = 1e-9 of the world mean of Q for that good.

## System order

| System ID | Name | Cadence | Reads | Writes | Position in tick pipeline (4.4) |
|---|---|---|---|---|---|
| T1 | deliver | Monthly | transit wheel | market inventories, wheel | before production |
| T2 | route | Monthly | supply Y, prices, kernel | shipments, wheel, payments, tariffs | inter-province trade |
| T3 | clear | Monthly | demand, supply, inventory | sales, rationing, inventory | province markets |
| T4 | reprice | Monthly | D, Q, S | prices | after clearing |
| T0 | kernel | On graph version change | path cache, wedges | kernel legs | load and shocks |

The full order with econ systems is in `specs/econ/SPEC.md`.

## Invariants

| Invariant | Statement | Tolerance | Checked where |
|---|---|---|---|
| Goods | Ledger goods identity (core) with transport and spoilage tallies | 1e-9 relative | `Ledger::check`, every tick |
| Gateway clearing | Each gateway's money and goods net to zero every tick | exact money, 1e-9 goods | every tick |
| Routing mass | sum_d X_od = Y_o for each origin and good | 1e-12 relative | debug every tick, property test |
| Inventory sign | S >= 0 | exact | every tick |
| Price bounds | Prices finite and positive | exact | every tick |
| Two-level vs single-level | On small worlds, flows from the gateway kernel versus direct-path kernel: median absolute log difference reported, and both nest in their GE solutions | report only; V1 tolerances apply to each | `tests/trade_nesting.rs` |
| Thread independence | 1 and 8 threads, identical hashes | exact | P2 acceptance |

## Parameters

| ID | Name | Prior / value | Class |
|---|---|---|---|
| P01 | Trade elasticity theta | 4 to 8, start 5 | Literature |
| P02 | Inventory target coverage s_g | 1 to 6 months by good, start: agriculture 3, oil and gas 1, coal and mining 2, food 2, manufactures 2; local goods 0 | Free |
| P03a, P03b | kappa1, kappa2 | 0.05 to 0.5 per month, start 0.2 and 0.1 | Free |
| P03c | Price step clip c_max | 0.1 log points per month | Free (numerical) |
| P04 | Mode costs | geo SPEC | Literature + calibrated |
| P05 | Border friction nu, ad valorem equivalent | log-uniform 0.1 to 1.0, start 0.3 | Calibrated (P9) |
| P41 | Time cost of transit lambda_g, ad valorem per day | manufactures (goods 4 to 6) 0.006 to 0.021, start 0.010 (Hummels and Schaur 2013); food 0.003 to 0.010, start 0.005; primary (goods 0 to 2) 0 to 0.006, start 0.002 | Literature for manufactures; free for food and primary |

Data, not parameters: weight per value w_g (BACI 2000), tariffs t (WDI applied rates, CEPII
agreement flags), gateway set (pack).

## Equilibrium nesting

Steady state of T1-T4 with any stable kappa: prices stop moving when D = Q and S = S*, which
is the spatial equilibrium of the logit routing with all markets clearing. That equilibrium is
computed directly by the GE reference model (`python/ceteris/reference`, econ SPEC) on the same
kernel. V1: frictionless runs reach it within 1%. H2: PPML on simulated country-pair flows
gives a distance elasticity in -0.7 to -1.1. Setting the routing's origin marginal hard and
adding a hard destination marginal turns T2 into balanced Sinkhorn; a test checks that on
small worlds the two coincide at the equilibrium prices.

## Test plan

- Unit: leg factors and tariff wedges; kernel low-rank products against dense K on small
  worlds; routing mass and Markov aggregation (property tests over random kernels, proptest);
  transit lags and wheel wraparound; market clearing, rationing, price clip, zero-supply floor.
- Property: goods and money conservation through T1-T4 under random supply and demand;
  gateway accounts net to zero; prices stay finite.
- Nesting: two-level versus direct kernel on small worlds; routing versus balanced Sinkhorn at
  equilibrium prices.
- Golden: covered by `tests/golden/p2-econ.txt` (econ SPEC).
- Benchmarks (criterion): `route` on the full pack (10 goods, budget 30 ms per tick on the
  M4 Pro, 19.2), `clear_reprice` (budget 2 ms), `kernel_build` (not per tick; budget 2 s).

## Open questions

- Gateway count and selection rule are an agent default (22.2), set by benchmark; starting
  rule in the econ SPEC pack section.
- Expectations are naive in P2; heuristic switching (12.2, P19) arrives with the expectations
  module.
- Freight capacity and endogenous freight rates (8.4) are P8.
