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
| Kernels per (theta class, weight class) (ADR 0018) | Kernels per traded good, each good at its theta class's value (4 classes, ADRs 0023 and 0101); `TradeParams.theta_class` carries the map | In P2 each good has its own weight w_g, so weight classes are goods and theta classes add no kernels while goods number 7 (ADR 0018, Mapping to P2 as built) |
| Utilities, construction and services traded through demand frictions (7.1) | Not traded between provinces in P2 | Province-local markets; cross-province service frictions arrive with P8 |
| Walrasian clearing mode (S01) | Deferred to P3 | V1 uses the stock-flow rule converging to the GE reference, which is the stronger test |
| Buyers price goods at the posted price (8.2) | Buyers of a traded good pay q_d = Gamma_d,g P_d, the CES index P_d = p_min Phi_d^(-1/theta) times a constant per cell fixed at the calibration point, where it makes q_d the delivered unit price of the physical arrivals, 1 / alpha_d (ADR 0200; owner ruling 2026-10-30, option A). Every route carries the selection multiplier s_r = c_r / q_d, c_r = p_o (1 + t) tau_r its physical delivered cost, so a unit of spending arrives as 1 / q_d units whatever its origin | Eaton-Kortum: a destination buys from each origin only the goods that origin is best at, so the price paid does not depend on the source and is the index (ADR 0200). The earlier rule, q_d = 1 / alpha_d at every price (owner decision, 2026-10-24), priced each origin's goods at its average cost: a rise in a border cost could lower q_d, and the ACR welfare formula failed (benchmark of 2026-10-28). Gamma per cell keeps the ADR 0089 steady state, and d ln q_d = d ln P_d away from it |
| Merchant stocks targeted by coverage (8.2) | Enterprises order up to an inventory position (stock plus goods on order) of (1 + cover + L) months of use, cover = 1, L the sourcing-weighted expected months in transit of the input (owner decision, 2026-10-24); see the econ SPEC | A fixed two-month position that counts goods on order cannot hold a month in transit: importers bound at the equilibrium and V1 failed on the island (MIGRATION Task 3) |
| Rationing within the tick after Pichler et al. (2022): priority by default (households' essential consumption, then intermediate users by contract, then the rest), proportional and mixed as S11 (8.2, 17.2) | S11 is `EconParams.rationing`, shared between intermediate users (index 0) and households (index 1) of what a market has: S + Q at an origin market (T3), the output left after the same-tick chain at a local market (T3; econ SPEC, Production). With A available and orders D_int, D_hh, D = D_int + D_hh, every class is filled when D <= A; otherwise: **Proportional** fills both at A / D. **Mixed** fills intermediate users at min(1, A / D_int) and households at max(A - D_int, 0) / D_hh (1 without household orders). **Priority** (default) serves households' essential consumption first, then intermediate users pro rata, then the rest; P2 has no essential consumption levels (they arrive in P4), so their P2 value is 0 and Priority fills as Mixed (`mixed_rationing_matches_definition`). Inside the same-tick chain intermediate users of a local good are served before households under every setting: the chain runs at E2, before household demand is posted (ADR 0020, Mapping to P2 as built) | Sources. Pichler, Pangallo, del Rio-Chanona, Lafond and Farmer (2022, JEDC 144, 104527, Section 2.2.2, eqs. (15) to (17), p. 8) use proportional rationing only: industry j delivers O_ji x_j / d_j to industry i, households receive c^d_i x_i / d_i; for other rules they refer to Pichler and Farmer (2021). That paper (Economic Systems Research 34(3), 2022, 273-293; read as arXiv 2101.07818v2, Section 4.2) defines (a) strict proportional rationing (Algorithm 1, eqs. (7) to (11), p. 8: r_i = x_i^max / d_i for all customers) and (b) mixed proportional/priority rationing (Algorithm 2, eqs. (12) to (16), p. 9): intermediate customers are rationed pro rata on r_i = x_i^max / sum_j a_ij d_j (eq. 12), so final demand plays no part in the bottleneck, and final demand takes the residual f_i = max(x_i - sum_j a_ij x_j, 0) (eq. 15). Mixed above is (b) with x_i^max = A, Proportional is (a). Their (c), priority as largest customer first, is not built: DESIGN 8.2 names its own priority order |
| Stock-flow price rule at kappa1 0.2, kappa2 0.1 (8.2) | Inventory term off by default (kappa2 = 0) and slower price speed (kappa1 = 0.05) (owner decisions 2026-10-24: kappa1 0.02 for stability, then 0.05 with expected inflation, MIGRATION Task 6) | The derived region (Stability region, below; ADR 0019) puts kappa1 0.2 outside for the theta-10.57 goods and kappa2 0.05 outside in the default mode. At kappa1 0.02 the world price level returns too slowly for the expectation loop: every P45 gain in the prior broke V1 (0.006: 1.02e-2) or the ledger (0.25 and up); at 0.05 with gain 0.006 V1 is 3.14e-5 and a small deviation decays by 0.99644 / 0.99676 per tick (frictionless / default; the blessed fixture after MIGRATION Task 8, reproduced by `p2_expectation_sweep.rs`; 2.4e-5 and 0.9963 / 0.9969 when measured at Task 6). Growth factor per tick of a small deviation from the fixture's GE equilibrium, ticks 300 to 600, at the per-good thetas (MIGRATION Task 6, pi^e = 0; frictionless / default): kappa1 0.02, kappa2 0: 0.9954 / 0.9954, the same with every crossing lag at 3; kappa1 0.05: 0.9862 / 0.9871; 0.1: 0.9718 / 0.9756 (lag 3 the same); 0.2: 1.0000 / 1.0000 (oscillates, lag 3 the same). kappa2 0.05: kappa1 0.02: 0.9876 / ledger overflow (lag 3: 0.9881 / 0.9998); 0.05: 0.9738 / 1.0014; 0.1: 0.9999 / 0.9978; 0.2: 1.0000 / 1.0000. The stock integrates the gap and feeds the money and trade-balance loop (Task 7b) |

## State components

| Component | Entity | Type | Units | Initialized from |
|---|---|---|---|---|
| Price p | (province, good) | `f64` | numeraire per unit | 1.0 (2000 world reference price) in the data state; the calibrated posted price in the calibrated steady state (econ SPEC, Initial state; ADR 0089) |
| Inventory S | (province, traded good) | `f64`, also held in the ledger | units | s_g x the origin's own base output Y0 in the data state; the calibrated S = s_g Q in the steady state (econ SPEC, Initial state) |
| Last demand D (orders by class at origins), last supply Q, last sales | (province, good) | `f64` | units per month | zero; the calibrated D = Q = sales in the steady state |
| Clip ticks: ticks at which the price step clip bound | (province, good) | `u32` | ticks | zero |
| Expected inflation pi^e and the world price index I of its last update | world | `f64` | log points per month; numeraire | 0 and the index at the initial prices (the data state's, or the calibrated steady state's) |
| Buyer price q_d (traded) or posted price (local) | (province, good) | `f64` | numeraire per arriving unit | sourcing at the initial posted prices (the data state's, or the calibrated steady state's) |
| Selection constant Gamma_d,g (ADR 0200) | (province, traded good) | `f64` (`ProvinceData::gamma`; static) | | the data state's 1 / (alpha_d P_d) at prices 1, or the calibration tables' value at the calibrated prices; 1 where no origin is reachable |
| Trade kernel legs | domestic pairs, province-gateway legs, gateway pairs, per traded good | `f64` K = tau^-theta_g and the reciprocals 1 / tau, 1 / (tau (1 + nu)), 1 / (1 + t), CSR | | least-cost leg costs (S05; route-choice factors from `Network.route_taus` are off in P2, ADR 0017) at load; rebuilt when the graph version moves |
| Transit lag | gateway pair | `u8` | months | gateway-pair hours |
| Pipeline: goods paid for and in flight | (delivery slot 1 to 4, traded good, buyer), stored good-major (ADR 0332; snapshot bytes buyer-major, ADR 0333) | `f64` | units | empty; in the calibrated steady state, j ticks ahead one month of the shipments of every arrival slot j or later |
| Tariff and border wedge | (importer country, exporter country, traded good) | `f32` | ad valorem | border friction P05 plus 2000 tariffs (econ pack) |
| Market, gateway, transit accounts | ledger | money `i64`, goods `f64` | | opened at load |

A unit of good g is the quantity worth one 2000 US dollar at the world reference price. Goods
are innermost in every goods-indexed array (19.4). The goods count is a pack value, not a
compile-time constant: `Goods` (`ceteris_trade::goods`) holds the rows of `econ_goods` (names,
the traded prefix, storable flags, tariff groups, PIGL categories), and every goods-indexed
array is sized from it. Traded goods come first, `0..traded`, local goods after. `TradeParams`
carries its `Goods`, and its per-good vectors have one entry per good (`check_goods`). P2 has 10
goods, 7 traded (`goods::p2`), which the fixture and the 2000 pack use.

## Equations

Symbols: o origin province, d destination province, a and b gateways, A and B countries,
g good, theta = theta_g = P01 of good g (its class's value, ADR 0023) times P150 (1 by default), w_g weight per value (tonnes per unit, econ pack), c path cost per tonne
(`ceteris-geo`).

**Leg factors.** Two sources, chosen by the network (`Network.route_taus`):

- Least-cost legs (S05; the P2 default, the fixture and the pack): for a leg with least-cost path cost c (USD per tonne)
  and transit time T (days, hours along the same path / 24),
  `tau_leg,g = 1 + c * w_g + lambda_g * T` (iceberg; geo SPEC mapping at p = 1).
  *Bulk freight rate (ADR 0247).* For a good of a theta class c with P154 `bulk_sea[c]` or P155
  `bulk_port[c]` other than 1, c and T are those of the least-cost path on the graph repriced
  with the sea cost per tonne-km (P04c, sea and canal edges) times `bulk_sea[c]` and the listed
  and minor port fees (P04m) times `bulk_port[c]`, everything else at P04 (`Network.bulk`, legs
  with the same endpoints in the same order as the base lists, built by
  `EconInputs::network_from_graph`). On by default (owner ruling 2026-10-30; ADR 0247, As built):
  class 3 (goods 1 and 2) uses the bulk legs at the sourced values, classes 0 to 2 the base
  legs. Every class off 1 must share one pair of multipliers. The fixture's hand-built network
  builds its bulk legs itself (sea crossings at P154 times its sea cost per tonne-km; it has no
  ports, so P155 scales nothing); hand-built test networks without bulk legs and the fixture's
  unreachable island (route taus) run with the rate off (`TradeParams::bulk_off`); the island
  folds the bulk legs into its route taus for the goods of a bulk class. A network
  with bulk legs and route taus is rejected. `BulkLegs`
  records the multipliers it was built with, and the kernel rejects parameters whose bulk rate
  (classes, P154, P155) differs from the network's, so P154 and P155 take effect only through
  the network built at load. Tests:
  `bulk_legs_price_the_bulk_classes`, `bulk_rate_must_match_the_network` (kernel),
  `bulk_rate_rejects_nan`, `bulk_rate_is_on_for_class_3_by_default` (params), `bulk_rate_reprices_sea_and_port_fees` (geo),
  `bulk_legs_follow_the_bulk_paths`, `bulk_rate_can_switch_the_path` (econ pack),
  `routing_matches_rust` (reference CLI).
- Least-iceberg legs (ADR 0279, accepted 2026-11-01; built, off until its ablation test passes:
  `TransportParams::path_choice = Iceberg`, read from the graph's parameters by
  `EconInputs::network_from_graph`): per traded good, the leg's path minimises
  `sum_e (w_g c_e + lambda_g h_e / 24)`, so `tau_leg,g = 1 + that minimum`; the hours come from
  the same path. Domestic, access and crossing legs all use it. Goods with equal
  `lambda_g / w_g` and the same bulk freight rate share one leg set (`Network.good_legs`:
  `GoodLegs::set` names each traded good's set in `GoodLegs::sets`, one entry per traded
  good (P3 G2: the traded prefix of the pack's goods);
  each set has the base lists' endpoints in their order, since the weight changes no edge's
  reachability). A bulk class's goods take their trees on the graph at the bulk rate, so the
  kernel's bulk check is unchanged. The kernel prices a good on its set when `good_legs` is
  present, else as above; route taus and good legs together are rejected. The inputs digest
  appends the sets only when present. `ceteris_calibrate::costs::country_costs` follows
  `Transport::path_choice` (default: `TransportParams::default().path_choice`) with the same
  per-good weights, so the gravity sample sees the kernel's paths. The export's `legs` table
  keeps the base and bulk legs only; the reference reads the kernel tables, which carry each
  good's factors. A graph repriced at other parameters takes their `path_choice`. The freight-cost trees stay for the map mode. Tests:
  `iceberg_flies_light_goods_and_ships_heavy_ones`, `iceberg_at_zero_time_cost_is_freight` and
  `iceberg_tree_is_least_weight` (geo paths), `good_legs_price_each_good_on_its_set`,
  `good_legs_must_match_the_time_costs` and `good_legs_must_match_the_base_legs` (kernel),
  `iceberg_legs_price_each_good_on_its_paths`, `iceberg_at_zero_time_cost_is_freight` and
  `iceberg_legs_are_thread_count_independent` (econ pack),
  `iceberg_country_costs_follow_the_kernel` (calibrate gravity).
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
K_od = sum over a in G(A), b in G(B) of  K_oa * m_a * K_ab * m_b * K_bd
K_ab = (tau_ab * (1 + nu_c) * (1 + t_AB,g)) ^ -theta
```

m_a is the gateway's mass (ADR 0092), its share of its country's 2000 trade throughput, so the
masses of a country's gateways sum to 1 (`Network.gateway_mass`, checked to 1e-12 at
`Kernel::build`): access no longer grows with the number of gateway pairs, and duplicating a
gateway with half its mass on each copy changes no flow (`duplicating_a_gateway_with_half_mass_changes_no_flow`,
1e-12). The pack has no throughput per gateway, so every gateway of a country with k gateways
takes 1/k. The masses fold into the crossing factor, k = m_a m_b K_ab, so the low-rank
evaluation and the stored reciprocals are unchanged. nu_c is the border friction of good g
(P05, one per good, `TradeParams::border_nu_of`): its theta class's value, except in class 3,
where each good has its own (ADR 0276). Factors are f64: an f32 ln K resolves K only to 1.5e-5 at tau 1e4 and theta 64.85
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
P_d  = p_min * Phi_d^(-1/theta)       (CES index, tariff included)
```

K carries the tariff on crossings, so X is what d pays: origin o is ordered X / (p_o (1 + t))
units and ships them, and its route melts them by the physical tau (iceberg, and the border loss
on crossings). The home route (o = d, K = 1) is part of the sum. Buyers pay the CES index
(ADR 0200, selection on trade links):

```
q_d = Gamma_d * P_d                   (buyer price, tariff included)
s_r = p_o (1 + t) tau_r / q_d         (selection multiplier of route r from o to d)
```

A shipped unit arrives as s_r delivered units, so every route delivers X / q_d units and a unit of
spending buys 1 / q_d units whatever the origin: in Eaton-Kortum a destination buys from a far
origin only the goods that origin is best at, so the price paid is the same for every source.
The gain is real output of the origin's best producers, which need 1 / s_r of the cell's
average inputs per delivered unit (econ SPEC, Production); the dispatch books it as the
selection term, delivered units less the units that survive the melt, beside the melt itself.
It is negative on a route whose physical cost is below q_d (the kernel cost also carries the
time cost, the border friction nu and the gateway masses, which melt nothing).

Gamma_d,g is fixed per cell where the world was calibrated: there q_d equals the delivered unit
price of the physical arrivals, 1 / alpha_d with alpha_d = sum over routes of X / (E_d p_o (1 + t)
tau), the buyer price before ADR 0200. In the data state that is 1 / (alpha_d P_d) at prices 1
(`world::base_delivered_prices`); a calibrated world reads it from the calibration tables (econ
SPEC, Calibration). Away from that point q_d moves with P_d alone, so d ln q_d = d ln P_d and the
ACR welfare formula holds exactly. No pow is needed: the dispatch carries value by arrival slot
through the gateways and divides by q_d at the destination. (ADR 0200 first wrote s_od =
Gamma^-1 (K_od / pi_od)^(1/theta); that is p_o / P_d, without the trade cost; errata in the ADR.)
The arrival slots (0 home, 1 domestic or a lag-0 crossing, 2 to 4 a crossing with lag 1 to 3)
weighted by delivered units, which is by value, give the expected lead L_d,g in months.

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
write-off applies to it. At a steady state sales equal Q and the cash flow is 0, except at the
origins of competitively stored goods, whose output also replaces spoilage: sales are Q / (1 +
delta_g s_g) and the cash flow is -p delta_g S (Competitive storage below).

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
dlnp = pi^e + clip( kappa1_cell (D - Q)/Q  -  kappa2 (S' - S*)/S*,  -c_max_g, c_max_g ),   S* = s_g D
```

kappa1_cell is kappa1 (P03a) capped per market at the share m (P03d) of its stability limit (ADR
0263; Stability region, Per-market gain) and at the share m of its coupled block's limit gain
(ADR 0267; Stability region, Block gain): kappa1 itself in every market of inputs without a
calibration.

The clip bounds the gap terms only (ADR 0106): expected inflation passes through unclipped, so a
sustained inflation or collapse is bounded by the expectation rule, not by the numerical guard.
c_max_g is c_max (P03c, 0.1) for most goods and c_max_storable (P151, 0.3) for the storable
commodities, agriculture, crude oil, gas and refined fuels, coal and other mining, and chemicals
and metals (goods 0, 1, 2, 5; `goods::STORABLE`), whose monthly price moves in the data exceed
0.1 (oil fell about 30% in one month in late 2008).

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
GE reference is unchanged. Every tick the clip binds on the gap terms at a cell is counted
(`clip_ticks`, run export).

**Competitive storage (ADR 0091)** replaces the stock rule for the storable commodities, goods 0,
1, 2 and 5 (`goods::STORABLE`), where it is on (`TradeParams::stores`: psi or delta positive;
a positive delta with psi 0, a stock that spoils with nothing targeting it, the slowly growing
root of the Storage loop below, is refused by `TradeParams::check_storage` in `EconWorld::new`
and in the calibration, as is a stored good without a positive cover s, zeta or k + delta, by
which the order of ADR 0280 divides). It
is off by default (owner ruling 2026-10-31; since g_s 0.006 the approved values pass the checks of
the Storage loop below, and turning storage on is the owner's call), so P144 and P56's delta
default to 0 and those goods run the stock rule, bit for bit as before ADR 0091, kappa2 included; `TradeParams::with_storage` sets the approved values. Each such origin market keeps an expected log-price change
pi^e_og, 0 at the start of a run and at a steady state, updated after its own T4 step:

```
pi^e_og <- pi^e_og + g_s ( dlnp_og - pi^e_og )        (P146; dlnp_og the step of the cell, pi^e included)
```

At T3, before clearing, the stock S carried into the tick loses delta_g S (P56, destroyed in the
ledger and tallied as spoilage), and the merchants place a storage order against this tick's
buyer orders D = D_int + D_hh, a partial adjustment toward a target stock (ADR 0280, amending
ADR 0091; Lovell 1961):

```
S*_og    = s_g D max( 0, 1 + pi^e_og / (zeta_g (k_g + delta_g)) )          (P02, P56 k_g and delta_g, P145)
b_og     = max( delta_g S + lambda_g (S*_og - S),  -(1 - delta_g) S )
lambda_g = psi_g zeta_g (k_g + delta_g) / s_g                                (P144)
```

S\* is the stock at which ADR 0091's expected net return of holding, pi^e + (k + delta)(z^-zeta -
1) with z = S / (s_g D) the cover and (k + delta) z^-zeta the convenience yield, is zero to first
order in pi^e (Working 1949); at pi^e = 0 it is P02 cover s_g D. lambda_g is the monthly
stock-persistence rate 1 - rho of Gorton, Hayashi and Rouwenhorst (2013) from which P144 psi is
derived (DESIGN 17), 0.144, 0.15, 0.05 and 0.067 for goods 0, 1, 2 and 5 at the approved values:
no new parameter (`TradeParams::storage_target`, `storage_adjustment`). The order is linear in S
and D, so its response to buyers' orders is lambda s_g (1 + pi^e / c) everywhere, where ADR
0091's first rule, delta S + psi D x, had psi (x + c z^-zeta), unbounded as cover falls (ADR 0280
Context). The floor is the stock left after spoilage: merchants never offer more than they hold,
so S' >= 0; at lambda < 1 it does not bind. With no buyers (D = 0) S\* = 0 and merchants release
lambda S a month. The storage order is an order at the origin: the price step reads D + b in
place of D,

```
dlnp = pi^e + clip( kappa1_cell (D + b - Q)/Q, -c_max_g, c_max_g )
```

and the kappa2 term of the stock rule is off for these goods. Physically the market's stock
stays what it does not sell (S' = (1 - delta) S + Q - sales, buyers served first by S11): the
order is filled through the price, so at a steady price D + b = Q and S' - S = lambda (S\* - S)
exactly.
A release (b < 0) is supply the price rule sees; a purchase is demand. At a steady state pi^e = 0
and S = S\* = s_g D, so b = delta S, and output covers sales and spoilage: Q = (1 + delta_g s_g) D,
the market's cash flow p (sales - Q) = -p delta S each tick, passed to the country's profit pool
(Market cash flow above). The calibration and the GE reference hold this point (econ SPEC,
Calibration and GE reference): origins sell Q / (1 + delta_g s_g) and stocks are s_g D. With
psi = delta = 0 (the default) storage is off and the stock rule applies.

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
Q\* = D\*, S\* = s_g D\*) with pi^e = 0 (the clip acts on the gap terms only, ADR 0106, so a
constant pi^e shifts every price by the same log step and leaves the region unchanged): x = ln p - ln p\*, q the output deviation (Q - Q\*)/Q\*,
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
| 0 Agriculture | 7.68 | 3 | 0.222 / 0.191 | 0.152 / 0.141 | 0.179 / 0.172 |
| 1 Crude oil, gas and refined fuels | 16.91 | 1 | 0.110 / 0.102 | 0.040 / 0.037 | 0.016 / 0.058 |
| 2 Coal and other mining | 16.91 | 2 | 0.110 / 0.101 | 0.048 / 0.044 | 0.040 / 0.070 |
| 3 Food | 2.91 | 2 | 0.473 / 0.349 | 0.360 / 0.321 | 0.040 / 0.353 |
| 4 Light manufacturing | 7.68 | 2 | 0.222 / 0.191 | 0.142 / 0.130 | 0.040 / 0.158 |
| 5 Chemicals and metals | 2.91 | 2 | 0.458 / 0.307 | 0.109 / 0.302 | 0.031 / 0.337 |
| 6 Machinery, electronics, vehicles | 4.14 | 2 | 0.366 / 0.287 | 0.265 / 0.240 | 0.040 / 0.270 |

Limits are the first failing value on a 0.0005 grid (`ceteris calibrate stability` prints the rows from the fixture export).

The default (kappa1 0.05, kappa2 0) lies on the edge kappa2 = 0 of the region and counts as
inside (owner ruling, MIGRATION Task 6): J1 holds with equality, so the stock has a unit root (it
keeps what a transient leaves, as decided: inventories are untargeted in P2) while J2 to J4 hold,
kappa1 at 0.11 to 0.50 of its limit (the top at theta 16.91; 0.10 to 0.30 at the P01 values
before ADR 0101), and prices and output return to the steady state (spectral
radius of the (x, q) block 0.29 to 0.93; 0.72 to 0.97 at kappa1 0.02, at the earlier values). The
derivation agreed with the fixture at the P01 values before ADR 0101 (10.57, 5.19 and 2.62),
where the rest of this paragraph was measured: kappa1 0.2 lies beyond the theta-10.57 limits and oscillates;
kappa2 0.05 at kappa1 0.02 lies outside in the default mode for goods 1 to 6 and inside
frictionless, and the fixture overflows in the first and decays in the second (Deviations).
`price_rule_stability.rs` checks the conditions against a toy market run through
`Markets::clear` and `Markets::reprice` (10 pairs inside and 10 outside by at least 10% in each
coordinate). This region, for the rule as built and with E from theta_g + 1 down to sigma,
replaces the simplified 3 x 3 of the migration plan (owner ruling, MIGRATION Task 6): reading the
stock before clearing against a fixed target misclassifies that toy market both ways (at E =
13.55, theta 12.55 as before the OECD key, it calls (0.02, 0.06) unstable and (0.1, 0.06) stable).

On the 2000 pack (PLAN Task 20; `ceteris calibrate stability EXPORT` on a pack export), at the
P01 values before ADR 0101 and with refined petroleum in good 5 (the pack is rebuilt and
recalibrated with the four classes in a later task, and this table with it).
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

Storage loop (ADR 0091). A competitively stored good adds the market's expectation pi^e_og as a
fourth state and replaces kappa2 by the storage order. Linearize around Q\* = D\* / rho, rho =
1 / (1 + delta s), S\* = s D\*, pi^e_og = 0, with c = zeta (k + delta), the slope of the
convenience yield at z = 1, and lambda s = psi c: b - delta S\* = D\* ( delta s sigma + psi pi -
psi c (sigma + E x) ), since S\* - S = s D\* (pi / c - E x - sigma) to first order. ADR 0091's
first rule, delta S + psi D x with z - 1 = sigma + E x, has the same first-order terms, so ADR 0280
leaves this map, J's storage factor, the psi limit and the block map as they were
(`partial_adjustment_keeps_the_storage_loop`, calibrate `tests/storage_loop.rs`: the toy market's
tangent map against `storage_matrix`). Following the tick as above, with pi the market's
expectation before the step and kappa = kappa1_cell:

```
q_t         = (1 - eta) q_{t-1} + eta eps x_t
e_t         = rho ( -E (1 + psi c) x_t + (delta s - psi c) sigma_t + psi pi_t ) - q_t
sigma_{t+1} = (1 - delta) sigma_t + (E x_t + q_t / rho) / s
x_{t+1}     = x_t + kappa e_t
pi_{t+1}    = (1 - g_s) pi_t + g_s kappa e_t
```

so the state (x_t, q_{t-1}, sigma_t, pi_t) moves by

```
      [ 1 - kappa A          -kappa B          kappa C           kappa rho psi             ]
M  =  [ eta eps              B                 0                 0                         ]
      [ (E + eta eps/rho)/s  B / (rho s)       1 - delta         0                         ]
      [ -g_s kappa A         -g_s kappa B      g_s kappa C       1 - g_s + g_s kappa rho psi ]
```

with A = rho E (1 + psi c) + eta eps, B = 1 - eta, C = rho (delta s - psi c). With psi = delta = 0
the pi row decouples (its root 1 - g_s) and the rest is the 3 x 3 above at kappa2 = 0. The stock
is now targeted: the storage order works as an inventory term of gain kappa rho (psi c - delta s),
so the unit root of the kappa2 = 0 edge leaves (for goods 0, 1, 2, 5 at the P144 to P146 values,
kappa rho psi c is 0.021, 0.0075, 0.005 and 0.0067 at kappa 0.05). The momentum loop through pi
has gain 1 - g_s + g_s kappa rho psi, near 1 where kappa psi is near 1 (agriculture at the
default kappa1: kappa psi = 1.0). Stability is the Schur-Cohn test on the characteristic
polynomial of M (`ceteris_calibrate::stability::storage_stable`; for a quartic, the Jury
conditions of the 3 x 3 in general form), and `stability_with_storage_matches_simulation`
(calibrate `tests/storage_loop.rs`, on `storage_matrix` itself) checks it against the toy market of agriculture run through
`Markets::store`, `clear` and `reprice`. The -1 root of the (x, q) block is J2 with E replaced by
rho E (1 + psi c): the storage order raises the static elasticity of the orders by psi c and
divides it by the spoilage share, so the Per-market gain below uses that J for these goods.

kappa1 limit of the storage loop at the approved values, g_s 0.006 (first failing value on the
0.0005 grid, over E in [0.6, theta_g + 1] and the fixture's eps; default mode / frictionless),
and psi limit (first failing psi on the 0.05 grid, both modes, over E in [0.3, theta_g + 1] and
the fixture's eps, each market at its Per-market gain with J carrying the storage response at
that psi; `ceteris_calibrate::stability::psi_limit`); `ceteris calibrate stability --storage`,
kept by `storage_table_of_the_fixture`:

| Good | theta_g | s_g | kappa1 limit with storage | psi limit, fixture eps |
|---|---|---|---|---|
| 0 Agriculture | 7.68 | 3 | 0.168 / 0.151 | 49.75 |
| 1 Crude oil, gas and refined fuels | 16.91 | 1 | 0.103 / 0.096 | 2.50 |
| 2 Coal and other mining | 16.91 | 2 | 0.102 / 0.095 | 25.60 |
| 5 Chemicals and metals | 2.91 | 2 | 0.424 / 0.295 | 28.70 |

The default kappa1 0.05 is at most 0.52 of every limit. det(I - M) = c eta g_s kappa psi (E + eps)
/ s is positive for positive parameters, so no real root crosses +1: the storage loop leaves the
region through a slow complex pair, the stock-price cycle that the momentum drives (loop gain
kappa rho psi, 0.99 for agriculture at kappa1 0.05). g_s scales that loop and leaves the stock's
pull psi c alone. At the earlier g_s 0.08 agriculture's limit was 0.054 / 0.170, and over the
pack's eps range (0.09 to 30.4) agriculture at E near 0.6 and low eps was outside (radius 1.003).
At g_s 0.006, over the pack's eps range, the limits are 0.121 / 0.0505 for agriculture, 0.083 /
0.0425 and 0.082 / 0.041 for goods 1 and 2 and 0.212 / 0.0595 for good 5; every frictionless
limit there comes from eps 30.4 at J above 36, the -1 side that the Per-market gain caps. With the
cap applied, over E down to 0.3 (the island's effective E, below) and the pack's eps, psi's limit
is 43.5, 2.1, 22.35 and 26.5 for goods 0, 1, 2, 5 (14.75 for agriculture at g_s 0.08), so the
approved psi sit at 46%, 48%, 16% and 22% of it, each within the share m = P03d
(`psi_limits_of_the_approved_storage_values`). Agriculture's limit binds in the default mode at
E 0.3 and eps 30.4, where J is 5.9 and the market runs at kappa1 0.05 uncapped: a complex pair
at about 40 degrees crosses the unit circle (`psi_limit_is_the_first_failing_grid_value`).
That corner pairs the island's E with the pack's largest eps, which no single market of the
fixture has; at E 0.2 the limit is 42.5. Without the cap it is 23.9, from the frictionless -1
side at large J.

In the fixture world, from the GE reference perturbed by 1% (`small_perturbation_decays`), the
approved values with g_s 0.006 decay by 0.991 a tick frictionless and 0.992 in the default mode.
At g_s 0.08 the default mode grew by 1.0069 a tick (agriculture of the island province,
calibrated E 1.76 and eps 0.99, whose own income feeds back on its demand and leaves an effective
E of about 0.3, below the region's sigma floor); with psi = 0 the stock's spoilage term alone
grows by 1.0004 (nothing targets the stock). V1 from the data state fails at 1,200 ticks at
every value set tried (a 1.5% to 1.65% price gap, coal), from slow convergence, not instability:
prices are within 0.06% at 3,600 ticks but flows 1.5% off, 2.1% at 4,800, and 1.4e-4 at 6,000, so
V1's horizon is 6,000 ticks where storage is on (econ SPEC, P2 gate; owner, 2026-10-31). And the stockout claim of ADR 0091 did not hold as first written: on the toy
agriculture market a 30% cut held 12 months raises the price by 0.64 in logs (0.20 for the
surplus) but the stock stops at 0.23 of target, since the Working-Kaldor convenience yield z^-zeta
grows without bound as cover falls (ADR 0091's first rule; under ADR 0280's partial adjustment the
values are in Tests). Against Deaton and Laroque's benchmark, no stock and the
flow clearing each month (0.59 for that cut at E 0.6), the rule smooths: 0.10 for a 3-month cut,
0.23 for 6 months. The stock rule with storage off is not that benchmark, since at kappa2 = 0 its
price does not read the stock (B7 follow-up). ADR 0091's Tests were amended to these two claims
(owner, 2026-10-31).

Per-market gain (ADR 0263). Each cell's gain is
kappa1_cell = kappa1 min(1, J_ref / J_cell), J_ref = 2 m / kappa1 with m = P03d (start 0.5, so
J_ref 20 at kappa1 0.05; `TradeParams::price_gain`), J_cell = E_cell + eta eps_cell / (2 - eta) at
the run's eta (P41; 1 frictionless). E_cell = 1 + theta_g - (theta_g + 1) Pi_cell for traded
cells, Pi_cell the cell's share of its buyers' purchases weighted by the units it sells to each
(Leontief inputs, sigma_d 0; at buyer price Gamma P_d, ADR 0200, the own-price elasticity of
these orders is exactly E), computed per leg with the leg factors squared (two passes over the
kernel legs: one destination sum of the spending, one with squared leg factors; a pair joined by
several routes leaves out the cross terms between them; a traded cell that reaches no buyer with
spending has Pi 0, so E = 1 + theta_g); local cells E = sigma (P06). At the origins of the
competitively stored goods 0, 1, 2 and 5 E_cell carries the storage order's response, the factor
(1 + psi_g zeta_g (k_g + delta_g)) / (1 + delta_g s_g) of the Storage loop above, at the default
P56, P144, P145 and P02 of the run (`TradeParams::order_response`, `MarketElasticity::j`); on
the calibrated fixture with storage on the largest J is 14.3 in the default mode (15.6
frictionless), below J_ref, so no cell is capped per market; with storage off (the default) the
factor is 1. eps_cell is
the largest over the cell's enterprises of s_L sigma_FL / ((1 - s_L) a_VA) (ADR 0263, As built: the
enterprise maximum, which the ADR's numbers rest on). The calibration computes E_cell and eps_cell
at the calibrated prices and spending (`ceteris_calibrate::elasticity`) and writes them to
`econ_calib_cells.arrow` (since calibration format 6); `EconWorld::new` combines them with the run's
eta, once, and the gain stays fixed for the run (`EconWorld::set_network` keeps it, as it keeps
the calibrated coefficients; only a recalibration recomputes J). Inputs without a calibration
(the generated fixture, an uncalibrated pack) run kappa1 in every market; a data-state start of
calibrated inputs keeps their gains, as it keeps their coefficients, and is refused under
structural parameters other than the calibration's (`structural_digest`), as a steady start is.
On main's 0256 pack the study found no cell capped in the default mode (J at most 16.9; on the
fixture `no_cell_capped_in_the_default_mode`); this branch's pre-0256 scratch pack caps 383
traded cells there (J up to 32.4, Lesotho g1 on Senegal's donor good-1 column, a_VA 0.0213,
which 14 no-rent economies share and ADR 0256 does not replace), so the default-mode claim is
pending B8 and an owner ruling (ADR 0263, As built). Frictionless it slows the traded cells
above 20 (298 on the 0256 pack) and removes the São Tomé cycle below. J2 bounds one market only: coupled modes
across goods (Albania g5 to g1 and g2) are bounded by the block gain below (ADR 0267).

Block gain (ADR 0267). A country's markets move together through the tick: buyers substitute
between the country's origins of a good, its enterprises order inputs from its markets in
proportion to their output and to the months their inputs spend in transit, their output answers
their input costs and their wage, the wage answers their labour demand, and its households spend
the wages and the national profit pool on its traded and local goods. Linearize the tick of a
country around the calibrated steady state with pi^e = 0 and kappa2 = 0, as for J2: x_i is the
log price deviation of cell i (its active traded cells and its local cells with output), q_k the
output deviation of enterprise k, omega_p the log wage deviation of province p, W_p the relative
deviation of its wage bill, P the relative deviation of the country's profit pool, and a slot
s = (province d, traded good h) a place where the country's enterprises or households buy h. A
place r is a cell or a slot; its price is x_r for a cell and y_s for a slot. Following the order
of a tick (E1 at this tick's own price and last tick's buyer prices, the wage, E2, E3 at this
tick's buyer prices and money, T3, T4, then E4's profit pool, spent at the next E3):

```
y_s,t   = sum_(o -> s) a_os x_o,t                              buyer price of slot s at E3
q*_k    = eps_k x_own(k),t - sum_(places r of k) c_kr price_r,t-1          labour target
omega_t = sum_(k at p) L_k q*_k / sum_(k at p) L_k beta_k                  frictionless
omega_t = omega_t-1 + kappa_w sum_(k at p) L_k (q*_k - beta_k omega_t-1)   wage rule
q_k     = (1 - eta) q_k,t-1 + eta (q*_k - beta_k omega_p)    omega_t frictionless, omega_t-1 rule
W_p     = omega_p,t + sum_(k at p) L_k q_k / (1 - u*_p)
L_s,t   = sum_(o -> s) l_os x_o,t                              months in transit at E3
h_r     = sum_(r') S_rr' price_r',t + A_r W_p + B_r P_t-1 - S_r price_r,t   households at r
z_s     = sum_(k at s) v_ks (q_k + H_s (q_k - q_k,t-1) + L_s,t - L_s,t-1) + h_s
o_i     = - sum_j E_ij x_j,t + sum_(i -> s) u_is z_s + sum_(k draws i) d_ki q_k + h_i
e_i     = o_i - sum_(k own i) w_k q_k
          [+ rho_i ((delta s - psi c) sigma_i,t + psi pi_i,t)          at a stored good's origin]
P_t     = sum_i R_i (x_i + o_i / r_i) - sum_p B_p W_p
          - sum_(k, r) b_kr (price_r,t + q_k [+ H_s (q_k - q_k,t-1) + L_s,t - L_s,t-1 at a slot])
x_i,t+1 = x_i,t + kappa_i e_i
sigma_i,t+1 = (1 - delta) sigma_i,t + (sum_(k own i) w_k q_k / rho_i - o_i / r_i) / s   stored
pi_i,t+1    = (1 - g_s) pi_i,t + g_s kappa_i e_i                                    stored
```

with, at the calibrated point (`ceteris_calibrate::block`; w_o the origin weight, Phi_d the
destination's sum and t_i the destination sum of the Per-market gain, k_od the factor of the
domestic leg o -> d, 1 at home; every money term over the country's profit pool
sum_k (1 - s_L) a_VA V_k):

- E_ii = E_cell for a traded cell; for two origins i != j of one good g in the country
  E_ij = -(1 + theta_g) w_j T2_ij / t_i, T2_ij the sum over shared last legs of the products of
  the two origins' factors times spend_d / Phi_d^2: destinations both reach at home or over one
  domestic leg, and gateways both access, weighted by the squared routes beyond (the Pi_cell
  convention: routes that do not share their last leg leave out their cross terms). This is the
  elasticity of i's orders to j's price from buyers holding their delivered units at Gamma P_d,
  the off-diagonal of (1 + theta) (I - Pi). A local cell's own-price response comes from its
  households (h_i) alone.
- Slots and routes: an origin o reaches slot s = (d, h) at home or over one domestic leg of the
  country (routes through gateways join provinces of different countries and are left out);
  a_os = w_o k_od / Phi_d, o's share of the slot's CES index (so of its buyer price, ADR 0200);
  u_is = k_id / t_i; l_os = -theta_h a_os (lag_os - L_s), lag 0 at home and 1 over a domestic
  leg: under selection every route delivers spending / q_d units (ADR 0200), so the slot's
  expected months in transit, the arrivals' mean slot, move with the origins' spending shares,
  d ln (w_o / Phi_d) = -theta (x_o - sum a x).
- Enterprise k at province p: eps_k = s_L sigma_FL / ((1 - s_L) a_VA) its own supply elasticity
  (the eps of the Per-market gain, per enterprise), beta_k = eps_k a_VA = s_L sigma_FL / (1 - s_L)
  the elasticity of its output to the wage, since d ln Y = (sigma_FL s_L / (1 - s_L)) d ln (p_VA /
  w) at the base point of the normalized CES and d ln p_VA = (x_own - sum_r s_kr price_r) / a_VA
  (econ SPEC, Labour target); c_kr = eps_k s_kr, s_kr = spend_kr / V_k the input's cost share at
  r, a traded slot (spend_kh) or a local cell (coef_kr Y_k at its posted price); L_k = L0_k /
  (L^s_p s_L), its weight in the province's excess labour demand (d x_p = sum_k L_k (q*_k -
  beta_k omega)); at a slot v_ks = spend_kh / Phi_d, so that sum_s u_is v_ks is the share of i's
  orders that k places; at a local cell d_ki = coef_ki Y_k / Q_i, its draw from this tick's
  output; b_kr = spend_kr; w_k is k's share of its cell's output, an enterprise selling in no
  cell (no output there) has no own term.
- The wage: frictionless, the province's wage clears its labour market within E1, sum_k L_k
  (q*_k - beta_k omega_t) = 0, and the targets and output are at omega_t; under the wage rule
  (P39) the targets are set at last tick's wage and the rule steps it on their excess demand.
  Downward rigidity (P40) makes the rule's gain kappa_w on a rise and (1 - delta_w) kappa_w on a
  cut: the block's radius is the larger of the map's at the two gains (each branch is the tick's
  linearization on one side). W_p is the wage bill paid at E2, B_p its size.
- At a competitively stored good's origin the order terms of its row (E_ij and u_is) carry the
  same factor r_i = (1 + psi c) / (1 + delta s) as its E_cell: every change in its buyers' orders
  moves the storage order with it, and the row is normalized by Q = D / rho. o_i is then r_i
  times the relative change of its buyers' orders D, which its sales follow. Its stock sigma_i =
  dS / S\* and its expectation pi^e_og are states, as in the Storage loop: the storage order adds
  rho ((delta s - psi c) sigma + psi pi) to the gap, the stock takes output less sales (S\* = s D,
  Q = D / rho) and loses delta of itself, and the expectation moves by g_s toward the step. With
  storage off (r_i = 1, no stored cell) the map is the one without storage, bit for bit. (The
  B7 merge's map took revenue as R_i (x + q + e), which overstates a stored cell's sales by
  r_i, and left the two states out; B7 review I1 and I2.)
- H_s = 1 + cover + L (econ SPEC, Production: the order-up-to position holds (1 + cover + L)
  months of use, so a change in use moves the order by H times the change on top of the use, and
  a change in L by L's change; cover is P149, `systems::COVER`).
- Households (h_r, at a slot over Phi_d, in z units, at a local cell over its output Q): S_r is
  their spending at r, S_rr' = sum_pops S_k,r d ln S_k,r / d ln q_r' at this tick's prices,
  A_r = sum_pops S_k,r (d ln S_k,r / d ln M_k) wage_k / M_k and B_r the same with the pop's
  dividend, its population share of the pool: the derivatives of each pop's PIGL and CES demand
  (econ SPEC, Final demand) in its buyer prices and its money M_k = wage_k + dividend_k + rebate_k
  at the calibrated point, by central differences of `demand::shares` (relative step 1e-5).
  Units are spending over price, hence the -S_r price_r term. Pops spend at E3 this tick's wages
  and last tick's dividends.
- The pool: P_t is what E4 pays out, the country's market revenue (traded cells sell their
  demand, merchant stock taking the gap, so revenue R_i (1 + x + o / r) with R_i = p D at the
  steady state: p Q, but p Q / (1 + delta s) at a stored good's origin, whose storage order is
  not a sale, ADR 0091) less wages and input purchases. A local cell's revenue is min(D, Q): the unsold
  output perishes and a shortfall leaves the households' money unspent, so the money the
  country's households hold next tick moves by R_i e_i either way; the linearization puts it in
  the pool.

Left out, each stated: rebates (held at the steady state; the tariffs they return come from
imports); expected inflation (0, as for J2); local input orders, which sit at their zero kink at the steady state (local stocks at target;
an order fills only where a user's output passes its past peak, so along a mode at radius r
its size is about (1 - 1/r) of the move: 0 at the stability limit); foreign prices and wages
(held, so a block is one country). The profit pool and the households' income join every cell
of a country, so a block is a country.

One cell with one enterprise at given wages, without households or pool, is the single-market
map above (kappa2 0), whose -1 root is J2 (`Wage::Given`, the measurement's reduction). The
block's gain scale b is 1 where the map with gains kappa_cell / m (m = P03d) is stable, so that
the block keeps the margin m of its stability limit that the Per-market gain gives each market;
else the lower end of a bisection of (0, 1] to 2^-12: a value at which the map is stable at
gains b kappa_cell / m and unstable one grid step above. The stable set need not be an interval,
so b is the boundary the bisection finds, not the largest stable value of the grid. A block
unstable at every scale of the grid has no stable scale, b = 0, and the calibration stops on
it (B9 review; it kept the smallest, 2^-12, at which it is unstable). The map is stable when its
spectral radius, measured as the growth rate of its iterates over the second half of 4,000
ticks from a fixed start vector, is at most 1 + 2e-3 (P157: a market at its own cap has radius 1
there; on the fixture's blocks the measurement is within 5.4e-4 of the Gelfand limit
||M^k||^(1/k) at k = 2^40, `block_radius_matches_the_matrix_powers`). ITERATIONS and BISECTIONS
are numerical constants of the measurement; the tolerance decides which blocks are capped, so
it is registered. The calibration checks every capped block's map stable at its gains
b kappa_cell by the same criterion (radius at most 1 + 2e-3) and stops otherwise.

The block's limit gain is b kappa_cell / m per cell, infinite for a block within its margin and
for a cell in no block. The calibration computes it at the default kappa1 (P03a), m (P03d),
eta and wage parameters (P39 to P41), for the default mode and frictionless, and writes it to
`econ_calib_cells.arrow` (`block_limit_default`, `block_limit_frictionless`; calibration format
9), into `MarketElasticity::limit` (inputs digest). A run takes the limit of its wage regime
(the frictionless switch) and caps each cell at its share m of it:

```
kappa1_cell = min( kappa1, 2 m / J_cell, m L_cell )                  (MarketElasticity::price_gain)
```

At the defaults this is b kappa1 min(1, J_ref / J_cell). The cap is continuous in kappa1, m and
eta: a run off the defaults keeps it (a larger kappa1 does not raise a capped block, a larger m
raises its cap with it, another eta moves the per-market term only). The limit is computed at
the default eta, wage parameters and structural parameters; a run elsewhere is capped at the
default limit's share, which the calibration at those settings would refine. The map is built
at the export's storage parameters (`goods.storage_loss` and the P56 money cost, P144, P145 and
P146 of `params.json`; storage off on the fixture and the pack, ADR 0091), and all five are in
the structural digest (`structural_digest`), so a run under other storage parameters than its
calibration's is refused, at a calibrated start and at a data-state start of calibrated inputs,
as for theta: its limits would be stale. Turning storage on is recalibrating (ADR 0091). The
steady state does not depend on the gains, so the limits leave it and the calibration unchanged.

On the calibrated fixture no block is beyond its margin with storage off (4 blocks, one per country, of 57 or 6
cells; largest radius at gains / m 0.974 at eta 0.3, 0.982 at eta 1, a slow mode of the
profit pool and the households near 1). With storage on at the approved values (g_s 0.006; the
fixture calibrated with storage on, `block_limits_with_storage_on`; 55 stored cells) the default
mode caps no block (largest radius at gains / m 0.994) and frictionless caps 1 of the 4 (57
cells; radius 1.0077 at gains / m). Leaving the stock and expectation states out reads 1.022 and
1.152 there: such a map has the storage order's response to the orders (the factor r) but not
its pull back toward the target stock, so it overstates the radius. The first build's map, of traded
cells only, scaled 3 of the 4 in both modes (1.075 and 1.208). On this branch's scratch pack (format 8, the k263 pack
recalibrated; 233 blocks, 39,888 cells, the largest Great Britain's 1,787) the default mode caps
11 blocks (1,551 cells): Sudan 0.244 and Lesotho 0.333, whose radius at the per-market gains
themselves is 1.359 and 1.197, then Zimbabwe, Benin, Togo (radius 1.034, 1.032, 1.007 there),
Tanzania, Madagascar, Zambia, Switzerland, Singapore and Belgium. Frictionless it caps 41 (9,454
cells), from Sudan 0.341 and Lesotho 0.432, with Albania 0.706 (radius 1.247 at gains / m),
France 0.721 (842 cells), Armenia 0.970 and Bosnia 0.998 (ADR 0267, As built, for the runs).

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
| Selection | Goods the dispatch creates less those it destroys, per good, equal the selection term less the melt of `dispatch` (`Sourced::selected - Sourced::lost`, ADR 0200) | 1e-9 of the good's delivered and shipped units, plus 1e-12 of its market turnover | every tick (`EconWorld::end_tick`); `goods_invariant_holds_with_selection` |
| Buyer price | Spending q_d at d, fully filled, delivers one unit; q_d = Gamma_d P_d | 1e-12 relative | `buyers_pay_the_ces_index` |
| Gateway clearing | Each gateway's money and goods net to zero every tick | exact money, 1e-9 goods | every tick |
| Sourcing | sum_o X_od = E_d for each reachable destination and good | 1e-12 relative | `source` unit and property tests |
| Dispatch mass | units dispatched = origin sales, each origin and good | 1e-12 of the larger, plus the rounding of products below the normal range that the kernel carries, plus 2^-1022 (derivation below) | every tick; `dispatch_carries_the_rounding_below_the_normal_range`, `dispatch_tolerance_covers_the_carried_rounding` |
| Inventory sign | S >= 0, and the market account holds S | exact; holding to 1e-9 of the turnover S + Q + D (Task 20 round 3) | every tick |
| Storage floor | b >= -(1 - delta) S at every stored good's origin, so the stock never goes below 0; spoilage delta S destroyed in the ledger and tallied | exact | `storage_never_sells_below_zero`, `storage_conserves_goods_and_money`, every tick (ledger) |
| Price bounds | Prices finite and positive | exact | every tick |
| Two-level vs single-level | On small worlds, flows from the gateway kernel versus direct-path kernel: median absolute log difference reported, and both nest in their GE solutions | report only; V1 tolerances apply to each | `tests/trade_nesting.rs` |
| Pipeline | goods in the transit accounts = goods in the pipeline, per good. True by construction right after T2b, which brings each transit account to the goods in flight (ADR 0200); the Selection invariant checks what that adjustment creates or destroys | 1e-9 relative | every tick |
| Thread independence | 1 and 8 threads, identical hashes | exact | P2 acceptance |

Dispatch mass tolerance (2026-10-31, owner ruling on x10, ADR 0090). Origin o's sales of good g
are S_o = sum_c f_c D_c, the fill f_c of each class times its orders, and `dispatch` ships the
same units along the routes again: each route starts from m = 2^up f_c w_o (w_o the origin's
weight, 2^up the reach rescale) and multiplies by the route's factors (K_oa, reach_d or z_a, the
net ratio, 1 / p_o). In the normal range each product rounds by at most eps / 2 relative, a few
per route, inside 1e-12 of the flows. A product below 2^-1022 (subnormal) rounds by up to
2^-1075 absolute, and every factor applied after it scales that error into the units, so its
relative error is 2^-1075 over the product, not eps: no tolerance relative to the cell's own
flows bounds it. The kernel carries the bound instead:

    |dispatched_o - S_o| <= 1e-12 max(dispatched_o, S_o) + R_o + 2^-1022
    R_o = shipped_rounding_o + sum_c f_c orders_rounding_oc + sum_c [f_c < 2^-1022] 2^-1075 D_c

shipped_rounding_o (`Sourced::shipped_rounding`, written by `dispatch`) adds, for each product
of a route below 2^-1022, 2^-1075 times the factors after it (2^up times them for f_c w_o, which
is formed before the rescale); orders_rounding_oc (`Sourced::orders_rounding`, written by
`source`) does the same for the products of the orders that carry w_o (w_o reach_o, K w_o, K w_o
/ v_a, then the division by p_o, before the rescale back by 2^up); and a subnormal fill holds
avail / D to 2^-1075 only. The destination-side sums the two passes compute separately (reach,
y, z and the backward gateway sums) are taken to be normal or 0: they scale with spending per
unit of access and are not tracked, an assumption. 2^-1022 is the floor for results below the
normal range, as before. On the x10 run (frictionless, every crossing's tau x 10) the reach
rescale is 0 at every tick; the stop at tick 55 (`scratchpad/block/pack`; tick 54 on the k263
pack) was a cell with fill 2.1e-303 and weight 1.3e-7, so m = 2.8e-310, its routes' first
products subnormal and multiplied back up to sales of 1.5e-297: dispatched and sales differed
by 1.3e-9 relative, which the bound covers. Under this tolerance the run passes the dispatch
check and stops at tick 61 on a payment that does not fit the ledger (world index 1.2e3), the
collapse ADR 0090 places beyond P2's range.

## Parameters

| ID | Name | Prior / value | Class |
|---|---|---|---|
| P01 | Trade elasticity theta_g per traded good, in classes (ADRs 0018, 0023 and 0101) | 2.91 to 16.91, the range of the class values (widened from 4 to 8, ADR 0018; from 2.62 to 10.57, ADR 0101). Values: class 0 (food; chemicals and metals) 2.91, class 1 (machinery, electronics and vehicles) 4.14, class 2 (agriculture; light manufacturing) 7.68, class 3 (crude oil, gas and refined fuels; coal and other mining) 16.91, derived by the ADR 0023 rule from the OECD concordance: Caliendo and Parro (2015, Table 1 p. 18, 99% sample) sector estimates with Auto and Other Transport at 8.22 as in the authors' replication vector (ADR 0023, Amendment 2026-10-29), harmonic means over each good's HS lines with 2000 BACI trade weights (9.11, 17.64, 13.53, 2.62, 7.46, 3.00, 4.14 for goods 0 to 6), grouped into the fewest classes within a factor 1.5, each at the harmonic mean over its lines; local goods 0. Direct PPML estimates recorded as diagnostics, ADR 0022 | Literature |
| P150 | Theta scale, multiplies every theta_g where the kernel is built (ADR 0101) | 1; tariff experiments report the lane {0.6, 1, 1.4} until direct estimates replace the literature values | Free (experiment lane) |
| P02 | Inventory target coverage s_g | 1 to 6 months by good, start: agriculture 3, oil and gas 1, coal and mining 2, food 2, manufactures 2; local goods 0 | Free |
| P03a | kappa1 | 0.01 to 0.5 per month, start 0.05 (prior widened from 0.05 by owner decision, for stability; start 0.02, then 0.05 by owner decision 2026-10-24 so the expectation loop of ADR 0019 passes V1, MIGRATION Task 6) | Free |
| P03b | kappa2 | 0 to 0.5 per month, start 0: inventory term off, inventories untargeted in P2 (prior widened from 0.05 by owner decision) | Free |
| P03c | Price and wage step clip c_max, on the gap terms only (pi^e passes through, ADR 0106) | 0.1 log points per month; every tick it binds is counted per market and per wage (`clip_ticks` in `run_prices` and `run_wages`) | Free (numerical) |
| P03d | Share m of each market's J2 limit that its gain may use, and of each coupled block's stability limit (ADRs 0263, 0267) | 0 to 1, start 0.5: kappa1_cell = min(kappa1, 2 m / J_cell, m L_cell), L_cell the block's limit gain | Free |
| P157 | Block stability tolerance (`block::TOLERANCE`, ADR 0267): a block is stable while its measured radius is at most 1 + this | 2e-3 (not tuned) | Free (numerical) |
| P56 | Storage loss delta_g and money cost k_g per month, goods 0, 1, 2, 5 (ADR 0091), `TradeParams::storage_loss`, `storage_cost` | delta 0 by default (storage off, owner ruling 2026-10-31); approved (`with_storage`) 0.003, 0.0005, 0.001, 0; k 0.009, 0.011, 0.007, 0.0067 (interest 0.0047 included; DESIGN 17, P56) | Literature (weak) |
| P144 | Speculation intensity psi_g, months, `TradeParams::speculation` | 0 by default (storage off); approved (`with_storage`) 20, 1.0, 3.571, 5.714; derived from a stock-persistence target, psi = lambda s / (zeta (k + delta)), except good 5: 5.714 is the source sheet's 40 (at zeta 0.5) rescaled to zeta 3.5, 40 x 0.5 / 3.5, where the formula at zeta 3.5 gives 5.54 (owner-approved as rescaled; DESIGN 17). The storage order adjusts at lambda = psi zeta (k + delta) / s, 0.144, 0.15, 0.05, 0.067 a month (ADR 0280): derived from psi, no new parameter | Free |
| P145 | Convenience-yield elasticity zeta_g, `TradeParams::convenience` | 1.8, 13, 3.5, 3.5 (DESIGN 17) | Free |
| P146 | Gain g_s of the per-market expectation pi^e_og, `TradeParams::storage_gain` | 0.006 per month (0.004 to 0.65; DESIGN 17) | Free |
| P151 | Price step clip c_max_storable of the storable commodities (goods 0, 1, 2, 5; ADR 0106) | 0.3 log points per month (oil fell about 30% in one month in late 2008) | Free (numerical) |
| P04 | Mode costs | geo SPEC (direct PPML estimate recorded as a diagnostic, ADR 0022: not identified) | Literature + calibrated |
| P154 | Bulk freight rate: multiplier on the sea cost per tonne-km per theta class, `TradeParams::bulk_sea` (ADR 0247) | 1, 1, 1 and 0.017 (class 3; 0.026 over the old P04c 0.008, rescaled to P04c's 0.0121 by ADR 0269, owner 2026-10-31), on by default (owner ruling 2026-10-30; ADR 0247, As built): the slope s = 2.05e-4 USD per tonne-km (s.e. 7.0e-5) of a least-squares line through the 2000 voyage rates per tonne of class-3 cargoes in UNCTAD's Review of Maritime Transport 2001 (ch. IV, paras. 82 and 96, pp. 51 and 54) and 2002 (ch. VII, pp. 107 to 108), eight routes (iron ore and coal in Capesize bulk carriers, crude and products in tankers) against their sea km on the pack graph (Port Hedland to Rotterdam round the Cape, 19,450 km: no loaded Capesize transited Suez in 2000; no route canal-routed, so no canal fee netted), over P04c; dry-bulk routes alone 0.039 over 0.008, 0.026 over 0.0121. Not fitted to the cross-border share | Literature |
| P155 | Bulk freight rate: multiplier on the listed and minor port fees per theta class, `TradeParams::bulk_port` (ADR 0247) | 1, 1, 1 and 0.40 (class 3), on by default (as P154): half the line's fixed part F = 6.47 USD/t (s.e. 0.89; one port call at each end) over P04m's listed-port fee of 8; dry-bulk routes alone 0.29. The quoted rates are port to port, so terminal cargo handling is not identified | Literature |
| P05 | Border friction nu per good, ad valorem equivalent (ADRs 0092, 0276) | Calibrated per friction group to ICIO 2000's cross-border share of its goods: a theta class, or in class 3 each good alone (ADR 0276) (`ceteris_calibrate::border`, econ SPEC Calibration), read from the calibration tables into `TradeParams::border_nu`, one value per good (`EconInputs::trade_params`); default (uncalibrated worlds and the fixture) 0.3 for every good, the start of the old prior log-uniform 0.1 to 1.0. Direct PPML estimate recorded as a diagnostic, ADR 0022 (2000 BACI with ICIO domestic flows; value kept, owner 2026-10-24): 0.204 (s.e. 0.035) at the same-fit thetas (11 to 14); about 3.0 for food at the class value 2.62 | Calibrated |
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
1%. H2 per theta class (ADR 0264): a PPML on ln(1 + tariff) of the flows valued at what the
importer pays (value times 1 + t: the kernel carries (1 + t)^-theta on spending, so the origin
value's tariff elasticity is theta + 1), with the model's country-pair kernel net of tariffs as the
cost control, recovers the class's theta within max(2 pair-clustered standard errors, 1% of
theta) (ADRs 0018 and 0092). The distance elasticity of PPML on simulated country-pair flows (the origin value of the
shipped units) is reported against the data's -0.7 to -1.1, not gated, until the log-distance
crossing friction lands in P4 (ADR 0264; ADR 0272 on main replaces its Decision 3). S02:
balanced Sinkhorn on the dense kernel with the equilibrium's origin supplies and destination
expenditures reproduces the sourcing flows (1e-8, reference test), and a Rust run started at the
reference with every crossing lag at 0 stays within 1e-6 of it (`equilibrium_is_a_fixed_point`).

H2 after the nu fit (pre-P3 B2, 2026-10-30; 2000 pack with the B1 econ tables, gateway masses and
home shares, scratch, not installed; `p2_gate.rs`): at nu 2.677, 0.803, 0.400 and 0 (class 3
clamped, ADR 0092), distance -0.509 (s.e. 0.014), **outside** -1.1 to -0.7; per class (theta_c
against the PPML estimate +- 2 pair-clustered s.e.) class 0 2.91 against 2.221 to 2.566, class 1
4.14 against 2.993 to 4.021, class 2 7.68 against 7.231 to 7.998 (pass), class 3 16.91 against
9.686 to 15.086. Before the fit (nu 0.3 for every class) distance was -0.433. With the bulk
freight rate on (ADR 0247; the default since the owner ruling of 2026-10-30) distance is -0.463,
class 3 has a root (nu 0.0078) and passes (15.692 to 17.171). No band was changed; the distance
miss is for the owner (B8).

## Test plan

- Unit: leg factors and tariff wedges; theta per good (`kernel_uses_theta_per_good`) and price
  homogeneity of the shares per good at each good's theta; route-choice factors and hours used per good when the
  network carries them (`kernel_uses_route_taus_when_present`), factors below 1 rejected; kernel low-rank products against dense K on small
  worlds; sourcing against the dense route table and closed-form Eaton-Kortum shares, dispatch
  conservation, unreachable destinations, extreme prices (proptest); selection (ADR 0200): the
  two-country toy's welfare change equals the ACR formula lambda_dd'^(-1/theta)
  (`two_country_welfare_equals_acr`), a higher tau on one link never lowers q_d
  (`raising_tau_never_lowers_the_buyer_price`, proptest), a leg without flow adds no selection
  term (`inactive_leg_has_no_selection_term`); pipeline slots and
  wraparound; origin clearing, rationing classes (`mixed_rationing_matches_definition`: S11 Mixed
  on a hand-computed two-class shortage, equal to Priority without essential consumption), price
  clip, zero-supply floor; expected
  inflation in the rule (`reprice_adds_expected_inflation`), outside the clip
  (`clip_lets_expected_inflation_through`), the wider clip of storable goods
  (`storable_goods_clip_at_0_3`) and clip counts (`clip_ticks_count_bindings`).
- Stability: the Jury conditions against a toy market run through `Markets`
  (`stability_region_matches_simulation`), the default on the region's kappa2 = 0 edge
  (`default_lies_on_the_untargeted_stock_edge`) and a frictionless market beyond J2 that cycles
  at kappa1 and settles at its capped gain (`capped_gain_settles_a_market_beyond_j2`), in
  `tests/price_rule_stability.rs`. Per-market gain (ADR 0263): the cap
  (`price_gain_caps_at_the_share_of_the_limit`), the gain per cell in the rule
  (`reprice_uses_the_gain_of_each_cell`), E_cell against a finite difference of the orders on
  the calibrated fixture (`own_price_elasticity_matches_finite_difference`, calibrate crate),
  eps_cell (`supply_elasticity_is_the_enterprise_maximum`) and J at the run's eta
  (`gain_per_cell_follows_eta`, econ crate). Block gain (ADR 0267): the single-market block at given wages is
  the J2 map (`single_market_is_the_j2_map`), a clearing wage absorbs the own price of a
  province's only enterprise (`a_clearing_wage_absorbs_the_own_price`), the wage rule takes the
  larger radius of its two branches (`the_rule_takes_the_larger_radius_of_its_branches`), a pair
  stable alone and unstable coupled is scaled into its margin
  (`coupled_pair_is_scaled_into_its_margin`) and an uncoupled pair keeps its gains
  (`uncoupled_pair_keeps_its_gains`, `ceteris_calibrate::block`); E_ij against a finite
  difference of the orders (`cross_price_elasticity_matches_finite_difference`) and the input
  order shares against the flows (`input_order_shares_match_the_flows`) on the calibrated
  fixture; the measured radius against the Gelfand limit of the map's matrix
  (`block_radius_matches_the_matrix_powers`); the map against the tick, a coupled block of the
  fixture beyond its limit growing in `EconWorld` at the map's radius to within P157, for
  substitution and for input orders, in both modes (`block_map_growth_matches_the_tick`, which
  fails with H = cover + L, with the input costs at this tick's prices and with the map of the
  first build); the written limits and no capped block on the fixture
  (`block_limits_of_the_calibration`), with storage on the map's stored states and the limits at
the export's storage parameters (`block_limits_with_storage_on`; a stored market at given wages
is the Storage loop's 4 x 4 map, `stored_market_is_the_storage_loop`; its revenue follows the
buyers' orders, `stored_revenue_follows_the_buyers_orders`), the refusal of other storage
parameters (`calibrated_start_rejects_other_storage_parameters`,
`data_state_start_refuses_elasticities_of_other_storage_parameters`, econ crate), the check on load (`block_limits_must_be_positive`) and
  the cap continuous in kappa1, P03d and eta (`block_limit_caps_the_gain_continuously`, econ
  crate).
- Storage (ADR 0091; the tests run with storage on, `TradeParams::with_storage`, and the
  default's off switch is checked in `storage_never_sells_below_zero`): orders equal spoilage at a steady state
  (`storage_orders_equal_spoilage_at_steady_state`), the floor (`storage_never_sells_below_zero`),
  the Deaton-Laroque asymmetry on agriculture, a 30% harvest cut for 12 months raising the price
  more than a 30% surplus lowers it (`stockout_raises_price_more_than_surplus_lowers_it`), a 3-month
  30% cut raising it by less than half the flow-clearing price without stock, ln(1 / 0.7) / 0.6
  (`storage_smooths_a_short_cut_below_flow_clearing`), and the 12-month cut drawing the stock below
  half its target while the geometric release at lambda keeps it above zero
  (`cut_draws_the_stock_to_a_pipeline_floor`; ADR 0280: rise 0.356 against fall 0.266, 3-month rise
  0.086 against a bound of 0.297, lowest stock 0.13 of target), a stockout spike below the
  flow-clearing price (`stockout_spike_stays_below_flow_clearing`: fuels at Algeria's E 11.24,
  orders doubled for one tick, peak below ln 2 / E = 0.062; 0.600 under the first rule), a dead
  market not bidding its price up (`stock_without_supply_or_buyers_does_not_bid_the_price_up`),
  the toy market's tangent map equal to the Storage loop's (`partial_adjustment_keeps_the_storage_loop`), conservation with spoilage tallied on the
  fixture world (`storage_conserves_goods_and_money`, econ `p2_properties.rs`: the market
  account loses delta S bit for bit, the ledger's tally of destroyed goods rises by exactly the
  losses, and at pi^e -10 the order is the release delta S - lambda S, above the floor), the refusal of a loss without
  speculation (`storage_loss_without_speculation_is_refused`), the stage 5 state
  with storage (`stage5_state_is_stationary_under_the_stock_rules`, calibrate) and the storage
  loop against the toy market (`stability_with_storage_matches_simulation`). The two last claims
  replace "stocks hit zero only on the cut side" (ADR 0091 Tests, amended 2026-10-31).
- Property: goods and money conservation through T1-T4 under random supply and demand;
  gateway accounts net to zero; prices stay finite.
- Nesting: two-level versus direct kernel on small worlds; sourcing versus balanced Sinkhorn at
  the equilibrium (S02).
- Golden: covered by `tests/golden/p2-econ.txt` (econ SPEC).
- Benchmarks (criterion, `cargo bench -p ceteris-econ`): `source` and `dispatch` on the full pack (7
  traded goods; `source_dispatch` budget 8 ms per tick on the M4 Pro, re-budgeted from 5 ms by the
  owner on 2026-10-26, measured 7.9 ms; 19.2 at rev 226), `clear_reprice` (budget 2 ms; 0.25 ms
with the per-cell gain of ADR 0263, M4 Pro on AC, the B scratch pack, criterion median 2026-10-30),
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
  Tick speed-up (2026-11-01, no output change: state hash and ledger journal identical over 24
  full-pack ticks on Linux x86 and the M4 Pro). Measured on the M4 Pro in a quiet sim loop
  before it: a tick 39 ms, of which T2b 17.9 (per-good dispatch tasks 10, then 7.3 serial:
  ledger posting 4.8, goods moves 2.5) and E3 8 (the unit dispatch of the price refresh 7).
  Changes: the E3 price refresh runs `arrivals`, the unit dispatch reduced to what the
  delivered prices and lead times read (bit-identical to `dispatch`'s arrivals, tested);
  T2b posts every good's entries in one `Ledger::post_all` after the goods moves, applies the
  chunk sums in parallel over accounts and copies the journal in parallel. Criterion on the
  idle M4 Pro (2026-11-03, two rounds, medians): a tick 46.1 ms before, 41.2 after; consume
  7.8 to 6.0; dispatch 24.3 to 22.6. Tried and dropped: a `trade_legs` that builds its legs in
  parallel (nested inside the per-good tasks, tick 43.7 ms, dispatch 24.8) and a chunked
  parallel dispatch per good (dispatch 27.2 against 24.4 ms). Profile of the per-good region
  (7 goods, 14 threads): its wall time is the slowest good, 11.9 ms, against 71 ms of task
  time, and the serial tail is 6.8 ms; the crossing pass (2.3 to 3.7 ms per good) is the
  largest part of the kernel dispatch.
  T2b goods moves by good (RFC 0287 item 2, ADR 0294; 2026-11-04): each good's goods moves run
  at the end of its own per-good task on its ledger column (`Ledger::take_columns`,
  `GoodColumn`, `Ledger::put_columns`, account chunks in parallel), its pipeline shipments are
  listed and applied by `Pipeline::push_all` with the four slots in parallel, each in good
  then list order. Every quantity and tally sums the same terms in the same order, so the
  `identity` example's state and journal hashes equal `main`'s over 24 full-pack ticks at 1 and
  14 threads. Sim loop with `--timing`, 60 ticks, three alternating rounds on the M4 Pro (load
  10 to 22): dispatch 17.4 to 17.5 ms before, 16.2 to 16.4 after; a tick 40.0 before, 38.8 to
  39.2 after. The serial tail is now the column write-back and `push_all`, 1.2 ms.
  The end_tick income check (RFC 0287 item 3, 2026-11-04) reads `Ledger::tally` of WAGE,
  DIVIDEND and REBATE, kept as they are posted, instead of scanning the whole journal; the
  journal stays one contiguous record. Same alternating rounds: end_tick 3.55 to 3.61 ms before,
  2.67 to 2.69 after; the tick 38.3 to 38.6 before, 37.8 to 38.2 after, within the noise of
  that load.
  `Pipeline::in_flight_all` (RFC 0287 item 4, 2026-11-04) runs the goods in parallel, each by
  `Pipeline::in_flight`, the same sum in the same order; its three calls a tick (T1, T2b, E5)
  stay three, since E5's transit check must read the pipeline itself. Alternating rounds
  against the serial loop, load 17 to 31: the three calls 2.30 to 2.39 ms a tick before, 1.18
  to 1.26 after in three of four rounds (2.52 in a noisy one).
  T1 and T3's local goods by good (RFC 0287 item 6, 2026-11-04): T1 delivers each traded good
  on its ledger column in parallel (its deliveries in buyer order, then its largest); T3 clears
  each local good on its column in parallel and posts the purchases with `Ledger::post` in
  province then good order, the serial loop's journal order. Same alternating rounds, load 6 to
  8: deliver 1.52 to 1.54 ms before, 1.11 to 1.14 after; clear 1.69 to 1.70 before, 1.36 to
  1.45 after. E2 produce was tried with its SALE, WAGE and PURCHASE transfers posted as one
  block: 4.5 ms before, 4.7 after (the per-chunk sums over every account cost more than the
  serial transfers), so it keeps its transfers; distribute reads balances and is unchanged.
  Crossing weights (RFC 0287 item 8, 2026-11-04): `dispatch` and `arrivals` keep each
  crossing's weight `k carry y_b` from the pass that sums z_a to the pass that shares it out
  (the same product), instead of recomputing it. Same alternating rounds, load 4 to 7: T2b's
  kernel time summed over goods 42.4 to 42.9 ms before, 39.7 to 40.9 after; its slowest good
  10.6 to 11.8 before, 10.1 to 10.2 after. Tried and not kept: the z pass parallel over origin
  gateways inside each good's task (slowest good unchanged, 10.3 to 10.5 ms), and two rows
  interleaved in the z pass (kernel time 41.9 to 42.7 ms, slower).
  E3 PIGL once per province (RFC 0287 item 10, 2026-11-04; econ SPEC demand): the part of a
  pop's PIGL demand that depends only on its country's weights and the buyer prices (category
  indexes, within-category shares, Dtilde(P), B(P) and `scale (Dt / B)^gamma`, `demand::PiglAt`)
  is computed once per province and shared by its pops, with the same operations in the same
  order (`pigl` itself goes through `PiglAt`). Same alternating rounds, load 4 to 8: demand
  5.91 to 5.96 ms before, 5.59 to 5.71 after (of which the price refresh, 4.2, is unchanged).
  The `pow` in E1 labour sits in each enterprise's CES target at each trial wage and is not
  shared; fewer parallel regions was not pursued after items 7 and 9 put allocation and
  posting below the noise.
  E3's price refresh overlaps T1 to E2 (RFC 0287 item 11, 2026-11-04; 19.4, systems with
  disjoint read and write sets run concurrently): it reads only the posted prices, the active
  markets, the kernel and Gamma, which nothing changes between T1 and E3 (T4 reprices), and
  writes the buyer price index and lead, which nothing reads before E3 (E1 reads last tick's
  index). T1 begins it on the thread pool (`EconWorld::start_refresh`) on copies of the prices
  and active markets, into a new index and lead and the traded goods' sourcing scratch; E3
  (`refresh_price_index`) waits for it, helping the pool, and takes its result, the same values
  as computing it there. A refresh begun and not taken (systems run singly) is dropped by the
  next start; a clone of the world waits for it. Alternating rounds against item 10, load 9 to
  14: a tick 35.9 to 36.0 ms before, 33.8 to 34.0 after in three of four rounds (one noisy
  round each); demand 5.9 to 6.1 before, 1.5 after, while deliver (1.3 to 3.0) and labour
  (1.2 to 1.5) slow as they share the cores with it.
  mimalloc (ADR 0297, 2026-11-04) is the global allocator of the `ceteris` binary and the
  `identity` example; allocation never touches results (identity equal to `main` at 1 and 14
  threads). Alternating rounds against the system allocator: E2's local chain 1.8 to 2.5 ms
  before, 0.6 to 0.8 after, produce 5.6 to 6.2 before, 4.2 to 4.8 after in every round; the tick
  lower in 5 of 6 rounds of a loaded run (44 to 48 ms), by 0.9 to 2.3 ms (median about 1.3).
  The criterion benches keep the system allocator.

## Open questions

- Pre-P3 benchmark (C7, 2026-11-06): the three ADR 0207 cases and the Brexit, distance and
  US-China records are in the econ SPEC, "Pre-P3 benchmark". H2 distance (-0.45) and Brexit
  magnitudes are tier-2 misses naming P4 (ADR 0272) as the removal phase.

- Gateway count and selection rule are an agent default (22.2); the rule is in the econ SPEC
  pack section. As built (Task 17, controller ruling): 600 gateways (286 ports, 171 airports,
  143 land crossings; ADR 0251). Each country's land crossing points, the merged endpoints of its
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
  not run there; at 600 H2 distance is -0.744 (-0.753 before ADR 0251). No count is proposed before the masses land.
  With the masses and the nu fit (2026-10-30, ADR 0097): the 300 pack fits nu but its calibration
  fails in pass 3's Sinkhorn (100,000 sweeps, residual 4.2e-5); no count is proposed.
- Expectations are adaptive only in P2 (S04): one pi^e for the numeraire area (ADR 0019); price
  sourcing still uses posted prices (naive). Heuristic switching (12.2, P19) arrives with the
  expectations module.
- Freight capacity and endogenous freight rates (8.4) are P8.
- Storage parameters (ADR 0091, DESIGN 17 P56, P144 to P146) are weakly sourced: delta_g and the
  physical part of k_g rest on 1995 and 2017 extension rates, post-2008 tank and warehouse rents
  and judgement for fuels and coal (no 2000-dated storage rates were found); psi_g is derived from
  a stock-persistence target, not estimated; zeta_g of goods 2 and 5 is the owner's choice between
  two routes that differ tenfold; g_s is judgement. P9 refits them to commodity price and stock
  dynamics (FAO, EIA, LME).
- Inventories of food, light manufacturing and machinery (goods 3, 4, 6) are untargeted in P2
  (kappa2 = 0): stocks keep what a transient leaves (4.7 to 47 times target cover on the fixture
  after 1,200 ticks from the data state, before ADR 0091). Revisit with expectations (12.2), which
  can make an inventory target stable, or with P9 calibration. Goods 0, 1, 2 and 5 target their
  stock through competitive storage (ADR 0091) where it is on; it is off by default (owner ruling
  2026-10-31), so in the default all seven traded goods are untargeted. The approved values with
  g_s 0.006 pass the stability checks and V1 at 6,000 ticks on the fixture; on the 2000 pack,
  under ADR 0280's order, the default mode passes V1 and the frictionless mode misses it (seed 20,
  Switzerland's good-1 cells; the data state, France's province 1094; ADR 0280 As built). Turning
  it on is open (Storage loop).
