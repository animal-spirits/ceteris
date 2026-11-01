# 0039. Automation and AI as a scenario lane

Status: proposed
Date: 2026-10-24
Design sections: 2.4, 6.3, 7.5, 13.1, 17.2
Phase: P8, extended profile

## Context

Automation and AI are the most asked-about drivers of 2025-2050 labour markets, and the least
validated. The task-based framework (Acemoglu and Restrepo 2018, 2022) models technology as
automating tasks, displacing labour from them while raising productivity, with effects by skill
group. Estimates of AI's aggregate effect differ widely: Acemoglu (2024) bounds the ten-year
TFP gain at no more than about 0.7%, while industry forecasts are several times larger. No
out-of-sample data can calibrate it.

## Options

1. Leave AI out (status quo: no named technologies).
2. A calibrated AI growth mechanism.
3. A scenario lane: task-automation shocks by sector and education, drawn from scenario ranges,
   labelled as scenarios.

## Decision

Option 3, in the extended profile (23.1).

- Each sector and education group has a task-exposure share (published exposure indices;
  licences to audit). A scenario sets the share of exposed tasks automated per year and the
  productivity gain on them.
- Effects run through existing channels: labour demand by education (8.7), sector productivity
  (7.5), capital investment (7.4) and the skill premium.
- Switch S18: none, low (Acemoglu 2024 bound) or high (published upper scenarios). Never
  calibrated; never in the scorecard.

## Migration

- P8: exposure shares as pack data; a scenario shock type on the shock tape.

## Tests

- With S18 = none, golden hashes are unchanged.
- The low lane reproduces Acemoglu's ten-year TFP bound in a closed toy economy.

## Consequences

Players and researchers can explore automation futures with the uncertainty explicit and
without contaminating the calibrated lab profile.
