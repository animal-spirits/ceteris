# Trade SPEC

Status: approved (owner, 2026-10-23)
Phase: P2
Design sections: 3.4, 4.4, 4.6, 5.4, 8.1, 8.2, 8.3, 8.4, 8.5, 17 (P01-P05, S01, S02, S05), 19.4, 21.1 (P2); ADRs 0012, 0013, 0018, 0019 and 0023

## Purpose

`ceteris-trade` owns province markets and the movement of goods between them: one price and
one inventory per (province, good), the stock-flow price rule, the two-level trade kernel built
from `ceteris-geo` path costs, destination sourcing (ADR 0015), origin clearing across ticks (ADR
0016), the per-buyer pipeline of goods on order, and the payments that go with shipments. It does not own production, labour or demand (`specs/econ/SPEC.md`); it sees
them only as quantities offered to and demanded from each market.

## Deviations from DESIGN.md (approved with the spec)

| Design | P2 | Why |
|---|---|---|
| Develop P1-P3 on a European slice (21) | Full globe, 4,796 provinces | Owner decision; ADR 0012 |
| Per-tick unbalanced Sinkhorn over both marginals (8.3) | Destination sourcing in value (ADR 0015): each destination spreads its spending over origins with Eaton-Kortum shares K_od p_o^-theta / Phi_d on the two-level kernel; origin markets clear the orders against output and merchant stock across ticks, one normalization per tick (ADR 0016). Balanced Sinkhorn is the S02 full solve, in the GE reference only | Flows keep the gravity form and are homogeneous of degree zero in prices; one destination normalization followed by the origin price update is one Sinkhorn sweep spread over ticks, with the same frictionless fixed point. ADRs 0013, 0015, 0016 |
| Per-unit plus iceberg costs (5.4, A7) | Iceberg only (S05 = iceberg): tau = 1 + c w_g + lambda_g T at reference prices, the second term the time cost of transit (Hummels and Schaur 2013) standing in for 5.4's carrying cost; risk and per-unit composition effects deferred to P8 (freight) | Per-unit costs need a transport sector that owns fleets; iceberg nests exactly in the GE reference. Freight alone gives distance elasticities far below H2's range (owner decision: add the literature time cost, nothing fitted to H2) |
| Route-choice costs with A = t^-theta, the trade elasticity (ADR 0017 as accepted) | Least-cost legs (S05) in P2; route choice with a separate dispersion theta_r (P44) is built and tested but off, deferred past P2 (owner decision, 2026-10-24) | With theta the spectral radius on the P1 graph is 6.5 (coal) to 27 (machinery) at theta 5; with theta_r at its prior's upper end (182) still 2.96 (coal) to 6.04 (machinery), and radius 0.99 needs theta_r 4,700 to 880,000, driven by short sea-grid edges. Per-edge factors also compound (coal Rotterdam to Shanghai 13.1 against 3.6 under S05). ADR 0017, Deferral |
| Kernels per (theta class, weight class) (ADR 0018) | Kernels per traded good, each good at its theta class's value (3 classes, ADR 0023); `TradeParams.theta_class` carries the map | In P2 each good has its own weight w_g, so weight classes are goods and theta classes add no kernels while goods number 7 (ADR 0018, Mapping to P2 as built) |
| Utilities, construction and services traded through demand frictions (7.1) | Not traded between provinces in P2 | Province-local markets; cross-province service frictions arrive with P8 |
| Walrasian clearing mode (S01) | Deferred to P3 | V1 uses the stock-flow rule converging to the GE reference, which is the stronger test |
| Buyers price goods at the posted price (8.2) | Buyers of a traded good price it at the delivered unit price q_d = 1 / alpha_d, the numeraire needed per arriving unit under the current sourcing shares, tariffs and iceberg losses included (owner decision, 2026-10-24) | A unit bought at the CES index P_d delivers fewer than one unit (P_d is below every delivered cost), so input orders at P_d never replace use; at q_d the units ordered are the units that arrive |
| Merchant stocks targeted by coverage (8.2) | Enterprises order up to an inventory position (stock plus goods on order) of (1 + cover + L) months of use, cover = 1, L the sourcing-weighted expected months in transit of the input (owner decision, 2026-10-24); see the econ SPEC | A fixed two-month position that counts goods on order cannot hold a month in transit: importers bound at the equilibrium and V1 failed on the island (MIGRATION Task 3) |
| Stock-flow price rule at kappa1 0.2, kappa2 0.1 (8.2) | Inventory term off by default (kappa2 = 0) and slower price speed (kappa1 = 0.05) (owner decisions 2026-10-24: kappa1 0.02 for stability, then 0.05 with expected inflation, MIGRATION Task 6) | The derived region (Stability region, below; ADR 0019) puts kappa1 0.2 outside for the theta-12.55 goods and kappa2 0.05 outside in the default mode. At kappa1 0.02 the world price level returns too slowly for the expectation loop: every P45 gain in the prior broke V1 (0.006: 1.02e-2) or the ledger (0.25 and up); at 0.05 with gain 0.006 V1 is 2.4e-5 and a small deviation decays by 0.9963 / 0.9969 per tick (frictionless / default). Growth factor per tick of a small deviation from the fixture's GE equilibrium, ticks 300 to 600, at the per-good thetas (MIGRATION Task 6, pi^e = 0; frictionless / default): kappa1 0.02, kappa2 0: 0.9954 / 0.9954, the same with every crossing lag at 3; kappa1 0.05: 0.9862 / 0.9871; 0.1: 0.9718 / 0.9756 (lag 3 the same); 0.2: 1.0000 / 1.0000 (oscillates, lag 3 the same). kappa2 0.05: kappa1 0.02: 0.9876 / ledger overflow (lag 3: 0.9881 / 0.9998); 0.05: 0.9738 / 1.0014; 0.1: 0.9999 / 0.9978; 0.2: 1.0000 / 1.0000. The stock integrates the gap and feeds the money and trade-balance loop (Task 7b) |

