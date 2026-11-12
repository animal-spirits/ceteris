## 7. Production, Enterprises and Technology

Production follows the OECD input-output structure at province level, and technology is sector productivity that grows at the frontier and diffuses to followers; there are no named technologies.

### 7.1 Goods and sectors

The 45 industries of the OECD Inter-Country Input-Output tables define the goods, with a few splits where physical quantities matter for prices, food security or emissions.

| Group | Sectors (examples) | Splits |
| --- | --- | --- |
| Primary | Agriculture, forestry, fishing; energy mining; non-energy mining | Grains, other crops, livestock; crude oil, natural gas, coal |
| Manufacturing | Food, textiles, chemicals, pharmaceuticals, basic metals, electronics, machinery, motor vehicles, other transport | None initially |
| Energy and utilities | Electricity, gas, water | Fossil, nuclear and renewable generation |
| Construction | Construction | None |
| Services | Trade, transport, IT and telecom, finance, real estate, professional, public administration, education, health | Traded through frictions, not shipments |

Each good has weight per unit of value, perishability and a consumption category; services are mostly local, with cross-border service trade handled as frictions on demand rather than physical shipments.

### 7.2 Production function

Each (province, sector) enterprise combines a CES value-added core with intermediate inputs. Intermediates are Leontief in a non-energy bundle, initialized from the country's ICIO coefficients, plus a CES energy nest so firms can substitute between fuels and electricity:

```latex
Y = \min\left( \frac{VA}{a_{VA}}, \; \frac{M}{a_{M}}, \; \frac{E}{a_{E}} \right), \qquad VA = A_{s,i,t} \left[ \alpha_K K^{\rho} + \alpha_L L^{\rho} + \alpha_T T^{\rho} \right]^{1/\rho}
```

L is effective labour by education level; T is land (agriculture) or resource deposits (mining). A combines sector productivity, province agglomeration (rising with local employment density) and climate damages. Until capital and land enter (P4), each (country, good) has its own labour share: the WIOD SEA 2000 relative share of the sector rescaled so that the VA-weighted mean is the PWT national share, each share within [0.05, 0.95] with the others rescaled to hold the mean, and economies outside SEA taking the mean of the five SEA economies nearest in GDP per person (ADR 0307).

**Putty-clay intermediates (ADR 0020).** The coefficients a belong to installed capacity, not to the sector. Each capacity vintage fixes its input coefficients when it is built, by cost minimization over a CES with a long-run elasticity (register P38) at expected prices; once installed it runs Leontief. Short-run complementarity matches the near-zero elasticities measured for intermediates (Atalay 2017; Boehm, Flaaen and Pandalai-Nayar 2019) and keeps rationing along supply chains. Long-run substitution lets the input-output structure drift from its 2000 values over decades instead of staying frozen to 2150, and gives large shocks the non-linear propagation studied by Baqaee and Farhi (2019). Vintages are aggregated per enterprise into a capacity-weighted coefficient vector, so per-tick cost is unchanged; the choice happens at the quarterly investment step (7.4). Initial capacity is one vintage with the 2000 ICIO coefficients.

**Firms within a cell (ADR 0026).** Each (province, sector, ownership) enterprise is a mass of firms with Pareto-distributed productivity, shape k\_g, above a cell-specific lower bound, selling CES varieties with elasticity sigma\_g (k\_g > sigma\_g - 1), as in Melitz (2003) and Chaney (2008). The firm count, size distribution and exporter share of each cell are closed form, so the extensive margin of trade and a firm-size target come at no per-tick cost. The quarterly investment step (7.4) becomes free entry and exit in the mass of firms.

### 7.3 Enterprise behaviour (monthly)

1. Forecast demand with the heuristic expectation rules of Section 12.
2. Set an output target from forecast demand and an inventory-coverage rule (as in data-driven ABMs such as Assenza-Delli Gatti-Grazzini 2015 and Poledna et al. 2023).
3. Post vacancies or lay off; a province matching function pairs vacancies with job seekers by education.
4. Buy inputs at province prices; shortfalls cap output, so rationing propagates through the input-output network (the supply-chain mechanism behind 2021-22 shortages).
5. Sell output at the province market price; profits go to owners.

### 7.4 Investment, entry and exit (quarterly)

Owners (households via wealth, states, banks, foreign investors) allocate new capital across (province, sector) options by a logit over expected return net of risk, compared to the cost of capital (a Tobin-q style rule), financed from retained earnings, bank credit, bonds and equity. Persistent losses with negative equity trigger exit. Foreign ownership is simply foreign-owned capital, so FDI and profit repatriation are ordinary flows.

### 7.5 Technology as sector productivity

Technology has three parts, all at the sector level:

- **Frontier growth.** Each sector's frontier productivity grows with world research effort, with diminishing returns as in Jones (1995):

```latex
g^{*}_{s,t} = \delta_s \, R_{t}^{\lambda} \, \left(A^{*}_{s,t}\right)^{\phi - 1}, \qquad \phi < 1
```

Because growth depends on research effort, a shrinking or aging world population slows frontier growth, the semi-endogenous mechanism studied by Jones (2022). This keeps open-ended runs from exploding or stalling arbitrarily.

- **Catch-up.** Provinces converge toward the frontier at a rate that rises with education, trade exposure to frontier producers (Buera and Oberfield 2020), foreign ownership and institutional quality:

```latex
\Delta \ln A_{i,s,t} = g^{*}_{s,t} + \lambda_{i,t} \left( \ln A^{*}_{s,t} - \ln A_{i,s,t} \right) + \varepsilon_{i,s,t}
```

- **Learning by doing in energy.** Renewable generation and storage-intensive sectors follow Wright's law: unit costs fall with cumulative global output, the pattern documented for solar, wind and batteries by Way et al. (2022). This captures the energy transition without naming technologies.

Initial productivity gaps are backed out from 2000 output, inputs and prices; frontier growth rates per sector are calibrated to productivity accounts. A share of frontier growth arrives as fat-tailed sector jumps with the same mean (ADR 0242).

### 7.6 Structural change

Non-homothetic demand (Section 8.6) and unequal sector productivity growth (Baumol cost disease in services) jointly drive sector shares. Targets: 2000-2024 sector employment and value-added shares by country.

### 7.7 Energy and emissions

Energy sectors produce physical energy; fossil inputs carry emission factors, so CO2 and other greenhouse-gas emissions are computed from the input-output flows every month and passed to the climate module (Section 13). Carbon prices and fuel taxes are policy wedges on those inputs. Country energy production and use are calibrated to EIA International Energy Statistics; IEA World Energy Balances are held out for validation only (ADR 0356).

