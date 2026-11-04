## 8. Markets, Price Formation and Trade

Every province prices every good, with inventories and adjustment lags, and goods move between provinces by structural gravity at posted prices, with origin markets clearing across ticks through inventories, so the equilibrium limit is entropic optimal transport (8.3).

### 8.1 Market topology

Each of about 4,000 provinces holds a price p(i,g) and a merchant inventory S(i,g) per good. Political borders do not partition markets: they act through tariffs, sanctions and frictions in the cost function. Services are priced locally and traded across provinces only through demand-side frictions.

### 8.2 Price formation (stock-flow rule)

Each month, supply (local output plus arrivals) meets demand at the posted price; inventories absorb the gap; the price then moves with excess demand and with deviation from target inventory coverage:

```latex
\Delta \ln p_{i,g,t+1} = \pi^{e}_{i,t} + \kappa_1 \frac{D_{i,g,t} - Q_{i,g,t}}{Q_{i,g,t}} - \kappa_2 \frac{S_{i,g,t} - S^{*}_{i,g,t}}{S^{*}_{i,g,t}}, \qquad S^{*} = \bar{s}_g \, D_{i,g}
```

pi^e is the merchants' expected inflation for the currency area (Section 12.2). It makes the rule homogeneous: after a permanent change in money growth, prices drift at the new rate with zero real excess demand, so the long-run Phillips curve is vertical and money is neutral in the long run (ADR 0019). The other two terms form a proportional-integral controller on the inventory gap: excess demand is the flow, the inventory deviation its integral. The P2 spec derives the stability region in (kappa1, kappa2) for the linearized market given local demand and supply elasticities; the step clip is a registered parameter, and every tick it binds is counted.

