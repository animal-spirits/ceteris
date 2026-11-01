# 0211. Run files and compatibility across versions

Status: proposed (owner chose the option, 2026-10-28)
Date: 2026-10-28
Design sections: 2.4, 4.5, 14.1, 14.3, 19.1, 23.3, 23.4; ADRs 0002, 0128, 0130, 0132, 0136, 0142, 0143, 0188, 0197
Phase: P3

## Context

Five names cover overlapping things: scenario file (ADRs 0113, 0188), run file (0142, 0143),
session file (0132), run store (19.1) and snapshot (0002). Replay is bit-exact only for the same
engine build and pack hash (4.5), yet nothing says what happens when 1.1 opens a run made in 1.0.
Pre-P3 makes an old snapshot a `DecodeError`; ADRs 0136 and 0197 promise a `ceteris migrate`;
ADR 0002 re-blesses every hash when snapshots move to Arrow in P3. DESIGN 23.3 calls the pack,
scenario and intervention formats public API under a versioning policy that exists only in the
23 October roadmap page, outside the repository. Audit gap 2.

## Options

1. Stored results open read-only; the app offers to re-simulate as a new, labelled branch.
2. Migrate snapshots forward with a converter per format change.
3. Install old engines side by side and open each run with its own.

## Decision

Option 1 (owner, 2026-10-28).

**One file family.**

| Object | What it holds | Status |
| --- | --- | --- |
| Pack | Immutable tables, identified by its hash | Rebuilt from sources when its schema changes (ADR 0215) |
| Scenario file | Pack hash, engine version, parameters, shock and policy tapes chosen, ordered edits (14.2) with ticks, seeds, branch tree | The save. Plain text, diffable, the stable public format |
| Run hash | Hash of (scenario file, pack hash, engine version) | The unit of sharing (ADR 0128) |
| Snapshots and run store | State and history derived by running a scenario | Caches keyed by run hash; deletable, recomputable |

"Run file" and "session file" in ADRs 0132, 0142 and 0143 mean the scenario file. Flags and name
sets chosen in a run (0142, 0143) are stored in it.

**Opening a run from another engine version.** Stored results open read-only with a banner naming
both versions. "Re-simulate" replays the scenario on the current engine as a sibling branch
labelled with its engine version; the branch manager can diff the two, which doubles as a report
of what the upgrade changed. Snapshots never cross engine versions; decoding one from another
version is the pre-P3 `DecodeError`, shown as "cache from another version".

**Compatibility promise.** Within a major, the scenario format and the edit vocabulary change only
additively: a 1.x scenario opens in any later 1.x. The pack schema may change in a minor, which
triggers a rebuild from cached sources. `ceteris migrate` exists only for scenario format breaks,
which wait for a major (0136). The API schema (0197) follows the scenario format's version.
This section is the repository's versioning policy that DESIGN 23.3 cites.

**Multiplayer.** No networked play in 1.x. Shared play is asynchronous through scenario files and
run hashes (Ironman 23.4, relay 0132). Lockstep seats (0130) are reconsidered with 2.0.

## Migration

- P3: scenario file format and `ceteris` verbs, run hash, cache keyed by run hash; ADR 0002's Arrow
  move lands as a cache format with no compatibility duty.
- ADRs 0128, 0132, 0142, 0143: wording follows this ADR when they are accepted.
- DESIGN 14.1 gains the file-family table; 23.3 points to this ADR for versioning.

## Tests

- A scenario written by build N opens in build N+1; stored results show read-only.
- Re-simulating under the same engine reproduces the stored hashes; under a different engine the
  branch is labelled and the old results stay untouched.
- Deleting the cache and replaying the scenario reproduces every stored state hash.

## Consequences

Determinism stays an honest promise: a result is always tied to the engine that produced it.
Saves are small text files that survive upgrades; old results are not lost, but continuing an old
run means re-simulating it.