## State components

| Component | Entity | Type | Units | Initialized from |
|---|---|---|---|---|
| Price p | (province, good) | `f64` | numeraire per unit | 1.0 (2000 world reference price), then the calibrated state |
| Inventory S | (province, traded good) | `f64`, also held in the ledger | units | s_g x initial demand |
| Last demand D (orders by class at origins), last supply Q, last sales | (province, good) | `f64` | units per month | zero |
| Clip ticks: ticks at which the price step clip bound | (province, good) | `u32` | ticks | zero |
| Expected inflation pi^e and the world price index I of its last update | world | `f64` | log points per month; numeraire | 0 and the index at the data state's prices |
| Buyer price q_d (traded) or posted price (local) | (province, good) | `f64` | numeraire per arriving unit | the data state's prices |
| Trade kernel legs | domestic pairs, province-gateway legs, gateway pairs, per traded good | `f32` K = tau^-theta_g, CSR | | least-cost leg costs (S05; route-choice factors from `Network.route_taus` are off in P2, ADR 0017) at load; rebuilt when the graph version moves |
| Transit lag | gateway pair | `u8` | months | gateway-pair hours |
| Pipeline: goods paid for and in flight | (delivery slot 1 to 4, buyer, traded good) | `f64` | units | empty |
| Tariff and border wedge | (importer country, exporter country, traded good) | `f32` | ad valorem | border friction P05 plus 2000 tariffs (econ pack) |
| Market, gateway, transit accounts | ledger | money `i64`, goods `f64` | | opened at load |

A unit of good g is the quantity worth one 2000 US dollar at the world reference price. Goods
are innermost in every goods-indexed array (19.4); P2 has 10 goods, padded to 12 for `f64`
alignment where it helps vectorization.

## Equations

