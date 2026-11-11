# 0001. Record architecture decisions

Status: accepted
Date: 2026-10-23
Design sections: 21.2, 22.2

## Context

The design document (`specs/DESIGN.md`, snapshot of the Claude Docs original
 at rev 89) is the contract. Section 21.2 requires every deviation to be recorded with
context, options and consequences, and 22.2 lists defaults the coding agent may take
as ADRs.

## Options

1. ADRs in `adr/`, numbered, one decision per file.
2. Decisions inline in specs.

## Decision

Option 1, using `adr/TEMPLATE.md`. Specs link to the ADRs that shape them.

## Consequences

`specs/DESIGN.md` is not edited to match the code; ADRs record where they differ.
When the Claude Docs original changes, the snapshot is refreshed in its own commit.
