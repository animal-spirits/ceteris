# RFC 0315. Snapshot chunks are zstd pieces of the canonical bytes, not Arrow tables

Status: accepted
Date: 2026-11-06
Design sections: 14.3
Release: 1.0, P3
Research: `specs/p3/PLAN.md` Task A2

## Summary

Section 14.3 says snapshots are "compressed columnar chunks", content-addressed so unchanged
tables are shared across branches, and the P3 plan reads this as zstd Arrow IPC chunks of at most
65,536 rows per table. That needs a columnar schema and writer for every world table. The same
sharing comes from cutting the canonical bytes of ADR 0002 into fixed-size pieces and compressing
each with zstd: an edit changes values, not lengths, so unchanged regions keep their hashes. State
hashes stay over the canonical bytes either way. Arrow remains the format of exports (14.8).

## Decisions

### D1. How are snapshot chunks stored?

Options:

- A. Zstd chunks of the canonical bytes, content-addressed, fixed 64 KiB cuts. Hashes unchanged;
  Arrow is for exports only.
- B. Arrow IPC chunks per table, as the plan reads 14.3. Needs a columnar schema and writer for
  each world table; roughly doubles the store task.
- C. Defer the chunk store and keep the present snapshot store.

Recommendation: A, because it gives the same sharing across branches at far lower cost and keeps
ADR 0002's rule that hashes are over canonical bytes.

Decision: A, Antonio, 2026-11-06.
Row: ADR 0326

## Spec changes

- 14.3: "compressed columnar chunks" becomes "compressed chunks of the canonical encoding" (ADR 0326).
- `specs/workbench/SPEC.md` and `specs/core/SPEC.md`: the chunk is a zstd piece of the canonical
  bytes cut at fixed 64 KiB offsets, not an Arrow table. Cutting at table marks is a later
  refinement if edits start changing lengths.
