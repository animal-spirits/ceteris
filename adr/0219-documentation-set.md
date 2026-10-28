# 0219. Documentation set

Status: proposed
Date: 2026-10-28
Design sections: 15.4, 15.7 (Documentation), 17, 20.2, 21.1 (P9, R); ADRs 0127, 0158, 0197, 0218
Phase: P9 (model description, methods), R (user and developer guides)

## Context

DESIGN 15.7 requires a model description in the ODD protocol (Grimm et al. 2020) "so outsiders
can review it"; no phase owns it. R ships a minimum pedia (ADR 0158). There is no user guide, no
methods page behind the scorecard, and no developer guide beyond CLAUDE.md. Audit gap 12.

## Options

1. Write documents by hand when needed.
2. One documentation set in the repository, partly generated from the specs, register and API
   schema, with owners in the phase plan.

## Decision

Option 2.

| Document | Content | Generated from | Phase |
| --- | --- | --- | --- |
| Model description (ODD) | Purpose, entities, process order, design concepts, initialization, inputs, submodels | SPEC state and order sections, register 17 | P9 |
| Methods | One page per scorecard target and natural experiment (15.4, 15.5): data, method, result | Scorecard output | P9 |
| User guide | Getting started, setup (ADR 0215), workbench, the pedia (0158) | Pedia files | R |
| Developer guide | Architecture (19.3), building, testing, ADR process, CONTRIBUTING | CLAUDE.md, SPEC template | R |
| API reference | Commands and tables | `ceteris api schema` (0197), rustdoc | R |

Format: mdBook in `docs/`, built in CI, published with GitHub Pages when the repository opens
(ADR 0218). A style note for user-facing text (neutral, descriptive, sourced) lives in the
developer guide and is used by ADR 0222.

## Migration

- P9 plan: ODD and methods pages beside the scorecard. R plan: guides and publishing.

## Tests

- The book builds without broken links.
- Every parameter ID in the register appears in the model description.

## Consequences

Outsiders can review the model the day 1.0 opens. Generation keeps the description in step with
the specs; the hand-written parts are a recurring cost.