Symbols: o origin province, d destination province, a and b gateways, A and B countries,
g good, theta = theta_g = P01 of good g (its class's value, ADR 0023), w_g weight per value (tonnes per unit, econ pack), c path cost per tonne
(`ceteris-geo`).

**Leg factors.** Two sources, chosen by the network (`Network.route_taus`):

- Least-cost legs (S05; the P2 default, the fixture and the pack): for a leg with least-cost path cost c (USD per tonne)
  and transit time T (days, hours along the same path / 24),
  `tau_leg,g = 1 + c * w_g + lambda_g * T` (iceberg; geo SPEC mapping at p = 1).
- Route choice (ADR 0017, `LegTaus`; built and tested but off in P2: `Network.route_taus` stays
  `None`, route choice is deferred past P2, owner 2026-10-24): per good,
  `tau_leg,g = z^(-1/theta_r)` over every walk on the transport graph with edge factors
  `t = 1 + c_kl w_g + lambda_g hours_kl / 24` and dispersion theta_r (P44), and the transit
  time is the route-probability-weighted hours (geo SPEC, Route choice). Domestic legs solve on
  the country's subgraph, access and crossing legs from each gateway on the whole graph. Factors
  below 1 are rejected (a leg would create goods).

```
K_leg,g   = tau_leg,g ^ -theta
```

lambda_g is the ad valorem cost of a day in transit (P43). Crossing lags use the leg's hours
under either source. Inside a province trade is costless (A2): K_oo = 1.

**Active markets.** A traded cell (o, g) has an origin market if an enterprise of province o
produces g with positive capacity (A VA0 > 0); a local cell (d, g) has a province market if
anyone at d buys g (its country's final demand share Omega_g > 0 or a local enterprise uses g
as an input). Only active cells carry a price state; destinations are every province with
spending. Origin weights are relative to the good's lowest active price, w_o = (p_o /
p_min)^-theta, and floored at WEIGHT_FLOOR = 1e-100: an origin below the floor is not a source,
and a destination whose Phi_d (in the same relative units) is below it reaches no origin, buys
nothing and has an infinite buyer price. A price spread above about 1e20 at theta 5, or 1e8 at
the largest class value 12.55, can make a destination unreachable. Since every weight is relative to one global p_min, a single cheap
origin anywhere sets the scale for all destinations.

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

**Sourcing (ADR 0015).** Each tick destination d spends E_d,g (numeraire, tariff included) on
good g: households by CES at the buyer price, enterprises their input orders times the buyer
price (econ SPEC). Spending goes over origins and routes with Eaton-Kortum shares at posted
prices (naive expectations, S04 adaptive-only in P2):

```
X_od = E_d * K_od * w_o / Phi_d,      Phi_d = sum_o K_od * w_o,      w_o = (p_o / p_min)^-theta
P_d  = p_min * Phi_d^(-1/theta)       (CES index, exported for information)
```

K carries the tariff on crossings, so X is what d pays: origin o is ordered X / (p_o (1 + t))
units, d receives them over the route's physical tau. The home route (o = d, K = 1) is part of
the sum. alpha_d = sum over routes of X / (E_d p_o (1 + t) tau) is the units arriving per unit of
spending, and q_d = 1 / alpha_d the delivered unit price buyers use (owner decision,
2026-10-24). The arrival slots of those units (0 home, 1 domestic or a lag-0 crossing, 2 to 4 a
crossing with lag 1 to 3) weighted by units give the expected lead L_d,g in months.

**Low-rank evaluation.** International terms never materialize K_od. Backwards, on the origin
weights:

```
v'_a = sum_{o in A} K_oa w_o          (origin access at gateway a)
u'_b = sum_a v'_a K_ab                 (access of destination gateway b to foreign origins)
Phi_d = w_d + sum_{o in B} K_od w_o + sum_b K_bd u'_b
```

Orders split as a Markov chain: d sends E_d K_bd u'_b / Phi_d to gateway b, b splits by
K_ab v'_a / u'_b over origin gateways (the tariff stays at the crossing), a by K_oa w_o / v'_a over
origins. Dispatch runs the same split forwards with destination weights E_d / Phi_d, scaled by
each origin's fill; iceberg losses apply per leg, the border loss on the crossing. Cost per good:
O(sum_A n_A^2 + P k + G^2), with k gateways per country and G about 600.

