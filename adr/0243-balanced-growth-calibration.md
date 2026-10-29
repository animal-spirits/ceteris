# 0243. Balanced-growth calibration

Status: proposed
Date: 2026-10-29
Design sections: 8.6, 8.7, 13, 15.2, 15.3 (V1); econ SPEC Calibration, Labour; ADRs 0089, 0207
Phase: P4 (design and build); P5a (nominal trend)

## Context

ADR 0089 calibrates the 2000 data state as a steady state with no growth: population, technology
and the price level are constant, and from the calibrated state 120 ticks move nothing by more
than 1e-9 (resolution-aware on the full pack). That holds while P2 and P3 have static pops.
Once P4 grows the population, P8 grows productivity and P5a sets a nonzero inflation target, the
data state is no longer a fixed point of anything, and a run starts away from its own path in
the way 0089 was written to stop. ADR 0207 asks P4 to design the detrended calibration that
replaces the steady state "once anything grows". This ADR makes that replacement.

Two facts shape it. Countries grow at different rates, so the world as a whole has no balanced
growth path in levels. And PIGL demand (8.6) is non-homothetic: as real income grows, category
shares drift towards services, so even a common trend leaves shares moving. Boppart (2014) shows
that PIGL is consistent with a generalized balanced growth path: aggregates grow at constant
rates while sector shares and relative prices change along a closed-form path.

## Options

Trend structure:

1. **One world trend.** The reference path has world labour-force growth n, labour-augmenting
   productivity growth g and the inflation target pi*, common to every country. The 2000 data
   state is calibrated as the point of that path at t0. Country differences in growth are what
   P4 and P8 simulate, as deviations from the path.
2. **Country trends.** Each country carries its own n_i and g_i. Relative country sizes change,
   so trade shares move and there is no fixed point of the detrended system to calibrate to.
3. **No detrending.** Keep 0089's zero-growth steady state and detrend only in reporting. Runs
   with growth on start off their path, as P2 runs did before 0089.

Money headroom (see Consequences): (a) widen ledger money to i128; (b) redenominate a currency
when its money stock passes a threshold; (c) coarser minor units.

## Decision

Option 1, a balanced-growth path with one world trend. 0089's zero-growth steady state is its
special case g = n = pi* = 0.

- **Trend.** Monthly factors: labour force (1 + n), labour-augmenting productivity (1 + g), price
  level (1 + pi*). n is the world labour-force growth rate, mean 1995 to 2005 (ILOSTAT), as u*
  is a 1995 to 2005 mean (0089). g is the world frontier rate that P8 sets (0 before P8). pi* is
  0 before P5a, then the inflation target of the anchor currency.
- **Detrended variables.** Real quantities over (A_t L_t), nominal ones over P_t A_t L_t, wages
  over P_t A_t, prices over P_t. 0089's stages 1 to 5 run unchanged on the detrended values of
  2000.
- **Stage 5, state.** Stocks and pipelines sit on the path, not at a level: pipeline slot s holds
  the shipments of s months ago, deflated by gamma^s with gamma = (1 + n)(1 + g)(1 + pi*) for
  values and (1 + n)(1 + g) for quantities. Inventory targets scale with sales (0090, 0091).
  pi^e = pi*.
- **Wage rule.** The data-state wage step becomes pi^e + g, so the natural-rate wedge holds wages
  on trend rather than against it:

  ```
  dlnw = clip( pi^e + g + kappa_w (x_i + u*_i), -c_max, c_max ),  times (1 - delta_w) when negative
  ```

- **Demand.** Stage 4 inverts PIGL at the t0 buyer prices, as now. Along the path, real income
  per person grows at g and category shares follow Boppart's closed form. That drift is part of
  the path, not a calibration error.
- **Frictionless reference.** The GE reference solves the detrended system at each date of the
  path; with PIGL its shares come from the same closed form.

## Tests

- Nesting: with g = n = pi* = 0, the calibration and the 0089 steady-state tests are unchanged,
  and every golden hash holds.
- Homothetic path: with PIGL's non-homotheticity off and g, n, pi* > 0, 120 ticks from the
  calibrated state move no detrended price, wage or flow by more than 0089's tolerances (1e-9 on
  the fixture; the resolution-aware rule on the full pack).
- PIGL path: with non-homotheticity on, detrended aggregates hold to the same tolerances and
  category shares stay within 1e-9 of the Boppart closed form.
- V1: from the path perturbed by 10% in prices, wages and stocks, frictionless runs return to
  the detrended path within the 1% tolerances.
- Wage rule: at x = -u* the step is exactly pi^e + g.

## Consequences

- 0089 stays the calibration; this ADR moves its target from a steady state to a point on a
  balanced-growth path. 0089's status line records the amendment.
- P2 and P3 are untouched: growth is zero until P4 turns n on, so no golden hash moves before
  then. P4 builds the detrending, P5a adds pi*, P8 sets g.
- Country growth differences are not calibrated in. A country whose 2000 growth differs from the
  world trend starts on the world path and diverges through its own demography (P4) and
  catch-up (P8). The 2000 to 2010 growth gate of P8 tests that.
- **Money headroom.** 0089 found the i64 ledger overflows near 800 times the 2000 money stock.
  Nominal growth of 5% a year reaches that factor in about 137 years, and real growth of 3% in
  about 226, so open-ended runs overflow. Money option (a), i128 ledger money, is the default
  unless the owner picks otherwise; it moves every golden hash once, in P4. Option (b)
  redenominations are visible events with no historical trigger (ADR 0241); option (c) loses
  the resolution that 0089's acceptance rule depends on.
- The 100-year invariant check (DESIGN 21.2) and the year-100 tick gate (ADR 0234) run on the
  growing path from P4.
- The econ SPEC Calibration and Labour sections change in P4, before the code.
