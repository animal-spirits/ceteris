# RFC 0355. Good 2 (coal and other mining) takes its own theta class

Status: proposed
Date: 2026-11-15
Design sections: 8.3, 17.1 (P01, P05); ADRs 0023, 0274, 0276, 0354
Release: 1.0 (P4)
Research: reports/class3-good2-spike-2026-11-15.md (project files); issue #595

## Summary

ADR 0023 puts crude oil and refined fuels (good 1) and coal and other mining (good 2) in one
theta class at the trade-weighted harmonic mean 16.91, set by refined petroleum (64.85). Good 2's
ICIO 2000 cross-border share is 0.2583; with every friction at its floor the model gives 0.2057 at
theta 16.91, 0.2209 at 13.53 and 0.2780 at 8.0 (nu 0, Mac runs 2026-11-15, theta verified in the
parameters). nu_2 therefore clamps at 0 (ADR 0092 amendment), and ADR 0354's s_c cannot help (it only
lowers the share). Linear interpolation of the three points crosses the target near theta 10, inside
two s.e. of the Mining estimate 13.53 (s.e. 3.67, ADR 0023).

This RFC lifts the class count from 4 to 5: good 2 becomes class 3, with its own theta P01, fitted so
its share at nu 0 meets the ICIO target within the literature band (13.53 +- 2 s.e. = 6.2 to 20.9);
fuels become class 4 at 16.91. Classes stay ordered by theta. The kernel is unchanged; the class
count only changes parameter arrays (bulk freight P154 and P155, rho P53, d_0 and s_c per class) and
H2 tests. ADR 0276's per-good split of class 3 is subsumed: each friction group is a class.

## Decisions

### D1. How does good 2's ICIO target become reachable?

Options:

- A. Own theta class for good 2, theta fitted to its target within the literature band.
- B. Accept good 2 as a known clamp (RFC 0354 D1 option B for that good).
- C. Allow nu below 0 (a cross-border subsidy; no economic reading).

Recommendation: A.

Decision: A, Antonio, 2026-11-15 (question card, class 3 good 2 thread).
Row: to be taken in the spec PR.

### D2. What is theta_2 in this change? (superseded for the epsilon-on case by D3)

Options:

- A. The 3-point estimate 10.0, then a Mac `calibrate --no-epsilon-fit` run refines it to the root
  of the nu-0 share residual, recorded as a dated default.
- B. The literature 13.53 until the fit runs.

Recommendation: A. Pending Antonio's decision.

### D3. How is theta_2 fitted once epsilon is on?

Context: the first epsilon refit with theta_2 = 9.27 (fitted at epsilon 0, share 0.258310 against the
target 0.258298) failed on the Mac (2026-11-16, 3,485 s): "the pooled distance coefficient is still
above the data's -0.7512 at epsilon 2"; the last step reported class 3 outside the model's reach
even at epsilon scale 0 (residual -0.00329). The distance cost lowers good 2's share, so a theta
fitted at epsilon 0 is too high once epsilon is on.

Options:

- A. Fit theta_2 jointly with epsilon: for each epsilon trial, theta_2 is the root in the literature
  band (6.2 to 20.9) of good 2's nu-0 share residual at that trial's distance cost.
- B. Report good 2 as a known clamp (epsilon closure over the other classes).
- C. Epsilon off for 1.0.

Recommendation: A.

Decision: A, Antonio, 2026-11-17 (question card, class 3 good 2 thread).
Row: to be taken in the spec PR.

### D4. How does a fit's scaling stop when good 2's residual plateaus? (decided: option A, Antonio 2026-11-17)

The joint theta_2 fit probes thetas where good 2's Brazil block (a near-closed block, 0.999 of its
coupling) leaves the scaling's residual at about 4e-13, above the absolute `SINKHORN_TOL` 1e-13,
and the scaling runs its 100,000-sweep cap and errors (Mac, 1893 s, 2026-11-17).

- A. Stagnation-aware stop, fits only: the evaluation the fits use (`steady::sinkhorn_plateau`,
  `data_state_spending`, `border` evaluations) ends the scaling when the residual is within
  `PLATEAU_TOL` 1e-11 and its smallest value in a window of `PLATEAU_SWEEPS` 5,000 sweeps is not
  under half the previous window's (the slow mode contracts 0.01 a window); the residual is returned, not hidden. The calibration proper keeps the strict rule.
  Recommended: it fixes the cause for every theta, and a share to 1e-9 needs no more than 1e-11.
- A (also, 2026-11-17, Antonio's card): the data state's rebate and weights passes contracted about
  3% a pass on the pack (300-pass cap overrun, Mac log), so they are Anderson-mixed as the
  calibration's are (fixture 17 passes to 11; same fixed point to 5e-14).
- B. Narrow the theta band to where calibrate settles: no tolerance change, but theta 9.27 sits
  next to the unstable range.
- C. Stop and treat it as a structure issue.

### D5. At which state is theta_2 fitted? (decided: refit inside the scale loop, Antonio 2026-11-18)

The first joint fit evaluated good 2's share with the other classes at s_c 1 and their starting nu.
The trial then refits those, which moves good 2's prices, and the Mac refit on 6f4b048 (9 h 11 min)
ended with class 3 0.00093 short of its target at scale 0, theta_2 strictly inside the band.
theta_2 is now refitted, in every pass of the trial's scale loop, at the current s_c and nu of the
other classes, until it moves less than 1e-4. Test: targets made with class 1 clamped return theta_2
to 1e-3 and class 3 meets its share at nu 0.

## Consequences

- N_CLASSES 5; `theta_class` [2, 4, 3, 0, 2, 0, 1, ...]; bulk freight rates apply to classes 3 and 4.
- The pack's theta table (`econ_goods.arrow`) is not read by the run: `export_pack` takes theta from
  `TradeParams`. The pack build's ADR 0023 rule still emits four classes and needs the same split.
- Fixture goldens, the committed fixture calibration and reference, and the fixture's BALANCE (ADR
  0299) move (a reviewed event, CHANGELOG).
- H2 per class gates fuels alone: with six crossings in the fixture its regression is not identified
  and is skipped below eight rows.

## Spec changes

Written in the spec PR once D2 is decided: 8.3 and 17.1 P01 (class count and theta_2), trade SPEC
Parameters, econ SPEC Calibration (friction groups are the classes).
