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

## Amendment (2026-10-29): one joint solve per country at the pops' own prices

Owner ruling (2026-10-29, option 3B of the first attempt). The Decision's 1-D root on lambda
with omega and alpha in closed form holds only where every pop of a country faces the same
prices. Pops buy at their own province's buyer prices, which differ within a country, so the
inversion is one joint solve per country, at the prices each pop faces, and the aggregate it
matches is spending-weighted, not population-weighted.

**System.** Country c has pops k with spending E_k (> 0), income per person m_k, buyer prices
q_kg (their province's; 1 where no prices are given) and the within-pop dispersion s_w of the
Decision. Pop k's budget share of good g is PIGL's, s_kg = S_kc(g) a_kg, with the within-category
share a_kg = alpha_g q_kg^(1 - sigma) / sum_{g' in c(g)} alpha_g' q_kg'^(1 - sigma) (P06) and the
category share S_kc = sB_kc + lambda (Dt_k / B_k)^gamma (m_k / B_k)^-eps exp(-eps (1 - eps)
s_w^2 / 2) (sD_kc - sB_kc), unclamped (P46, P47). The equations are, for every good,

    sum_k E_k s_kg = Omega_g sum_k E_k,

with Omega normalized. The unknowns are lambda, the category weights omega with omega_A = 0
and omega_M + omega_S = 1, and alpha_g summing to 1 within each category over the goods with
Omega_g > 0 (alpha_g = 0 elsewhere). Equations and unknowns both number one less than the goods
with demand.

**Solver.** Nested, from alpha_g = Omega_g / Omega_c(g):

1. At the current alpha, every pop's category indices, within shares, Dt_k and sD_k are fixed.
   For a split t = omega_M, lambda follows in closed form from food (sB_A = 0):
   lambda(t) = Omega_A sum E_k / sum_k E_k f_k(t) sD_kA, f_k the factor of lambda above. The
   split solves the M equation, h(t) = sum_k E_k S_kM(t) - Omega_M sum E_k = 0, by a bracketed
   secant (Illinois) on [0, 1] to the last bit. A bracket without a sign change means a
   negative weight, and the load or calibration stops as before.
2. At those category shares, alpha_g is rescaled by Omega_g sum E_k / sum_k E_k s_kg and
   renormalized within its category (the contraction of Berry, Levinsohn and Pakes 1995 on a
   mixture of CES shares).

The two steps repeat until the largest |sum_k E_k s_kg / (Omega_g sum E_k) - 1| over goods with
demand is at most 1e-14, or stops falling after it is below 1e-12; more than 1,000 rounds is an
error. With one pop, or every pop at the same prices and income, step 2 returns the start and
step 1 the closed form, so the old inversion is the one-pop case.

**Where it runs.** Calibration stage 4 solves it each pass at the pass's province buyer prices
and pop incomes (rebates included), weights E_k the pops' incomes. Without a calibration the
simulator and the GE reference solve it at prices 1 over the pops' base incomes, rebates aside.
s_w uses each pop's base income per person rebates aside, w0 (1 - u*) times its effective labour
plus its population share of the country's profit pool, over its population; it is fixed by the
inputs, travels with the weights (`pigl_sigma` in the calibration's country table) and is the
dispersion every pop of the country uses.

**Tests.** The aggregate data-state shares match Omega per country at 1e-12, with the shares
evaluated at the prices and incomes the inversion used; with one pop per country and prices 1
the new inversion equals the closed form; s_w^2 plus the between-pop variance equals s_nat^2
wherever s_w > 0.

## As built

Built in pre-p3/econ-review as the Amendment states: `ceteris_econ::demand::invert_pigl_pops` and
`within_sigma` (the simulator's load without a calibration), `ceteris_reference::demand::invert_pops`
and `within_sigma` (the reference and calibration stage 4), each written independently. The
weights carry s_w (`PiglWeights::sigma`, `Weights::sigma`, `pigl_sigma` in
`econ_calib_countries.arrow`), so a calibrated world uses the dispersion its weights were
inverted with. The calibration summary lists the countries with s_w = 0 (`within_sigma_zero`);
none on the fixture. On the fixture the joint inversion settles in a few rounds per country and
the aggregate data-state shares match Omega at 1e-12.