When inventory runs out, demand is rationed within the tick, which is how shortages propagate. The scheme follows the dynamic input-output model of Pichler et al. (2022), whose rationing rules were tested against 2020 data: priority by default (households' essential consumption, then intermediate users by contract, then the rest), with proportional and mixed rationing as switch S11. A Walrasian mode solves clearing prices by Newton iteration each tick, for equilibrium-nesting tests and as a robustness switch.

### 8.3 Inter-province trade: structural gravity cleared across ticks

Within each good, buyers in province d spread spending across origin provinces by a logit over delivered prices (A8). This is the structural gravity system of Eaton-Kortum and Anderson-van Wincoop with sector trade elasticity theta\_g (ADRs 0015, 0018). Flows are in value, and the kernel is built from iceberg factors, not from costs in currency:

```latex
X_{od,g} = \frac{K_{od,g} \, p_{o,g}^{-\theta_g}}{\sum_{k} K_{kd,g} \, p_{k,g}^{-\theta_g}} \, E_{d,g}, \qquad K_{od,g} = \tau_{od,g}^{-\theta_g}
```

E(d,g) is province d's expenditure on good g at posted prices (households, intermediate users, investment and government), and the denominator is the price-index term P(d,g)^(-theta). The kernel is dimensionless and prices enter only as ratios, so flows are homogeneous of degree zero in nominal prices and exchange rates: inflation or a currency move does not change trade dispersion. Quantities shipped are X / (p\_o tau); per-unit costs (5.4) are charged to the shipment. With firms heterogeneous within cells (ADR 0026), theta\_g is the Pareto shape of productivity and fixed export costs multiply the kernel on gateway links by f^(1 - theta/(sigma - 1)), so its form and factorization are unchanged.

**Clearing across ticks (ADR 0016).** Each tick applies the destination-side normalization above once, at posted prices. Origin sales are not forced to equal origin supply within the tick: the gap goes into the origin's merchant inventory and moves p\_o through the rule of 8.2 at the next tick. A destination normalization followed by an origin price adjustment is one Sinkhorn sweep, so the price dynamics are Sinkhorn spread over time. With adjustment speeds high and lags zero, the fixed point is the doubly constrained entropic transport solution, which is the structural-gravity equilibrium (Galichon 2016; Allen, Arkolakis and Takahashi 2020). The frictionless limit is therefore the GE reference model by construction, and the within-tick work is one kernel product per good class instead of 5 to 10 full Sinkhorn iterations.

Per-unit costs make tau depend on the origin price (tau = 1 + c / p\_o); the kernel uses last tick's prices, which adds no iteration. Supply that cannot be sold at the posted price stays in inventory, which replaces the unbalanced-transport relaxation. The full within-tick Sinkhorn solve (balanced, or unbalanced as in Chizat et al. 2018) stays available as structural switch S02 for nesting tests.

**Computation.** The kernel factorizes through about 600 gateways (ports, border crossings, airports), because every international route passes through a gateway pair. Under the least-cost legs P2 keeps (S05) the factorization approximates the path minimum; under route choice (5.4, deferred) it is exact, because route-choice costs sum over routes:

```latex
K_{od} = \mathbb{1}[o, d \text{ same country}] \, K^{\text{dom}}_{od} + \sum_{a,b} K^{\text{dom}}_{oa} \, K^{\text{int}}_{ab} \, K^{\text{dom}}_{bd}
```

Per tick and per (theta class, weight class) this is one sparse product with the province-to-gateway factor, one G x G product and one product back: O(P x k + G^2) with k gateways reachable per province, batched across goods in the class so the products are matrix-matrix (provinces x goods). Tariffs are constant within a country pair, so they scale blocks of the gateway product and are never materialized per good. The estimated cost is a few milliseconds per tick (19.2), to be replaced by the P2 benchmark.

### 8.4 Transit, shipping capacity and freight rates

Shipments arrive after the expected transit time over routes (5.4), so prices respond to news with realistic lags. Shipping capacity (container fleet, bulk carriers, tankers, rail and truck fleets) is capital owned by enterprises; freight rates rise with utilization and fuel prices and feed back into c. Chokepoint closures and port disruptions are shocks on specific edges. Validation: container and dry-bulk freight-rate indices, including the 2021 spike.

### 8.5 Trade policy

Policy wedges enter c(o,d,g): MFN and preferential tariffs (free trade agreements, customs unions such as the EU), anti-dumping and safeguard duties, quotas, export controls, export bans (as on food in 2008 and 2022), and sanctions (embargoes, secondary-sanction compliance costs). Tariff revenue accrues to the importing state. Optimal-tariff and retaliation logic, as quantified by Ossa (2014), is available to state AI.

### 8.6 Demand system

Households allocate spending across three consumption categories, agriculture A (goods 0 and 3), manufacturing M (goods 1, 2, 4, 5 and 6) and services S (goods 7, 8 and 9), the ones Alder, Boppart and Muller (2022) estimate, with PIGL preferences in their multi-sector form (ADR 0021). PIGL generates Engel's law and the rising services share and, unlike non-homothetic CES, aggregates exactly: a pop's category shares depend on its income distribution only through the moment E\[y^(1-epsilon)\], which for the lognormal income of A3 has a closed form:

```latex
\mathbb{E}\left[ y^{1-\varepsilon} \right] = \exp\left( (1-\varepsilon)\,\mu + \tfrac{1}{2}(1-\varepsilon)^2 \sigma^2 \right)
```

Category demand per pop is therefore closed form, with no root-finding or quadrature. Non-homothetic CES (Comin, Lashkari and Mestieri 2021) stays as switch S03; under it each pop needs a Newton solve for its utility index and a 5-node Gauss-Hermite quadrature over its income distribution, warm-started from the last tick. The sigma of a pop's lognormal income comes from its country's 2000 Gini, WDI's where it exists, else SWIID's (ADR 0021).

Within each category a CES at P06 aggregates goods and origins. A subsistence floor for food and energy enters through rationing priority, not preferences. Governments, armies and investment add their own demand. In P2 to P4 final demand has three demanders per country from the ICIO columns (ADR 0099): households (HFCE), government (GGFC and NPISH) and investment (GFCF), each good's inventory change shared over the three in proportion to their purchases (ADR 0302). Their shares are shares of absorption, income plus the transfer D_c (ADR 0303). Each pop spends its households' share by PIGL and the government and investment shares at fixed nominal shares by good, a good with no reachable origin passing its share to the demander's other goods (ADRs 0301, 0305); PIGL is inverted on the households' part only, after calibration's balance rescales all final demand with one factor per country and good (ADR 0304). The inversion takes each country's income-effect weight as the largest at which every category weight of B(P) is nonnegative: the pooled estimate's omega\_A = 0 wherever a country's shares allow it, else omega\_M or omega\_S = 0 and food in B(P) (ADR 0313). It meets the data on the shares the run spends: a pop far below its country's base whose raw share of a category is negative spends nothing on it and the rest rescaled, and the inversion counts it so (ADR 0316). The investment accelerator of ADR 0099 waits for capital accumulation in P4 (ADR 0306).

**Consumption and saving (ADR 0021).** Total spending follows a buffer-stock rule: pops consume out of expected permanent income and correct toward a target ratio of wealth to income,

```latex
C_{p,t} = m_p \, \bar{y}^{e}_{p,t} + \lambda_p \left( W_{p,t} - w^{*}_{p} \, \bar{y}^{e}_{p,t} \right)
```

with propensities differing by wealth bracket and labour status (register P37). The rule is disciplined by intertemporal MPCs, the response of spending in every later month to a one-off transfer (Fagereng, Holm and Natvik 2021; Auclert, Rognlie and Straub 2024), not only by impact MPCs; this is the main channel through which fiscal and monetary policy reach demand. With static pops in P2 the rule runs with m = 1 and lambda = 0 (spend current income), which closes the accounting loop; it is calibrated in P5 together with money and banks.

### 8.7 Factor markets

- **Labour**: per province and education level, wage posting with a matching function; wages move with expected inflation plus a term in the vacancy-unemployment ratio (ADR 0019), with downward nominal rigidity and minimum wages where legislated.
- **Land and housing**: rents clear demand by use; housing is a durable stock with construction lags, so house-price booms and busts are possible.
- **Capital**: allocated by investment and credit (Sections 7.4 and 9).

### 8.8 Currencies in trade

Merchants invoice in a currency chosen by a dominant-currency rule: most international trade is invoiced in dollars or euros regardless of origin, as documented by Gopinath and coauthors. Exchange-rate changes therefore pass through to prices according to the invoicing currency, not just bilateral rates.

Dominant-currency pass-through needs export prices that are sticky in the invoicing currency, and the market prices of 8.2 move every tick. P5 therefore adds a thin posted-price layer for exporters: each exporting enterprise posts a price in its invoicing currency and resets it with a Calvo hazard calibrated to export-price change frequencies. Without that layer, the pass-through and price-change-frequency targets of 15.7 are dropped.

