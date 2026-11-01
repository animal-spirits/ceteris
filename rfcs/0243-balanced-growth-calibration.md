# 0243. Balanced-growth calibration

Status: accepted (owner, 2026-10-29)
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

Option 2, country trends (owner, 2026-10-29). There is no fixed point of the detrended world
system, so the calibration target is the t0 point of a moving equilibrium path, and the test
target is the frictionless equilibrium solved along that path. 0089's zero-growth steady state
is the special case where every trend is zero.

- **Trends.** Per country i, monthly factors: labour force (1 + n_i), labour-augmenting
  productivity (1 + g_i), price level (1 + pi*_i). n_i is the country's labour-force growth,
  mean 1995 to 2005 (ILOSTAT), as u*_i is a 1995 to 2005 mean (0089). g_i is set by P8
  (frontier growth and catch-up; 0 before P8). pi*_i is 0 before P5a, then the country's
  inflation target. The trends are inputs to the calibration, not targets it hits: from t0 on,
  population and productivity are simulated (P4, P8), and the trends only set the t0 state.
- **Calibration.** 0089's stages 1 to 5 run unchanged on the 2000 values, giving the static
  equilibrium at t0. Only stage 5 and the wage rule change.
- **Stage 5, state.** Stocks and pipelines carry each country's trend: pipeline slot s holds
  the shipments of s months ago, deflated at the origin country's quantity trend
  (1 + n_o)(1 + g_o) per month and, for values, its price trend too. Inventory targets scale with
  sales (0090, 0091). pi^e_i = pi*_i.
- **Wage rule.** The data-state wage step becomes pi^e + g_i, so the natural-rate wedge holds
  wages on the country's trend rather than against it:

  ```
  dlnw = clip( pi^e + g_i + kappa_w (x_i + u*_i), -c_max, c_max ),  times (1 - delta_w) when negative
  ```

- **Demand.** Stage 4 inverts PIGL at the t0 buyer prices, as now. Along the path, real income
  per person grows at each country's rate and category shares follow Boppart's (2014) closed
  form at that income. That drift is part of the path, not a calibration error.
- **Moving reference.** Relative country sizes change, so trade shares and relative prices move
  from tick 1. The GE reference (`ceteris-reference`) solves the frictionless equilibrium at
  each date with every country's effective labour A_i(t) L_i(t) on its trend. Growth is slow
  (about 0.2% a month), so the ABM should track this quasi-static path closely; the gap
  measures the frictions, as V1 does today.

## Tests

- Nesting: with every trend zero, the calibration and the 0089 steady-state tests are
  unchanged, and every golden hash holds.
- Common trend: with equal n_i, g_i, pi*_i in every country and PIGL's non-homotheticity off,
  120 ticks from the calibrated state move no detrended price, wage or flow by more than 0089's
  tolerances (1e-9 on the fixture; the resolution-aware rule on the full pack).
- Country trends, frictionless: with the pack's n_i and g_i, detrended prices, wages and trade
  shares stay within 1% of the moving GE reference at every year to year 25 where the
  uniqueness condition holds (ADR 0022).
- No burn-in: with frictions on, the gap to the moving reference over ticks 1 to 12 is no larger
  than over ticks 109 to 120, so the start carries no transient of the kind 0089 removed.
- PIGL: category shares stay within 1e-9 of the Boppart closed form at each pop's income.
- Wage rule: at x = -u* the step is exactly pi^e + g_i.

## Consequences

- 0089 stays the calibration of the t0 equilibrium; this ADR adds the trends to its state and
  wage rule and replaces its fixed-point test with the moving reference. 0089's status line
  records the amendment.
- P2 and P3 are untouched: trends are zero until P4 turns n_i on, so no golden hash moves before
  then. P4 builds the trends and the moving reference, P5a adds pi*_i, P8 sets g_i.
- The exact detrended fixed-point test survives only in the common-trend case; with country
  trends, V1 becomes tracking of the moving reference, which costs one reference solve per
  sampled date (seconds each on the full pack, 0089).
- 2000 growth differences are carried into the start through n_i and g_i, so a country does not
  begin on a world-average path; whether they persist is up to P4 demography and P8 catch-up.
  The 2000 to 2010 growth gate of P8 tests that.
- **Money headroom.** 0089 found the i64 ledger overflows near 800 times the 2000 money stock.
  Nominal growth of 5% a year reaches that factor in about 137 years, and real growth of 3% in
  about 226, so open-ended runs overflow. Ledger money becomes i128 (owner, 2026-10-29, option
  (a)), built in P4; it moves every golden hash once. Redenominations would be visible events
  with no historical trigger (ADR 0241), and coarser minor units lose the resolution 0089's
  acceptance rule depends on.
- The 100-year invariant check (DESIGN 21.2) and the year-100 tick gate (ADR 0234) run on the
  growing path from P4.
- The econ SPEC Calibration and Labour sections change in P4, before the code.
