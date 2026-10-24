# 0015. Trade flows in value with a dimensionless gravity kernel

Status: accepted (owner, 2026-10-24, PR 1); implementation pending on `p2-econ`
Date: 2026-10-24
Design sections: 1, 8, 8.3, 17.1 (P01)

## Context

Section 8.3 maximized the sum of X(E p_d - p_o - c) minus epsilon times the entropy of X,
with epsilon in currency units, and then wrote c = theta ln tau to claim structural gravity.
Price levels and log costs cannot be added. With epsilon in currency units, flows depend on the
nominal price level and exchange rates: inflation or a depreciation changes trade dispersion,
which no gravity model allows, and V1 fails for reasons unrelated to frictions.

## Options

(1) Keep the level objective and rescale epsilon by a price index each tick. (2) Write flows in
value with kernel tau^(-theta) and destination expenditure as the demand side, so the
potentials are p_o^(-theta) and the importer price index.

## Decision

Option 2. Option 1 restores homogeneity only approximately and keeps a parameter, epsilon, with
no literature counterpart.

## Migration

- Flows X(o,d,g) become values; quantities shipped are X / (p_o tau). Per-unit costs are
  booked to the shipment owner in the ledger, not netted from the price.
- Build the kernel from iceberg factors, K = tau^(-theta_g), not exp(-c / epsilon). Remove
  epsilon and the export-supply and import-demand KL weights from the register.
- Destination expenditure E(d,g) comes from the demand systems at posted prices: consumption,
  intermediate use, investment and government.
- Rewrite the objective section of the trade SPEC.md.

## Tests

- Property test: scaling all prices and expenditures by lambda > 0 leaves trade shares
  unchanged to 1e-12.
- Three-province toy: flows equal the closed-form Eaton-Kortum shares.
- Goods conservation (V0) holds after converting value flows to quantities.

## Mapping to P2 as built

P2 (branch `p2-econ`, ADR 0013) already builds K = tau^(-theta) from iceberg factors, has no
epsilon and no KL weights, and its routing is homogeneous of degree zero in prices. What differs is
the side that allocates: P2 has each origin spread its supply over destinations by K p_d^theta
(`ceteris-trade` route.rs), paid by delivered pricing so the router bears its costs.

- Kept: kernel construction and gateway factorization (kernel.rs), iceberg-only costs (S05), the
  homogeneity property test (degree zero, real quantities within 1e-9).
- Replaced: the origin logit, netback and delivered-pricing payments. Destination expenditure E(d,g)
  is household demand plus enterprise input orders at posted prices, spread over origins with
  shares K_od p_o^(-theta) / P_d^(-theta). Origin o ships X / p_o units, d receives
  X / (p_o tau), and o's market receives X net of the tariff share t / (1 + t), which goes to
  the importer's rebate account. Per-unit costs do not exist in P2 (iceberg only), so nothing is
  booked to the shipment owner yet.
- Retired: the inactive-cell netback rule and the trade SPEC deviation rows for FOB, delivered
  pricing and pro rata rationing.
- The GE reference (`python/ceteris/reference`) mirrors the new routing; V1 and the fixture
  golden are re-derived once, after all eight ADRs land (Section 25).

## Consequences

Trade no longer depends on the price level. Epsilon leaves the register; theta_g is the only
trade dispersion parameter.