**Payments.** Buyers pay at dispatch for what is filled: p_o (1 + t) per shipped unit, so a
fully filled destination pays E_d. Each buyer pays its share of its class's filled spending to
its destination market, rounded half to even per buyer and good; unfilled spending stays with
the buyer. The market passes it on: domestic legs to the origin market directly; on a crossing
the tariff t p_o per unit goes to the importer's rebate account (`specs/econ/SPEC.md`, income)
and the rest moves d to b to a to o along the same split, so each gateway account nets to zero
every tick (payments round to minor units per leg; gateway splits round cumulatively). A home
purchase moves no money across legs: the market is both the buyer's market and the origin. The
origin market receives p_o per unit sold.

**Transit.** A shipment dispatched at tick t is delivered at t + s, s its arrival slot: home
purchases at once (s = 0), domestic shipments and lag-0 crossings at t + 1, crossings at
t + 1 + lag_ab, lag_ab = round(hours_ab / 730), capped at 3. Goods in flight sit in a transit
account per good and in the buyer's pipeline; on arrival pops consume them and enterprises add
them to their input stocks. The iceberg and border losses are destroyed at dispatch (tallied
as transport).

**Origin market, traded good (ADR 0016).** At tick t, supply Q = this tick's output of the
province's producers; buyers everywhere place orders D at the origin (intermediate users and
households, origin units). Sales = min(D, S + Q); a shortfall is rationed by S11 (default:
intermediate users first, households share the rest; DESIGN 8.2). S' = S + Q - sales: excess
supply goes to merchant inventory. Then (8.2):

```
dlnp = clip( pi^e + kappa1 (D - Q)/Q  -  kappa2 (S' - S*)/S*,  -c_max, c_max ),   S* = s_g D
```

pi^e is the merchants' expected inflation (ADR 0019), with unit pass-through: a homogeneity
restriction, not a parameter. P2 has one currency area, the numeraire, and adaptive expectations
only (S04): after T4 each tick

```
pi^e <- pi^e + g ( ln(I_t / I_{t-1}) - pi^e )      (P45)
```

with I_t the world price index over buyer prices (q_d of traded goods, the posted price of local
goods) at this tick's E3, weighted by base household consumption (`price_index_weights`, the
weights of V1 and of the GE reference's normalization; cells with an infinite buyer price are
left out). The inflation observed is that of the last repricing, as the adaptive heuristic of
12.2 forecasts from the last observed price. At a steady state I is constant and pi^e = 0, so the
GE reference is unchanged. Every tick the clip binds at a cell is counted (`clip_ticks`, run
export).

**Province market, local good** (utilities, construction, services): Q = local output; unsold
output perishes (destroyed, tallied as spoilage); s_g = 0, so only the first term applies.

Q or S* of zero (a province with no supply of g) uses the denominator max(Q, Q_floor) with
Q_floor = 1e-9 of the world mean of Q for that good.

### Stability region

The rule is a proportional-integral controller on the inventory gap (8.2): excess demand is the
flow, the stock deviation its integral. Linearize one origin market around a steady state (p\*,
Q\* = D\*, S\* = s_g D\*) with pi^e = 0: x = ln p - ln p\*, q the output deviation (Q - Q\*)/Q\*,
sigma = S / (s_g D\*) - 1. As built, E1 hires toward the labour target at this tick's price (P41
eta), T3 clears this tick's orders and T4 reads the stock after clearing against this tick's
target s_g D_t:

```
q_t         = (1 - eta) q_{t-1} + eta eps x_t
e_t         = (D_t - Q_t) / Q_t = -E x_t - q_t
sigma_{t+1} = sigma_t - e_t / s_g
x_{t+1}     = x_t + kappa1 e_t - kappa2 (sigma_{t+1} + E x_t)
```

The last term is the moving target: a price rise cuts D by E x and with it S\*, so the stock
looks in excess and kappa2 adds kappa2 E to the proportional gain. The state (x_t, q_{t-1},
sigma_t) moves by

