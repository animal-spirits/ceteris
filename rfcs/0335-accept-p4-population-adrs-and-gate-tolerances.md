# RFC 0335. Accept the P4 population ADRs and fix the P4 gate tolerances

Status: accepted
Date: 2026-11-09
Design sections: 6.1 to 6.3, 6.5, 15.4
Release: 1.0, P4
Research: none new; `specs/pop/SPEC.md` (this PR), `specs/p4/PLAN.md` Branch 0

## Summary

P4 builds on seven ADRs that still read proposed: 0031, 0032, 0033, 0074, 0105, 0109 and 0167.
No P4 code starts until they are accepted (`specs/p4/PLAN.md`, open point O1). This RFC puts the
acceptance, the tolerances of gate item 1 (fixed before any run, per the development laws), and
the licence line for cause shares (O3) to the owner. `specs/pop/SPEC.md` is the contract the
branches build against. The register codes P60 to P65, P126 and P127 named in those ADRs are not
in `spec/rows.toml` or the register; codes and decision ids are taken in the PR that accepts
this RFC, not here.

## Decisions

### D1. Accept ADRs 0031, 0032, 0033, 0074, 0105, 0109 and 0167 as written for the 1.0 path?

Options:

- A. Accept all seven as written, one decision, each keeping its text and its "Migration"
  phasing (0031 P2 part already built, P4 labour-status part here; 0033 hazards for P8 stay
  deferred; 0109 option 3 stays P6).
- B. Accept all except those the owner names, which stay proposed and their branches wait.
- C. Leave them proposed.

Recommendation: A, because each is scoped to P4 by `specs/p4/PLAN.md`, the post-1.0 parts are
already fenced out (issue #258), and Branch F plus A1 can start the day this is accepted.

Decision: A, Antonio, 2026-11-09 (decision card).
Row: none new; ADRs 0031, 0032, 0033, 0074, 0105, 0109 and 0167 keep their rows and read accepted (RFC 0335) in the index.

### D2. Tolerances for gate item 1 (UN 2000-2010)

Options:

- A. Pre-register: total fertility rate within 0.3 births per woman and life expectancy at
  birth within 1.5 years for at least 90% of countries weighted by population, urbanization
  share within 3 percentage points, total population within 3% in 2010, all against UN WPP
  2022 and UN DESA urbanization. Tolerances fixed now; no change after a run without an RFC.
- B. The same, with tolerances at twice A's, as a first gate to tighten at P9.
- C. Leave to Branch H.

Recommendation: A, because the development laws forbid moving a pass criterion after seeing
results, and 2000-2010 is in the calibration window, so the check is a fit check, not a
forecast.

Decision: A, Antonio, 2026-11-09 (decision card).
Row: ADR 0335 (spec/21 section 21.1, P4 gate).

### D3. Cause shares in the paid build (O3)

Options:

- A. Research pack only (WHO GHE, IHME are non-commercial, ADR 0050); the paid build keeps the
  all-cause Brass level and ships no cause split until the NC audit (#231) says otherwise.
- B. Ship World Bank re-published cause groups where CC BY 4.0 exists, else all-cause.

Recommendation: A, as the plan's default; B can follow #231 without code change.

Decision: B, Antonio, 2026-11-09 (decision card; recommendation was A).
Row: none new. Release pack: World Bank CC BY 4.0 cause groups where they exist, else the all-cause Brass level; `specs/pop/SPEC.md` Open questions and ADR 0033's data line follow this.

## Spec changes

- `specs/pop/SPEC.md` new (this PR).
- `rfcs/README.md`: the seven ADRs show `accepted (RFC 0335)`; ADR files are not rewritten.
- Register codes P162 to P170 in `spec/17` replace the codes written in the ADRs: P60 is P162,
  P61 P163, P62 P164, P63 P165, P65 P166, P126 P167, P127 P168; ADR 0167's labour loss is P169
  and ADR 0109's irregular share z is P170.
- `spec/21` section 21.1 item 5 carries the D2 tolerances; `spec/rows.toml` entry for ADR 0335.
- `specs/pop/SPEC.md` and `specs/p4/PLAN.md`: codes replaced, O1 and O3 closed.
