# RFC 0340. Drop usage statistics for 1.0

Status: accepted
Date: 2026-11-10
Design sections: 4.6, 14.3, 20
Release: 1.0
Research: [Infrastructure gaps for 1.0, 2026-11-10](../reports/infrastructure-gaps-2026-11-10.md)

## Summary

ADR 0214 chose opt-in usage statistics sent weekly to a collector the project runs, and left the
collector's host and payer open. A collector is the project's first hosted service, with a cost,
an operator and a privacy duty, and 1.0 is a free pure simulator with no server. This RFC drops the
usage statistics part of ADR 0214 for 1.0: no collector, no opt-in counters, no install ID, no
settings panel for them, no network request to a project host. The rest of ADR 0214 stands: typed
halts, local repro bundles, `ceteris replay`, local rotating logs. Bundles are still never
uploaded; the user attaches one to an issue.

## Decisions

### D1. Does 1.0 send usage statistics?

Options:

- A. Drop for 1.0: no collector and no counters. ADR 0214's usage statistics section is
  superseded; halts, bundles and logging are unchanged. Revisiting is a new RFC.
- B. Keep opt-in statistics and name a collector host and payer before 1.0.

Recommendation: A, because it removes the project's only planned infrastructure and its privacy
surface, and local bundles already make every failure reproducible.

Decision: A, Antonio, 2026-11-10.
Row: ADR 0342

## Spec changes

- ADR 0214: the usage statistics paragraph, its tests (no request to the collector, payload
  schema) and the R-phase statistics client, settings panel and collector are superseded
  (ADR 0342). Halts, bundles and logging are unchanged.
- `docs/privacy.md`: the usage statistics bullet is replaced by a statement that the app sends no
  usage statistics.
- 1.0 estimate (RFC 0248): the R phase loses the usage statistics line.
