# 0200. Selection on trade links

Status: proposed
Date: 2026-10-28
Design sections: 4.6, 7.2, 8.3, 15.3, 15.5; ADRs 0015, 0016, 0026, 0089, 0095; trade SPEC (buyer price row)
Phase: P2 fix, ahead of the P3 shock cases

## Context

Destinations spread spending over origins with Eaton-Kortum shares
pi_od = K_od c_od^-theta / Phi_d (ADR 0015), but buyers pay q_d = 1 / alpha_d with
alpha_d = sum_o pi_od / c_od, a spending-weighted harmonic mean of delivered costs (trade
SPEC, owner decision 2026-10-24). The goods bought from each origin are priced as if they were
that origin's average goods. In Eaton-Kortum a destination buys from a far origin only the goods
that origin is best at, so the price paid is the same for every origin and equals the index
P_d = Gamma Phi_d^(-1/theta). Melitz-Chaney (ADR 0026) gives the same result through firms:
only a cell's most productive firms serve distant markets.

Without selection, a rise in a border cost can lower q_d. The P2 benchmark of 2026-10-28
(https://claude.ai/artifact/RKzmS4xciyidJXTp1y1sja) found the consequences: the ACR welfare
formula (Arkolakis, Costinot and Rodriguez-Clare 2012) fails, Japan gains when its borders get
costlier, and the EU gains from Brexit. The trade SPEC rejected P_d because "a unit bought at
P_d delivers fewer than one unit"; that is the missing selection term, not a reason to drop the
index.
Deep dive: https://claude.ai/artifact/J7jLsKygA6qyLfLmvNXhNa

## Options

1. Status quo: q_d = 1 / alpha_d.
2. Buyers pay P_d and the quantity gap is absorbed as an unbooked residual.
3. A selection multiplier per link, booked as origin productivity, so buyers pay P_d and goods
   are conserved.

## Decision

Option 3. Option 2 breaks the goods invariant of 4.6.

Each active kernel leg carries

```latex
s_{od} = \frac{c_{od}}{P_d} = \Gamma^{-1} \left( \frac{K_{od}}{\pi_{od}} \right)^{1/\theta_g}
```

computed from the shares the kernel already holds. A unit shipped from o in base units arrives
in d as s_od delivered units, so alpha_d = sum_o pi_od s_od / c_od = 1 / P_d and q_d = P_d
exactly. The gain is real output, not a transfer: the goods o sells to d are made by the firms
of o that are best at them, which need 1 / s_od of the cell's average inputs per delivered
unit. The goods invariant counts selection as one explicit term beside iceberg melt.

With ADR 0026 built, the same multiplier has a firm reading. The share of cell o's firms that
serve d is n_od = (phi_min / phi*_od)^k, and the mean efficiency of those firms rises with the
cutoff, so s_od is proportional to n_od^(-1/k). Fixed export costs (P51) enter through the
cutoff.

Normalization: Gamma and the base unit are set so that s_dd = 1 at the 2000 data state. The
ADR 0089 steady state is then unchanged, and only counterfactuals move.

## Migration

- Trade SPEC: replace the buyer price row; s_od per active leg (one powf per leg per tick).
- Econ SPEC: enterprise input use per unit sold to d scales by 1 / s_od; the goods invariant
  gains the selection term.
- Calibration: check that stage 1 of the 0089 calibration reproduces the data state with
  s_dd = 1.
- Rerun the benchmark harness (branch claude/project-thread-cy0p06) once this lands.

## Tests

- Two-country toy: welfare change equals lambda_dd'^(-1/theta) to machine precision.
- A rise in tau on one link never lowers P_d in the destination.
- Benchmark: Japan no longer gains from costlier borders, and the EU no longer gains from Brexit.
- With all links at s = 1 (autarky toy), hashes equal the status quo.
- Goods invariant holds every tick with the selection term.

## Consequences

Welfare results of trade counterfactuals become comparable with the quantitative trade
literature. The 2000 calibration is unchanged. One float per active leg. No register entry.
