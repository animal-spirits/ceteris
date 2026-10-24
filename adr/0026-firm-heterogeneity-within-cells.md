# 0026. Firm heterogeneity within cells

Status: proposed
Date: 2026-10-24
Design sections: 7.2, 7.4, 8.3, 15.3 (V2), 17.1

## Context

One enterprise per (province, sector, ownership) has no extensive margin of trade, no firm-size
distribution and no exporter selection. The V2 firm-size target had to be dropped, and trade
liberalization worked only through the intensive margin.

## Options

(1) One representative enterprise per cell. (2) Explicit firms. (3) A Pareto distribution of
productivity within each cell, aggregated in closed form (Melitz 2003; Chaney 2008).

## Decision

Option 3. Explicit firms multiply agents and parameters for little out-of-sample gain, which
15.7 already excludes. With Pareto productivity of shape k_g and CES varieties of elasticity
sigma_g (k_g > sigma_g - 1), aggregate flows keep the gravity form of 8.3 with trade
elasticity k_g, times a fixed-export-cost term:

```latex
X_{od,g} \propto \tau_{od,g}^{-k_g} \, f_{od,g}^{\,1 - k_g / (\sigma_g - 1)}
```

Fixed export costs vary by border crossing, so they multiply the kernel on gateway links and
the factorization of 8.3 is unchanged. Each cell's firm count, size distribution and exporter
share are closed form.

## Migration

- Reinterpret theta_g (P01) as the Pareto shape k_g; the PPML estimates of 0022 are
  unchanged.
- Add sigma_g (P50), fixed export costs by border (P51, identified by exporter shares) and
  entry costs (P52, identified by firm counts by size class) to the register.
- Gateway links carry the fixed-cost factor beside tariffs.
- Each enterprise cell stores its mass of firms and productivity lower bound; the quarterly
  investment step (7.4) becomes free entry and exit in that mass.
- Restore the V2 firm-size target as the Pareto tail within cells.

## Tests

- A toy cell's exporter share and size distribution match the closed form.
- With fixed export costs at zero, flows equal the 8.3 kernel.
- PPML on simulated flows recovers k_g.

## Consequences

Liberalization now moves the extensive margin. Granular shocks from individual large firms
(Gabaix 2011) are still absent; they could later come from drawing each country-sector's
largest firms explicitly from the tail.