```
      [ 1 - k A - kappa2 E   -k B      -kappa2 ]
M  =  [ eta eps               B         0      ]      A = E + eta eps,  B = 1 - eta,
      [ A / s_g               B / s_g   1      ]      k = kappa1 + kappa2 / s_g
```

with characteristic polynomial P(lambda) = lambda^3 + a2 lambda^2 + a1 lambda + a0:

```
a2 = E (kappa1 + kappa2) + E kappa2 / s_g + eta eps (kappa1 + kappa2 / s_g) + eta - 3
a1 = (eta - 2) E (kappa1 + kappa2) + (eta - 1) E kappa2 / s_g - eta eps kappa1 - 2 eta + 3
a0 = (1 - eta) (E (kappa1 + kappa2) - 1)
```

The region is the set of (kappa1, kappa2) where the spectral radius of M is below 1, which by the
Jury conditions for a cubic holds if and only if

```
(J1)  P(1)  = eta kappa2 (E + eps) / s_g > 0
(J2)  P(-1) < 0,  i.e.  kappa1 (E + eta eps / (2 - eta))
                      + kappa2 (E (1 + 1 / (2 s_g)) + eta eps / (2 s_g (2 - eta))) < 2
(J3)  |a0| < 1
(J4)  1 - a0^2 > |a1 - a0 a2|
```

Elasticities. E is the elasticity of the units ordered at the origin to its own price. Under
origin clearing with destination sourcing the margin of one origin among many is the sourcing
elasticity: orders in value fall with p^-theta_g and units are value over p, so E = theta_g + 1.
An origin that is its destinations' only source faces their total demand instead, bounded by the
CES elasticity sigma (P06, 0.6) for households (Leontief input orders are inelastic). The region
below is the intersection over E from sigma to theta_g + 1: the kappa1 limit comes from the
small origin (J2), the kappa2 limit in the default mode mostly from total demand (J4). eps is the supply
elasticity of the labour target at given wage and input prices, s_L sigma_FL / ((1 - s_L) a_VA)
(normalized CES, Equations of the econ SPEC), 0.90 to 2.61 on the fixture; eta is P41 (0.3, or 1
frictionless); s_g is P02.

Region at the P01 values, kappa1 limit as kappa2 goes to 0 (J2) and first kappa2 that fails at
kappa1 = 0.05 (the default) and 0.02 (the earlier start value), default mode (eta 0.3) / frictionless (eta 1), over E in [0.6, theta_g + 1] and the
fixture's eps:

| Good | theta_g | s_g | kappa1 limit | kappa2 limit at kappa1 0.05 (default) | at kappa1 0.02 |
|---|---|---|---|---|---|
| 0 Agriculture | 12.55 | 3 | 0.144 / 0.130 | 0.083 / 0.077 | 0.109 / 0.105 |
| 1 Crude oil and gas | 12.55 | 1 | 0.145 / 0.132 | 0.049 / 0.059 | 0.016 / 0.080 |
| 2 Coal and other mining | 12.55 | 2 | 0.144 / 0.130 | 0.077 / 0.071 | 0.040 / 0.097 |
| 3 Food | 2.62 | 2 | 0.507 / 0.368 | 0.392 / 0.347 | 0.040 / 0.380 |
| 4 Light manufacturing | 5.88 | 2 | 0.278 / 0.230 | 0.189 / 0.173 | 0.040 / 0.202 |
| 5 Chemicals, refined fuels, metals | 12.55 | 2 | 0.143 / 0.124 | 0.077 / 0.068 | 0.031 / 0.095 |
| 6 Machinery, electronics, vehicles | 5.88 | 2 | 0.278 / 0.230 | 0.189 / 0.173 | 0.040 / 0.202 |

Limits are the first failing value on a 0.0005 grid.

