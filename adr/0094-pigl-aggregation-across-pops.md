# 0094. PIGL aggregation across pops

Status: proposed
Date: 2026-10-25
Design sections: 8.6; econ SPEC Final demand; ADR 0021

## Context

Each pop draws income per person from a lognormal whose dispersion s comes from the national
Gini, s = 2 erfinv(G). Pops within a country already differ in mean income by province and
education, so the national Gini contains that between-pop spread and it is counted twice. The
Engel term exp(-eps (1 - eps) s^2 / 2) is biased, and the inversion, done for one lognormal at the
country mean and at prices 1, misses the 2000 aggregate shares (by up to 0.0018 on the fixture).

## Options

(1) Keep the national dispersion and the one-lognormal inversion. (2) Use within-pop
dispersion and invert over the country's pops at the calibrated buyer prices.

## Decision

Option 2.

- Within-pop dispersion per country: s_w^2 = max(0, s_nat^2 - Var_k(ln m_k)), the variance
  weighted by pop population, m_k the pop's base income per person.
- Inversion at q_d (ADR 0089 stage 4): with omega_A = 0, the country's population-weighted
  category shares over its pops are monotone in lambda. Solve lambda by a 1-D root so that they
  match Omega_A exactly, then omega_M, omega_S and alpha_g in closed form as now. The load still
  stops if z >= 1 or any omega_c < 0.

## Tests

- The aggregate data-state shares match Omega per country (1e-12).
- With one pop per country, the new inversion equals the old one at prices 1.
- The between and within variances add up to the national s^2 (or s_w = 0 is reported).

## Consequences

- One 1-D root per country at load; negligible cost.
- Countries whose between-pop spread already exceeds the national Gini get s_w = 0 and are
  listed in the calibration report.
