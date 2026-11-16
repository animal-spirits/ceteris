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

### D2. What is theta_2 in this change?

Options:

- A. The 3-point estimate 10.0, then a Mac `calibrate --no-epsilon-fit` run refines it to the root
  of the nu-0 share residual, recorded as a dated default.
- B. The literature 13.53 until the fit runs.

Recommendation: A. Pending Antonio's decision.

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
