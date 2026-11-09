# RFC 0336. Border and non-tariff edit targets for the showcase scenarios

Status: draft
Date: 2026-11-09
Design sections: 14.2 (edit targets), 8 (trade frictions)
Release: 1.0
Research: `reports/pre-1.0-polish-plan-2026-11-09.md` (P2, issue #409)

## Summary

The edit log reaches tariffs and a few scalar parameters only. Two showcase scenarios cannot be
written: "border friction x10" (deglobalisation) and Hard versus soft Brexit, whose benchmark
shock files (`research/benchmark-2026-10-28/shocks/`) carry non-tariff barriers (NTB) as iceberg
factors per country pair. The kernel already has a per-good border iceberg `nu` (`border_nu_of`)
and a per-pair tariff wedge, so both edits are small additions with all-default values that leave
every golden unchanged.

## Decisions

### D1. How is a global border-friction change edited?

Options:

- A. New target `trade.border_nu_scale`, a multiplier on every good's border iceberg nu
  (default 1.0): one edit, matches "x10".
- B. One target per good (`trade.border_nu.<good>`): finer, ten edits for the scenario.

Recommendation: A, because the scenario is one edit and per-good control can be added later.

Decision: B, Antonio, 2026-11-09 (decision card in the #409 thread).
Row: to be taken when this PR writes the spec.

### D2. How is a pair-specific NTB edited?

Options:

- A. New wedge `ntb.<importer>.<exporter>.<group>`, an iceberg factor (1.0 default) multiplying
  the crossing loss alongside nu, in the same index order and range as `tariff.*`.
- B. Express NTB as an equivalent tariff: no new target, but the mechanism is misstated (a
  tariff is a duty that is not destroyed at the crossing; an iceberg is).

Recommendation: A, because the benchmark NTB values are iceberg estimates and B changes
revenue and prices differently.

Decision: pending.
Row: to be taken when this PR writes the spec.

## Spec changes

- 14.2: list `trade.border_nu.<good>` (kind `param`, replaces the good's border iceberg nu; border x10 is one edit per traded good) and `ntb.<i>.<e>.<g>` as edit targets (kind `param` and
  `policy`).
- 8: the crossing loss is `tau * (1 + nu_g) * ntb`; `Wedges` gains an `ntb` vector of the
  tariff's shape. The state hash sees `ntb` only when it differs from all ones, so existing goldens
  do not move; a test asserts this.

## Open points for the review

- "x10" of friction means x10 of nu per good, not of 1 + nu.
- Whether `ntb` enters the snapshot format, and a migration for old snapshots.
