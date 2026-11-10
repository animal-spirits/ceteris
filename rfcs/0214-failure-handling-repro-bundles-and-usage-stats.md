# 0214. Failure handling, repro bundles and opt-in usage statistics

Status: proposed (owner chose opt-in usage statistics, 2026-10-28; usage statistics superseded for 1.0 by ADR 0342, RFC 0340)
Date: 2026-10-28
Design sections: 4.5, 4.6, 14.3, 15.7 (fuzz testing), 20; ADRs 0088, 0120, 0197, 0211
Phase: App shell (halt panel, logging), P3 (repro bundles), R (usage statistics)

## Context

DESIGN 4.6 says an invariant violation halts the run, and the core panics on bad inputs (the geo
SPEC radius check, ledger overflow). Nothing says what the app does then, where the boundary
between panic and `Result` lies, how logging works, or whether anything is reported. The project
has no server and no stance on telemetry. Audit gaps 5 and 10.

## Options

Failure handling: (a) let the app crash; (b) a typed halt that keeps the session and writes a
bundle that replays the failure exactly.

Telemetry: (1) none, with a local bundle; (2) opt-in crash upload; (3) opt-in usage statistics.

## Decision

(b), and option 3 for telemetry (owner, 2026-10-28), with the local bundle kept.

**Halts.** The core returns `Halt { tick, system, kind, detail }` for invariant violations,
non-finite values and overflow. `panic!` stays for programmer errors only; the app runs the
simulation thread under `catch_unwind` and turns a panic into a halt of kind `panic`. On a halt
the app keeps the last good snapshot, shows what broke (invariant, values, tick), and offers to
fork from the last snapshot or open the bundle folder. No state is lost.

**Repro bundle.** Written on every halt: the scenario file (ADR 0211), pack hash, engine version
and build hash, the halt record and the log tail. Because runs are deterministic, `ceteris
replay <bundle>` reproduces the failure on any machine with the same pack. The fuzzer of ADR 0120
writes the same bundle. Bundles are never uploaded; the user attaches one to an issue.

**Logging.** `tracing` to a rotating file in the app's log folder, `info` by default, `debug` from
a setting. No run contents are logged.

**Usage statistics.** Superseded for 1.0 by ADR 0342 (RFC 0340): no collector, no counters, no install ID,
no settings panel, no request to a project host. Original text kept below as history.
- Off by default; asked once at first launch; can be switched off at any time.
- A settings panel shows the exact payload before anything is sent.
- Contents: app version, macOS version, chip and memory class, counts of modes, panels and
  commands used, run lengths, ensemble sizes, and counts of halts by kind. No scenario contents,
  edits, file paths, names or free text. A random install ID, rotated yearly.
- Sent as one batch a week over HTTPS to a collector the project runs; aggregates are published.
- A privacy notice ships in the app and the repository.
- Open point for R: the collector's host and who pays for it.

## Migration

- App shell: halt type, `catch_unwind`, halt panel, logging. P3: bundles and `ceteris replay`.
- R: statistics client, settings panel, privacy notice, collector (dropped, ADR 0342).

## Tests

- A planted invariant violation halts with the right record, and the app forks from the last
  snapshot.
- A bundle replays to the same halt on Linux and macOS.
- Dropped (ADR 0342): no-request-to-collector network test and payload schema test.

## Consequences

Every failure a user sees is reproducible by the maintainer, which few simulators can offer.
Usage statistics would have needed a small hosted service and a privacy notice, the project's first
infrastructure; 1.0 drops them (ADR 0342).
