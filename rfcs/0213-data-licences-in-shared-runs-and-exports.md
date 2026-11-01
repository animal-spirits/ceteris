# 0213. Data licences in shared runs and exports

Status: proposed (owner chose the sharing option, 2026-10-28)
Date: 2026-10-28
Design sections: 2.5 (principle 9), 14.3, 16, 19.1; ADRs 0005, 0050, 0128, 0197, 0211, 0217
Phase: P3

## Context

Licence checks stop at the pack (ADRs 0005, 0050). Snapshots derive from every source, including
ones that forbid redistribution in sources.toml (the IFPRI SPAM terms, GADM, the flood and
multi-hazard source). ADR 0128's `ceteris fetch <hash>` pulls snapshot chunks "from any mirror or
peer", and ADR 0197 exports tables to Parquet, Arrow and GeoPackage. Both would pass on derived
data with no filter or notice. Audit gap 4. This is a design reading of the licences, not legal
advice.

## Options

1. Sharing sends the scenario file and hashes only; the recipient rebuilds and verifies.
2. Sharing sends chunks filtered by licence tags.
3. Sharing sends chunks unfiltered.

## Decision

Option 1 for sharing (owner, 2026-10-28), and a licence tag on every table for exports.

- **Lineage.** Each pack table records the set of sources it derives from, carried from the build
  provenance. A table's terms are the strictest of its sources'.
- **Sharing.** `ceteris share` sends the scenario file and its run hash (ADR 0211). The recipient
  runs it on their own pack and checks the hashes. No snapshot chunk leaves the machine. ADR 0128's
  mirror and peer fetch of chunks is dropped.
- **Exports.** Two kinds of output:
  - Initial-state and input tables (tick 0, the pack itself): exported only when every source in
    the lineage allows redistribution; otherwise the export is refused with the source named.
  - Simulated outcomes after tick 0, charts and maps: model results, exported with the
    attribution text of every source in the lineage embedded (file metadata, plus a
    `NOTICE.txt` beside multi-file exports).
- The attribution text comes from sources.toml (ADR 0217).

## Migration

- P3: lineage column in the pack manifest, `share` as above, export notices and refusals.
- ADR 0128: Decision bullet "Runs as hashes" reads "recomputes it locally" only.

## Tests

- Exporting a tick-0 table derived from a no-redistribution source fails and names the source.
- Every exported file carries the notices of its lineage.
- A shared hash recomputed on another machine reproduces the state hashes.

## Consequences

Sharing needs no hosting and cannot leak restricted data; the cost is that a recipient must have
built the same pack. Exports of model results stay free to publish with credit.