The default (kappa1 0.05, kappa2 0) lies on the edge kappa2 = 0 of the region and counts as
inside (owner ruling, MIGRATION Task 6): J1 holds with equality, so the stock has a unit root (it
keeps what a transient leaves, as decided: inventories are untargeted in P2) while J2 to J4 hold,
kappa1 at 0.10 to 0.40 of its limit, and prices and output return to the steady state (spectral
radius of the (x, q) block 0.19 to 0.93; 0.68 to 0.97 at kappa1 0.02). The
derivation agrees with the fixture: kappa1 0.2 lies beyond the theta-12.55 limits and oscillates;
kappa2 0.05 at kappa1 0.02 lies outside in the default mode for goods 1 to 6 and inside
frictionless, and the fixture overflows in the first and decays in the second (Deviations).
`price_rule_stability.rs` checks the conditions against a toy market run through
`Markets::clear` and `Markets::reprice` (10 pairs inside and 10 outside by at least 10% in each
coordinate). This region, for the rule as built and with E from theta_g + 1 down to sigma,
replaces the simplified 3 x 3 of the migration plan (owner ruling, MIGRATION Task 6): reading the
stock before clearing against a fixed target misclassifies that toy market both ways (at E =
13.55 it calls (0.02, 0.06) unstable and (0.1, 0.06) stable).

Nominal mode. pi^e adds a world mode outside the one-market region: with money fixed, a common
deviation X of the price level lowers household real demand, e = -h X, and the rule pulls X back
at a = kappa1 h per tick (h the household share of orders, about 0.24 on the fixture: a = 0.0048
at kappa1 0.02, the convergence rate of the world index from the data state, and about 0.012 at
the default 0.05). With the update above,
X_{t+1} = (1 - a) X_t + pi^e_t and pi^e_{t+1} = (1 - g) pi^e_t + g (X_t - X_{t-1}), with
characteristic polynomial lambda^3 + (a + g - 2) lambda^2 + (a g - a - 2 g + 1) lambda + g:
stable if and only if 0 < g < (4 - 2a) / (4 - a) and a < 2 (1 - g^2), but the first-order return
at 1 - a becomes a damped oscillation of modulus about sqrt(1 - a (1 - g)): at a = 0.0048 the
slowest root is 0.9952 at g = 0 and 0.9976 to 0.9993 for g from 0.006 to 0.9 (1.0012 at g = 1),
too slow for V1 from the data state; at a = 0.012 it is 0.9941 at g = 0.006 (0.9950 at 0.1,
0.9978 at 0.65), and the defaults pass V1 (MIGRATION Task 6).

## System order

| System ID | Name | Cadence | Reads | Writes | Position in tick pipeline (4.4) |
|---|---|---|---|---|---|
| T1 | deliver | Monthly | pipeline | buyers' goods (consumed or stocked), pipeline | before production |
| T2 | source | Monthly | destination spending, prices, kernel | orders at origins | after demand (E3) |
| T3 | clear | Monthly | orders, output, inventory | sales, fills, inventory; local markets | markets |
| T2b | dispatch | Monthly | fills, sourcing of T2 | shipments, pipeline, payments, tariffs | after clearing |
| T4 | reprice | Monthly | D, Q, S, pi^e, buyer prices | prices, clip counts, pi^e and I | after dispatch |
| T0 | kernel | On graph version change | path cache, wedges | kernel legs | load and shocks |

The full order with econ systems is in `specs/econ/SPEC.md`.

## Invariants

| Invariant | Statement | Tolerance | Checked where |
|---|---|---|---|
| Goods | Ledger goods identity (core) with transport and spoilage tallies | 1e-9 relative | `Ledger::check`, every tick |
| Gateway clearing | Each gateway's money and goods net to zero every tick | exact money, 1e-9 goods | every tick |
| Sourcing | sum_o X_od = E_d for each reachable destination and good | 1e-12 relative | `source` unit and property tests |
| Dispatch mass | units dispatched = origin sales, each origin and good | 1e-12 relative | every tick |
| Inventory sign | S >= 0 | exact | every tick |
| Price bounds | Prices finite and positive | exact | every tick |
| Two-level vs single-level | On small worlds, flows from the gateway kernel versus direct-path kernel: median absolute log difference reported, and both nest in their GE solutions | report only; V1 tolerances apply to each | `tests/trade_nesting.rs` |
| Pipeline | goods in the transit accounts = goods in the pipeline, per good | 1e-9 relative | every tick |
| Thread independence | 1 and 8 threads, identical hashes | exact | P2 acceptance |

