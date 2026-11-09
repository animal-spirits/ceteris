# RFC 0332. Status of ADR 0108 (estimation windows and long-run lanes)

Status: accepted
Date: 2026-11-09
Design sections: 6.2, 7.5, 10, 11, 15.1, 15.2, 17
Release: 1.0, P8
Research: issue #173, `specs/p8/PLAN.md` Branch A

## Summary

Issue #173 asks to implement ADR 0108's catch-up term; ADR 0108 is `proposed`. An audit against
`main` (2026-11-09) finds none of the decision built. Only the data registrations it needs exist.
This RFC records that and recommends leaving ADR 0108 proposed. ADRs 0001 to 0281 are a record
(rfcs/README.md), so a decision is this RFC and a status line, not an edit to the ADR file.

| Part of ADR 0108 | Built on main | Verdict |
|---|---|---|
| Catch-up from PWT 1960-2019, education and V-Dem interactions (2 parameters), offline panel regression | Nothing in `ceteris-calibrate`. PWT, V-Dem and WPP are registered in `sources.toml` (PWT is read by the ADR 0103 drift code, `ceteris-pack/src/econ/drift.rs`, which states the catch-up term "is not built") | not built |
| Catch-up term in the model, times the ADR 0031 sector-class factor; ADR 0103 drifts become 1995-2000 residuals | Not in `ceteris-econ`. ADR 0103 drifts are not netted. Needs the P4 sector-class factor (`specs/p4/PLAN.md`) | not built |
| Politics hazards from V-Dem 1900+ | V-Dem registered; no estimation | not built |
| War onset and termination from COW 1816+ and UCDP 1946+ | COW and UCDP registered; no estimation (ADR 0110) | not built |
| Mortality and fertility from HMD and WPP 1950+ | WPP registered; HMD not; no estimation | not built |
| Lanes: Jones lambda and phi, fertility floor, nuclear deterrence | No lane parameters; lanes exist for damages only (ADR 0036) | not built |
| Register: windows per parameter, lane flags | Not recorded | not built |

Every test named by ADR 0108 and by `specs/p8/PLAN.md` Tasks A1 and A2
(`catch_up_estimate_reproduces_the_published_benchmark_w...`, `catch_up_off_equals_status_quo`,
`drift_plus_catch_up_do_not_double_count_convergence`) is absent. The work is scheduled
as P8 Branch A (issues #173 and #220), after P4 and P7.

## Decisions

### D1. What status for ADR 0108?

Options:

- A. Keep proposed. Branch A builds it in P8; accept when its tests and goldens are green.
- B. Accept now as the design of record, with the build as P8 work.
- C. Accept the windows and lane list (the design) but keep the catch-up equation proposed
  until the PWT estimate exists.

Recommendation: A, because nothing is built or measured, and the 0318 precedent accepts what
has code and numbers behind it. The Right gate (CLAUDE.md) needs the published-benchmark
reproduction before acceptance; the catch-up equation's home depends on the P4 factor.

Decision: A, Antonio, 2026-11-09.
Row: none new; ADR 0108 keeps its row

## Spec changes

- None if D1 = A. `rfcs/README.md` keeps ADR 0108 `proposed`.
- If D1 = B or C: `rfcs/README.md` shows `accepted (RFC 0332)` or `accepted in part (RFC 0332)`.
- Issue #173 stays open, labelled P8 Branch A.
