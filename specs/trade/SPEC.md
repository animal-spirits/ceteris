# Trade SPEC

Status: approved (owner, 2026-10-23)
Phase: P2
Design sections: 3.4, 4.4, 4.6, 5.4, 8.1, 8.2, 8.3, 8.4, 8.5, 17 (P01-P05, S01, S02, S05), 19.4, 21.1 (P2); ADRs 0012, 0015, 0016, 0017, 0018, 0019, 0020, 0021, 0022, 0023, 0093 (kernel range, log-domain weights) and 0097 (dispatch)

## Purpose

`ceteris-trade` owns province markets and the movement of goods between them: one price and
one inventory per (province, good), the stock-flow price rule, the two-level trade kernel built
from `ceteris-geo` path costs, destination sourcing (ADR 0015), origin clearing across ticks (ADR
0016), the per-buyer pipeline of goods on order, and the payments that go with shipments. It does not own production, labour or demand (`specs/econ/SPEC.md`); it sees
them only as quantities offered to and demanded from each market.

## Deviations from DESIGN.md (approved with the spec)

| Design | P2 | Why |
|---|---|---|
| Develop P1-P3 on a European slice (21) | Full globe, 4,594 provinces | Owner decision; ADR 0012 |
| Per-tick unbalanced Sinkhorn over both marginals (8.3) | Destination sourcing in value (ADR 0015): each destination spreads its spending over origins with Eaton-Kortum shares K_od p_o^-theta / Phi_d on the two-level kernel; origin markets clear the orders against output and merchant stock across ticks, one normalization per tick (ADR 0016). Balanced Sinkhorn is the S02 full solve, in the GE reference only | Flows keep the gravity form and are homogeneous of degree zero in prices; one destination normalization followed by the origin price update is one Sinkhorn sweep spread over ticks, with the same frictionless fixed point. ADRs 0013, 0015, 0016 |
| Per-unit plus iceberg costs (5.4, A7) | Iceberg only (S05 = iceberg): tau = 1 + c w_g + lambda_g T at reference prices, the second term the time cost of transit (Hummels and Schaur 2013) standing in for 5.4's carrying cost; risk and per-unit composition effects deferred to P8 (freight) | Per-unit costs need a transport sector that owns fleets; iceberg nests exactly in the GE reference. Freight alone gives distance elasticities far below H2's range (owner decision: add the literature time cost, nothing fitted to H2) |
| Route-choice costs with A = t^-theta, the trade elasticity (ADR 0017 as accepted) | Least-cost legs (S05) in P2; route choice with a separate dispersion theta_r (P44) is built and tested but off, deferred past P2 (owner decision, 2026-10-24) | With theta the spectral radius on the P1 graph is 6.5 (coal) to 27 (machinery) at theta 5; with theta_r at its prior's upper end (182) still 2.96 (coal) to 6.04 (machinery), and radius 0.99 needs theta_r 4,700 to 880,000, driven by short sea-grid edges. Per-edge factors also compound (coal Rotterdam to Shanghai 13.1 against 3.6 under S05). ADR 0017, Deferral |
| Kernels per (theta class, weight class) (ADR 0018) | Kernels per traded good, each good at its theta class's value (3 classes, ADR 0023); `TradeParams.theta_class` carries the map | In P2 each good has its own weight w_g, so weight classes are goods and theta classes add no kernels while goods number 7 (ADR 0018, Mapping to P2 as built) |
| Utilities, construction and services traded through demand frictions (7.1) | Not traded between provinces in P2 | Province-local markets; cross-province service frictions arrive with P8 |
| Walrasian clearing mode (S01) | Deferred to P3 | V1 uses the stock-flow rule converging to the GE reference, which is the stronger test |
| Buyers price goods at the posted price (8.2) | Buyers of a traded good price it at the delivered unit price q_d = 1 / alpha_d, the numeraire needed per arriving unit under the current sourcing shares, tariffs and iceberg losses included (owner decision, 2026-10-24) | A unit bought at the CES index P_d delivers fewer than one unit (P_d is below every delivered cost), so input orders at P_d never replace use; at q_d the units ordered are the units that arrive |
| Merchant stocks targeted by coverage (8.2) | Enterprises order up to an inventory position (stock plus goods on order) of (1 + cover + L) months of use, cover = 1, L the sourcing-weighted expected months in transit of the input (owner decision, 2026-10-24); see the econ SPEC | A fixed two-month position that counts goods on order cannot hold a month in transit: importers bound at the equilibrium and V1 failed on the island (MIGRATION Task 3) |
| Rationing within the tick after Pichler et al. (2022): priority by default (households' essential consumption, then intermediate users by contract, then the rest), proportional and mixed as S11 (8.2, 17.2) | S11 is `EconParams.rationing`, shared between intermediate users (index 0) and households (index 1) of what a market has: S + Q at an origin market (T3), the output left after the same-tick chain at a local market (T3; econ SPEC, Production). With A available and orders D_int, D_hh, D = D_int + D_hh, every class is filled when D <= A; otherwise: **Proportional** fills both at A / D. **Mixed** fills intermediate users at min(1, A / D_int) and households at max(A - D_int, 0) / D_hh (1 without household orders). **Priority** (default) serves households' essential consumption first, then intermediate users pro rata, then the rest; P2 has no essential consumption levels (they arrive in P4), so their P2 value is 0 and Priority fills as Mixed (`mixed_rationing_matches_definition`). Inside the same-tick chain intermediate users of a local good are served before households under every setting: the chain runs at E2, before household demand is posted (ADR 0020, Mapping to P2 as built) | Sources. Pichler, Pangallo, del Rio-Chanona, Lafond and Farmer (2022, JEDC 144, 104527, Section 2.2.2, eqs. (15) to (17), p. 8) use proportional rationing only: industry j delivers O_ji x_j / d_j to industry i, households receive c^d_i x_i / d_i; for other rules they refer to Pichler and Farmer (2021). That paper (Economic Systems Research 34(3), 2022, 273-293; read as arXiv 2101.07818v2, Section 4.2) defines (a) strict proportional rationing (Algorithm 1, eqs. (7) to (11), p. 8: r_i = x_i^max / d_i for all customers) and (b) mixed proportional/priority rationing (Algorithm 2, eqs. (12) to (16), p. 9): intermediate customers are rationed pro rata on r_i = x_i^max / sum_j a_ij d_j (eq. 12), so final demand plays no part in the bottleneck, and final demand takes the residual f_i = max(x_i - sum_j a_ij x_j, 0) (eq. 15). Mixed above is (b) with x_i^max = A, Proportional is (a). Their (c), priority as largest customer first, is not built: DESIGN 8.2 names its own priority order |
| Stock-flow price rule at kappa1 0.2, kappa2 0.1 (8.2) | Inventory term off by default (kappa2 = 0) and slower price speed (kappa1 = 0.05) (owner decisions 2026-10-24: kappa1 0.02 for stability, then 0.05 with expected inflation, MIGRATION Task 6) | The derived region (Stability region, below; ADR 0019) puts kappa1 0.2 outside for the theta-10.57 goods and kappa2 0.05 outside in the default mode. At kappa1 0.02 the world price level returns too slowly for the expectation loop: every P45 gain in the prior broke V1 (0.006: 1.02e-2) or the ledger (0.25 and up); at 0.05 with gain 0.006 V1 is 3.14e-5 and a small deviation decays by 0.99644 / 0.99676 per tick (frictionless / default; the blessed fixture after MIGRATION Task 8, reproduced by `p2_expectation_sweep.rs`; 2.4e-5 and 0.9963 / 0.9969 when measured at Task 6). Growth factor per tick of a small deviation from the fixture's GE equilibrium, ticks 300 to 600, at the per-good thetas (MIGRATION Task 6, pi^e = 0; frictionless / default): kappa1 0.02, kappa2 0: 0.9954 / 0.9954, the same with every crossing lag at 3; kappa1 0.05: 0.9862 / 0.9871; 0.1: 0.9718 / 0.9756 (lag 3 the same); 0.2: 1.0000 / 1.0000 (oscillates, lag 3 the same). kappa2 0.05: kappa1 0.02: 0.9876 / ledger overflow (lag 3: 0.9881 / 0.9998); 0.05: 0.9738 / 1.0014; 0.1: 0.9999 / 0.9978; 0.2: 1.0000 / 1.0000. The stock integrates the gap and feeds the money and trade-balance loop (Task 7b) |

## State components

| Component | Entity | Type | Units | Initialized from |
|---|---|---|---|---|
| Price p | (province, good) | `f64` | numeraire per unit | 1.0 (2000 world reference price) in the data state; the calibrated posted price in the calibrated steady state (econ SPEC, Initial state; ADR 0089) |
| Inventory S | (province, traded good) | `f64`, also held in the ledger | units | s_g x initial demand; the calibrated S = s_g Q in the steady state (econ SPEC, Initial state) |
| Last demand D (orders by class at origins), last supply Q, last sales | (province, good) | `f64` | units per month | zero; the calibrated D = Q = sales in the steady state |
| Clip ticks: ticks at which the price step clip bound | (province, good) | `u32` | ticks | zero |
| Expected inflation pi^e and the world price index I of its last update | world | `f64` | log points per month; numeraire | 0 and the index at the initial prices (the data state's, or the calibrated steady state's) |
| Buyer price q_d (traded) or posted price (local) | (province, good) | `f64` | numeraire per arriving unit | sourcing at the initial posted prices (the data state's, or the calibrated steady state's) |
| Trade kernel legs | domestic pairs, province-gateway legs, gateway pairs, per traded good | `f64` K = tau^-theta_g and the reciprocals 1 / tau, 1 / (tau (1 + nu)), 1 / (1 + t), CSR | | least-cost leg costs (S05; route-choice factors from `Network.route_taus` are off in P2, ADR 0017) at load; rebuilt when the graph version moves |
| Transit lag | gateway pair | `u8` | months | gateway-pair hours |
| Pipeline: goods paid for and in flight | (delivery slot 1 to 4, buyer, traded good) | `f64` | units | empty; in the calibrated steady state, j ticks ahead one month of the shipments of every arrival slot j or later |
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
spending. Origin weights are held in log scale, per country (ADR 0093): ln w_o = -theta ln(p_o /
p_A) with p_A the lowest active price of o's country A, and the country's shift s_A = -theta
ln(p_A / p_min). Each destination d of country B splits its access Phi_d into a home part (its
home and domestic routes, in B's origin scale s_B) and an abroad part (through its gateways, in
B's destination scale s'_B, the largest shift among the foreign countries with a gateway that has
access and a crossing with K > 0 into B), and combines the two in log space. A crossing's terms
move from A's origin scale into B's destination scale by exp(s_A - s'_B) <= 1, computed once per
good and price vector for linked pairs only, and cannot overflow. Spending per unit of access
E_d / Phi_d is taken per part, in that part's scale: where the two scales differ by more than
709.78, so that the exponential of their difference overflows, the other part is moved into this
part's scale in log space, and it overflows only when its true value does, leaving this part a
share below part / f64::MAX, taken as 0. Where E_d / Phi_d itself is beyond f64 (a part near the
smallest normal f64 times large spending), every destination's E_d / Phi_d is carried as 2^-k of
its value and the orders and dispatched flows built on it are scaled back by 2^k, so flows stay
finite and spending is conserved; k is 0 otherwise, which leaves every bit unchanged. The GE
reference keeps one global scale and divides E_d by Phi_d directly, so it matches only where
neither underflows nor overflows, as on every pack and fixture price vector. A term is lost to
underflow only when it is below about 1e-308 of the largest term of its part, kernel factors
aside, so a price spread between countries alone makes no destination unreachable. There is no
weight floor: a destination is unreachable (buys nothing, infinite buyer price) only when
neither part is at least the smallest normal f64, that is when no path with K > 0 leads to it
from an active origin or every such path's product of weight and kernel factors underflows (for
example legs near tau 1e4 at theta 64.85, each K about 4e-260). Where a source is so dear (about
1e57 against its buyers' best at theta 5) that the units it ships, value / p_o, fall below the
smallest normal f64, those units lose their precision.

**Two-level kernel.** Domestic pairs (o and d in the same country) use the direct path:
K_od = tau_od^-theta, dense within each country. A cross-border pair is reached only through
gateways of the two countries:

```
K_od = sum over a in G(A), b in G(B) of  K_oa * K_ab * K_bd
K_ab = (tau_ab * (1 + nu_AB) * (1 + t_AB,g)) ^ -theta
```

ADR 0092's gateway mass weights (K_oa m_a K_ab m_b K_bd with the masses of a country's gateways
summing to 1, so access no longer grows with the number of gateway pairs) are not built: they
are deferred to the fit of the border friction per theta class, with which they land (owner,
2026-10-26). Factors are f64: an f32 ln K resolves K only to 1.5e-5 at tau 1e4 and theta 64.85
(about 4e-260, `kernel_round_trip_at_extreme_tau`), beyond ADR 0093's 1e-7, so K is
stored in f64 (ADR 0093's second option), which holds it down to about 1e-308.

nu is the non-tariff border friction (P05): an iceberg cost, destroyed at the crossing and
tallied as border loss. t is the applied tariff: a routing wedge only, no goods lost; its revenue
is paid in money (Payments, below). The sum over gateway pairs is
the entropic route choice of Allen and Arkolakis (2022): parallel routes add access. Gateways
belong to one country; a land crossing is two gateways, one on each side, joined by the crossing
edge.

**Sourcing (ADR 0015).** Each tick destination d spends E_d,g (numeraire, tariff included) on
good g: households by PIGL at the buyer price (ADR 0021), enterprises their input orders times
the buyer price (econ SPEC). Spending goes over origins and routes with Eaton-Kortum shares at posted
prices (naive expectations, S04 adaptive-only in P2):

```
X_od = E_d * K_od * w_o / Phi_d,      Phi_d = sum_o K_od * w_o,      w_o = (p_o / p_min)^-theta
P_d  = p_min * Phi_d^(-1/theta)       (CES index, for information; not exported)
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
O(sum_A n_A^2 + P k + G^2 + C^2), with k gateways per country, G about 600 and C countries (the
scale factors).

**Dispatch (ADR 0097).** Each good's dispatch is one serial pass per buyer class that adds every
leg's terms to its targets in a fixed order, so its bits do not depend on the thread count; the
tick runs the goods in parallel. Losses and duties multiply by the reciprocals the kernel stores
(1 / tau, 1 / (tau (1 + nu)), 1 / (1 + t)), and the divisions by the price p_o, the onward access
z_a and the destination access y_b are by reciprocals hoisted per origin, per origin gateway and
per destination gateway (by division where y_b or z_a is subnormal and its reciprocal
overflows). x (1 / d) rounds differently from x / d, so these bits differ from the divisions of
the first build. A gather by target, bit-identical to this pass, was built and measured slower
inside the tick (ADR 0097, As built), and removed. So was a fused pass of the seven goods, goods
innermost and bit-identical: it does a quarter less arithmetic than seven passes, but as one task
the parallel source and dispatch stage with it took 28.5 ms against about 7.7 ms for the stage
with the per-good scatter (ADR 0097, As built).

**Payments.** Buyers pay at dispatch for what is filled: p_o (1 + t) per shipped unit, so a
fully filled destination pays E_d. Each buyer pays its share of its class's filled spending to
its destination market, rounded half to even per buyer and good; unfilled spending stays with
the buyer. The market passes it on: domestic legs to the origin market directly; on a crossing
the tariff t p_o per unit goes to the importer's rebate account (`specs/econ/SPEC.md`, income)
and the rest moves d to b to a to o along the same split, so each gateway account nets to zero
every tick (payments round to minor units per leg; gateway splits round cumulatively). A home
purchase moves no money across legs: the market is both the buyer's market and the origin. The
origin market receives p_o per unit sold.

**Market cash flow (merchant rule, ADR 0112).** A province market buys all the output of its
producers at its posted price at E2 (p Q, flow SALE), whether or not it sells it, and carries
what it does not sell as stock without revaluing it; a local good's unsold output perishes
(`clear_local` tallies it as spoilage), a pure cash loss of p times the unsold amount. Buyers pay
it at E2 (local draws by enterprises), T2b and T3 (p_o per unit sold at an origin, and the buyer
price for local goods), so its signed cash flow in a tick is
revenue minus p Q: negative while it builds stock or sells below the price it paid, positive while
it draws stock down. At E4 it passes that cash flow, whatever its sign, to its country's profit pool
(econ SPEC, Income), so a market carries no balance between ticks and no limited liability or
write-off applies to it. At a steady state sales equal Q and the cash flow is 0.

**Transit.** A shipment dispatched at tick t is delivered at t + s, s its arrival slot: home
purchases at once (s = 0), domestic shipments and lag-0 crossings at t + 1, crossings at
t + 1 + lag_ab, lag_ab = round(hours_ab / 730), capped at 3. Goods in flight sit in a transit
account per good and in the buyer's pipeline; on arrival pops consume them and enterprises add
them to their input stocks. The iceberg and border losses are destroyed at dispatch (tallied
as transport).

**Origin market, traded good (ADR 0016).** At tick t, supply Q = this tick's output of the
province's producers; buyers everywhere place orders D at the origin (intermediate users and
households, origin units). Sales = min(D, S + Q); a shortfall is rationed by S11 (Deviations, rationing row; default
Priority: intermediate users first, households share the rest, DESIGN 8.2). S' = S + Q - sales: excess
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

**Province market, local good** (utilities, construction, services): Q = local output. Its
intermediate users draw from it within the tick, at E2, in the same-tick chain (ADR 0020; econ
SPEC, Production); those draws are part of D and of sales. At T3 the rest of Q is shared between
enterprise orders (stock refills) and households by S11; unsold output perishes (destroyed,
tallied as spoilage); s_g = 0, so only the first term applies.

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
| 0 Agriculture | 10.57 | 3 | 0.168 / 0.149 | 0.104 / 0.096 | 0.130 / 0.126 |
| 1 Crude oil and gas | 10.57 | 1 | 0.169 / 0.151 | 0.048 / 0.074 | 0.016 / 0.095 |
| 2 Coal and other mining | 10.57 | 2 | 0.168 / 0.149 | 0.097 / 0.089 | 0.040 / 0.116 |
| 3 Food | 2.62 | 2 | 0.508 / 0.368 | 0.392 / 0.347 | 0.040 / 0.380 |
| 4 Light manufacturing | 10.57 | 2 | 0.168 / 0.149 | 0.097 / 0.089 | 0.040 / 0.116 |
| 5 Chemicals, refined fuels, metals | 10.57 | 2 | 0.166 / 0.141 | 0.096 / 0.085 | 0.031 / 0.114 |
| 6 Machinery, electronics, vehicles | 5.19 | 2 | 0.307 / 0.250 | 0.214 / 0.195 | 0.040 / 0.225 |

Limits are the first failing value on a 0.0005 grid (`ceteris calibrate stability` prints the rows from the fixture export).

The default (kappa1 0.05, kappa2 0) lies on the edge kappa2 = 0 of the region and counts as
inside (owner ruling, MIGRATION Task 6): J1 holds with equality, so the stock has a unit root (it
keeps what a transient leaves, as decided: inventories are untargeted in P2) while J2 to J4 hold,
kappa1 at 0.10 to 0.30 of its limit, and prices and output return to the steady state (spectral
radius of the (x, q) block 0.29 to 0.93; 0.72 to 0.97 at kappa1 0.02). The
derivation agrees with the fixture: kappa1 0.2 lies beyond the theta-10.57 limits and oscillates;
kappa2 0.05 at kappa1 0.02 lies outside in the default mode for goods 1 to 6 and inside
frictionless, and the fixture overflows in the first and decays in the second (Deviations).
`price_rule_stability.rs` checks the conditions against a toy market run through
`Markets::clear` and `Markets::reprice` (10 pairs inside and 10 outside by at least 10% in each
coordinate). This region, for the rule as built and with E from theta_g + 1 down to sigma,
replaces the simplified 3 x 3 of the migration plan (owner ruling, MIGRATION Task 6): reading the
stock before clearing against a fixed target misclassifies that toy market both ways (at E =
13.55, theta 12.55 as before the OECD key, it calls (0.02, 0.06) unstable and (0.1, 0.06) stable).

On the 2000 pack (PLAN Task 20; `ceteris calibrate stability EXPORT` on a pack export).
The supply elasticities eps of the pack's traded enterprises run from 0.09 to 30.4 (medians 1.5 to
3.0 by good; fixture 0.90 to 2.61), since value-added shares go down to 0.04:

| Good | theta_g | s_g | kappa1 limit | kappa2 limit at kappa1 0.05 (default) | at kappa1 0.02 |
|---|---|---|---|---|---|
| 0 Agriculture | 10.57 | 3 | 0.150 / 0.094 | 0.097 / 0.062 | 0.033 / 0.104 |
| 1 Crude oil and gas | 10.57 | 1 | 0.124 / 0.053 | 0.026 / 0.004 | 0.009 / 0.041 |
| 2 Coal and other mining | 10.57 | 2 | 0.152 / 0.097 | 0.062 / 0.058 | 0.021 / 0.095 |
| 3 Food | 2.62 | 2 | 0.276 / 0.083 | 0.056 / 0.082 | 0.020 / 0.157 |
| 4 Light manufacturing | 10.57 | 2 | 0.118 / 0.048 | 0.055 / 0.000 | 0.019 / 0.053 |
| 5 Chemicals, refined fuels, metals | 10.57 | 2 | 0.125 / 0.054 | 0.055 / 0.008 | 0.019 / 0.061 |
| 6 Machinery, electronics, vehicles | 5.19 | 2 | 0.203 / 0.074 | 0.056 / 0.050 | 0.020 / 0.113 |

In the default mode the default kappa1 0.05 is 0.18 to 0.42 of its limit. Frictionless it lies
outside for light manufacturing: J2 fails for the two light-manufacturing enterprises of São Tomé
and Príncipe (eps 30.4 against 2 / kappa1 - theta - 1 = 28.4; no other enterprise of any good is
above its limit). Their prices settle on a period-2 cycle (about 0.7% either side of the
reference, output alternating between about 32 and 48 thousand units, the merchant stock growing
by about 2,300 units a tick, which kappa2 = 0 leaves untargeted) in every frictionless run moved
off the steady state; the default mode damps it. Growth per tick of the largest price deviation
from the steady state, ticks 300 to 600 after the 1% perturbation, crossing lags as built, with
limited liability of enterprises (Task 20 fix round 1; `p2_expectation_sweep.rs`,
`pack_decay_by_gain`, logs in the Task 20 scratchpad, `r1/pack_decay*.log`; frictionless /
default): at the defaults (gain 0.006) 1.00000 (the cycle) / 0.99230, and 0.99007 / 0.99230
leaving out São Tomé's two provinces (`EXCLUDE=3545,3546`); at gain 0 1.00000 / 0.99280, and
0.98951 / 0.99280 without them (before limited liability, frictionless without them 0.99169 and
0.99148, the default mode the same). The grid (`p2_price_rule_sweep.rs`, `pack_grid`,
`r1/pack_grid.log`, largest deviation over every cell, growth and the deviation at 600 in
brackets): kappa2 0: kappa1 0.02: 0.9932 (4.6e-4) / 0.9935 (5.5e-4); 0.05: 1.0000 (9.7e-3) /
0.9923 (4.3e-5); 0.1: 1.0000 (0.43) / 0.9962 (1.5e-5); 0.2: 1.0000 (1.1) / 1.0003 (0.46)
(frictionless first stopped at tick 202 on a false inventory violation, since fixed; rerun with
the write-off, `r3/pack_grid_k1_0.2.log`). kappa2 0.05: kappa1 0.02:
1.0016 (0.12) / ledger overflow; 0.05: 1.0001 (0.14) / ledger overflow; 0.1: 1.0010 (0.56) /
0.9980 (0.096); 0.2: 0.9997 (1.7) / 1.0003 (0.50). The pack's small value-added shares shrink
the region that the fixture drew: every kappa2 0.05 point and kappa1 0.2 leave it, and at
kappa1 0.1 the frictionless deviation does not decay. The table above is
`r1/stability_table.log`.

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
| Inventory sign | S >= 0, and the market account holds S | exact; holding to 1e-9 of the turnover S + Q + D (Task 20 round 3) | every tick |
| Price bounds | Prices finite and positive | exact | every tick |
| Two-level vs single-level | On small worlds, flows from the gateway kernel versus direct-path kernel: median absolute log difference reported, and both nest in their GE solutions | report only; V1 tolerances apply to each | `tests/trade_nesting.rs` |
| Pipeline | goods in the transit accounts = goods in the pipeline, per good | 1e-9 relative | every tick |
| Thread independence | 1 and 8 threads, identical hashes | exact | P2 acceptance |

## Parameters

| ID | Name | Prior / value | Class |
|---|---|---|---|
| P01 | Trade elasticity theta_g per traded good, in classes (ADRs 0018 and 0023) | 2.62 to 10.57, the range of the class values (widened from 4 to 8, ADR 0018). Values: class 0 (food) 2.62, class 1 (machinery, electronics and vehicles) 5.19, class 2 (agriculture; crude oil and gas; coal and other mining; light manufacturing; chemicals, refined fuels and metals) 10.57, derived by the ADR 0023 rule from the OECD concordance. Caliendo and Parro (2015, Table 1 p. 18, 99% sample) sector estimates averaged over each good's HS lines with 2000 BACI trade weights (9.11, 13.53, 13.53, 2.62, 9.15, 10.57, 5.19 for goods 0 to 6), grouped into the fewest classes within a factor 1.5, each at its goods' trade-weighted mean; local goods 0. Direct PPML estimate recorded as a diagnostic, ADR 0022 (2000 BACI, S05 costs, on the classes of the time: food; goods 4 and 6; goods 0, 1, 2, 5; values kept, owner 2026-10-24): 13.36 (s.e. 1.24), 13.20 (1.42), 11.82 (1.02); 8.4 to 9.4 with contiguity, language and colony controls | Literature |
| P02 | Inventory target coverage s_g | 1 to 6 months by good, start: agriculture 3, oil and gas 1, coal and mining 2, food 2, manufactures 2; local goods 0 | Free |
| P03a | kappa1 | 0.01 to 0.5 per month, start 0.05 (prior widened from 0.05 by owner decision, for stability; start 0.02, then 0.05 by owner decision 2026-10-24 so the expectation loop of ADR 0019 passes V1, MIGRATION Task 6) | Free |
| P03b | kappa2 | 0 to 0.5 per month, start 0: inventory term off, inventories untargeted in P2 (prior widened from 0.05 by owner decision) | Free |
| P03c | Price and wage step clip c_max | 0.1 log points per month; every tick it binds is counted per market and per wage (`clip_ticks` in `run_prices` and `run_wages`) | Free (numerical) |
| P04 | Mode costs | geo SPEC (direct PPML estimate recorded as a diagnostic, ADR 0022: not identified) | Literature + calibrated |
| P05 | Border friction nu, ad valorem equivalent | log-uniform 0.1 to 1.0, start 0.3. Direct PPML estimate recorded as a diagnostic, ADR 0022 (2000 BACI with ICIO domestic flows; value kept, owner 2026-10-24): 0.204 (s.e. 0.035) at the same-fit thetas (11 to 14); about 3.0 for food at the class value 2.62 | Calibrated (P9) |
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
(`ceteris-reference`, econ SPEC) on the same kernel. V1: frictionless runs reach it within
1%. H2: PPML on simulated country-pair flows (the origin value of the shipped units) gives a distance
elasticity in -0.7 to -1.1, and per
theta class, a PPML on ln(1 + tariff) of the flows valued at what the importer pays (value times 1 + t:
the kernel carries (1 + t)^-theta on spending, so the origin value's tariff elasticity is theta + 1) recovers the class's theta within its prior band (ADR 0018,
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
  wraparound; origin clearing, rationing classes (`mixed_rationing_matches_definition`: S11 Mixed
  on a hand-computed two-class shortage, equal to Priority without essential consumption), price
  clip, zero-supply floor; expected
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
- Benchmarks (criterion, `cargo bench -p ceteris-econ`): `source` and `dispatch` on the full pack (7
  traded goods; `source_dispatch` budget 8 ms per tick on the M4 Pro, re-budgeted from 5 ms by the
  owner on 2026-10-26, measured 7.9 ms; 19.2 at rev 226), `clear_reprice` (budget 2 ms),
  `kernel_build` (not per tick; budget 2 s).
  After the stored reciprocals and the log-domain weights (ADRs 0093, 0097; M4 Pro, AC, 98 W, full
  pack from its calibrated steady state, criterion medians, 2026-10-26), with the serial dispatch
  per good: `source_dispatch_parallel` 7.73 to 7.80 ms in three runs and 8.24 ms in one, at the
  8 ms budget; `unit_source_dispatch_parallel` 6.1 to 6.5 ms; a whole tick 35.1 to 35.2 ms in
  three runs and 38.4 ms in a noisy fourth, just over the 35 ms budget; one good's dispatch 4.8
  to 6.1 ms (bimodal by the core it lands on). The gather dispatch by target measured 10.3 to
  10.4 ms and 44.4 to 44.7 ms in the same alternating rounds and was removed (ADR 0097, As
  built). Measured 2026-10-27 in alternating rounds against the fused multi-good pass:
  `source_dispatch_parallel` 7.70 to 7.78 ms with the scatter, 28.3 to 28.7 ms with the fused
  pass, which was not kept.
  Re-measured on the branch head (2026-10-27, AC, 98 W, no other job; two rounds, each a
  separate criterion run): `source_dispatch_parallel` 8.54 and 8.41 ms, which missed the 8 ms
  budget in both runs (5 to 7% over; the earlier runs on 2026-10-26 and 2026-10-27 gave 7.7 to
  7.8 ms); a whole tick 37.3 and 35.5 ms, which missed the 35 ms budget in both runs;
  `unit_source_dispatch_parallel` 6.55 and 6.56 ms.

## Open questions

- Gateway count and selection rule are an agent default (22.2); the rule is in the econ SPEC
  pack section. As built (Task 17, controller ruling): 600 gateways (287 ports, 168 airports,
  145 land crossings). Each country's land crossing points, the merged endpoints of its
  cross-border road and rail edges, join its ports and airports as candidates of the k-medoids,
  one count rule for all; a crossing is not a gateway pair of its own, which would have made
  about 2,700 gateways. Every pair of gateways of different countries is a crossing leg: 356,080
  per traded good, beside 265,558 domestic and 20,705 access legs. That count sets the cost of
  `source` and `dispatch` (Benchmarks above). Task 19b tried to prune crossings by contribution
  and kept nothing: within a country pair every crossing's tau is within about 1.3 times the
  best and each live one carries at least 1% of its pair's largest flow, only the 10,710
  crossings to countries without an economy province are exactly negligible (about 0.25 ms), and
  the 5 ms budget needs about 190,000 crossings, which moves Phi_d by 50% to 95%. Owner decision
  (2026-10-26): the budget moves to 8 ms and the gateway count and rule move to the ADR 0092
  rework after the P2 gate. Once the gateway masses (ADR 0092) land with the border friction fit,
  access no longer grows with the number of gateway pairs and the count can be set for speed.
  Measured without the masses (ADR 0097, As built, 2026-10-27): 300 gateways by the same rule
  halve parallel source and dispatch (4.7 against 9.7 ms, cold, unit spending) but move ln Phi_d
  by medians of 0.03 to 2.0 by good, and the 300 pack does not calibrate (100 passes), so H2 was
  not run there; at 600 H2 distance is -0.753. No count is proposed before the masses land.
- Expectations are adaptive only in P2 (S04): one pi^e for the numeraire area (ADR 0019); price
  sourcing still uses posted prices (naive). Heuristic switching (12.2, P19) arrives with the
  expectations module.
- Freight capacity and endogenous freight rates (8.4) are P8.
- Inventories are untargeted in P2 (kappa2 = 0): stocks keep what a transient leaves (4.7 to 47 times
  target cover on the fixture after 1,200 ticks from the data state). Revisit with
  expectations (12.2), which can make an inventory target stable, or with P9 calibration.
