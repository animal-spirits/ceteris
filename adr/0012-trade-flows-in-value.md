# 0012. Trade flows in value with a dimensionless gravity kernel

Status: proposed
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

## Consequences

Trade no longer depends on the price level. Epsilon leaves the register; theta_g is the only
trade dispersion parameter.
