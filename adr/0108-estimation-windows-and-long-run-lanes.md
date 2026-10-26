# 0108. Estimation windows per module and long-run lanes

Status: proposed
Date: 2026-10-26
Design sections: 6.2, 7.5, 10, 11, 15.1, 15.2, 17; amends ADRs 0024, 0031, 0032, 0036, 0103

## Context

- **One window for all modules.** Every module initializes in 2000 and, as designed, calibrates
  on 2000-2010. Ten years contain one cycle, a handful of interstate wars, few regime changes and
  almost no fertility variation in most countries. For politics, war and demography the window
  identifies little; their parameters are set by priors while looking estimated.
- **Catch-up fitted on an exceptional decade.** Technology catch-up (7.5) estimated on
  2000-2010 inherits China's and the commodity boom's convergence and projects it everywhere.
  Long-run growth data show convergence that is conditional, slow and interrupted (middle-income
  stagnation).
- **Unidentifiable long-run drivers.** Frontier research elasticities (Jones), the fertility
  floor and climate damages cannot be identified on 2000-2024 and dominate runs over 30 years.
  Only damages are lanes (0036).

## Options

1. One window, as designed.
2. Initial state from 2000 for all; each module estimated offline on the longest reliable
   panel for its process; unidentifiable drivers as lanes.
3. Joint estimation over full history by simulated likelihood.

## Decision

Option 2.

- **Windows.**
  - Economy: 2000-2010 (unchanged).
  - Technology catch-up: PWT 1960-2019, with catch-up speed depending on education and the V-Dem
    institution vector (interaction terms, two parameters), by panel regression offline. This is
    the catch-up equation's single home: ADR 0031's sector-class factor (P60) multiplies its
    rate, and ADR 0103's drifts are the 1995-2000 residuals left after it, so convergence is not
    counted twice.
  - Politics (10): V-Dem 1900+ for transition hazards.
  - War (11): COW 1816+ and UCDP 1946+ for onset and termination (with ADR 0110).
  - Mortality and fertility (6.2): HMD and WPP 1950+.
  Each estimate is a Literature-type register entry with its range, so it enters ensembles
  through parameter lanes (0107).
- **Lanes.** Frontier growth (Jones lambda and phi: low, central, high from Bloom et al. 2020
  and Jones 2022) and the fertility floor (WPP low, medium and high variants) become lanes
  like damages. Nuclear deterrence strength (11.2) becomes a lane (weak, central, strong).
  Long-run outputs are reported per lane, never pooled across lanes.

## Migration

- python/ceteris/calibrate: per-module estimation scripts with their own data manifests.
- Register: windows recorded per parameter; lane parameters marked as lanes (excluded from
  stacking, as 0053 does for damages).

## Tests

- Each offline estimate reproduces a published benchmark on the same data within its standard
  error (e.g. the peace-years coefficient in Beck, Katz and Tucker 1998).

## Consequences

- Modules are identified on data that contain their events.
- Long-run results become conditional statements, which is what the data support.
- More offline estimation work, paced with each module's phase.
