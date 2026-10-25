# 0089. Data-state steady-state calibration and natural-rate unemployment

Status: accepted (owner, 2026-10-25; core pulled into P2 before the gate)
Date: 2026-10-25
Design sections: 8.7, 15.2, 15.3 (V1), 19.4; econ SPEC Calibration, Labour; ADR 0022

## Context

P2 calibrates productivity A jointly with prices and wages so that the equilibrium reproduces
2000 value added. The joint system has several roots, and the homotopy from A = 1 stops at a fold
(lambda 0.199 on the fixture, PLAN Task 18b). Runs then start from a data state that is not the
equilibrium: prices 1, 2000 unemployment against a full-employment steady state, and an empty
transit pipeline. On the fixture the run passes through a deep coal-led shortage and needs
1,200 ticks to reach V1 tolerance. Every calibration, ensemble and scenario run would pay that
burn-in.

## Options

(1) Keep the joint inversion and improve the continuation. (2) Hold the 2000 values fixed and
solve prices first: at given values, origin prices are the unique solution of a matrix-scaling
problem, and everything else follows in closed form, so the data state is the steady state.

For labour at the data state: (a) labour supply equal to 2000 employment; (b) wages that clear
the full labour force; (c) a natural-rate wedge in the wage rule.

## Decision

Option 2, with labour option (c) (owner, 2026-10-25).

**Stage 1, values.** Output value per enterprise V_o = VA_o / (1 - sum_k a_kg). Destination
spending by good in value, from data alone:

```
E_dg = Omega_g I_d + sum_g' a_gg' V_dg',    I_d = VA_d + rebate_d
```

World supply and spending of each good differ by the 2000 imbalances: rescale each good's V_o by
one scalar r_g and report it. The rebate depends on sourcing: iterate stages 1 and 2 until it
settles (3 to 5 passes).

**Stage 2, origin prices.** Per traded good, balanced matrix scaling on the two-level kernel:

```
sum_d E_d K_od w_o / ((1 + t_od) Phi_d) = V_o,    Phi_d = sum_o K_od w_o
```

The solution is unique up to scale (Sinkhorn 1967). The scale is fixed by the world index at 1.
Then p_o = w_o^(-1/theta), and q_d and L_d follow from sourcing. Solve in the log domain
(ADR 0093); if plain sweeps contract slowly at theta 12.55, use Anderson acceleration or Newton
on the convex dual.

**Stage 3, technology.** Keep the 2000 value shares at the solved prices:

```
coef_kg = a_kg p_o / q_ik,    Y_o = V_o / p_o,    A = 1 / p_VA,    L0 = s_L VA0 / w0
```

With L = L0 the normalized CES gives nominal VA = p_VA A VA0 = VA0, and the labour first-order
condition holds at L0. `Layout.coef` already stores coefficients per enterprise. S09
FixedLeontief reads the same calibrated column, so one vintage still equals fixed Leontief.
Local goods take p = 1, and local output solves the 3 x 3 Leontief system per province given
local demand. Local VA is therefore an output of calibration; its national gap to ICIO is
reported.

**Stage 4, demand weights.** Invert PIGL at q_d, not at prices 1 (ADR 0094).

**Stage 5, state.** Stocks at their steady-state level (ADRs 0090, 0091). Each pipeline slot s is
pre-filled with one month of steady-state shipments for every route of lag s or more. Money is
one month of income, pi^e = 0, and the last D, Q and sales are their steady-state values.

**Natural-rate wedge.** L^s stays the full effective labour force. The wage rule becomes

```
dlnw = clip( pi^e + kappa_w (x_i + u*_i), -c_max, c_max ),  times (1 - delta_w) when negative
```

u*_i is the country's mean unemployment rate 1995 to 2005 (ILOSTAT modelled estimates), so the
2000 cycle is not frozen in. The base point uses it: w0_i = sum_g s_L VA0_ig / ((1 - u*_i) L^s_i).
At the data state x_i = -u*_i, and the wage step is zero. Employment may exceed (1 - u*) L^s in
a boom, up to L^s. Frictionless mode solves sum_g L*_ig(w) = (1 - u*_i) L^s_i.

## Tests

- Fixed point: from the calibrated state, 120 ticks with all frictions on move no price, wage
  or flow by more than 1e-9 relative.
- Uniqueness of stage 2: two starts of the scaling reach the same w up to scale (1e-10).
- V1: from the steady state perturbed by 10% in prices, wages and stocks, and from the A = 1
  equilibrium, frictionless runs return within the 1% tolerances.
- Stage 3 closed forms against the reference model's residual at the calibrated point (1e-10).
- Wage wedge: at x = -u* the step is exactly pi^e; frictionless unemployment equals u*.

## Consequences

- The homotopy and its fold are removed. Static calibration is about seven Sinkhorn solves plus
  closed forms: seconds on the full pack, with no simulation.
- The reference solver becomes a residual check and the tool for counterfactual equilibria.
- Steady-state unemployment is u*, not zero: this supersedes the full-employment owner choice
  of the econ SPEC. P4 matching replaces u* with matching frictions.
- Input coefficients now differ across enterprises of one country, by relative delivered
  prices.
- `reference/calibrate.py`, the econ SPEC Calibration section and the pack's `econ_countries`
  table (u* column) change.
