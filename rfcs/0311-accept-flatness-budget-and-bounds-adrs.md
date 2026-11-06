# RFC 0311. Accept the late-game flatness gate, per-good budget lines and bounded state tables

Status: accepted
Date: 2026-11-06
Design sections: 19.2, 19.4, 21.2
Release: 1.0, P3
Research: ADRs 0234, 0235 and 0236 (assessment linked there)

## Summary

ADRs 0234, 0235 and 0236 record the owner's choices of 2026-10-29 but still read
`Status: proposed`, so their migrations to the design were never applied. This RFC accepts them as
written and applies the migrations.

## Decisions

### D1. Accept ADR 0234, the late-game flatness gate?

Options:

- A. Accept as written: option 3 of ADR 0234 and its gate.
- B. Reopen.

Recommendation: A, because the owner chose it on 2026-10-29.

Decision: A, Antonio, 2026-10-29 (accepted in the PR for #248, 2026-11-06).
Row: ADR 0234

### D2. Accept ADR 0235, budget lines from measured per-good cost?

Options:

- A. Accept as written: option 3 of ADR 0235.
- B. Reopen.

Recommendation: A, because the owner chose it on 2026-10-29.

Decision: A, Antonio, 2026-10-29 (accepted in the PR for #248, 2026-11-06).
Row: ADR 0235

### D3. Accept ADR 0236, bounded state tables?

Options:

- A. Accept as written: option 2 of ADR 0236.
- B. Reopen.

Recommendation: A, because the owner chose it on 2026-10-29.

Decision: A, Antonio, 2026-10-29 (accepted in the PR for #248, 2026-11-06).
Row: ADR 0236

## Spec changes

- 19.2: the per-system lines are restated as fixed cost plus cost per good once the P3 scaling
  benchmark fits them (ADR 0235); the 160 ms ceiling stays.
- 19.4: the bounded-state-table rule (ADR 0236).
- 21.2 item 8: the definition of done includes the flatness gate (ADR 0234).
- ADRs 0234 to 0236: no rewrite; this RFC is their acceptance record.
- ADR 0212's disk row for 19.2 waits for its own acceptance.

## Consequences

- Each module SPEC from P3 on names its table bounds.
- The 100-year flatness run and its report are P3 deliverables (Branch G).
