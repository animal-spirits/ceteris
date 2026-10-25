# 0027. Short- and long-run trade elasticities

Status: proposed
Date: 2026-10-24
Design sections: 8.3, 15.5, 17.1 (P01), 17.2
Phase: P2

## Context

ADR 0018 sets one trade elasticity per sector class, taken from long-run estimates (Caliendo
and Parro 2015). Measured responses to tariffs are much smaller in the first year and build up
over several years: Boehm, Levchenko and Pandalai-Nayar (2023, AER) find an elasticity below 1
in the short run and about 2 after 7 to 10 years. With a single long-run theta, a tariff branch
moves sourcing within months, overstates trade diversion in 2018-19 and 2022, and understates
the price and welfare cost borne in the first years (Amiti, Redding and Weinstein 2019;
Fajgelbaum et al. 2020). The validation episodes in 15.5 are exactly these short-horizon tariff
and sanctions shocks.

## Options

1. One theta per class (status quo).
2. Two thetas, switching by horizon: no state, but discontinuous and not a model of anything.
3. Partial adjustment of the gravity kernel toward its target: sourcing relationships adjust at
   a monthly rate, so the short-run elasticity is the long-run one times the cumulative
   adjustment.
4. Explicit supplier relationships per province pair: realistic but dense (4,796 squared pairs
   per class).

## Decision

Option 3, at the gateway level so the state stays small. Each (theta class, gateway pair)
carries a lagged kernel that adjusts in logs toward the current target, and each (origin, good)
carries a smoothed price index used in the logit:

```latex
\ln K_{ab,t} = (1 - \rho_c) \ln K_{ab,t-1} + \rho_c \ln K^{*}_{ab,t}, \qquad
\tilde{p}_{o,g,t}^{-\theta} = (1 - \rho_c)\, \tilde{p}_{o,g,t-1}^{-\theta} + \rho_c\, p_{o,g,t}^{-\theta}
```

rho_c is the monthly adjustment rate of class c (register P53). The long-run elasticity stays
theta_c and is still estimated by PPML (0022); rho_c is identified from the dynamic response of
bilateral flows to tariff changes in BACI (local projections around 2018-19), outside SBI.
Domestic legs adjust instantly: inertia sits in cross-border sourcing, which is where the
evidence is. Switch S12 selects instant (rho = 1) or partial adjustment.

## Migration

- ceteris-trade: add the lagged gateway kernel (G x G per class, about 360,000 entries for 600
  gateways) and the smoothed origin price term (provinces x goods) to tick state and snapshots.
- The steady state is unchanged, so the V1 nesting test (0022) holds at any rho; only
  transition paths differ.
- Register P53 (rho by theta class), prior: half-life 12 to 48 months. Switch S12.

## Tests

- With rho = 1 the golden hashes equal the instant-adjustment path.
- A permanent tariff shock on a toy world produces an elasticity path that rises from rho times
  theta in month one to theta in the limit, matching the closed form.
- Local projection on simulated BACI-like flows recovers rho within its prior band.

## Consequences

Short-run tariff incidence and trade diversion become realistic without a second elasticity in
the register. The 2018-19 tariff and 2022 sanctions natural experiments become fair tests.
Memory cost is small; per-tick cost is one extra blend per kernel entry.
