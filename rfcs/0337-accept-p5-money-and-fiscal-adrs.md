# RFC 0337. Accept the P5 money and fiscal ADRs

Status: accepted
Date: 2026-11-10
Design sections: 3, 4.4, 7.2, 7.4, 8.1 to 8.3, 9.1 to 9.6, 9.10, 9.11, 15.3 to 15.5, 21.1
Release: 1.0, P5
Research: none new; `specs/money/SPEC.md` (separate draft PR), `specs/p5/PLAN.md` Branch 0

## Summary

P5 builds on ten ADRs that still read proposed: 0025, 0040, 0042, 0043, 0080, 0090, 0168, 0176, 0202
and 0204 (`specs/p5/PLAN.md`, open point O1). No P5 code starts until they are accepted. 0090 was
deferred to P5 by the owner on 2026-10-30 and its sibling RFC 0286 (storage-on fix, status deferred) is
built by the same branch (Branch E), so 0090 is put to the owner here with the rest. One decision per
ADR, so the owner can accept some and hold others. `specs/money/SPEC.md` is the contract the branches
build against and is drafted in its own PR.

Dependencies this RFC does not accept: ADR 0095 (0042 is read with the amendment already merged into
its text), ADR 0172 (forbearance and trade finance; 0202's zombie share is zero without it), ADR 0026
(firm heterogeneity), ADR 0170 and 0111 (cited by 0204 and 0168). They stay as they are; none is
a P5 gate dependency.

Register codes P74, P75, P77, P78, P79, P80 and P143 are named in these ADRs but are not in
`spec/17` or `spec/rows.toml`; codes P10, P35, P37, P50, P52, P56, P03a and P03b exist and widen as the
ADRs say. Codes P171 to P177 for those seven are taken in this accepting PR (table under Spec changes); no new decision id is needed.

## Decisions

### D1. Accept ADR 0025 (Sequence-space HANK reference) as written for the 1.0 path?

Options:

- A. Accept as written: Branch H builds the reference in `ceteris-reference` and gate item 3 compares to it; P37 is the shared MPC target.
- B. Keep proposed; the branches that depend on it wait.

Recommendation: A, because the P5b gate has no reference without it and the MPC harness already runs.

Decision: A, Antonio, 2026-11-10 (decision card).
Row: none new; ADR 0025 keeps its row and reads accepted (RFC 0337) in the index.

### D2. Accept ADR 0040 (Housing supply elasticity and mortgage underwriting) as written for the 1.0 path?

Options:

- A. Accept as written: Branch F2: province supply elasticity (P74), LTV, LTI and DSTI caps, investor demand (P75).
- B. Keep proposed; the branches that depend on it wait.

Recommendation: A, because spec/09 9.10 and 9.11 already assume the housing block and the caps are macroprudential levers.

Decision: A, Antonio, 2026-11-10 (decision card).
Row: none new; ADR 0040 keeps its row and reads accepted (RFC 0337) in the index.

### D3. Accept ADR 0042 (Public capital and fiscal dynamics, with its ADR 0095 amendment) as written for the 1.0 path?

Options:

- A. Accept as written: Branch C2: public capital in A (P77), efficiency factor (P78), r minus g reported.
- B. Keep proposed; the branches that depend on it wait.

Recommendation: A, because fiscal capacity and public investment are needed for the P5a government account; the P77 = 0 switch reproduces the status quo.

Decision: A, Antonio, 2026-11-10 (decision card).
Row: none new; ADR 0042 keeps its row and reads accepted (RFC 0337) in the index.

### D4. Accept ADR 0043 (Sovereign debt restructuring and creditor structure) as written for the 1.0 path?

Options:

- A. Accept as written: Branch G2: creditor classes, bargaining on capacity to pay, reputation period (P79, P80).
- B. Keep proposed; the branches that depend on it wait.

Recommendation: A, because it is a P5b system with a switch and a literature target (Cruces and Trebesch); gate item 4 needs only G1.

Decision: A, Antonio, 2026-11-10 (decision card).
Row: none new; ADR 0043 keeps its row and reads accepted (RFC 0337) in the index.

### D5. Accept ADR 0080 (Polycrisis coupling audit) as written for the 1.0 path?

Options:

- A. Accept as written: `specs/COUPLINGS.md` and a register check at every gate from P5a.
- B. Keep proposed; the branches that depend on it wait.

Recommendation: A, because it is a process rule with no free parameter; the register starts with the couplings already tested through P4.

Decision: A, Antonio, 2026-11-10 (decision card).
Row: none new; ADR 0080 keeps its row and reads accepted (RFC 0337) in the index.

### D6. Accept ADR 0090 (Split price rule and inventory gain (deferred to P5 by the owner, 2026-10-30)) as written for the 1.0 path?

Options:

- A. Accept as written: Branch E1: origin-market rule with kappa_r, kappa_n (P143), kappa2; one world reduction per tick.
- B. Keep proposed; the branches that depend on it wait.

Recommendation: A, because the P5 criterion (x10 border shock converges frictionless) rests on it, and Branch E runs before any re-bless so one golden move covers it.

Decision: A, Antonio, 2026-11-10 (decision card).
Row: none new; ADR 0090 keeps its row and reads accepted (RFC 0337) in the index.

### D7. Accept ADR 0168 (Cross-border services trade and tourism) as written for the 1.0 path?

Options:

- A. Accept as written: Branch J: services gravity kernel without routes, tourism logit, border closures as wedges.
- B. Keep proposed; the branches that depend on it wait.

Recommendation: A, because it is independent of the finance branches after A; BaTiS and WDI data are free.

Decision: A, Antonio, 2026-11-10 (decision card).
Row: none new; ADR 0168 keeps its row and reads accepted (RFC 0337) in the index.

### D8. Accept ADR 0176 (Endogenous neutral rate in the policy rule) as written for the 1.0 path?

Options:

- A. Accept as written: Branch B3: annual HLW Kalman filter, P35 as the 2000 initial value.
- B. Keep proposed; the branches that depend on it wait.

Recommendation: A, because without it the 2009-2021 policy rates sit too high and the 15.4 lower-bound target is missed.

Decision: A, Antonio, 2026-11-10 (decision card).
Row: none new; ADR 0176 keeps its row and reads accepted (RFC 0337) in the index.

### D9. Accept ADR 0202 (Firm entry, exit and the zombie band) as written for the 1.0 path?

Options:

- A. Accept as written: Branch D2: free entry, exit cutoff, zombie share; sigma_g identified by Zipf (P50 range widens to 2 to 11).
- B. Keep proposed; the branches that depend on it wait.

Recommendation: A, because entry and exit are needed for the investment step; with the bank capital gap at zero there are no zombies, so it can land before banks.

Decision: A, Antonio, 2026-11-10 (decision card).
Row: none new; ADR 0202 keeps its row and reads accepted (RFC 0337) in the index.

### D10. Accept ADR 0204 (Capital formation by asset class and time to build) as written for the 1.0 path?

Options:

- A. Accept as written: Branch D1: five asset classes, orders with a schedule, geometric depreciation by class (P10 range widens to 1 to 35%).
- B. Keep proposed; the branches that depend on it wait.

Recommendation: A, because the investment step of 7.4 depends on it and the 2000 pipeline is the 0089 steady state, so the data state stays a fixed point.

Decision: A, Antonio, 2026-11-10 (decision card).
Row: none new; ADR 0204 keeps its row and reads accepted (RFC 0337) in the index.

## Spec changes

Made in this PR:

- `rfcs/README.md`: each accepted ADR shows `accepted (RFC 0337)`; ADR files are not rewritten.
- `spec/17`: P74 is P171, P75 P172, P77 P173, P78 P174, P79 P175, P80 P176 and P143 P177 (from
  `-- next-code P`), replacing the codes written in the ADRs; widened ranges of P10 and P50 as ADRs 0204 and 0202 state.
- `spec/rows.toml`: entry for this RFC and the status of each decided ADR.
- `specs/p5/PLAN.md`: O1 closed, codes replaced. `specs/money/SPEC.md`: status follows its own PR.
- `specs/COUPLINGS.md` (ADR 0080), with the module pairs implemented through P4 and their tests.