## Parameters

| ID | Name | Prior / value | Class |
|---|---|---|---|
| P01 | Trade elasticity theta_g per traded good, in classes (ADRs 0018 and 0023) | 2.62 to 12.55, the range of the class values (widened from 4 to 8, ADR 0018). Values: class 0 (food) 2.62, class 1 (light manufacturing; machinery, electronics and vehicles) 5.88, class 2 (agriculture; crude oil and gas; coal and other mining; chemicals, refined fuels and metals) 12.55. Caliendo and Parro (2015, Table 1 p. 18, 99% sample) sector estimates averaged over each good's HS lines with 2000 BACI trade weights (9.11, 13.53, 13.53, 2.62, 6.73, 12.64, 5.33 for goods 0 to 6), grouped into the fewest classes within a factor 1.5, each at its goods' trade-weighted mean; local goods 0 | Literature |
| P02 | Inventory target coverage s_g | 1 to 6 months by good, start: agriculture 3, oil and gas 1, coal and mining 2, food 2, manufactures 2; local goods 0 | Free |
| P03a | kappa1 | 0.01 to 0.5 per month, start 0.05 (prior widened from 0.05 by owner decision, for stability; start 0.02, then 0.05 by owner decision 2026-10-24 so the expectation loop of ADR 0019 passes V1, MIGRATION Task 6) | Free |
| P03b | kappa2 | 0 to 0.5 per month, start 0: inventory term off, inventories untargeted in P2 (prior widened from 0.05 by owner decision) | Free |
| P03c | Price and wage step clip c_max | 0.1 log points per month; every tick it binds is counted per market and per wage (`clip_ticks` in `run_prices` and `run_wages`) | Free (numerical) |
| P04 | Mode costs | geo SPEC | Literature + calibrated |
| P05 | Border friction nu, ad valorem equivalent | log-uniform 0.1 to 1.0, start 0.3 | Calibrated (P9) |
| P43 | Time cost of transit lambda_g, ad valorem per day | manufactures (goods 4 to 6) 0.006 to 0.021, start 0.010 (Hummels and Schaur 2013); food 0.003 to 0.010, start 0.005; primary (goods 0 to 2) 0 to 0.006, start 0.002 | Literature for manufactures; free for food and primary |
| P45 | Expectation gain g of pi^e (ADR 0019) | 0.004 to 0.65 per month. Low end: Poledna et al. (2023, EER 151, Section 3.3 p. 7; IIASA WP 2020 version, Appendix A.1.2 eqs. 8 to 10 pp. 23 to 24) let firms scale every cost component of the price by (1 + pi^e) (unit pass-through), with pi^e from an AR(1) on past inflation re-estimated by OLS every quarter on all data since 1997Q1: decreasing-gain learning, gain 1/n on the mean, 0.011 to 0.019 per quarter over their reference quarters 2010 to 2019 (n = 53 to 91), about 0.004 to 0.006 per month. High end: the adaptive heuristic of 12.2, p^e_{t+1} = w p_{t-1} + (1 - w) p^e_t (Anufriev and Hommes 2012, AEJ Micro 4(4), eq. 6 p. 45), w = 0.65 in their four-heuristic model (Table 1 p. 51; Figure 4 p. 44 also 0.25; individual estimates up to 1, fn. 4 p. 45), per experimental period, which has no calendar length Start 0.006, the Poledna end (owner decision 2026-10-24, MIGRATION Task 6; at kappa1 0.02 every value in the prior broke V1) | Literature |
| P44 | Route-choice dispersion theta_r (ADR 0017); unused in P2 (ADR 0017 deferred) | 41 to 182, value 111.5: Fan, Lu and Luo (2021, pp. 21 and 23, Table 3; Table 4 p. 28), routing elasticity on China's road network from exporters' port choice and price gaps, 111.5 (s.e. 35.4; bootstrap p10 to p90 103.5 to 127.3), prior = estimate +- 2 s.e. The other route-choice models set it equal to the trade elasticity by assumption: 8 in Allen and Arkolakis (2022, p. 2936) and Fuchs and Wong (2026, pp. 16 and 36), 4 in Ganapati, Wong and Ziv (2024, p. 268; p. 254 fn. 27 names a separate route elasticity as an alternative, not estimated). On the P1 graph the spectral radius at the prior's upper end is 2.96 (coal) to 6.04 (machinery), and radius 0.99 needs theta_r 4,700 to 880,000 (MIGRATION Task 4 report), so no value in the prior passes | Literature |

