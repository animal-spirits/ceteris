# 0125. LLM tools outside the core

Status: proposed
Date: 2026-10-28
Design sections: 12.5, 14.2, 14.8; ADRs 0078, 0113
Phase: P3 (compiler, research assistant); P4 (focus groups)

## Context

LLM use is limited to deferred Tier 2 leaders (12.5) and presentation text (ADR 0078). Three
uses help without touching determinism, because they sit outside the simulation and produce
ordinary manifests or presentation. Brainstorm (ideas 19, 20 and 22).

## Options

1. Status quo.
2. Three tools that read and write only manifests, CLI output and presentation text.

## Decision

Option 2. No LLM output enters state; every tool is optional and off by default.

- Intervention compiler: natural language ("no 2018 US steel tariffs") to a validated edit list
  from 14.2, shown for confirmation and stored as ordinary edits.
- Research assistant: a script or MCP server that turns a question into an experiment manifest,
  runs it through the CLI (ADR 0113), and writes a memo whose every number cites a run hash and
  output column. A validator rejects numbers not found in the outputs.
- Focus groups: interview a pop cell. The LLM speaks from a fact bundle (income, prices faced,
  job history, beliefs) and cites fact ids, with the validator of the narrative notes.
  Presentation only.

## Migration

- P3: compiler and research assistant against the CLI. P4: focus groups once pops have
  histories.

## Tests

- Compiled edits round-trip: the edit list renders back to a description that names the same
  targets and ticks.
- The memo validator rejects a planted number absent from the run outputs.

## Consequences

Counterfactuals become cheaper to pose and results cheaper to read, with claims checkable by
rerunning. Model and API choice follow the licence rules of ADR 0050.
