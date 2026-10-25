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

## As built (Task 18e)

`python/ceteris/reference/calibrate.py` implements the decision. It departs from the text above
where the text's form cannot give an exact steady state of the model as built:

- **Household spending.** Stage 1 uses the households' PIGL spending at their own province's q_d
  and income, not Omega_g I_d. Stage 4 inverts the PIGL weights per country, at the unit value of
  the country's household purchases of each good and its mean income per person. Exactness: PIGL
  weights are per country, while buyer prices and incomes vary across a country's provinces and
  pops, so Omega_g I_d is not what the model's households spend at any weights. The national
  shares then miss Omega by at most 0.0060, with a median over countries of 0.0003 (2000 pack).
- **Passes and run time.** One outer loop settles rebates, r_g, buyer prices and the PIGL weights
  together, Anderson-mixed, to 1e-11 relative. It is not a rebate-only iteration of 3 to 5
  passes. On the 2000 pack it takes 46 passes and 11,010 Sinkhorn sweeps (about 40 ms each, cold
  start about 1,000), 473 s and 0.85 GB. Exactness: r_g and the PIGL weights depend on sourcing as
  the rebates do. The Sinkhorn tolerance (1e-13 in ln) leaves a floor near 1e-12 on the passes.
- **Per-good index scaling.** Stage 2 fixes one scale per traded good, not one in total: each
  good's part of the world price index (Omega_g times province household income as weights) is
  1, and local goods take p = 1, so the index is 1. Exactness: the scaling is unique up to a
  scale per good, since fixed values leave each good's units free, and the PIGL weights inverted
  at q absorb the choice.
- **a_VA.** Stage 3 uses a_VA = 1 - sum_k a_kg, the cost shares at the calibrated prices, so
  p_VA = p_o and A = 1 / p_o. Exactness: a_VA only sets the units of real VA. At base prices
  (`EconInputs::base_a_va`), coef = a_kg p_o / q_ik makes it nonpositive for 7,519 pack
  enterprises.
- **Local output with household closure.** Household income includes the calibrated local VA,
  so local output and income are one fixed point per province. Exactness: a steady state pays
  households the VA the model produces.
- **National profit ownership** (owner, 2026-10-25). Enterprise profits, VA less labour income
  (with the markets' cash flow), are pooled per country and paid per person to every pop of the
  country. Wages stay local, and the tariff rebate keeps its national per-person rule. Exactness:
  under provincial ownership, the 35 pack provinces without traded output earned only their
  rebate share. At that income PIGL buys no local goods, so their only steady state was empty
  (output 0, w0 = 0). With national ownership every province produces: the smallest local output
  is 67.8 and the smallest w0 is 2.05.

On the 2000 pack the reference model's residual at the calibrated point is 1.0e-11. The econ
SPEC (Income, Calibration) states the rules.