Data, not parameters: weight per value w_g (BACI 2000), tariffs t (WDI applied rates, CEPII
agreement flags), gateway set (pack).

## Equilibrium nesting

Steady state of T1-T4 with any stable kappa: prices stop moving when every origin sells its
output (D = Q) and every local market clears (and S = S* when kappa2 > 0; with the default
kappa2 = 0 the stock stays where the transient left it). That is the structural-gravity
equilibrium of destination sourcing, computed directly by the GE reference model
(`python/ceteris/reference`, econ SPEC) on the same kernel. V1: frictionless runs reach it within
1%. H2: PPML on simulated country-pair flows gives a distance elasticity in -0.7 to -1.1, and per
theta class, a PPML on ln(1 + tariff) recovers the class's theta within its prior band (ADR 0018,
`ppml.trade_elasticity`). S02:
balanced Sinkhorn on the dense kernel with the equilibrium's origin supplies and destination
expenditures reproduces the sourcing flows (1e-8, reference test), and a Rust run started at the
reference with every crossing lag at 0 stays within 1e-6 of it (`equilibrium_is_a_fixed_point`).

## Test plan

- Unit: leg factors and tariff wedges; theta per good (`kernel_uses_theta_per_good`) and price
  homogeneity of the shares per good at each good's theta; route-choice factors and hours used per good when the
  network carries them (`kernel_uses_route_taus_when_present`), factors below 1 rejected; kernel low-rank products against dense K on small
  worlds; sourcing against the dense route table and closed-form Eaton-Kortum shares, dispatch
  conservation, unreachable destinations, extreme prices (proptest); pipeline slots and
  wraparound; origin clearing, rationing classes, price clip, zero-supply floor; expected
  inflation in the rule (`reprice_adds_expected_inflation`) and clip counts
  (`clip_ticks_count_bindings`).
- Stability: the Jury conditions against a toy market run through `Markets`
  (`stability_region_matches_simulation`) and the default on the region's kappa2 = 0 edge
  (`default_lies_on_the_untargeted_stock_edge`), in `tests/price_rule_stability.rs`.
- Property: goods and money conservation through T1-T4 under random supply and demand;
  gateway accounts net to zero; prices stay finite.
- Nesting: two-level versus direct kernel on small worlds; sourcing versus balanced Sinkhorn at
  the equilibrium (S02).
- Golden: covered by `tests/golden/p2-econ.txt` (econ SPEC).
- Benchmarks (criterion): `source` and `dispatch` on the full pack (10 goods, budget 5 ms per tick
  on the M4 Pro, 19.2 at rev 226), `clear_reprice` (budget 2 ms), `kernel_build` (not per tick; budget 2 s).

## Open questions

- Gateway count and selection rule are an agent default (22.2), set by benchmark; starting
  rule in the econ SPEC pack section.
- Expectations are adaptive only in P2 (S04): one pi^e for the numeraire area (ADR 0019); price
  sourcing still uses posted prices (naive). Heuristic switching (12.2, P19) arrives with the
  expectations module.
- Freight capacity and endogenous freight rates (8.4) are P8.
- Inventories are untargeted in P2 (kappa2 = 0): stocks keep what a transient leaves (4.7 to 47 times
  target cover on the fixture after 1,200 ticks from the data state). Revisit with
  expectations (12.2), which can make an inventory target stable, or with P9 calibration.
