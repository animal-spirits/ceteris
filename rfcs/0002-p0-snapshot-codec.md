# 0002. Canonical binary snapshots before Arrow

Status: accepted
Date: 2026-10-23
Design sections: 4.1, 14.3, 19.1 (Snapshots: zstd-compressed Arrow, content-addressed)

## Context

Section 19.1 specifies zstd-compressed Arrow snapshots in about 64k-row
content-hashed chunks. In P0 the only state is the ledger, the event log, ID
allocators and the toy world; no Arrow consumer (query API, Python, map client)
exists until P1 to P3.

## Options

1. Arrow IPC now, via `arrow-rs`.
2. A canonical little-endian encoding, zstd-compressed, hashed with BLAKE3; move to
   Arrow chunks when the query API and branch store land.

## Decision

Option 2. The state hash is defined over the canonical encoding, not over the
compressed bytes, and is the contract the golden runs check.

## Consequences

- Smaller dependency tree and simpler code in P0.
- Moving to Arrow chunks (P3, branch store) changes the storage format and
  therefore the golden hashes; that change will be its own commit with re-blessed
  hashes and a superseding ADR.
- Snapshots are held in memory in P0; on-disk storage arrives with the run store.
