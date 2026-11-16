# RFC 0358. Scenario edits may multiply a value instead of setting it

Status: proposed
Date: 2026-11-16
Design sections: 14.2
Release: 1.0 (P2 showcase set)
Research: none; follows #409 (border friction x10 scenario)

## Summary

A scenario edit sets a target to an absolute `value`. The border friction x10 showcase scenario
("deglobalisation") must scale `trade.border_nu.<g>`, which `ceteris calibrate` fits per pack, so
an absolute value is right for one pack fit only and goes stale at every refit. The same holds
for any edit meant as "n times what the pack has": non-tariff barriers, wage speed, trade
elasticities. Antonio chose a relative-edit form over hardcoded fitted values (question card,
2026-11-16).

## Decisions

### D1. How does an edit say "multiply"?

Options:

- A. `Edit` gains an optional field `op`, `"set"` (default) or `"mul"`. With `mul`, `value` is
  the factor applied to the target's value at the edit's tick. Edits without `op` are read,
  hashed and written exactly as before, so no existing run hash moves; `op` enters the run hash
  only when it is `mul`. Accepted for `param` and `policy` targets: `econ.*`, `trade.*`,
  `trade.border_nu.<g>`, and `ntb.<i>.<e>.<g>` (the product must stay at least 1, else the edit
  is rejected like any out-of-range value). `tariff.*`, `state`, `switch` and the other kinds
  reject `mul`: a tariff is a rate, so a product of a zero tariff is meaningless.
- B. A new `EditKind` `scale`. No new field, but a new enum value in a public format, a new
  branch in every `match` on the kind, and `param` and `policy` each need a twin.
- C. A target suffix, `trade.border_nu.3*`. No schema change, but it overloads the target
  string that edit-by-query and replay tapes also parse.

Recommendation: A, because it adds one optional field that leaves every existing file and hash
unchanged, and the validation lives in one place.

Decision: <open>
Row: <filled in once accepted>

### D2. Which targets are in scope for the first version?

Options:

- A. The ones above: parameters and ntb. Enough for border x10 and the Brexit NTB variants.
- B. Also `state` edits (for example, scale a stock). State edits have no calibrated value to
  go stale against, so there is no need yet, and a multiply on selected rows needs a defined
  order against edit-by-query.

Recommendation: A, because nothing on the showcase list needs more, and B can follow by RFC.

Decision: <open>
Row: <filled in once accepted>

## Spec changes

- 14.2: the edit record has an optional `op` (`set` default, `mul`); the targets that accept
  `mul`; the run hash includes `op` only when it is `mul`.

## Work

`ceteris-run` `Edit` gains `op` (serde default, skipped when `set`, hashed only when `mul`);
`ceteris-econ::edit` applies it for the accepted targets and rejects it elsewhere; tests: an
existing scenario file round-trips byte-identical and keeps its run hash, a `mul` edit equals
the `set` edit of the product, `mul` on a tariff is rejected. Then the border x10 scenario
file is written (#409).
