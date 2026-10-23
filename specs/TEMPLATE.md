# <Module> SPEC

Status: draft | approved (by, date)
Phase: P<n>
Design sections: <links into DESIGN.md>

## Purpose

One paragraph: what this module owns and what it does not.

## State components

| Component | Entity | Type | Units | Initialized from |
|---|---|---|---|---|

## Equations

Each equation with its symbols, units and the parameter IDs (Section 17) it uses.

## System order

| System ID | Name | Cadence | Reads | Writes | Position in tick pipeline |
|---|---|---|---|---|---|

## Invariants

| Invariant | Statement | Tolerance | Checked where |
|---|---|---|---|

## Parameters

| ID | Name | Prior / value | Class (data, literature, free) |
|---|---|---|---|

## Equilibrium nesting

Which benchmark the module collapses to with frictions off, and the test that checks it. "n/a" with a reason if none.

## Test plan

- Unit tests per equation.
- Property tests per invariant.
- Golden scenario and its hash file.
- Benchmarks and their budget (Section 19.2).

## Open questions
